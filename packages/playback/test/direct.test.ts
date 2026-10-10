// <META - FILE SUMMARY - The DirectPlaybackPort: the same engine calls as the worklet, on the main thread.>

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  RENDER_BLOCK,
  DirectPlaybackPort,
  type DirectPortOptions,
  TRANSPORT_PLAY,
  TRANSPORT_STOP,
  buildMixRows,
  encodeMixRows,
  type EngineAbi,
  type PortStatus,
  type ReplayOp,
  type SyncDocument,
} from '../src/index.ts';

// --- spy engine ----------------------------------------------------------------------------

/**
 * Records every call the port makes. `engineRender` fills the caller's buffers
 * with a known waveform so a block peak can be asserted without a wasm binary.
 */
class SpyAbi implements EngineAbi {
  calls: string[] = [];
  renderResult: { playing: boolean; ended: boolean } = { playing: false, ended: false };
  renderPeak = 0;
  rendered = 0;

  engineInit(sampleRate: number, synthKind: number, maxVoices: number): void {
    this.calls.push(`init:${sampleRate}:${synthKind}:${maxVoices}`);
  }

  engineSetSchedule(bytes: Uint8Array): void {
    this.calls.push(`schedule:${bytes.length}`);
  }

  engineSetMix(bytes: Uint8Array): void {
    this.calls.push(`mix:${bytes.length}`);
  }

  engineSetLoop(start: number, end: number, enabled: boolean): void {
    this.calls.push(`loopSamples:${start}:${end}:${enabled}`);
  }

  engineSetLoopTicks(startTick: number, endTick: number, enabled: boolean): void {
    this.calls.push(`loopTicks:${startTick}:${endTick}:${enabled}`);
  }

  engineAudition(kind: number, program: number, key: number, vel: number, on: boolean): void {
    this.calls.push(`audition:${kind}:${program}:${key}:${vel}:${on}`);
  }

  engineTransport(cmd: number, arg: number): void {
    this.calls.push(`transport:${cmd}:${arg}`);
  }

  engineRender(l: Float32Array, r: Float32Array): { playing: boolean; ended: boolean } {
    this.rendered += 1;
    l[0] = this.renderPeak;
    r[0] = -this.renderPeak;
    return this.renderResult;
  }

  enginePositionSample(): number {
    return this.rendered * RENDER_BLOCK;
  }

  enginePositionTick(): number {
    return this.rendered * 11;
  }
}

const DOC: SyncDocument = {
  ppq: 480,
  tempo: [{ tick: 0, bpm: 124 }],
  meter: [{ tick: 0, num: 4, den: 4 }],
  loop: { start: 0, end: 7680 },
  lanes: [
    { id: 1, kind: 'melodic', volume: 100, pan: 64, mute: false, solo: false },
    { id: 2, kind: 'drum', volume: 90, pan: 20, mute: true, solo: false },
  ],
  notes: [],
};

function makePort(
  options: Omit<DirectPortOptions, 'abi'> = {},
): { abi: SpyAbi; port: DirectPlaybackPort } {
  const abi = new SpyAbi();
  const port = new DirectPlaybackPort({ abi, ...options });
  return { abi, port };
}

// --- replay ops ----------------------------------------------------------------------------

test('direct: an op stream becomes the same engine calls the worklet would make', () => {
  const { abi, port } = makePort();
  port.apply({ op: 'init', sampleRate: 44100 });
  port.apply({ op: 'schedule', bytes: new Uint8Array([1, 2, 3]) });
  port.apply({ op: 'mix', rows: buildMixRows(DOC) });
  port.apply({ op: 'loop', range: { start: 0, end: 7680 } });
  port.apply({ op: 'bank', bytes: new Uint8Array([9, 9]) });

  assert.deepEqual(abi.calls, [
    'init:44100:0:64',
    'schedule:3',
    `mix:${2 * 6}`,
    'loopTicks:0:7680:true',
    // P2 has no bank loader (T3.3): the op is accepted and dropped.
  ]);
});

test('direct: clearing the loop is an explicit disabled loopTicks call', () => {
  const { abi, port } = makePort();
  port.apply({ op: 'loop', range: null });
  assert.deepEqual(abi.calls, ['loopTicks:0:0:false']);
});

test('direct: a zero-lane document sends no mix message at all', () => {
  const { abi, port } = makePort();
  port.apply({ op: 'mix', rows: [] });
  assert.deepEqual(abi.calls, [], 'len 0 is E_FORMAT at the engine, so it is not sent');
});

test('direct: audition is forwarded verbatim', () => {
  const { abi, port } = makePort();
  port.audition(0, 0, 60, 100, true);
  assert.deepEqual(abi.calls, ['audition:0:0:60:100:true']);
});

// --- transport and rendering ---------------------------------------------------------------

test('direct: play and stop are the transport commands, and playing follows them', () => {
  const { abi, port } = makePort();
  port.play();
  assert.ok(port.status().playing);
  port.stop();
  assert.equal(port.status().playing, false);
  assert.deepEqual(abi.calls, [
    `transport:${TRANSPORT_PLAY}:0`,
    `transport:${TRANSPORT_STOP}:0`,
  ]);
});

test('direct: renderBlock renders one engine block and reports its peak and positions', () => {
  const statuses: PortStatus[] = [];
  const { abi, port } = makePort({ onStatus: (s) => statuses.push(s) });
  const peaks: number[] = [];

  abi.renderPeak = 0.5;
  abi.renderResult = { playing: true, ended: false };
  peaks.push(port.renderBlock());

  abi.renderPeak = 0;
  abi.renderResult = { playing: false, ended: true };
  peaks.push(port.renderBlock());

  // 0.5 is exactly representable in f32; anything else would be quantized
  // and the measured peak would differ from the written one.
  assert.deepEqual(peaks, [0.5, 0], 'the peak is measured, not inferred');
  assert.equal(abi.rendered, 2, 'exactly one engine call per block');
  assert.equal(port.status().playing, false);
  assert.equal(port.status().ended, true);
  assert.equal(port.status().sample, 2 * RENDER_BLOCK);
  assert.equal(port.status().tick, 2 * 11);

  assert.deepEqual(statuses.map((s) => s.peak), [0.5, 0], 'the peak reaches the editor');
  assert.equal(statuses[0].playing, true);
});

test('direct: an injected renderer replaces the engine call, deterministically', () => {
  const { abi, port } = makePort({
    // A test drives time by hand instead of racing a timer.
    render: (_abi, left, _right) => {
      left[0] = 0.125;
      return 0.125;
    },
  });
  assert.equal(port.renderBlock(), 0.125);
  assert.equal(abi.rendered, 0, 'engineRender is not called when a renderer is injected');
});

test('direct: a closed port refuses further work', () => {
  const { port } = makePort();
  port.close();
  assert.throws(() => port.apply({ op: 'loop', range: null } as ReplayOp), /closed/);
  assert.throws(() => port.renderBlock(), /closed/);
  assert.throws(() => port.play(), /closed/);
});

// --- mix encoding through the port ---------------------------------------------------------

test('direct: the mix op carries the encoded rows of the resolved document mix', () => {
  const { abi, port } = makePort();
  const rows = buildMixRows(DOC);
  port.apply({ op: 'mix', rows });
  assert.equal(abi.calls[0], `mix:${encodeMixRows(rows).length}`);
  // The resolved mix is what the port encodes: volume/pan/send in the 0..=127
  // document scale, with the solo rule already folded into mute.
  assert.deepEqual(
    rows.map((r) => [r.lane, r.volume, r.pan, r.muted]),
    [
      [0, 100, 64, false],
      [1, 90, 20, true],
    ],
  );
});
