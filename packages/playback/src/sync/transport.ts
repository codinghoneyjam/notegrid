// <META - FILE SUMMARY - Transport factory contract and the fixed replay order a recreated worklet node receives.>

import type { EngineSnapshot, PortFactory, PlaybackPort, ReplayOp } from './types.ts';

export type { EngineSnapshot, PortFactory, PlaybackPort, ReplayOp } from './types.ts';

/**
 * The order a fresh node is fed in, and it is fixed, not incidental.
 *
 * T03.R05 requires that schedule/mix/bank order is preserved on one FIFO port, and a
 * recreated node has no memory at all, so recovery has to rebuild the state in the one
 * order the engine can accept: `init` sets the sample rate every later message depends
 * on, `schedule` maps slots before `mix` targets them, and `bank` is last because
 * re-pointing the instrument source under a running schedule is the expensive step.
 */
export const REPLAY_ORDER: readonly ReplayOp['op'][] = ['init', 'schedule', 'mix', 'loop', 'bank'];

export function planReplay(snapshot: EngineSnapshot): ReplayOp[] {
  const ops: ReplayOp[] = [{ op: 'init', sampleRate: snapshot.sampleRate }];
  if (snapshot.schedule !== null) {
    ops.push({ op: 'schedule', bytes: snapshot.schedule });
  }
  if (snapshot.mix.length > 0) {
    ops.push({ op: 'mix', rows: snapshot.mix });
  }
  ops.push({ op: 'loop', range: snapshot.loop });
  if (snapshot.bank !== null) {
    ops.push({ op: 'bank', bytes: snapshot.bank });
  }
  return ops;
}