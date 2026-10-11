// <META - FILE SUMMARY - Typed TS wrapper over the C ABI (C4/R07). Hides pointer management; copies buffers.>

export class NgError extends Error {
  code: string;
  path?: string;
  constructor(code: string, message: string, path?: string) {
    super(message);
    this.code = code;
    this.path = path;
  }
}

interface WasmExports {
  ng_abi_version(): number;
  ng_alloc(len: number): number;
  ng_free(ptr: number, len: number): void;
  ng_result_ptr(): number;
  ng_result_len(): number;
  ng_aux_ptr(): number;
  ng_aux_len(): number;
  ng_error_ptr(): number;
  ng_error_len(): number;
  ng_project_validate(ptr: number, len: number): number;
  ng_project_migrate(ptr: number, len: number): number;
  ng_project_canonical(ptr: number, len: number): number;
  ng_compile(ptr: number, len: number, sr: number): number;
  ng_gm_tables(): number;
  ng_render_wav(ptr: number, len: number, sr: number, depth: number): number;
  ng_engine_init(sr: number, synth_kind: number, max_voices: number): number;
  ng_engine_set_schedule(ptr: number, len: number): number;
  ng_engine_set_mix(ptr: number, len: number): number;
  ng_engine_transport(cmd: number, arg: number): number;
  ng_engine_set_loop(start: number, end: number, enabled: number): number;
  ng_engine_set_loop_ticks(start_tick: number, end_tick: number, enabled: number): number;
  ng_engine_audition(kind: number, program: number, key: number, vel: number, on: number): number;
  ng_engine_render(l_ptr: number, r_ptr: number, frames: number): number;
  ng_engine_position_sample(): number;
  ng_engine_position_tick(): number;
  ng_offline_begin(proj_ptr: number, proj_len: number, opts_ptr: number, opts_len: number): number;
  ng_offline_step(max_frames: number): number;
  ng_offline_progress(): number;
  ng_offline_cancel(): void;
  ng_offline_finish(): number;
  memory: WebAssembly.Memory;
}

export interface Report {
  errors: Array<{ code: string; message: string; path: string }>;
  warnings: Array<{ code: string; message: string; path: string }>;
}

export class Ng {
  private exports: WasmExports;
  /**
   * Resolved lazily, because the AudioWorkletGlobalScope has no `TextDecoder`
   * (DECIDE D05, spike SP-1). As a field initializer this ran at construction and threw a
   * `ReferenceError` inside the audio thread, which the page only ever saw as a bare
   * `onprocessorerror`: the worklet died before answering `ready`, so playback was silence
   * with no message anywhere. The engine path never decodes UTF-8 -- its calls are numbers
   * and binary buffers -- so the decoder is only built if a JSON method is actually used.
   */
  private td: TextDecoder | null = null;

  constructor(exports: WasmExports) {
    this.exports = exports;
  }

  private decoder(): TextDecoder {
    if (this.td === null) {
      const Ctor = (globalThis as { TextDecoder?: new () => TextDecoder }).TextDecoder;
      if (Ctor === undefined) {
        throw new NgError(
          "E_NO_TEXT_DECODER",
          "TextDecoder is unavailable in this realm; the JSON methods cannot run here",
        );
      }
      this.td = new Ctor();
    }
    return this.td;
  }

  abiVersion(): number {
    return this.exports.ng_abi_version();
  }

  private call(fn: (ptr: number, len: number, ...rest: number[]) => number, input: Uint8Array, ...rest: number[]): { out: Uint8Array; aux: Uint8Array } {
    const ex = this.exports;
    const inPtr = ex.ng_alloc(input.length);
    try {
      new Uint8Array(ex.memory.buffer, inPtr, input.length).set(input);
      const rc = fn(inPtr, input.length, ...rest);
      if (rc !== 0) {
        throw this.readError(rc);
      }
      const outPtr = ex.ng_result_ptr();
      const outLen = ex.ng_result_len();
      const auxPtr = ex.ng_aux_ptr();
      const auxLen = ex.ng_aux_len();
      const out = new Uint8Array(outLen);
      out.set(new Uint8Array(ex.memory.buffer, outPtr, outLen));
      const aux = new Uint8Array(auxLen);
      aux.set(new Uint8Array(ex.memory.buffer, auxPtr, auxLen));
      return { out, aux };
    } finally {
      ex.ng_free(inPtr, input.length);
    }
  }

  private readError(rc: number): NgError {
    const ex = this.exports;
    const ptr = ex.ng_error_ptr();
    const len = ex.ng_error_len();
    const bytes = new Uint8Array(ex.memory.buffer, ptr, len);
    try {
      const parsed = JSON.parse(this.decoder().decode(bytes));
      return new NgError(parsed.code ?? `E_RC_${rc}`, parsed.message ?? "unknown");
    } catch {
      return new NgError(`E_RC_${rc}`, "unknown");
    }
  }

  validate(json: string): Report {
    const { out } = this.call(this.exports.ng_project_validate.bind(this.exports), new TextEncoder().encode(json));
    return JSON.parse(this.decoder().decode(out)) as Report;
  }

  migrate(json: string): string {
    const { out } = this.call(this.exports.ng_project_migrate.bind(this.exports), new TextEncoder().encode(json));
    return this.decoder().decode(out);
  }

  canonical(json: string): string {
    const { out } = this.call(this.exports.ng_project_canonical.bind(this.exports), new TextEncoder().encode(json));
    return this.decoder().decode(out);
  }

  gmTables(): unknown {
    const ex = this.exports;
    const rc = ex.ng_gm_tables();
    if (rc !== 0) throw this.readError(rc);
    const ptr = ex.ng_result_ptr();
    const len = ex.ng_result_len();
    const bytes = new Uint8Array(ex.memory.buffer, ptr, len);
    return JSON.parse(this.decoder().decode(bytes));
  }

  renderWav(json: string, sampleRate = 44100, depth: 0 | 1 | 2 = 0): Uint8Array {
    const { out } = this.call(
      this.exports.ng_render_wav.bind(this.exports),
      new TextEncoder().encode(json),
      sampleRate,
      depth,
    );
    return out;
  }

  compile(json: string, sampleRate = 44100): Uint8Array {
    const { out } = this.call(
      this.exports.ng_compile.bind(this.exports),
      new TextEncoder().encode(json),
      sampleRate,
    );
    return out;
  }

  engineInit(sampleRate = 44100, synthKind = 0, maxVoices = 64): void {
    const rc = this.exports.ng_engine_init(sampleRate, synthKind, maxVoices);
    if (rc !== 0) throw this.readError(rc);
  }

  engineSetSchedule(bytes: Uint8Array): void {
    const ex = this.exports;
    const inPtr = ex.ng_alloc(bytes.length);
    try {
      new Uint8Array(ex.memory.buffer, inPtr, bytes.length).set(bytes);
      const rc = ex.ng_engine_set_schedule(inPtr, bytes.length);
      if (rc !== 0) throw this.readError(rc);
    } finally {
      ex.ng_free(inPtr, bytes.length);
    }
  }

  engineSetMix(bytes: Uint8Array): void {
    const ex = this.exports;
    const inPtr = ex.ng_alloc(bytes.length);
    try {
      new Uint8Array(ex.memory.buffer, inPtr, bytes.length).set(bytes);
      const rc = ex.ng_engine_set_mix(inPtr, bytes.length);
      if (rc !== 0) throw this.readError(rc);
    } finally {
      ex.ng_free(inPtr, bytes.length);
    }
  }

  engineTransport(cmd: number, arg = 0): void {
    const rc = this.exports.ng_engine_transport(cmd, arg);
    if (rc !== 0) throw this.readError(rc);
  }

  engineSetLoop(start: number, end: number, enabled: boolean): void {
    const rc = this.exports.ng_engine_set_loop(start, end, enabled ? 1 : 0);
    if (rc !== 0) throw this.readError(rc);
  }

  engineSetLoopTicks(startTick: number, endTick: number, enabled: boolean): void {
    const rc = this.exports.ng_engine_set_loop_ticks(startTick, endTick, enabled ? 1 : 0);
    if (rc !== 0) throw this.readError(rc);
  }

  engineAudition(kind: number, program: number, key: number, vel: number, on: boolean): void {
    const rc = this.exports.ng_engine_audition(kind, program, key, vel, on ? 1 : 0);
    if (rc !== 0) throw this.readError(rc);
  }

  engineRender(l: Float32Array, r: Float32Array): { playing: boolean; ended: boolean } {
    const ex = this.exports;
    const lPtr = ex.ng_alloc(l.length * 4);
    const rPtr = ex.ng_alloc(r.length * 4);
    try {
      new Float32Array(ex.memory.buffer, lPtr, l.length).set(l);
      new Float32Array(ex.memory.buffer, rPtr, r.length).set(r);
      const status = ex.ng_engine_render(lPtr, rPtr, l.length);
      const playing = (status & 1) !== 0;
      const ended = (status & 2) !== 0;
      l.set(new Float32Array(ex.memory.buffer, lPtr, l.length));
      r.set(new Float32Array(ex.memory.buffer, rPtr, r.length));
      return { playing, ended };
    } finally {
      ex.ng_free(lPtr, l.length * 4);
      ex.ng_free(rPtr, r.length * 4);
    }
  }

  enginePositionSample(): number {
    return this.exports.ng_engine_position_sample();
  }

  enginePositionTick(): number {
    return this.exports.ng_engine_position_tick();
  }

  offlineBegin(projectJson: string, options?: string): void {
    const projBytes = new TextEncoder().encode(projectJson);
    const optsBytes = options ? new TextEncoder().encode(options) : new Uint8Array(0);
    const ex = this.exports;
    const projPtr = ex.ng_alloc(projBytes.length);
    const optsPtr = ex.ng_alloc(optsBytes.length);
    try {
      new Uint8Array(ex.memory.buffer, projPtr, projBytes.length).set(projBytes);
      new Uint8Array(ex.memory.buffer, optsPtr, optsBytes.length).set(optsBytes);
      const rc = ex.ng_offline_begin(projPtr, projBytes.length, optsPtr, optsBytes.length);
      if (rc !== 0) throw this.readError(rc);
    } finally {
      ex.ng_free(projPtr, projBytes.length);
      ex.ng_free(optsPtr, optsBytes.length);
    }
  }

  offlineStep(maxFrames: number): number {
    return this.exports.ng_offline_step(maxFrames);
  }

  offlineProgress(): number {
    return this.exports.ng_offline_progress();
  }

  offlineCancel(): void {
    this.exports.ng_offline_cancel();
  }

  offlineFinish(): Uint8Array {
    const rc = this.exports.ng_offline_finish();
    if (rc !== 0) throw this.readError(rc);
    const ptr = this.exports.ng_result_ptr();
    const len = this.exports.ng_result_len();
    const bytes = new Uint8Array(len);
    bytes.set(new Uint8Array(this.exports.memory.buffer, ptr, len));
    return bytes;
  }
}

export async function loadNg(module?: WebAssembly.Module | BufferSource): Promise<Ng> {
  let instance: WebAssembly.Instance;
  if (module instanceof WebAssembly.Module) {
    instance = await WebAssembly.instantiate(module);
  } else if (module) {
    ({ instance } = await WebAssembly.instantiate(module as BufferSource, {}));
  } else {
    const bytes = await (await import("node:fs/promises")).readFile(
      new URL("../dist/ng-wasm.wasm", import.meta.url),
    );
    ({ instance } = await WebAssembly.instantiate(bytes as unknown as BufferSource, {}));
  }
  const exports = instance.exports as unknown as WasmExports;
  const ng = new Ng(exports);
  if (ng.abiVersion() !== 1) {
    throw new NgError("E_VERSION_MISMATCH", `ng_abi_version ${ng.abiVersion()} != 1`);
  }
  return ng;
}
