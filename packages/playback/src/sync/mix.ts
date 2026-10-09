// <META - FILE SUMMARY - Resolves a document into per-lane mix rows, including the global solo rule.>

import type { MixRow, SyncDocument } from './types.ts';

/** C0.4: volume / pan / reverb_send are 0..=127, pan 64 is centre. Same defaults as the compiler. */
export const DEFAULT_VOLUME = 100;
export const DEFAULT_PAN = 64;
export const DEFAULT_SEND = 0;

export const MIX_MAX = 127;
export const PAN_CENTRE = 64;

/**
 * One row per lane, indexed by position in `lanes[]` because that index is the renderer
 * slot (C2.8). Solo is global: one soloed lane silences every un-soloed lane.
 *
 * Values stay in the document's 0..=127 scale; turning them into gain and pan factors
 * is the port's job, because that scale is the only thing the score format defines.
 */
export function buildMixRows(doc: SyncDocument): MixRow[] {
  const anySolo = doc.lanes.some((lane) => lane.solo === true);
  return doc.lanes.map((lane, index) => ({
    lane: index,
    volume: lane.volume ?? DEFAULT_VOLUME,
    pan: lane.pan ?? DEFAULT_PAN,
    send: lane.reverb_send ?? DEFAULT_SEND,
    muted: lane.mute === true || (anySolo && lane.solo !== true),
  }));
}

/** Mix-only edits touch nothing else, so this is the cheap equality check the policy uses. */
export function mixRowsEqual(a: MixRow[], b: MixRow[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i += 1) {
    if (
      a[i].lane !== b[i].lane ||
      a[i].volume !== b[i].volume ||
      a[i].pan !== b[i].pan ||
      a[i].send !== b[i].send ||
      a[i].muted !== b[i].muted
    ) {
      return false;
    }
  }
  return true;
}