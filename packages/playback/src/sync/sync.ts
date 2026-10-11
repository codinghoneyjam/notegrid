// <META - FILE SUMMARY - The document->schedule synchronisation policy: change routing, debounce, error retention, position watch, watchdog recovery.>

import { AUDITION_HOLD_MS, AuditionController } from './audition.ts';
import { Debouncer, systemClock, type Clock } from './debounce.ts';
import { diffDocuments } from './diff.ts';
import { buildMixRows, mixRowsEqual } from './mix.ts';
import { planReplay } from './transport.ts';
import type {
  EngineSnapshot,
  LoopRange,
  MixRow,
  PlaybackPort,
  PortFactory,
  ReplayOp,
  ScheduleCompiler,
  SyncDocument,
  SyncError,
} from './types.ts';

/** T03 SYNC: schedule-affecting edits coalesce for 50 ms. */
export const SCHEDULE_DEBOUNCE_MS = 50;

/** T03 SYNC: "worklet error or silence > 2 s watchdog". */
export const WATCHDOG_SILENCE_MS = 2000;

/** T03.AC2: adding a note during playback must not move the playhead by more than this. */
export const MAX_POSITION_STEP_TICKS = 1;

export type PositionViolationKind = 'non-monotonic' | 'jump';

export interface PositionViolation {
  kind: PositionViolationKind;
  from: number;
  to: number;
  delta: number;
  at: number;
}

export interface DocumentSyncOptions {
  /** Engine and compiler must agree, or a tick maps to two different sample positions. */
  sampleRate: number;
  compiler: ScheduleCompiler;
  port: PlaybackPort;
  /** Required for watchdog recovery: without it a dead node cannot be replaced (T03.AC4). */
  portFactory?: PortFactory;
  clock?: Clock;
  debounceMs?: number;
  holdMs?: number;
  watchdogMs?: number;
  maxStepTicks?: number;
  onError?: (error: SyncError) => void;
  onRecover?: (reason: string, ops: ReplayOp[]) => void;
}

export interface RecoverOutcome {
  recovered: boolean;
  reason: string;
  ops: ReplayOp[];
}

/**
 * Keeps the engine's idea of the song in step with the editor's document.
 *
 * Three properties are load bearing and are asserted by the tests:
 *
 * 1. A note added while playing never moves the playhead. The schedule swap is the only
 *    thing that can move it, and C2.6 says the swap preserves the tick, so the position
 *    stream stays monotonic (T03.AC2).
 * 2. A compile failure never reaches the engine. The previous schedule stays loaded and
 *    the editor gets an error instead of silence (T03.AC3).
 * 3. A dead worklet is replaced and the full state is replayed in a fixed order, so
 *    recovery restores the sound instead of just restarting the node (T03.AC4).
 */
export class DocumentSync {
  private readonly sampleRate: number;
  private readonly compiler: ScheduleCompiler;
  private readonly clock: Clock;
  private readonly watchdogMs: number;
  private readonly maxStepTicks: number;
  private readonly portFactory: PortFactory | undefined;
  private readonly onError: ((error: SyncError) => void) | undefined;
  private readonly onRecover: ((reason: string, ops: ReplayOp[]) => void) | undefined;
  private readonly debouncer: Debouncer;
  private readonly audition: AuditionController;

  private port: PlaybackPort;
  private document: SyncDocument | null = null;
  private pending: SyncDocument | null = null;

  private schedule: Uint8Array | null = null;
  private mix: MixRow[] = [];
  private loop: LoopRange | null = null;
  /** The document's loop, held until the engine has a schedule to attach it to. */
  private loopWithheld = false;
  private bank: Uint8Array | null = null;

  private playing = false;
  private playingSince = 0;
  private lastReportAt: number | null = null;
  private lastTick: number | null = null;
  private awaitingSwap = false;
  private readonly violations: PositionViolation[] = [];

  constructor(options: DocumentSyncOptions) {
    this.sampleRate = options.sampleRate;
    this.compiler = options.compiler;
    this.port = options.port;
    this.portFactory = options.portFactory;
    this.clock = options.clock ?? systemClock;
    this.watchdogMs = options.watchdogMs ?? WATCHDOG_SILENCE_MS;
    this.maxStepTicks = options.maxStepTicks ?? MAX_POSITION_STEP_TICKS;
    this.onError = options.onError;
    this.onRecover = options.onRecover;
    this.debouncer = new Debouncer(options.debounceMs ?? SCHEDULE_DEBOUNCE_MS, this.clock);
    this.audition = new AuditionController(
      (kind, program, key, vel, on) => this.port.audition(kind, program, key, vel, on),
      { holdMs: options.holdMs ?? AUDITION_HOLD_MS, clock: this.clock },
    );
  }

  // --- document -> engine -------------------------------------------------------------------

  /**
   * Applies an edit. Mix and loop changes go out immediately; schedule work is debounced
   * so a drag compiles once instead of once per mouse-move.
   */
  applyDocument(doc: SyncDocument): void {
    const first = this.document === null;
    const diff = diffDocuments(this.document, doc);
    this.document = doc;
    this.audition.setDocument(doc);

    // The first message a port ever receives has to be init: every later
    // message depends on the sample rate it sets, and a worklet that has not
    // been initialised rejects anything else. Recovery replays the same first
    // message (planReplay), so the startup order is one definition, not two.
    if (first) {
      this.send({ op: 'init', sampleRate: this.sampleRate });
    }

    if (diff.mix) {
      const rows = buildMixRows(doc);
      if (!mixRowsEqual(rows, this.mix)) {
        this.mix = rows;
        this.send({ op: 'mix', rows });
      }
    }

    if (diff.loop) {
      this.loop = doc.loop ?? null;
      this.sendLoop();
    }

    for (const note of diff.addedNotes) {
      this.audition.noteOn(note.lane, note.pitch, note.velocity);
    }

    if (diff.schedule) {
      this.pending = doc;
      this.debouncer.schedule(() => this.compilePending());
    }
  }

  /** Runs the pending compile now. Used by tests and by edits that cannot wait 50 ms. */
  flush(): void {
    this.debouncer.flush();
  }

  /** The schedule currently loaded on the engine, or null before the first good compile. */
  loadedSchedule(): Uint8Array | null {
    return this.schedule;
  }

  // --- audition ------------------------------------------------------------------------------

  auditionNoteOn(laneId: number, key: number, velocity: number): boolean {
    return this.audition.noteOn(laneId, key, velocity);
  }

  auditionNoteOff(laneId: number, key: number): void {
    this.audition.noteOff(laneId, key);
  }

  auditionHold(laneId: number, key: number): void {
    this.audition.hold(laneId, key);
  }

  // --- engine state --------------------------------------------------------------------------

  /** Banks load only while stopped (T03 SYNC), so this is the policy's only bank entry point. */
  setBank(bytes: Uint8Array | null): void {
    this.bank = bytes;
    if (bytes !== null) {
      this.send({ op: 'bank', bytes });
    }
  }

  setLoop(range: LoopRange | null): void {
    this.loop = range;
    this.sendLoop();
  }

  snapshot(): EngineSnapshot {
    return {
      sampleRate: this.sampleRate,
      schedule: this.schedule,
      mix: this.mix,
      loop: this.loop,
      bank: this.bank,
    };
  }

  // --- position watch (T03.AC2) ---------------------------------------------------------------

  setPlaying(playing: boolean): void {
    if (playing === this.playing) return;
    this.playing = playing;
    if (playing) {
      this.playingSince = this.clock.now();
      this.lastReportAt = this.playingSince;
    }
  }

  /**
   * Feeds one position report in. Reports are the only evidence the engine is alive, so
   * this doubles as the watchdog's heartbeat.
   *
   * Two different rules apply to two different things, and conflating them would make
   * the check useless. Ordinary playback always has to be monotonic, because the playhead
   * only ever moves forward. The one-tick bound is narrower and applies only across the
   * one instant a schedule swap can move it: C2.6 says the swap preserves the tick, so
   * the first report after a swap must land within `maxStepTicks` of the last one before.
   */
  recordPosition(tick: number): void {
    const at = this.clock.now();
    this.lastReportAt = at;
    if (this.lastTick !== null) {
      const delta = tick - this.lastTick;
      if (delta < 0) {
        this.violations.push({ kind: 'non-monotonic', from: this.lastTick, to: tick, delta, at });
      } else if (this.awaitingSwap && delta > this.maxStepTicks) {
        this.violations.push({ kind: 'jump', from: this.lastTick, to: tick, delta, at });
      }
    }
    this.lastTick = tick;
    this.awaitingSwap = false;
  }

  positionViolations(): PositionViolation[] {
    return [...this.violations];
  }

  // --- watchdog and recovery (T03.AC4) --------------------------------------------------------

  private silentFor(now: number): number {
    if (!this.playing) return 0;
    const since = this.lastReportAt ?? this.playingSince;
    return now - since;
  }

  /**
   * Call on a timer while playing. Returns the recovery outcome when the watchdog fired,
   * so a caller that only polls occasionally still gets one recovery per silent gap.
   */
  checkWatchdog(now: number = this.clock.now()): RecoverOutcome | null {
    if (this.silentFor(now) <= this.watchdogMs) return null;
    return this.recover('silence');
  }

  /** Called when the worklet reports an error or the port throws. */
  reportTransportFailure(reason: string): RecoverOutcome {
    return this.recover(reason);
  }

  /**
   * Replaces the port and replays the full state in a fixed order.
   *
   * Recovery is deliberately not a silent retry: with no factory there is nothing to
   * rebuild, so the caller is told instead of being left with a dead node.
   */
  recover(reason: string): RecoverOutcome {
    if (!this.portFactory) {
      const error: SyncError = { code: 'E_NO_PORT_FACTORY', message: reason, source: 'transport' };
      this.onError?.(error);
      return { recovered: false, reason, ops: [] };
    }

    this.audition.releaseAll();
    try {
      this.port.close();
    } catch {
      // A dead node can throw on close; the factory call below is what actually matters.
    }
    this.port = this.portFactory();

    const ops = planReplay(this.snapshot());
    for (const op of ops) {
      this.port.apply(op);
    }
    // The replay carries the loop (planReplay sends it), so the engine has it again.
    this.loopWithheld = false;
    // The fresh node owes us a first report before the watchdog trusts it again.
    this.lastReportAt = this.playing ? this.clock.now() : null;
    this.lastTick = null;
    this.onRecover?.(reason, ops);
    return { recovered: true, reason, ops };
  }

  dispose(): void {
    this.debouncer.cancel();
    this.audition.releaseAll();
    this.port.close();
  }

  // --- internals ------------------------------------------------------------------------------

  private send(op: ReplayOp): void {
    this.port.apply(op);
  }

  /**
   * The engine ties the loop to the loaded schedule (`set_loop_ticks` with
   * enabled != 0 and no schedule is E_ENGINE), so a loop change that arrives
   * before the first compile lands is held until a schedule does. That is
   * the same order the recovery replay uses, and the only order the engine
   * accepts; sending it earlier would be refused, and a refused loop is never
   * retried, so the engine would play on without one.
   */
  private sendLoop(): void {
    if (this.schedule === null) {
      this.loopWithheld = true;
      return;
    }
    this.loopWithheld = false;
    this.send({ op: 'loop', range: this.loop });
  }

  private compilePending(): void {
    const doc = this.pending;
    this.pending = null;
    if (!doc) return;

    let bytes: Uint8Array;
    try {
      bytes = this.compiler.compile(doc, this.sampleRate);
    } catch (cause) {
      // T03.AC3: the engine keeps playing the last good schedule; the editor is told why.
      const error: SyncError = {
        code: errorCode(cause),
        message: errorMessage(cause),
        source: 'compile',
      };
      this.onError?.(error);
      return;
    }

    this.schedule = bytes;
    // The next position report is the one that has to prove C2.6 held.
    this.awaitingSwap = true;
    this.send({ op: 'schedule', bytes });
    if (this.loopWithheld) {
      this.loopWithheld = false;
      this.send({ op: 'loop', range: this.loop });
    }
  }
}

function errorCode(cause: unknown): string {
  if (typeof cause === 'object' && cause !== null && 'code' in cause) {
    const code = (cause as { code: unknown }).code;
    if (typeof code === 'string') return code;
  }
  return 'E_COMPILE';
}

function errorMessage(cause: unknown): string {
  if (cause instanceof Error) return cause.message;
  if (typeof cause === 'object' && cause !== null && 'message' in cause) {
    const message = (cause as { message: unknown }).message;
    if (typeof message === 'string') return message;
  }
  return String(cause);
}