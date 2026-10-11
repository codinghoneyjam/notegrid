// <META - FILE SUMMARY - The worklet-backed PlaybackPort: the C5 message protocol over an AudioWorkletNode.>

import type { PlaybackPort, ReplayOp } from '../sync/index.ts';
import { TRANSPORT_PAUSE, TRANSPORT_PLAY, TRANSPORT_STOP, loopTickMessage } from '../engine.ts';
import { encodeMixRows } from './mixwire.ts';

/** Must match the name `registerProcessor` was called with in the processor module. */
export const PROCESSOR_NAME = 'ng-engine';

/**
 * The two browser objects this port touches, declared structurally.
 *
 * The port is the only part of playback that runs in a browser, and it is also the part the
 * Node tests need to drive. Structural types let one fake node exercise the whole FIFO,
 * the queueing-before-ready path and the replay order without a DOM, and they keep the
 * surface this package depends on small enough to be obvious at a glance.
 */
export interface PortMessagePort {
  postMessage(message: unknown, transfer?: Transferable[]): void;
  onmessage: ((event: { data: unknown }) => void) | null;
}

export interface PortNode {
  port: PortMessagePort;
  connect(destination?: unknown): void;
  disconnect(): void;
}

/** worklet -> main, mirroring the processor's event union. */
export type PortEvent =
  | { t: 'ready'; sampleRate: number }
  | { t: 'position'; sample: number; tick: number; playing: boolean }
  | { t: 'block'; frames: number; peak: number }
  | { t: 'ended' }
  | { t: 'error'; code: string; message: string };

export interface WorkletPortOptions {
  node: PortNode;
  onReady?(sampleRate: number): void;
  onPosition?(sample: number, tick: number, playing: boolean): void;
  onBlock?(frames: number, peak: number): void;
  onEnded?(): void;
  onError?(code: string, message: string): void;
}

/**
 * The production port: one FIFO channel to the engine on the audio thread.
 *
 * Everything the sync policy sends goes through `apply`, so the ordering guarantee T03.R05
 * asks for is a property of this one method rather than of call-site discipline.
 *
 * Messages issued before the node has answered `ready` are queued and flushed in order. A
 * watchdog-driven recovery builds a fresh port and immediately replays the snapshot into it,
 * so a port that dropped those messages would lose exactly the state recovery exists to
 * restore.
 */
export class WorkletPlaybackPort implements PlaybackPort {
  private readonly node: PortNode;
  private readonly options: WorkletPortOptions;
  private readonly pending: unknown[] = [];
  private ready = false;
  /** Whether the bootstrap `init` has been posted. */
  private booted = false;
  private closed = false;

  constructor(options: WorkletPortOptions) {
    this.options = options;
    this.node = options.node;
    this.node.port.onmessage = (event: { data: unknown }) => {
      this.receive(event.data);
    };
  }

  apply(op: ReplayOp): void {
    this.assertOpen();
    switch (op.op) {
      case 'init':
        this.send({ t: 'init', sampleRate: op.sampleRate });
        return;
      case 'schedule':
        // Copied, never transferred: the policy keeps the same Uint8Array in its snapshot
        // for replay, and transferring would detach it out from under that snapshot.
        this.send({ t: 'schedule', bytes: copyOf(op.bytes) });
        return;
      case 'mix':
        if (op.rows.length === 0) return;
        this.send({ t: 'mix', bytes: copyOf(encodeMixRows(op.rows)) });
        return;
      case 'loop':
        this.send({ t: 'loop', ...loopTickMessage(op.range) });
        return;
      case 'bank':
        this.send({ t: 'bank', image: copyOf(op.bytes) });
        return;
    }
  }

  audition(kind: number, program: number, key: number, vel: number, on: boolean): void {
    this.assertOpen();
    this.send({ t: 'audition', kind, program, key, vel, on: on ? 1 : 0 });
  }

  play(): void {
    this.assertOpen();
    this.send({ t: 'transport', cmd: TRANSPORT_PLAY, arg: 0 });
  }

  pause(): void {
    this.assertOpen();
    this.send({ t: 'transport', cmd: TRANSPORT_PAUSE, arg: 0 });
  }

  stop(): void {
    if (this.closed) return;
    this.send({ t: 'transport', cmd: TRANSPORT_STOP, arg: 0 });
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.pending.length = 0;
    this.node.port.onmessage = null;
    this.node.disconnect();
  }

  /** Messages held until the worklet answered `ready`. Zero once it has. */
  queued(): number {
    return this.pending.length;
  }

  private send(message: unknown): void {
    this.pending.push(message);
    this.flush();
  }

  private flush(): void {
    while (this.pending.length > 0) {
      // The first message is the bootstrap, and it cannot wait for `ready`:
      // the processor answers `ready` only after it has processed `init`, so a
      // port that held it would deadlock. Every later message waits, because
      // the engine rejects any command that arrives before `init` has run.
      if (!this.ready && this.booted) break;
      const message = this.pending.shift();
      if (message === undefined) break;
      this.node.port.postMessage(message);
      this.booted = true;
    }
  }

  private receive(data: unknown): void {
    if (typeof data !== 'object' || data === null) return;
    const event = data as PortEvent;
    switch (event.t) {
      case 'ready':
        this.ready = true;
        this.options.onReady?.(event.sampleRate);
        this.flush();
        return;
      case 'position':
        this.options.onPosition?.(event.sample, event.tick, event.playing);
        return;
      case 'block':
        this.options.onBlock?.(event.frames, event.peak);
        return;
      case 'ended':
        this.options.onEnded?.();
        return;
      case 'error':
        this.options.onError?.(event.code, event.message);
        return;
    }
  }

  private assertOpen(): void {
    if (this.closed) {
      throw new Error('playback port is closed');
    }
  }
}

function copyOf(bytes: Uint8Array): ArrayBuffer {
  const out = new Uint8Array(bytes.length);
  out.set(bytes);
  return out.buffer;
}

export interface CreateWorkletPortOptions extends WorkletPortOptions {
  context: AudioContext;
  /** URL of the bundled processor module (an `audioWorklet.addModule` target). */
  moduleUrl: string;
  /** The already-compiled wasm module, handed to the processor through processorOptions. */
  wasmModule: WebAssembly.Module;
}

/**
 * Loads the processor module onto a context.
 *
 * `audioWorklet.addModule` is asynchronous, and a node constructed before its
 * processor is registered throws instead of waiting -- so the composition root
 * awaits this before it builds a port. A module URL loaded once stays
 * registered for the context's lifetime, which is what lets the watchdog's
 * synchronous `PortFactory` rebuild a node: the name is already there.
 */
export async function loadWorkletModule(
  context: AudioContext,
  moduleUrl: string,
): Promise<void> {
  const audioWorklet = context.audioWorklet;
  if (!audioWorklet || typeof AudioWorkletNode === 'undefined') {
    throw new Error('AudioWorklet is unavailable on this context');
  }
  await audioWorklet.addModule(moduleUrl);
}

/**
 * Builds the node and the port.
 *
 * The module must already be registered on the context
 * (`loadWorkletModule`): `addModule` is asynchronous, and a node
 * constructed before its processor is registered throws rather than
 * waits, so a fire-and-forget load here would only lose the race.
 * Registration is a once-per-context event, which is what keeps the
 * watchdog's synchronous factory honest.
 */
export function createWorkletPort(options: CreateWorkletPortOptions): WorkletPlaybackPort {
  const context = options.context;
  const audioWorklet = context.audioWorklet;
  if (!audioWorklet || typeof AudioWorkletNode === 'undefined') {
    throw new Error('AudioWorklet is unavailable on this context');
  }
  const node = new AudioWorkletNode(context, PROCESSOR_NAME, {
    numberOfInputs: 0,
    numberOfOutputs: 1,
    outputChannelCount: [2],
    processorOptions: { module: options.wasmModule },
  }) as unknown as PortNode;
  const port = new WorkletPlaybackPort({
    node,
    onReady: options.onReady,
    onPosition: options.onPosition,
    onBlock: options.onBlock,
    onEnded: options.onEnded,
    onError: options.onError,
  });
  node.connect(context.destination);
  return port;
}