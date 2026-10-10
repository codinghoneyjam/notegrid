// <META - FILE SUMMARY - WAV export adapter: the encoder seam the composition root fills with the abi wrapper.>

/**
 * The offline WAV render, declared as a seam rather than imported.
 *
 * T06.R01 makes `app` the only package allowed to bind another package's implementation, so
 * this adapter takes the renderer as a constructor argument typed by what it actually calls.
 * `Ng.renderWav` satisfies it structurally; nothing here knows that `@ng/abi` exists.
 */
export interface WavRenderer {
  renderWav(projectJson: string, sampleRate: number, depth: 0 | 1 | 2): Uint8Array;
}

export interface WavOptions {
  sampleRate?: number;
  /** 0 = Int16, 1 = Int24, 2 = Float32 (C4 depth argument). */
  depth?: 0 | 1 | 2;
}

export const DEFAULT_WAV_SAMPLE_RATE = 44100;

export class WavExportAdapter {
  private readonly renderer: WavRenderer;

  constructor(renderer: WavRenderer) {
    this.renderer = renderer;
  }

  async export(projectJson: string, options: WavOptions = {}): Promise<Uint8Array> {
    const sampleRate = options.sampleRate ?? DEFAULT_WAV_SAMPLE_RATE;
    const depth = options.depth ?? 0;
    return this.renderer.renderWav(projectJson, sampleRate, depth);
  }
}