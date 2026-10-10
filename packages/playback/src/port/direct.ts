// <META - FILE SUMMARY - The main-thread PlaybackPort: drives the wasm engine directly, one block at a time.>

import type { PlaybackPort, ReplayOp } from '../sync/index.ts';
import { TRANSPORT_PLAY, TRANSPORT_STOP, type EngineAbi } from '../engine.ts';
import { encodeMixRows } from './mixwire.ts';

export const RENDER_BLOCK = 128;

/** What a port reports back so the editor can watch the playhead (T03.AC2). */
export interface PortStatus {
  sample: number;
  tick: number;
  playing: boolean;
  ended: boolean;
  /** Peak absolute sample of the last rendered block; 0 for a silent block. */
  peak: number;
}

export interface DirectPortOptions {
  abi: EngineAbi;
  onStatus?(status: PortStatus): void;
  /** Renders a block and returns its peak. Injected so a test can drive time deterministically. */
  render?(abi: EngineAbi, left: Float32Array, right: Float32Array): number;
}

/**
 * The port the composition root falls back to when `AudioWorkletNode` is missing.
 *
 * It is not a stub: it makes the identical wasm calls the worklet makes, on the main thread,
 * and produces real audio in `render()`. What it cannot promise is the real-time budget of
 * R-RT-1, which is why `WorkletPlaybackPort` is the default wherever AudioWorklet exists
 * (T06.AC5 "missing features fall back").
 *
 * `render()` is a separate, synchronous call rather than a timer. A timer would make every
 * assertion about "a block was rendered" a race against it; driving it by hand keeps the
 * port usable from a Node test with no browser at all, which is what T06.AC2 asks for.
 */
export class DirectPlaybackPort implements PlaybackPort {
  private readonly abi: EngineAbi;
  private readonly onStatus: ((status: PortStatus) => void) | undefined;
  private readonly renderFn: ((abi: EngineAbi, left: Float32Array, right: Float32Array) => number) | undefined;
  private left: Float32Array = new Float32Array(RENDER_BLOCK);
  private right: Float32Array = new Float32Array(RENDER_BLOCK);
  private playing = false;
  private ended = false;
  private closed = false;

  constructor(options: DirectPortOptions) {
    this.abi = options.abi;
    this.onStatus = options.onStatus;
    this.renderFn = options.render;
  }

  apply(op: ReplayOp): void {
    this.assertOpen();
    switch (op.op) {
      case 'init':
        this.abi.engineInit(op.sampleRate, 0, 64);
        break;
      case 'schedule':
        this.abi.engineSetSchedule(op.bytes);
        break;
      case 'mix':
        // A document with no lanes has nothing to say; the engine treats a zero-length
        // payload as malformed, so the op is dropped rather than sent empty.
        if (op.rows.length > 0) {
          this.abi.engineSetMix(encodeMixRows(op.rows));
        }
        break;
      case 'loop':
        if (op.range) {
          this.abi.engineSetLoopTicks(op.range.start, op.range.end, true);
        } else {
          this.abi.engineSetLoopTicks(0, 0, false);
        }
        break;
      case 'bank':
        // P2 has no bank loader yet (T3.3); the op is accepted and dropped so the policy can
        // carry it without the port pretending to have handled it.
        break;
    }
  }

  audition(kind: number, program: number, key: number, vel: number, on: boolean): void {
    this.assertOpen();
    this.abi.engineAudition(kind, program, key, vel, on);
  }

  play(): void {
    this.assertOpen();
    this.ended = false;
    this.playing = true;
    this.abi.engineTransport(TRANSPORT_PLAY, 0);
  }

  stop(): void {
    this.playing = false;
    this.abi.engineTransport(TRANSPORT_STOP, 0);
  }

  /** Renders exactly one block and returns its peak absolute sample. */
  renderBlock(): number {
    this.assertOpen();
    const injected = this.renderFn;
    if (injected) {
      const peak = injected(this.abi, this.left, this.right);
      this.report(peak);
      return peak;
    }
    const status = this.abi.engineRender(this.left, this.right);
    this.playing = status.playing;
    this.ended = status.ended;
    const peak = peakOf(this.left, this.right);
    this.report(peak);
    return peak;
  }

  status(): PortStatus {
    return {
      sample: this.abi.enginePositionSample(),
      tick: this.abi.enginePositionTick(),
      playing: this.playing,
      ended: this.ended,
      peak: 0,
    };
  }

  close(): void {
    this.closed = true;
  }

  private report(peak: number): void {
    this.onStatus?.({
      sample: this.abi.enginePositionSample(),
      tick: this.abi.enginePositionTick(),
      playing: this.playing,
      ended: this.ended,
      peak,
    });
  }

  private assertOpen(): void {
    if (this.closed) {
      throw new Error('playback port is closed');
    }
  }
}

function peakOf(l: Float32Array, r: Float32Array): number {
  let peak = 0;
  for (let i = 0; i < l.length; i += 1) {
    const a = Math.abs(l[i]);
    if (a > peak) peak = a;
    const b = Math.abs(r[i]);
    if (b > peak) peak = b;
  }
  return peak;
}