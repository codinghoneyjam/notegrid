// WAV export adapter for P1
import { Ng } from '@ng/abi';

export interface WavOptions {
  sampleRate?: number;
  depth?: 0 | 1 | 2; // 0=Int16, 1=Int24, 2=Float32
}

export class WavExportAdapter {
  private ng: Ng;

  constructor(ng: Ng) {
    this.ng = ng;
  }

  async export(projectJson: string, options: WavOptions = {}): Promise<Uint8Array> {
    const sampleRate = options.sampleRate ?? 44100;
    const depth = options.depth ?? 0;
    
    return this.ng.renderWav(projectJson, sampleRate, depth);
  }
}
