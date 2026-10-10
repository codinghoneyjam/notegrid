// <META - FILE SUMMARY - The AudioWorkletProcessor: the engine on the audio thread, instantiated by app.>

/**
 * The engine, on the audio thread.
 *
 * This file lives in `app` and not in `playback` for one reason: it is the only thing in
 * the browser that instantiates `@ng/abi`, and T06.R01 makes `app` the only package allowed
 * to bind another package's implementation. `playback` owns the message protocol; `app`
 * owns the composition, in the main realm and in the worklet realm alike.
 *
 * The processor owns its own wasm instance. `Ng` keeps one on the main thread for compile,
 * validate and MIDI, and engine state cannot cross a thread boundary, so the audio thread
 * instantiates from the same already-compiled `WebAssembly.Module` that
 * `processorOptions` carries (C5).
 */

import { Ng } from '@ng/abi';
import { PROCESSOR_NAME } from '@ng/playback';

export const BLOCK = 128;
/** C5 `{t:'position', ...} ~30 Hz`: one report per ~23 ms of audio at this block size. */
const POSITION_EVERY_BLOCKS = 8;

/** main -> worklet. The wire shape is fixed by C5 and 02_contracts.dsl. */
type Command =
  | { t: 'init'; sampleRate: number }
  | { t: 'schedule'; bytes: ArrayBuffer }
  | { t: 'mix'; bytes: ArrayBuffer }
  | { t: 'loop'; start_tick: number; end_tick: number; enabled: number }
  | { t: 'transport'; cmd: number; arg: number }
  | { t: 'audition'; kind: number; program: number; key: number; vel: number; on: number }
  | { t: 'bank'; image: ArrayBuffer };

/** worklet -> main. `peak` is gate g10's evidence that a block actually sounded. */
type Event =
  | { t: 'ready'; sampleRate: number }
  | { t: 'position'; sample: number; tick: number; playing: boolean }
  | { t: 'block'; frames: number; peak: number }
  | { t: 'ended' }
  | { t: 'error'; code: string; message: string };

interface WorkletScope {
  registerProcessor(name: string, ctor: unknown): void;
}

interface WorkletHost {
  readonly port: {
    postMessage(message: unknown): void;
    onmessage: ((event: { data: Command }) => void) | null;
  };
}

interface ProcessorOptions {
  processorOptions?: { module?: WebAssembly.Module };
}

/**
 * `AudioWorkletProcessor` cannot be named here: it only exists inside the worklet global
 * scope, and this file is also type-checked on the main thread where it does not. The two
 * members actually used are declared structurally instead, which is enough to keep the
 * render loop honest without a DOM lib in the type graph.
 */
class NgEngineProcessor {
  private readonly host: WorkletHost;
  private readonly options: ProcessorOptions;
  private ng: Ng | null = null;
  private readonly left = new Float32Array(BLOCK);
  private readonly right = new Float32Array(BLOCK);
  private blocks = 0;

  constructor(options: ProcessorOptions, host: WorkletHost) {
    this.options = options ?? {};
    this.host = host;
    // Bound in the constructor rather than in `init`: a message can arrive on the port
    // before any render callback does, and an unbound handler would drop it.
    host.port.onmessage = (event: { data: Command }) => {
      this.receive(event.data);
    };
  }

  private receive(command: Command): void {
    if (command.t === 'init') {
      this.initialise(command.sampleRate);
      return;
    }
    const ng = this.ng;
    if (!ng) {
      this.fail('E_ENGINE', `engine command ${command.t} arrived before init`);
      return;
    }
    try {
      switch (command.t) {
        case 'schedule':
          ng.engineSetSchedule(new Uint8Array(command.bytes));
          break;
        case 'mix':
          ng.engineSetMix(new Uint8Array(command.bytes));
          break;
        case 'loop':
          ng.engineSetLoopTicks(
            command.start_tick,
            command.end_tick,
            command.enabled !== 0,
          );
          break;
        case 'transport':
          ng.engineTransport(command.cmd, command.arg);
          break;
        case 'audition':
          ng.engineAudition(
            command.kind,
            command.program,
            command.key,
            command.vel,
            command.on !== 0,
          );
          break;
        case 'bank':
          // T3.3 owns bank loading; in P2 the op is accepted so the FIFO order is unbroken.
          break;
      }
    } catch (cause) {
      const error = cause as { code?: string; message?: string };
      this.fail(error.code ?? 'E_ENGINE', error.message ?? String(cause));
    }
  }

  private initialise(sampleRate: number): void {
    try {
      const module = this.options?.processorOptions?.module;
      if (!module) {
        this.fail('E_MODULE_MISSING', 'processorOptions.module was not a WebAssembly.Module');
        return;
      }
      const instance = new WebAssembly.Instance(module, {});
      this.ng = new Ng(instance.exports as never);
      this.ng.engineInit(sampleRate, 0, 64);
      this.post({ t: 'ready', sampleRate });
    } catch (cause) {
      this.fail('E_INIT', String(cause));
    }
  }

  /** Called by the audio thread once per render quantum. */
  process(): boolean {
    const ng = this.ng;
    if (!ng) {
      // true keeps the node alive: `init` may still be on its way, and the port decides
      // when playback ends, not the node's lifetime.
      return true;
    }
    this.left.fill(0);
    this.right.fill(0);
    const status = ng.engineRender(this.left, this.right);

    let peak = 0;
    for (let i = 0; i < BLOCK; i += 1) {
      const a = Math.abs(this.left[i]);
      if (a > peak) peak = a;
      const b = Math.abs(this.right[i]);
      if (b > peak) peak = b;
    }

    this.blocks += 1;
    this.post({ t: 'block', frames: BLOCK, peak });
    if (this.blocks % POSITION_EVERY_BLOCKS === 0) {
      this.post({
        t: 'position',
        sample: ng.enginePositionSample(),
        tick: ng.enginePositionTick(),
        playing: status.playing,
      });
    }
    if (status.ended) {
      this.post({ t: 'ended' });
    }
    return true;
  }

  private post(event: Event): void {
    this.host.port.postMessage(event);
  }

  private fail(code: string, message: string): void {
    this.post({ t: 'error', code, message });
  }
}

const scope = globalThis as unknown as WorkletScope;
if (typeof scope.registerProcessor === 'function') {
  scope.registerProcessor(PROCESSOR_NAME, NgEngineProcessor);
}