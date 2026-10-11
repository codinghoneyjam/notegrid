// <META - FILE SUMMARY - Tests for the document->schedule sync policy: routing units plus T03.AC2/AC3/AC4 against the real engine.>

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadNg, Ng, NgError } from '@ng/abi';
import {
  AuditionController,
  buildMixRows,
  Debouncer,
  diffDocuments,
  DocumentSync,
  planReplay,
  REPLAY_ORDER,
  SCHEDULE_DEBOUNCE_MS,
  WATCHDOG_SILENCE_MS,
  type Clock,
  type PlaybackPort,
  type ReplayOp,
  type ScheduleCompiler,
  type SyncDocument,
  type SyncError,
} from '../src/sync/index.ts';

const SAMPLE_RATE = 44100;

// --- test doubles ------------------------------------------------------------------------------

class ManualClock implements Clock {
  private current = 0;
  private nextId = 1;
  private readonly timers = new Map<number, { at: number; fn: () => void }>();

  setTimeout(fn: () => void, ms: number): number {
    const id = this.nextId++;
    this.timers.set(id, { at: this.current + ms, fn });
    return id;
  }

  clearTimeout(id: number): void {
    this.timers.delete(id);
  }

  now(): number {
    return this.current;
  }

  advance(ms: number): void {
    const target = this.current + ms;
    for (;;) {
      const due = [...this.timers.entries()]
        .filter(([, t]) => t.at <= target)
        .sort((a, b) => a[1].at - b[1].at)[0];
      if (!due) break;
      this.timers.delete(due[0]);
      this.current = due[1].at;
      due[1].fn();
    }
    this.current = target;
  }
}

interface Recorded {
  ops: ReplayOp[];
  auditions: Array<[number, number, number, number, boolean]>;
  closed: number;
}

class RecordingPort implements PlaybackPort {
  ops: ReplayOp[] = [];
  auditions: Array<[number, number, number, number, boolean]> = [];
  closed = 0;

  apply(op: ReplayOp): void {
    this.ops.push(op);
  }

  audition(kind: number, program: number, key: number, vel: number, on: boolean): void {
    this.auditions.push([kind, program, key, vel, on]);
  }

  close(): void {
    this.closed += 1;
  }

  opsOfType(op: ReplayOp['op']): ReplayOp[] {
    return this.ops.filter((o) => o.op === op);
  }
}

/** Forwards schedule and loop to the real wasm engine, and records what it was asked to do. */
class EnginePort implements PlaybackPort {
  ops: ReplayOp[] = [];

  private readonly ng: Ng;

  constructor(ng: Ng) {
    this.ng = ng;
  }

  apply(op: ReplayOp): void {
    this.ops.push(op);
    switch (op.op) {
      case 'schedule':
        this.ng.engineSetSchedule(op.bytes);
        break;
      case 'loop':
        if (op.range) {
          this.ng.engineSetLoop(op.range.start, op.range.end, true);
        } else {
          this.ng.engineSetLoop(0, 0, false);
        }
        break;
      case 'init':
        this.ng.engineInit(op.sampleRate, 0, 64);
        break;
      default:
        // The double forwards what the tests assert on; mix reaches the
        // engine through engineSetMix, but no test here asserts engine mix
        // state, so it is recorded only.
        break;
    }
  }

  audition(): void {
    // ng_engine_audition is E_NOT_IMPLEMENTED; the audition unit tests cover the policy.
  }

  close(): void {
    // nothing to release: the engine lives in the wasm instance, not in this port
  }

  opsOfType(op: ReplayOp['op']): ReplayOp[] {
    return this.ops.filter((o) => o.op === op);
  }
}

class ThrowingCompiler implements ScheduleCompiler {
  calls = 0;
  private readonly code: string;
  private readonly message: string;

  constructor(code: string, message: string) {
    this.code = code;
    this.message = message;
  }

  compile(): Uint8Array {
    this.calls += 1;
    const error = new Error(this.message) as Error & { code: string };
    error.code = this.code;
    throw error;
  }
}

class CountingCompiler implements ScheduleCompiler {
  calls = 0;
  private readonly ng: Ng;

  constructor(ng: Ng) {
    this.ng = ng;
  }

  compile(doc: SyncDocument, sampleRate: number): Uint8Array {
    this.calls += 1;
    return this.ng.compile(toProjectJson(doc), sampleRate);
  }
}

// --- document fixtures -------------------------------------------------------------------------

function makeDoc(overrides: Partial<SyncDocument> = {}): SyncDocument {
  const doc: SyncDocument = {
    ppq: 480,
    tempo: [{ tick: 0, bpm: 124 }],
    meter: [{ tick: 0, num: 4, den: 4 }],
    loop: { start: 0, end: 7680 },
    lanes: [
      { id: 1, kind: 'melodic', program: 0, volume: 100, pan: 64, reverb_send: 30, mute: false, solo: false },
      { id: 2, kind: 'melodic', program: 33, volume: 105, pan: 60, reverb_send: 0, mute: false, solo: false },
      { id: 4, kind: 'drum', kit: 0, volume: 100, pan: 64, reverb_send: 10, mute: false, solo: false },
    ],
    notes: [
      { lane: 1, start: 0, duration: 480, pitch: 72, velocity: 100 },
      { lane: 1, start: 480, duration: 480, pitch: 75, velocity: 96 },
      { lane: 2, start: 0, duration: 960, pitch: 41, velocity: 105 },
      { lane: 4, start: 0, duration: 120, pitch: 36, velocity: 110 },
    ],
  };
  return { ...doc, ...overrides };
}

/** The score format stores notes as tuples, not objects (C1 NOTE_ENCODING). */
function toProjectJson(doc: SyncDocument): string {
  return JSON.stringify({
    format_version: 1,
    title: 'sync test',
    ppq: doc.ppq,
    tempo: doc.tempo,
    meter: doc.meter,
    loop: doc.loop ?? undefined,
    length_ticks: 7680,
    lanes: doc.lanes.map((lane) => ({ ...lane, name: `lane ${lane.id}` })),
    notes: doc.notes.map((n) => [n.lane, n.start, n.duration, n.pitch, n.velocity]),
  });
}

function renderBlocks(ng: Ng, blocks: number, frames = 128): void {
  const l = new Float32Array(frames);
  const r = new Float32Array(frames);
  for (let i = 0; i < blocks; i += 1) {
    ng.engineRender(l, r);
  }
}

/**
 * The playhead tests run with no loop on purpose: the renderer loop is in samples
 * (`ng_engine_set_loop`, C2 `set_loop(Option<(u64,u64)>)`) while the document loop is
 * in ticks, and there is no tick->sample entry point in the ABI yet. Passing the
 * document loop straight through would wrap the playhead every 7680 *samples* and
 * make "position is monotonic" untestable. See `T2.6 unit boundary: loop` below.
 */
function makeDocNoLoop(overrides: Partial<SyncDocument> = {}): SyncDocument {
  return makeDoc({ loop: null, ...overrides });
}

// --- routing unit tests ------------------------------------------------------------------------

test('routing: notes and tempo recompile, mix and loop do not', () => {
  const base = makeDoc();

  const noteEdit = diffDocuments(base, makeDoc({
    notes: [...base.notes, { lane: 1, start: 960, duration: 480, pitch: 77, velocity: 100 }],
  }));
  assert.equal(noteEdit.schedule, true);
  assert.equal(noteEdit.mix, false);
  assert.equal(noteEdit.loop, false);
  assert.equal(noteEdit.addedNotes.length, 1);

  const volumeEdit = diffDocuments(base, makeDoc({
    lanes: base.lanes.map((l, i) => (i === 0 ? { ...l, volume: 42 } : l)),
  }));
  assert.equal(volumeEdit.schedule, false, 'a fader move must not recompile');
  assert.equal(volumeEdit.mix, true);

  const loopEdit = diffDocuments(base, makeDoc({ loop: { start: 480, end: 7680 } }));
  assert.equal(loopEdit.schedule, false);
  assert.equal(loopEdit.mix, false);
  assert.equal(loopEdit.loop, true);

  const tempoEdit = diffDocuments(base, makeDoc({ tempo: [{ tick: 0, bpm: 90 }] }));
  assert.equal(tempoEdit.schedule, true);

  const programEdit = diffDocuments(base, makeDoc({
    lanes: base.lanes.map((l, i) => (i === 0 ? { ...l, program: 40 } : l)),
  }));
  assert.equal(programEdit.schedule, true, 'a program change remaps the timbre');
});

test('routing: a lane reorder is both a schedule and a mix change (C2.8)', () => {
  const base = makeDoc();
  const reordered = makeDoc({ lanes: [base.lanes[2], base.lanes[0], base.lanes[1]] });
  const diff = diffDocuments(base, reordered);
  assert.equal(diff.schedule, true);
  assert.equal(diff.mix, true);
  assert.ok(diff.reasons.some((r) => r.includes('C2.8')));
});

test('mix rows use the C0.4 0..127 scale and apply solo globally', () => {
  const rows = buildMixRows(makeDoc());
  assert.equal(rows.length, 3);
  assert.equal(rows[0].volume, 100);
  assert.equal(rows[0].pan, 64);
  assert.equal(rows[2].lane, 2, 'row index is the renderer slot');

  const soloed = buildMixRows(makeDoc({
    lanes: makeDoc().lanes.map((l, i) => (i === 0 ? { ...l, solo: true } : l)),
  }));
  assert.equal(soloed[0].muted, false);
  assert.equal(soloed[1].muted, true);
  assert.equal(soloed[2].muted, true);
});

test('debouncer coalesces a burst into one compile after 50 ms', () => {
  const clock = new ManualClock();
  const debouncer = new Debouncer(SCHEDULE_DEBOUNCE_MS, clock);
  let runs = 0;
  debouncer.schedule(() => {
    runs += 1;
  });
  clock.advance(40);
  debouncer.schedule(() => {
    runs += 1;
  });
  clock.advance(49);
  assert.equal(runs, 0, 'still inside the window');
  clock.advance(1);
  assert.equal(runs, 1, 'one compile for the whole burst');
});

test('audition sounds on the lane instrument and auto-releases after 250 ms', () => {
  const clock = new ManualClock();
  const port = new RecordingPort();
  const audition = new AuditionController(
    (kind, program, key, vel, on) => port.audition(kind, program, key, vel, on),
    { clock },
  );
  audition.setDocument(makeDoc());

  assert.equal(audition.noteOn(1, 60, 100), true);
  assert.deepEqual(port.auditions, [[0, 0, 60, 100, true]], 'melodic lane 1, program 0');

  clock.advance(249);
  assert.equal(audition.sounding, 1);
  clock.advance(1);
  assert.deepEqual(port.auditions[1], [0, 0, 60, 0, false]);
  assert.equal(audition.sounding, 0);

  audition.noteOn(4, 36, 110);
  assert.deepEqual(port.auditions[2], [1, 0, 36, 110, true], 'drum lane 4 plays the kit');
  assert.equal(audition.noteOn(99, 60, 100), false, 'unknown lane is reported, not silently ignored');
});

test('audition stays sounding while the caller holds the key', () => {
  const clock = new ManualClock();
  const port = new RecordingPort();
  const audition = new AuditionController(
    (kind, program, key, vel, on) => port.audition(kind, program, key, vel, on),
    { clock },
  );
  audition.setDocument(makeDoc());

  audition.noteOn(1, 64, 100);
  for (let i = 0; i < 10; i += 1) {
    clock.advance(200);
    audition.hold(1, 64);
  }
  assert.equal(audition.sounding, 1, 'still held after 2 s');
  audition.noteOff(1, 64);
  assert.equal(audition.sounding, 0);
});

test('replay order is fixed and skips state that was never set', () => {  const schedule = new Uint8Array([1, 2, 3]);
  const bank = new Uint8Array([9]);
  const ops = planReplay({
    sampleRate: SAMPLE_RATE,
    schedule,
    mix: [{ lane: 0, volume: 100, pan: 64, send: 0, muted: false }],
    loop: { start: 0, end: 7680 },
    bank,
  });
  assert.deepEqual(ops.map((o) => o.op), ['init', 'schedule', 'mix', 'loop', 'bank']);
  assert.deepEqual(ops.map((o) => o.op), [...REPLAY_ORDER]);

  const bare = planReplay({
    sampleRate: SAMPLE_RATE,
    schedule: null,
    mix: [],
    loop: null,
    bank: null,
  });
  assert.deepEqual(bare.map((o) => o.op), ['init', 'loop'], 'loop null is still sent, it clears the range');
});

test('the first document initialises the port before anything else', () => {
  const port = new RecordingPort();
  const sync = new DocumentSync({
    sampleRate: SAMPLE_RATE,
    compiler: new StubCompiler(),
    port,
    clock: new ManualClock(),
  });

  // A fresh worklet rejects every command that arrives before init,
  // so init has to be the first op the policy ever sends -- the same
  // first message recovery replays, from the same one definition.
  sync.applyDocument(makeDoc());
  assert.deepEqual(
    port.ops.slice(0, 3).map((o) => o.op),
    ['init', 'mix'],
    'init, then the immediate mix; the loop and schedule wait for the compile',
  );
  const init = port.opsOfType('init')[0];
  assert.ok(init.op === 'init');
  assert.equal(init.sampleRate, SAMPLE_RATE);

  // Only the first document initialises: later edits do not re-init.
  sync.applyDocument(makeDocNoLoop({ notes: [] }));
  assert.equal(port.opsOfType('init').length, 1);
});

test('the first document withholds the loop until a schedule lands', () => {
  // set_loop_ticks with enabled != 0 and no schedule is E_ENGINE, and a refused
  // loop is never retried, so the policy holds the document's loop until the
  // first compile puts a schedule on the engine -- the recovery replay's order.
  const port = new RecordingPort();
  const sync = new DocumentSync({
    sampleRate: SAMPLE_RATE,
    compiler: new StubCompiler(),
    port,
    clock: new ManualClock(),
  });

  sync.applyDocument(makeDoc());
  assert.deepEqual(
    port.ops.map((o) => o.op),
    ['init', 'mix'],
    'the loop is not sent before the engine has a schedule to attach it to',
  );
  assert.equal(port.opsOfType('loop').length, 0, 'nothing refused, nothing lost');

  sync.flush();
  assert.deepEqual(
    port.ops.map((o) => o.op),
    ['init', 'mix', 'schedule', 'loop'],
    'the loop follows the schedule, the one order the engine accepts',
  );
});

test('T2.6 unit boundary: the policy emits the loop in document ticks, the port converts', () => {
  const port = new RecordingPort();
  const sync = new DocumentSync({
    sampleRate: SAMPLE_RATE,
    compiler: new StubCompiler(),
    port,
    clock: new ManualClock(),
  });

  sync.applyDocument(makeDoc());
  sync.flush();

  const loop = port.opsOfType('loop')[0];
  assert.ok(loop.op === 'loop');
  assert.deepEqual(loop.range, { start: 0, end: 7680 }, 'ticks, verbatim from the document');

  // The renderer loop is samples (`set_loop(Option<(u64,u64)>)`, C2.4) and the ABI has no
  // tick->sample entry point, so whoever implements PlaybackPort owns that conversion.
  // This test is the one that changes when the conversion becomes reachable.
  sync.setLoop(null);
  const cleared = port.opsOfType('loop')[1];
  assert.ok(cleared.op === 'loop');
  assert.equal(cleared.range, null, 'null clears the range on the engine side too');
});

// --- T03.AC2 ------------------------------------------------------------------------------------

test('T03.AC2: a note added during playback does not move the playhead', async () => {
  const ng = await loadNg();
  ng.engineInit(SAMPLE_RATE, 0, 64);

  const clock = new ManualClock();
  const port = new EnginePort(ng);
  const errors: SyncError[] = [];
  const sync = new DocumentSync({
    sampleRate: SAMPLE_RATE,
    compiler: new CountingCompiler(ng),
    port,
    clock,
    onError: (e) => errors.push(e),
  });

  sync.applyDocument(makeDocNoLoop());
  sync.flush();
  ng.engineTransport(0); // play
  sync.setPlaying(true);

  renderBlocks(ng, 200);
  const tickBefore = ng.enginePositionTick();
  sync.recordPosition(tickBefore);
  assert.ok(tickBefore > 0, 'the engine actually advanced');

  const before = makeDocNoLoop();
  sync.applyDocument(makeDocNoLoop({
    notes: [...before.notes, { lane: 1, start: 960, duration: 480, pitch: 77, velocity: 100 }],
  }));
  sync.flush();

  const tickAfter = ng.enginePositionTick();
  sync.recordPosition(tickAfter);

  assert.deepEqual(errors, [], 'the edit compiled cleanly');
  assert.ok(
    Math.abs(tickAfter - tickBefore) <= 1,
    `playhead jumped ${Math.abs(tickAfter - tickBefore)} ticks across the schedule swap`,
  );

  // The whole position stream, not just the swap, has to stay monotonic.
  let previous = tickAfter;
  for (let i = 0; i < 300; i += 1) {
    renderBlocks(ng, 1);
    const tick = ng.enginePositionTick();
    sync.recordPosition(tick);
    assert.ok(tick >= previous, `position went backwards: ${previous} -> ${tick}`);
    previous = tick;
  }
  assert.deepEqual(sync.positionViolations(), [], 'no backward or oversized steps');
});

test('T03.AC2: the note added mid-playback is audible in the recompiled schedule', async () => {
  const ng = await loadNg();
  ng.engineInit(SAMPLE_RATE, 0, 64);

  const port = new EnginePort(ng);
  const sync = new DocumentSync({
    sampleRate: SAMPLE_RATE,
    compiler: new CountingCompiler(ng),
    port,
    clock: new ManualClock(),
  });

  sync.applyDocument(makeDocNoLoop());
  sync.flush();
  ng.engineTransport(0);
  renderBlocks(ng, 100);

  const before = makeDocNoLoop();
  const newNote = { lane: 1, start: 1440, duration: 960, pitch: 84, velocity: 110 };
  sync.applyDocument(makeDocNoLoop({ notes: [...before.notes, newNote] }));
  sync.flush();

  const schedule = sync.loadedSchedule();
  assert.ok(schedule !== null);
  // Larger schedule than before means the note made it into the bytes, not just into the document.
  const baseSync = new DocumentSync({
    sampleRate: SAMPLE_RATE,
    compiler: new CountingCompiler(ng),
    port: new EnginePort(ng),
    clock: new ManualClock(),
  });
  baseSync.applyDocument(before);
  baseSync.flush();
  assert.ok(schedule.length > baseSync.loadedSchedule()!.length);
});

// --- T03.AC3 ------------------------------------------------------------------------------------

test('T03.AC3: a compile failure keeps the previous schedule and reports an error', async () => {
  const ng = await loadNg();
  ng.engineInit(SAMPLE_RATE, 0, 64);

  const clock = new ManualClock();
  const port = new EnginePort(ng);
  const errors: SyncError[] = [];
  const good = new CountingCompiler(ng);
  let failing = false;
  const flaky: ScheduleCompiler = {
    compile(doc, sampleRate) {
      if (failing) throw new NgError('E_RANGE', 'notes/0/pitch out of range');
      return good.compile(doc, sampleRate);
    },
  };

  const sync = new DocumentSync({
    sampleRate: SAMPLE_RATE,
    compiler: flaky,
    port,
    clock,
    onError: (e) => errors.push(e),
  });

  sync.applyDocument(makeDocNoLoop());
  sync.flush();
  const goodSchedule = sync.loadedSchedule();
  assert.ok(goodSchedule !== null);

  ng.engineTransport(0);
  renderBlocks(ng, 200);
  const positionBefore = ng.enginePositionSample();
  assert.ok(positionBefore > 0);

  // A document the compiler must reject: pitch 999 is out of the MIDI range.
  failing = true;
  sync.applyDocument(makeDocNoLoop({
    notes: [{ lane: 1, start: 0, duration: 480, pitch: 999, velocity: 100 }],
  }));
  sync.flush();

  assert.equal(errors.length, 1, 'the editor is told once');
  assert.equal(errors[0].source, 'compile');
  assert.equal(errors[0].code, 'E_RANGE');
  assert.ok(sync.loadedSchedule() === goodSchedule, 'the last good schedule is untouched');
  assert.equal(port.opsOfType('schedule').length, 1, 'no failed schedule ever reached the engine');

  // And playback is unbroken: the engine keeps running the old schedule.
  ng.engineRender(new Float32Array(128), new Float32Array(128));
  assert.ok(ng.enginePositionSample() > positionBefore, 'playback did not break mid-edit');
});

test('T03.AC3: a compile failure on the very first document loads nothing', () => {
  const port = new RecordingPort();
  const errors: SyncError[] = [];
  const sync = new DocumentSync({
    sampleRate: SAMPLE_RATE,
    compiler: new ThrowingCompiler('E_FORMAT', 'project is not valid json'),
    port,
    clock: new ManualClock(),
    onError: (e) => errors.push(e),
  });

  sync.applyDocument(makeDoc());
  sync.flush();

  assert.equal(sync.loadedSchedule(), null);
  assert.deepEqual(port.opsOfType('schedule'), []);
  assert.equal(errors[0].code, 'E_FORMAT');
});

// --- T03.AC4 ------------------------------------------------------------------------------------

test('T03.AC4: a killed worklet is recreated and the full state is replayed in order', () => {
  const clock = new ManualClock();
  const first = new RecordingPort();
  const created: RecordingPort[] = [first];
  const errors: SyncError[] = [];
  const recoveries: string[] = [];

  const sync = new DocumentSync({
    sampleRate: SAMPLE_RATE,
    compiler: new StubCompiler(),
    port: first,
    portFactory: () => {
      const port = new RecordingPort();
      created.push(port);
      return port;
    },
    clock,
    onError: (e) => errors.push(e),
    onRecover: (reason) => recoveries.push(reason),
  });

  sync.applyDocument(makeDoc());
  sync.flush();
  sync.setBank(new Uint8Array([7, 7, 7]));
  assert.deepEqual(first.ops.map((o) => o.op), [
    'init',
    'mix',
    'schedule',
    'loop',
    'bank',
  ]);

  const outcome = sync.reportTransportFailure('worklet error: InvalidStateError');

  assert.equal(outcome.recovered, true);
  assert.deepEqual(errors, [], 'a successful recovery is not an error');
  assert.equal(first.closed, 1, 'the dead node is closed');
  assert.equal(created.length, 2, 'a replacement was created');

  const replayed = created[1];
  assert.deepEqual(replayed.ops.map((o) => o.op), ['init', 'schedule', 'mix', 'loop', 'bank']);
  assert.deepEqual(outcome.ops.map((o) => o.op), [...REPLAY_ORDER]);

  const restored = replayed.opsOfType('schedule')[0];
  assert.ok(restored.op === 'schedule');
  assert.deepEqual(restored.bytes, sync.loadedSchedule(), 'the same schedule bytes come back');

  const bank = replayed.opsOfType('bank')[0];
  assert.ok(bank.op === 'bank');
  assert.deepEqual(bank.bytes, new Uint8Array([7, 7, 7]), 'the bank is not lost with the node');

  const init = replayed.opsOfType('init')[0];
  assert.ok(init.op === 'init');
  assert.equal(init.sampleRate, SAMPLE_RATE);
  assert.deepEqual(recoveries, ['worklet error: InvalidStateError']);
});

test('T03.AC4: the watchdog fires after 2 s of silence while playing', () => {
  const clock = new ManualClock();
  const port = new RecordingPort();
  const created: RecordingPort[] = [port];
  const recoveries: string[] = [];

  const sync = new DocumentSync({
    sampleRate: SAMPLE_RATE,
    compiler: new StubCompiler(),
    port,
    portFactory: () => {
      const next = new RecordingPort();
      created.push(next);
      return next;
    },
    clock,
    onRecover: (reason) => recoveries.push(reason),
  });

  sync.applyDocument(makeDoc());
  sync.flush();
  sync.setPlaying(true);
  sync.recordPosition(120);

  clock.advance(WATCHDOG_SILENCE_MS);
  assert.equal(sync.checkWatchdog(), null, 'exactly at the limit is not yet a stall');

  clock.advance(1);
  const outcome = sync.checkWatchdog();
  assert.ok(outcome !== null);
  assert.equal(outcome.reason, 'silence');
  assert.deepEqual(recoveries, ['silence']);
  assert.equal(created.length, 2);

  // A fresh node owes a position report; the watchdog must not immediately fire again.
  assert.equal(sync.checkWatchdog(), null);
  sync.recordPosition(200);
  clock.advance(WATCHDOG_SILENCE_MS + 1);
  assert.ok(sync.checkWatchdog() !== null, 'a genuinely dead replacement is caught again');
  assert.equal(created.length, 3);
});

test('T03.AC4: with no factory the caller is told instead of a silent retry', () => {
  const port = new RecordingPort();
  const errors: SyncError[] = [];
  const sync = new DocumentSync({
    sampleRate: SAMPLE_RATE,
    compiler: new StubCompiler(),
    port,
    clock: new ManualClock(),
    onError: (e) => errors.push(e),
  });

  const outcome = sync.reportTransportFailure('worklet error');
  assert.equal(outcome.recovered, false);
  assert.deepEqual(errors, [{ code: 'E_NO_PORT_FACTORY', message: 'worklet error', source: 'transport' }]);
  assert.equal(port.closed, 0, 'nothing is torn down when there is no replacement');
});

// --- helpers ------------------------------------------------------------------------------------

class StubCompiler implements ScheduleCompiler {
  private seq = 0;

  compile(): Uint8Array {
    this.seq += 1;
    return new Uint8Array([this.seq, this.seq, this.seq]);
  }
}