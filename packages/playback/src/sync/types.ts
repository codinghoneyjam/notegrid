// <META - FILE SUMMARY - Structural document view and port contracts for the document->schedule sync policy (T03 SYNC).>

/** Lane fields the sync policy reads. Structurally compatible with `@ng/document` `Lane`. */
export interface SyncLane {
  id: number;
  kind: 'melodic' | 'drum';
  program?: number;
  kit?: number;
  volume?: number;
  pan?: number;
  reverb_send?: number;
  mute?: boolean;
  solo?: boolean;
}

/** Note fields the sync policy reads. Structurally compatible with `@ng/document` `Note`. */
export interface SyncNote {
  lane: number;
  start: number;
  duration: number;
  pitch: number;
  velocity: number;
}

export interface SyncTempo {
  tick: number;
  bpm: number;
}

export interface SyncMeter {
  tick: number;
  num: number;
  den: number;
}

export interface LoopRange {
  start: number;
  end: number;
}

/**
 * Structural view of the editor document.
 *
 * Declared here rather than imported from `@ng/document`: the sync policy must stay
 * a pure consumer of shape (T03 `DEPENDS_ON { abi }`, `FORBIDDEN { ui-grid }`), and
 * TypeScript structural typing keeps `Project` assignable to it without a package edge.
 */
export interface SyncDocument {
  ppq: number;
  tempo: SyncTempo[];
  meter: SyncMeter[];
  loop?: LoopRange | null;
  lanes: SyncLane[];
  notes: SyncNote[];
}

/**
 * Resolved per-lane mix row, in the document's 0..=127 scale (C0.4).
 *
 * `lane` is the index in `lanes[]`, which is the slot the renderer uses (C2.8).
 * The byte encoding of a mix message is the port's business (`port/mixwire.ts`):
 * 6 packed little-endian bytes per row on the `ng_engine_set_mix` wire (C5).
 */
export interface MixRow {
  lane: number;
  volume: number;
  pan: number;
  send: number;
  muted: boolean;
}

/** Error surfaced to the editor instead of breaking playback mid-edit (T03 SYNC). */
export interface SyncError {
  code: string;
  message: string;
  source: 'compile' | 'audition' | 'transport';
}

/** Compile step: document -> schedule bytes. Throws so the last good schedule survives. */
export interface ScheduleCompiler {
  compile(doc: SyncDocument, sampleRate: number): Uint8Array;
}

export type ReplayOp =
  | { op: 'init'; sampleRate: number }
  | { op: 'schedule'; bytes: Uint8Array }
  | { op: 'mix'; rows: MixRow[] }
  | { op: 'loop'; range: LoopRange | null }
  | { op: 'bank'; bytes: Uint8Array };

/**
 * The single channel between the editor and the engine.
 *
 * Normal edits and watchdog recovery go through the same `apply` call, which is what
 * makes T03.R05 ("schedule/mix/bank order preserved on one FIFO port") a property of
 * the type rather than of call-site discipline: there is exactly one place a message
 * can be sent from.
 */
export interface PlaybackPort {
  apply(op: ReplayOp): void;
  audition(kind: number, program: number, key: number, vel: number, on: boolean): void;
  close(): void;
}

/** A factory is what lets the watchdog replace a dead worklet node (T03 SYNC watchdog clause). */
export type PortFactory = () => PlaybackPort;

/** Everything a freshly created port needs before it can sound (T03 SYNC recovery). */
export interface EngineSnapshot {
  sampleRate: number;
  schedule: Uint8Array | null;
  mix: MixRow[];
  loop: LoopRange | null;
  bank: Uint8Array | null;
}