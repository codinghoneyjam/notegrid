// <META - FILE SUMMARY - The composition root: createApp(deps) wires editing, sync and persistence into one editor.>

import type { Intent, Project } from '@ng/document';
import {
  HistoryImpl,
  addLane,
  addNote,
  deleteNotes,
  moveNotes,
  resizeNotes,
  setLaneProgram,
  setLoop,
  setTempo,
  setVelocity,
} from '@ng/editing';
import {
  DocumentSync,
  type Clock as SyncClock,
  type PlaybackPort,
  type PortFactory,
  type ScheduleCompiler,
  type SyncDocument,
  type SyncError,
} from '@ng/playback';
import { ProjectStore, type LogEntry, type OpenResult, type SaveResult } from '@ng/persistence';

import { buildLayout, type LayoutModel } from './layout/layout.ts';
import { LocalLog, type Clock } from './log/log.ts';
import { SettingsStore, type Settings, type SettingsStorage } from './settings/settings.ts';

export const DEFAULT_SAMPLE_RATE = 44100;
export const DEFAULT_PROJECT_ID = 'current';

/**
 * A playback port the app can drive transport through.
 *
 * T03's `PlaybackPort` is deliberately just `apply` / `audition` / `close`: the sync policy
 * has no business starting playback. The shell does, so the extra three calls live here, on
 * the app's side of the seam, instead of widening a contract four packages share.
 */
export interface TransportPort extends PlaybackPort {
  play(): void;
  pause(): void;
  stop(): void;
}

export interface AppDeps {
  /** The document the editor opens on. Everything else derives from it. */
  document: Project;
  /** document -> schedule bytes. `Ng.compile` satisfies it; a test injects a recording one. */
  compiler: ScheduleCompiler;
  /** The engine channel. This is the single place a real implementation is bound. */
  port: TransportPort;
  /** Needed for watchdog recovery: without it the policy says so instead of retrying. */
  portFactory?: PortFactory;
  /** Save, open, autosave, recovery and the durable half of the log. */
  store: ProjectStore;
  /** Engine and compiler must agree, or one tick maps to two sample positions (T03 sync). */
  sampleRate?: number;
  clock?: Clock;
  /** Shortens the 50 ms compile debounce so a test does not have to wait for it. */
  debounceMs?: number;
  projectId?: string;
  settingsStorage?: SettingsStorage;
  settings?: Settings;
}

/** What changed, for a UI to subscribe to. One event type keeps the subscription list short. */
export type AppEvent =
  | { type: 'document'; doc: Project }
  | { type: 'transport'; playing: boolean }
  | { type: 'log'; entries: readonly LogEntry[] }
  | { type: 'settings'; settings: Settings }
  | { type: 'recovered'; reason: string };

export interface App {
  readonly id: string;
  readonly sampleRate: number;
  document(): Project;
  selection(): readonly number[];
  select(indices: readonly number[]): void;
  apply(intent: Intent): Project;
  undo(): Project;
  redo(): Project;
  canUndo(): boolean;
  canRedo(): boolean;
  /** Runs the debounced compile now. The editor calls this before it needs the sound. */
  flush(): void;
  play(): void;
  pause(): void;
  stop(): void;
  togglePlay(): boolean;
  playing(): boolean;
  auditionNoteOn(lane: number, key: number, velocity: number): boolean;
  auditionNoteOff(lane: number, key: number): void;
  auditionHold(lane: number, key: number): void;
  layout(): LayoutModel;
  settings(): Settings;
  updateSettings(patch: Partial<Settings>): Settings;
  log(): readonly LogEntry[];
  onLog(listener: (entries: readonly LogEntry[]) => void): () => void;
  save(): Promise<SaveResult>;
  open(): Promise<OpenResult>;
  autosave(): void;
  requestPersistentStorage(): Promise<boolean>;
  /** Feeds a position report in; the watchdog reads the same stream (T03.AC2 / AC4). */
  recordPosition(tick: number): void;
  /** Returns true when the watchdog fired and the port was rebuilt. */
  checkWatchdog(now?: number): boolean;
  onEvent(listener: (event: AppEvent) => void): () => void;
  dispose(): void;
}

const SYSTEM_CLOCK: Clock = { now: () => Date.now() };

const FALLBACK_SETTINGS: Settings = {
  exportSampleRate: DEFAULT_SAMPLE_RATE,
  metronome: false,
  theme: 'system',
  exportDestination: 'download',
};

/**
 * Builds the editor.
 *
 * Nothing is constructed here that a test could not replace: the three things that touch
 * the outside world -- the compiler, the engine channel and storage -- arrive as arguments.
 * That is what makes T06.AC2 possible. "edit -> schedule sync -> save" runs in Node against
 * a recording port and an in-memory backend, and it exercises the same wiring the browser
 * gets rather than a parallel test-only path.
 */
export function createApp(deps: AppDeps): App {
  const clock = deps.clock ?? SYSTEM_CLOCK;
  const id = deps.projectId ?? DEFAULT_PROJECT_ID;
  const sampleRate = deps.sampleRate ?? DEFAULT_SAMPLE_RATE;

  const settingsStore = deps.settingsStorage ? new SettingsStore(deps.settingsStorage, deps.settings) : null;
  let settings: Settings = deps.settings ?? settingsStore?.read() ?? { ...FALLBACK_SETTINGS };

  const listeners = new Set<(event: AppEvent) => void>();
  const log = new LocalLog(deps.store, clock);

  let history = new HistoryImpl(deps.document);
  let current = deps.document;
  let dirty = false;
  let playing = false;
  let selected: readonly number[] = [];
  let errorCount = 0;
  let warningCount = 0;

  const emit = (event: AppEvent): void => {
    for (const listener of [...listeners]) {
      listener(event);
    }
  };

  const record = (code: string, message: string): void => {
    log.record(code, message);
    emit({ type: 'log', entries: log.entries() });
  };

  const syncOptions: ConstructorParameters<typeof DocumentSync>[0] = {
    sampleRate,
    compiler: deps.compiler,
    port: deps.port,
    clock: syncClockOf(clock),
    onError: (error: SyncError) => record(error.code, `${error.source}: ${error.message}`),
    onRecover: (reason) => record('E_RECOVERED', `engine recovered after ${reason}`),
  };
  if (deps.portFactory) syncOptions.portFactory = deps.portFactory;
  if (deps.debounceMs !== undefined) syncOptions.debounceMs = deps.debounceMs;
  const sync = new DocumentSync(syncOptions);

  /** The document -> engine step every mutation ends with, plus autosave and the event. */
  const commit = (doc: Project): Project => {
    current = doc;
    dirty = true;
    sync.applyDocument(doc as SyncDocument);
    deps.store.autosave(current);
    emit({ type: 'document', doc: current });
    return current;
  };

  const run = (label: string, apply: (doc: Project) => Project): Project => {
    try {
      return commit(history.execute({ label, apply }));
    } catch (cause) {
      // An edit the document model rejects is an error the user has to see (T06.R03): never
      // a silent no-op, never an exception across the UI boundary.
      record('E_EDIT', `${label}: ${messageOf(cause)}`);
      return current;
    }
  };

  const setPlaying = (next: boolean): void => {
    if (next) deps.port.play();
    else deps.port.pause();
    playing = next;
    sync.setPlaying(next);
    emit({ type: 'transport', playing });
  };

  const app: App = {
    id,
    sampleRate,

    document: () => current,
    selection: () => selected,
    select: (indices) => {
      selected = [...indices];
    },

    apply(intent) {
      const ids = intentIds(intent, selected);
      switch (intent.type) {
        case 'addNote':
          return run('add note', (doc) =>
            addNote(doc, intent.lane, intent.start, intent.pitch, intent.duration ?? 480),
          );
        case 'moveNotes':
          return run('move notes', (doc) =>
            moveNotes(doc, ids, intent.dTick, intent.dPitch, intent.toLane),
          );
        case 'resizeNotes':
          return run('resize notes', (doc) => resizeNotes(doc, ids, intent.edge, intent.dTicks));
        case 'deleteNotes':
          return run('delete notes', (doc) => deleteNotes(doc, ids));
        case 'setVelocity':
          return run('set velocity', (doc) => setVelocity(doc, ids, intent.vel));
        case 'addLane':
          return run('add lane', (doc) => addLane(doc, intent.kind, intent.program));
        case 'setLaneProgram':
          return run('set program', (doc) => setLaneProgram(doc, intent.lane, intent.program));
        case 'setTempo':
          return run('set tempo', (doc) => setTempo(doc, intent.tick, intent.bpm));
        case 'setLoop':
          return run('set loop', (doc) => setLoop(doc, intent.range));
        default:
          // paste / duplicate / quantize / transpose need the clipboard and quantize
          // policies, which `@ng/editing` does not own yet. Saying so is the difference
          // between a missing feature and a broken one.
          record('E_UNSUPPORTED', `intent ${intent.type} is not implemented yet`);
          return current;
      }
    },

    undo: () => commit(history.undo()),
    redo: () => commit(history.redo()),
    canUndo: () => history.canUndo(),
    canRedo: () => history.canRedo(),

    flush: () => sync.flush(),

    play: () => setPlaying(true),
    pause: () => setPlaying(false),
    stop: () => {
      deps.port.stop();
      if (playing) {
        playing = false;
        sync.setPlaying(false);
        emit({ type: 'transport', playing });
      }
    },
    togglePlay: () => {
      setPlaying(!playing);
      return playing;
    },
    playing: () => playing,

    auditionNoteOn: (lane, key, velocity) => sync.auditionNoteOn(lane, key, velocity),
    auditionNoteOff: (lane, key) => sync.auditionNoteOff(lane, key),
    auditionHold: (lane, key) => sync.auditionHold(lane, key),

    layout: () =>
      buildLayout({
        lanes: current.lanes.length,
        notes: current.notes.length,
        canUndo: history.canUndo(),
        canRedo: history.canRedo(),
        playing,
        bpm: current.tempo[0]?.bpm ?? 120,
        meterNum: current.meter[0]?.num ?? 4,
        meterDen: current.meter[0]?.den ?? 4,
        loop: current.loop ?? null,
        errorCount,
        warningCount,
        logCount: log.entries().length,
        dirty,
      }),

    settings: () => ({ ...settings }),
    updateSettings(patch) {
      settings = settingsStore ? settingsStore.update(patch) : { ...settings, ...patch };
      emit({ type: 'settings', settings });
      return { ...settings };
    },

    log: () => log.entries(),
    onLog: (listener) => log.onChange(listener),

    async save() {
      const result = await deps.store.save(id, current);
      if (result.ok) {
        dirty = false;
      } else {
        record(result.code, result.message);
      }
      return result;
    },

    async open() {
      const result = await deps.store.open(id);
      if (!result.ok) {
        errorCount = result.errors.length;
        record(result.errors[0]?.code ?? 'E_OPEN', result.errors[0]?.message ?? 'open failed');
        return result;
      }
      warningCount = result.warnings.length;
      current = result.doc;
      history = new HistoryImpl(result.doc);
      selected = [];
      dirty = false;
      sync.applyDocument(current as SyncDocument);
      emit({ type: 'document', doc: current });
      return result;
    },

    autosave: () => deps.store.autosave(current),
    requestPersistentStorage: () => deps.store.requestPersistent(),

    recordPosition: (tick) => sync.recordPosition(tick),

    checkWatchdog(now) {
      const outcome = sync.checkWatchdog(now);
      if (outcome === null) return false;
      // The watchdog closed and rebuilt the port, so the shell's transport state no longer
      // describes the engine. Reporting "stopped" is the honest answer; the next play()
      // drives the fresh port.
      if (playing) {
        playing = false;
        emit({ type: 'transport', playing });
      }
      return outcome.recovered;
    },

    onEvent(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },

    dispose() {
      sync.dispose();
      listeners.clear();
    },
  };

  return app;
}

/** T03 intents carry `ids: []` when the caller means "the current selection". */
function intentIds(intent: Intent, selected: readonly number[]): number[] {
  const ids = 'ids' in intent ? intent.ids : undefined;
  return ids && ids.length > 0 ? [...ids] : [...selected];
}

function syncClockOf(clock: Clock): SyncClock {
  // The debouncer only ever stores the id and hands it back, so the DOM/Node difference in
  // what setTimeout returns is cast away here rather than leaking into the policy's types.
  return {
    now: () => clock.now(),
    setTimeout: (fn, ms) => globalThis.setTimeout(fn, ms) as unknown as number,
    clearTimeout: (id) => {
      globalThis.clearTimeout(id as unknown as ReturnType<typeof globalThis.setTimeout>);
    },
  };
}

function messageOf(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}