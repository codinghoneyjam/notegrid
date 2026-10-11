// <META - FILE SUMMARY - The mix wire format and the worklet port's FIFO, queueing and event pump.>

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  MIX_RECORD_BYTES,
  PROCESSOR_NAME,
  TRANSPORT_PAUSE,
  TRANSPORT_PLAY,
  TRANSPORT_STOP,
  WorkletPlaybackPort,
  buildMixRows,
  encodeMixRows,
  loopTickMessage,
  type MixRow,
  type PortEvent,
  type PortNode,
  type ReplayOp,
  type SyncDocument,
} from '../src/index.ts';

// --- mix wire -----------------------------------------------------------------------------

/** Three lanes with distinct mix values, and nobody soloed: the solo case is built on top. */
const DOC: SyncDocument = {
  ppq: 480,
  tempo: [{ tick: 0, bpm: 124 }],
  meter: [{ tick: 0, num: 4, den: 4 }],
  loop: { start: 0, end: 7680 },
  lanes: [
    { id: 1, kind: 'melodic', volume: 100, pan: 64, mute: false, solo: false },
    { id: 2, kind: 'drum', volume: 90, pan: 20, mute: true, solo: false },
    { id: 3, kind: 'melodic', volume: 127, pan: 100, mute: false, solo: false },
  ],
  notes: [],
};

/** The 6 bytes of one lane record, read little-endian as the engine reads them. */
function view(bytes: Uint8Array, lane: number): DataView {
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
}

function record(bytes: Uint8Array, lane: number): DataView {
  const at = lane * MIX_RECORD_BYTES;
  return new DataView(bytes.buffer, bytes.byteOffset + at, MIX_RECORD_BYTES);
}

test('mixwire: the record is 6 packed little-endian bytes per lane', () => {
  const bytes = encodeMixRows(buildMixRows(DOC));
  assert.equal(bytes.length, DOC.lanes.length * MIX_RECORD_BYTES);
  assert.equal(bytes.byteOffset + bytes.byteLength, view(bytes, 0).byteLength + bytes.byteOffset);

  const first = record(bytes, 0);
  assert.equal(first.getUint16(0, true), 0, 'slot is the lane index (C2.8)');
  assert.equal(first.getUint8(2), 100, 'volume, document scale');
  assert.equal(first.getUint8(3), 64, 'pan');
  assert.equal(first.getUint8(4), 0, 'reverb send');
  assert.equal(first.getUint8(5), 0, 'flags bit0 = mute');
});

test('mixwire: solo is folded into mute and never travels as its own bit', () => {
  // The port policy owns the solo rule (mix.ts:20-29), so the wire only ever carries mute
  // and the engine recomputes any_solo = false. One definition of the rule, not two.
  assert.deepEqual(
    buildMixRows(DOC).map((r) => r.muted),
    [false, true, false],
    'with nothing soloed, the explicit mute on lane 2 is the only mute',
  );

  // A solo on lane 3 mutes lanes 1 and 2. The wire carries only the resulting mute bit.
  const soloed = buildMixRows({
    ...DOC,
    lanes: DOC.lanes.map((lane) => (lane.id === 3 ? { ...lane, solo: true } : { ...lane })),
  });
  assert.deepEqual(
    soloed.map((r) => r.muted),
    [true, true, false],
    'the un-soloed lanes are muted while lane 3 is soloed',
  );

  const bytes = encodeMixRows(soloed);
  assert.equal(record(bytes, 0).getUint8(5) & 0b1111_1110, 0, 'reserved flag bits are zero');
  assert.equal(record(bytes, 0).getUint8(5), 1, 'lane 1 is muted on the wire');
  assert.equal(record(bytes, 2).getUint8(5), 0, 'the soloed lane itself is not muted');
  assert.equal(record(bytes, 2).getUint8(2), 127, 'and its own volume is untouched');
});

test('mixwire: a value outside 0..=127 is refused at the edit, not at the engine', () => {
  const bad: MixRow = { lane: 0, volume: 128, pan: 64, send: 0, muted: false };
  assert.throws(() => encodeMixRows([bad]), RangeError);
  assert.throws(
    () => encodeMixRows([{ lane: 128, volume: 100, pan: 64, send: 0, muted: false }]),
    RangeError,
  );
  assert.throws(
    () => encodeMixRows([{ lane: 0, volume: 100, pan: -1, send: 0, muted: false }]),
    RangeError,
  );
});

test('mixwire: no lanes means no bytes and no message', () => {
  // len 0 is E_FORMAT at the engine (read_bytes rejects it), so the port must not send it.
  assert.equal(encodeMixRows([]).length, 0);
});

// --- loop message -------------------------------------------------------------------------

test('loop: the message carries document ticks, and clearing is enabled = 0', () => {
  assert.deepEqual(loopTickMessage({ start: 0, end: 7680 }), {
    start_tick: 0,
    end_tick: 7680,
    enabled: 1,
  });
  assert.deepEqual(loopTickMessage(null), { start_tick: 0, end_tick: 0, enabled: 0 });
});

// --- worklet port -------------------------------------------------------------------------

class FakeNode implements PortNode {
  readonly port = {
    sent: [] as unknown[],
    onmessage: null as ((event: { data: unknown }) => void) | null,
    postMessage: (message: unknown): void => {
      this.port.sent.push(message);
    },
  };
  connected = 0;
  disconnected = 0;

  connect(): void {
    this.connected += 1;
  }

  disconnect(): void {
    this.disconnected += 1;
  }

  /** Simulates the processor answering. */
  emit(event: PortEvent): void {
    this.port.onmessage?.({ data: event });
  }

  sentOps(): string[] {
    return this.port.sent.map((m) => (m as { t: string }).t);
  }
}

function makePort(): { node: FakeNode; port: WorkletPlaybackPort; events: PortEvent[] } {
  const node = new FakeNode();
  const events: PortEvent[] = [];
  const port = new WorkletPlaybackPort({
    node,
    onReady: (sampleRate) => events.push({ t: 'ready', sampleRate }),
    onPosition: (sample, tick, playing) => events.push({ t: 'position', sample, tick, playing }),
    onBlock: (frames, peak) => events.push({ t: 'block', frames, peak }),
    onEnded: () => events.push({ t: 'ended' }),
    onError: (code, message) => events.push({ t: 'error', code, message }),
  });
  return { node, port, events };
}

const OPS: ReplayOp[] = [
  { op: 'init', sampleRate: 44100 },
  { op: 'schedule', bytes: new Uint8Array([1, 2, 3, 4]) },
  { op: 'mix', rows: buildMixRows(DOC) },
  { op: 'loop', range: { start: 0, end: 7680 } },
];

test('port: only the bootstrap reaches the wire before the worklet answers ready', () => {
  // Recovery builds a port and immediately replays the snapshot into it. The bootstrap
  // `init` is posted at once -- the processor answers `ready` only after it has processed
  // it, so a port that held it would deadlock -- and every later message is held, not
  // dropped.
  const { node, port } = makePort();
  for (const op of OPS) port.apply(op);
  assert.deepEqual(node.sentOps(), ['init'], 'the bootstrap is on the wire');
  assert.equal(port.queued(), OPS.length - 1);

  node.emit({ t: 'ready', sampleRate: 44100 });
  assert.deepEqual(node.sentOps(), ['init', 'schedule', 'mix', 'loop'], 'T03.R05 FIFO order');
  assert.equal(port.queued(), 0, 'the queue is drained');
});

test('port: a message sent after ready goes straight out', () => {
  const { node, port } = makePort();
  node.emit({ t: 'ready', sampleRate: 44100 });
  port.apply({ op: 'transport' as never } as never); // not a ReplayOp: must not throw
  port.apply({ op: 'loop', range: null });
  assert.deepEqual(node.sentOps(), ['loop']);
  assert.deepEqual(node.port.sent[0], { t: 'loop', start_tick: 0, end_tick: 0, enabled: 0 });
});

test('port: a zero-lane document sends no mix message at all', () => {
  const { node, port } = makePort();
  node.emit({ t: 'ready', sampleRate: 44100 });
  port.apply({ op: 'mix', rows: [] });
  assert.deepEqual(node.sentOps(), [], 'len 0 is E_FORMAT at the engine, so it is not sent');
});

test('port: play, pause and stop are distinct transport commands', () => {
  const { node, port } = makePort();
  node.emit({ t: 'ready', sampleRate: 44100 });
  port.play();
  port.pause();
  port.stop();
  assert.deepEqual(
    node.port.sent.map((m) => (m as { cmd: number }).cmd),
    [TRANSPORT_PLAY, TRANSPORT_PAUSE, TRANSPORT_STOP],
    'the engine has three commands; the port does not collapse them',
  );
});

test('port: the schedule bytes are copied, so the replay snapshot is not detached', () => {
  // The policy keeps the same Uint8Array in its snapshot for recovery. Transferring it
  // would detach the buffer the snapshot still points at.
  const bytes = new Uint8Array([9, 9, 9, 9]);
  const { node, port } = makePort();
  node.emit({ t: 'ready', sampleRate: 44100 });
  port.apply({ op: 'schedule', bytes });

  const sent = node.port.sent[0] as { bytes: ArrayBuffer };
  assert.equal(bytes.length, 4, 'the caller still has its buffer');
  assert.equal(bytes[0], 9);
  assert.equal(new Uint8Array(sent.bytes)[0], 9, 'and the worklet got the same bytes');
  assert.notEqual(sent.bytes, bytes.buffer, 'through a copy, not the original buffer');
});

test('port: audition goes out with on as 1 or 0, matching the C5 field', () => {  const { node, port } = makePort();
  node.emit({ t: 'ready', sampleRate: 44100 });
  port.audition(0, 0, 60, 100, true);
  port.audition(0, 0, 60, 0, false);
  assert.deepEqual(node.port.sent[0], { t: 'audition', kind: 0, program: 0, key: 60, vel: 100, on: 1 });
  assert.deepEqual(node.port.sent[1], { t: 'audition', kind: 0, program: 0, key: 60, vel: 0, on: 0 });
});

test('port: worklet events are surfaced, and a block peak is what g10 reads', () => {
  const { node, events } = makePort();
  node.emit({ t: 'block', frames: 128, peak: 0 });
  node.emit({ t: 'block', frames: 128, peak: 0.42 });
  node.emit({ t: 'position', sample: 1024, tick: 11, playing: true });
  node.emit({ t: 'ended' });
  node.emit({ t: 'error', code: 'E_ENGINE', message: 'render failed' });

  assert.deepEqual(events.map((e) => e.t), ['block', 'block', 'position', 'ended', 'error']);
  const loud = events[1] as { peak: number };
  assert.equal(loud.peak, 0.42, 'the non-silent block is observable, not inferred');
});

test('port: an unknown event is ignored rather than crashing the pump', () => {
  const { node, events } = makePort();
  node.emit({ t: 'nonsense' } as never);
  node.emit(null as never);
  assert.deepEqual(events, []);
});

test('port: close detaches the handler and cannot be done twice', () => {
  const { node, port } = makePort();
  node.emit({ t: 'ready', sampleRate: 44100 });
  port.close();
  assert.equal(node.disconnected, 1);
  assert.equal(node.port.onmessage, null, 'a late message has nowhere to land');

  port.close();
  assert.equal(node.disconnected, 1, 'closing twice is not a second disconnect');
  assert.throws(() => port.apply({ op: 'loop', range: null }), /closed/);
});

test('port: the processor name is the one the app registers', () => {
  assert.equal(PROCESSOR_NAME, 'ng-engine', 'playback owns the name; app registers the processor');
});