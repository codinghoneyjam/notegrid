import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { loadNg } from '../src/index.ts';

test('abi exports match expected set (g08)', async () => {
  const bytes = readFileSync(new URL('../dist/ng-wasm.wasm', import.meta.url));
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const names = Object.keys(instance.exports).sort();
  assert.deepEqual(names, [
    'memory', 'ng_abi_version', 'ng_alloc', 'ng_aux_len', 'ng_aux_ptr',
    'ng_error_len', 'ng_error_ptr', 'ng_free', 'ng_gm_tables',
    'ng_project_canonical', 'ng_project_migrate', 'ng_project_validate',
    'ng_result_len', 'ng_result_ptr', 'ng_compile',
    'ng_engine_init', 'ng_engine_set_schedule', 'ng_engine_set_mix',
    'ng_engine_transport', 'ng_engine_set_loop', 'ng_engine_set_loop_ticks',
    'ng_engine_audition',
    'ng_engine_render', 'ng_engine_position_sample', 'ng_engine_position_tick',
    'ng_offline_begin', 'ng_offline_step', 'ng_offline_progress',
    'ng_offline_cancel', 'ng_offline_finish', 'ng_render_wav',
  ].sort());
});

test('wrappers drive validate/migrate/canonical/gm_tables', async () => {
  const ng = await loadNg();
  assert.equal(ng.abiVersion(), 1);
  const project = readFileSync(new URL('../../../spec/fixture/example_project.json', import.meta.url), 'utf8');
  const report = ng.validate(project);
  assert.equal(report.errors.length, 0);
  const canonical = ng.canonical(project);
  const again = ng.canonical(canonical);
  assert.equal(canonical, again);
  const migrated = ng.migrate(project);
  assert.ok(migrated.includes('"format_version": 1'));
  const tables = ng.gmTables() as { programs: string[]; families: string[] };
  assert.equal(tables.programs.length, 128);
  assert.equal(tables.families.length, 16);
});

test('invalid payload returns error / failing report without trap', async () => {
  const ng = await loadNg();
  const bad = ng.validate('{not json');
  assert.ok(bad.errors.length >= 1);
  assert.throws(() => ng.canonical('{"format_version": 99}'));
});

test('invalid pointer/length tolerated (no trap)', async () => {
  const { instance } = await WebAssembly.instantiate(readFileSync(new URL('../dist/ng-wasm.wasm', import.meta.url)), {});
  const ex = instance.exports as Record<string, Function> & { memory: WebAssembly.Memory };
  // ptr 0 / len 0 -> error code, never a trap
  assert.notEqual(ex.ng_project_validate(0, 0), 0);
  assert.notEqual(ex.ng_project_migrate(0, 0), 0);
  assert.notEqual(ex.ng_project_canonical(0, 0), 0);
  // buffer with garbage content -> error code, never a trap
  const ptr = ex.ng_alloc(16);
  new Uint8Array(ex.memory.buffer, ptr, 16).fill(0xff);
  const rc = ex.ng_project_validate(ptr, 16);
  assert.equal(rc, 0); // validate always returns a Report (0)
  const report = JSON.parse(new TextDecoder().decode(new Uint8Array(ex.memory.buffer, ex.ng_result_ptr(), ex.ng_result_len())));
  assert.ok(report.errors.length >= 1);
  ex.ng_free(ptr, 16);
});

test('R07.AC1: Node drives example project -> schedule -> offline WAV', async () => {
  const ng = await loadNg();
  const project = readFileSync(new URL('../../../spec/fixture/example_project.json', import.meta.url), 'utf8');
  
  // Compile to schedule
  const scheduleBytes = ng.compile(project, 44100);
  assert.ok(scheduleBytes.length > 0);
  
  // Render to WAV using the existing renderWav (which uses the same path as offline)
  const wavBytes = ng.renderWav(project, 44100, 0); // 0 = Int16
  assert.ok(wavBytes.length > 0);
  
  // Verify WAV header
  assert.equal(Buffer.from(wavBytes.slice(0, 4)).toString(), 'RIFF');
  assert.equal(Buffer.from(wavBytes.slice(8, 12)).toString(), 'WAVE');
  
  // T05.AC1: verify byte size matches ng-cli output (2447006 bytes for Int16)
  assert.equal(wavBytes.length, 2447006);
});

// --- helpers for the engine-entry-point tests ---------------------------------------------

const BLOCK = 128;
const EXAMPLE_PROJECT = new URL('../../../spec/fixture/example_project.json', import.meta.url);

function exampleProject(): string {
  return readFileSync(EXAMPLE_PROJECT, 'utf8');
}

/** C3 header decode, written against the contract layout rather than against any Rust code. */
interface C3Header {
  sampleRate: number;
  noteCount: number;
  laneCount: number;
  tempoCount: number;
  firstMpq: number;
}

function decodeC3(bytes: Uint8Array): C3Header {
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  assert.equal(
    String.fromCharCode(bytes[0], bytes[1], bytes[2], bytes[3]),
    'NGSC',
    'C3 magic',
  );
  assert.equal(v.getUint16(4, true), 1, 'C3 version');
  const sampleRate = v.getUint32(8, true);
  const noteCount = v.getUint32(12, true);
  const laneCount = v.getUint32(16, true);
  const tempoCount = v.getUint32(20, true);
  assert.ok(tempoCount >= 1, 'a schedule always carries at least one tempo entry');
  // lanes are 8 bytes each and come first; the tempo table is 16 bytes per entry
  const tempoBase = 32 + laneCount * 8;
  const firstMpq = v.getUint32(tempoBase + 4, true);
  return { sampleRate, noteCount, laneCount, tempoCount, firstMpq };
}

/**
 * C0.3 tick -> sample for a single tempo segment: sample = round_half_up(T * sr / 480e6)
 * with T = delta_tick * mpq. Computed here from the rule, not copied from the Rust side, so a
 * change to the rule is caught (the second-opinion pattern of gen_tempo_vectors.py).
 */
function c03Samples(tick: number, mpq: number, sr: number): number {
  const t = tick * mpq;
  const den = 480 * 1_000_000;
  return Math.floor((2 * t * sr + den) / (2 * den));
}

function mixRecords(laneCount: number, flags: number): Uint8Array {
  const bytes = new Uint8Array(laneCount * 6);
  const v = new DataView(bytes.buffer);
  for (let i = 0; i < laneCount; i++) {
    const at = i * 6;
    v.setUint16(at, i, true); // slot
    v.setUint8(at + 2, 100); // volume
    v.setUint8(at + 3, 64); // pan (centre)
    v.setUint8(at + 4, 0); // reverb send
    v.setUint8(at + 5, flags); // bit0 = mute
  }
  return bytes;
}

function isSilent(l: Float32Array, r: Float32Array): boolean {
  return l.every((x) => x === 0) && r.every((x) => x === 0);
}

// --- the engine entry points ---------------------------------------------------------------

test('ng_engine_set_loop_ticks: the tick-domain loop wraps at the C0.3 example sample', async () => {
  const ng = await loadNg();
  const scheduleBytes = ng.compile(exampleProject(), 44100);
  const c3 = decodeC3(scheduleBytes);
  assert.equal(c3.sampleRate, 44100, 'C3 header carries the compile rate');
  assert.equal(c3.firstMpq, 483871, 'the fixture is 124 bpm per C0.2 (mpq = 60e6 / bpm)');

  // The expected wrap point is the contract example, recomputed here from the C0.3 rule.
  const expected = c03Samples(7680, c3.firstMpq, 44100);
  assert.equal(expected, 341419, 'C0.3 EXAMPLE: 7680 ticks -> 341419 samples @44100');

  ng.engineInit(44100, 0, 64);
  ng.engineSetSchedule(scheduleBytes);
  ng.engineSetLoopTicks(0, 7680, true);

  ng.engineTransport(0, 0); // play
  const l = new Float32Array(BLOCK);
  const r = new Float32Array(BLOCK);
  let prev = 0;
  let wrappedFrom = -1;
  for (let i = 0; i < 10_000 && wrappedFrom < 0; i++) {
    ng.engineRender(l, r);
    const pos = ng.enginePositionSample();
    if (pos < prev) {
      wrappedFrom = prev;
      break;
    }
    prev = pos;
  }
  assert.ok(wrappedFrom >= 0, 'the playhead must wrap at the loop end');
  // A block never crosses the loop end, so the wrap is visible within one block of `expected`.
  assert.ok(
    wrappedFrom >= expected - BLOCK && wrappedFrom < expected,
    `wrap must land within one block of ${expected}, got ${wrappedFrom}`,
  );
  assert.ok(
    ng.enginePositionSample() < BLOCK,
    'after the wrap the playhead is back inside the loop',
  );
});

test('ng_engine_set_loop_ticks: a loop set before any schedule fails, clearing does not', async () => {
  const ng = await loadNg();
  ng.engineInit(44100, 0, 64);
  assert.throws(
    () => ng.engineSetLoopTicks(0, 7680, true),
    (err: any) => err.code === 'E_ENGINE',
    'enabled != 0 with no schedule is E_ENGINE (-3), never a silent success',
  );
  ng.engineSetLoopTicks(0, 0, false); // clearing is idempotent and needs no schedule
});

test('ng_engine_set_mix: the 6-byte record drives gain and mute', async () => {
  const ng = await loadNg();
  const scheduleBytes = ng.compile(exampleProject(), 44100);
  const laneCount = decodeC3(scheduleBytes).laneCount;
  ng.engineInit(44100, 0, 64);
  ng.engineSetSchedule(scheduleBytes);
  ng.engineSetMix(mixRecords(laneCount, 0));
  ng.engineTransport(0, 0); // play

  const l = new Float32Array(BLOCK);
  const r = new Float32Array(BLOCK);
  let audible = false;
  for (let i = 0; i < 1000 && !audible; i++) {
    ng.engineRender(l, r);
    audible = !isSilent(l, r);
  }
  assert.ok(audible, 'the unmixed lanes must sound');

  // Muting every lane is what silences the mix: any_solo is recomputed false, so an
  // unmuted lane would keep sounding.
  ng.engineSetMix(mixRecords(laneCount, 1));
  for (let i = 0; i < Math.ceil(44100 / BLOCK); i++) {
    ng.engineRender(l, r);
  }
  assert.ok(isSilent(l, r), 'the mix must be silent once every lane is muted (5 ms ramp)');
});

test('ng_engine_set_mix: a malformed payload is refused without a trap (C4.2)', async () => {
  const ng = await loadNg();
  ng.engineInit(44100, 0, 64);
  // len % 6 != 0 -> E_FORMAT
  assert.throws(
    () => ng.engineSetMix(new Uint8Array(7)),
    (err: any) => err.code === 'E_FORMAT',
  );
  // a reserved flag bit set -> E_FORMAT
  assert.throws(
    () => ng.engineSetMix(mixRecords(1, 0b100)),
    (err: any) => err.code === 'E_FORMAT',
  );
  // slot 128 is AUDITION_SLOT and is unreachable from the mix wire -> E_RANGE
  const badSlot = mixRecords(1, 0);
  new DataView(badSlot.buffer).setUint16(0, 128, true);
  assert.throws(
    () => ng.engineSetMix(badSlot),
    (err: any) => err.code === 'E_RANGE',
  );
});

test('ng_engine_audition: the note sounds with the transport stopped, and releases', async () => {
  const ng = await loadNg();
  ng.engineInit(44100, 0, 64);
  // No schedule is loaded and the transport stays stopped: AUDITION_SLOT is the only voice,
  // and voices render regardless of transport state (the core T03 SYNC behaviour).
  ng.engineAudition(0, 0, 60, 100, true);
  assert.equal(ng.enginePositionSample(), 0, 'a stopped transport does not advance');

  const l = new Float32Array(BLOCK);
  const r = new Float32Array(BLOCK);
  let audible = false;
  for (let i = 0; i < 100 && !audible; i++) {
    ng.engineRender(l, r);
    audible = !isSilent(l, r);
  }
  assert.ok(audible, 'the audition voice must sound while the transport is stopped');
  assert.equal(ng.enginePositionSample(), 0, 'still stopped after rendering');

  ng.engineAudition(0, 0, 60, 0, false);
  // Render 2 s and look only at the last second: the release must have finished by then.
  let tailSilent = false;
  for (let i = 0; i < Math.ceil(88_200 / BLOCK); i++) {
    ng.engineRender(l, r);
    if (i * BLOCK >= 44_100) {
      tailSilent = isSilent(l, r);
    }
  }
  assert.ok(tailSilent, 'the last second after release must be silent');

  assert.throws(() => ng.engineAudition(2, 0, 60, 100, true), (err: any) => err.code === 'E_RANGE');
  assert.throws(() => ng.engineAudition(0, 128, 60, 100, true), (err: any) => err.code === 'E_RANGE');
  assert.throws(() => ng.engineAudition(0, 0, 128, 100, true), (err: any) => err.code === 'E_RANGE');
  assert.throws(() => ng.engineAudition(0, 0, 60, 0, true), (err: any) => err.code === 'E_RANGE');
});

test('wrapper: engineSetLoopTicks forwards (startTick, endTick, enabled ? 1 : 0)', async () => {
  const ng = await loadNg();
  assert.equal(typeof ng.engineSetLoopTicks, 'function');
  ng.engineInit(44100, 0, 64);
  ng.engineSetSchedule(ng.compile(exampleProject(), 44100));
  ng.engineSetLoopTicks(0, 7680, true);
  ng.engineSetLoopTicks(0, 7680, false);
  // Forwarding is observable: the argument order is not swapped, so a reversed call would
  // be a degenerate range that set_loop drops and the playhead would never wrap.
  ng.engineSetLoopTicks(0, 7680, true);
  ng.engineTransport(0, 0);
  const l = new Float32Array(BLOCK);
  const r = new Float32Array(BLOCK);
  let wrapped = false;
  let prev = 0;
  for (let i = 0; i < 10_000 && !wrapped; i++) {
    ng.engineRender(l, r);
    const pos = ng.enginePositionSample();
    if (pos < prev) {
      wrapped = true;
      break;
    }
    prev = pos;
  }
  assert.ok(wrapped, 'a forwarded (0, 7680) loop must wrap the playhead');
});
