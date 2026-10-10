// <META - FILE SUMMARY - The engine surface playback consumes, and the transport command constants.>

/**
 * The engine surface `playback` consumes, declared here rather than imported from `@ng/abi`.
 *
 * T06.R01 makes `app` the only package allowed to bind another package's implementation, and
 * the sync policy already set the precedent (`SyncDocument` in `sync/types.ts`): a consumer
 * declares the shape it consumes, and TypeScript structural typing keeps the real wrapper
 * assignable to it without a package edge. `import type` erases at compile time; a value
 * import does not, and that surviving edge is exactly the wiring this rule forbids.
 *
 * Only the calls a port actually makes are listed. A port that needed a new engine call
 * would widen this interface rather than reach for `@ng/abi`, so the dependency check stays
 * a structural statement about the seam instead of a naming convention.
 */
export interface EngineAbi {
  engineInit(sampleRate: number, synthKind: number, maxVoices: number): void;
  engineSetSchedule(bytes: Uint8Array): void;
  engineSetMix(bytes: Uint8Array): void;
  /** Sample domain. Kept for completeness; the document loop uses `engineSetLoopTicks`. */
  engineSetLoop(start: number, end: number, enabled: boolean): void;
  engineSetLoopTicks(startTick: number, endTick: number, enabled: boolean): void;
  engineAudition(kind: number, program: number, key: number, vel: number, on: boolean): void;
  engineTransport(cmd: number, arg: number): void;
  engineRender(l: Float32Array, r: Float32Array): { playing: boolean; ended: boolean };
  enginePositionSample(): number;
  enginePositionTick(): number;
}

/** C4 `ng_engine_transport(cmd, arg)` commands. */
export const TRANSPORT_PLAY = 0;
export const TRANSPORT_PAUSE = 1;
export const TRANSPORT_STOP = 2;
export const TRANSPORT_SEEK = 3;

/** The loop message carries document ticks (C0.1); the engine converts (DECISIONS.md 2026-10-09). */
export function loopTickMessage(range: { start: number; end: number } | null): {
  start_tick: number;
  end_tick: number;
  enabled: number;
} {
  if (!range) {
    return { start_tick: 0, end_tick: 0, enabled: 0 };
  }
  return { start_tick: range.start, end_tick: range.end, enabled: 1 };
}