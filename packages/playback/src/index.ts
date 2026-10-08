// AudioEngine interface for P1 minimal implementation
import { Ng } from '@ng/abi';

export interface AudioEngine {
  init(): Promise<void>;
  setSchedule(bytes: Uint8Array): void;
  play(): void;
  pause(): void;
  stop(): void;
  seek(tick: number): void;
  setLoop(range: { start: number; end: number } | null): void;
  onPosition(cb: (sample: number, tick: number, playing: boolean) => void): () => void;
  dispose(): void;
}

export class AudioEngineImpl implements AudioEngine {
  private ng: Ng;
  private context: AudioContext | null = null;
  private workletNode: AudioWorkletNode | null = null;
  private positionCallbacks: Array<(sample: number, tick: number, playing: boolean) => void> = [];
  private sampleRate: number = 44100;

  constructor(ng: Ng) {
    this.ng = ng;
  }

  async init(): Promise<void> {
    this.context = new AudioContext();
    this.sampleRate = this.context.sampleRate;
    
    // For P1, we initialize the WASM engine
    this.ng.engineInit(this.sampleRate, 0, 64); // OscSynth, 64 voices
  }

  setSchedule(bytes: Uint8Array): void {
    this.ng.engineSetSchedule(bytes);
  }

  play(): void {
    this.ng.engineTransport(0); // play
  }

  pause(): void {
    this.ng.engineTransport(1); // pause
  }

  stop(): void {
    this.ng.engineTransport(2); // stop
  }

  seek(tick: number): void {
    // Convert tick to sample (simplified for P1)
    const sample = Math.floor(tick * this.sampleRate / 480);
    this.ng.engineTransport(3, sample); // seek
  }

  setLoop(range: { start: number; end: number } | null): void {
    if (range) {
      this.ng.engineSetLoop(range.start, range.end, true);
    } else {
      this.ng.engineSetLoop(0, 0, false);
    }
  }

  onPosition(cb: (sample: number, tick: number, playing: boolean) => void): () => void {
    this.positionCallbacks.push(cb);
    // For P1, we poll position in a simple loop
    // In P2, this will be driven by worklet messages
    return () => {
      const idx = this.positionCallbacks.indexOf(cb);
      if (idx >= 0) this.positionCallbacks.splice(idx, 1);
    };
  }

  dispose(): void {
    this.stop();
    if (this.context) {
      this.context.close();
      this.context = null;
    }
    if (this.workletNode) {
      this.workletNode.disconnect();
      this.workletNode = null;
    }
  }
}
