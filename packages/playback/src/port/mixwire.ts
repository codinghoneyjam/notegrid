// <META - FILE SUMMARY - The 6-byte little-endian mix record, encoded at the port boundary.>

import type { MixRow } from '../sync/index.ts';

/** Bytes per lane, per the DECISIONS.md 2026-10-09 entry and the C5 `{t:'mix'}` line. */
export const MIX_RECORD_BYTES = 6;

/**
 * Serializes mix rows into the engine's wire record.
 *
 * `{ slot u16, volume u8, pan u8, reverb_send u8, flags u8 }`, packed, little-endian, with
 * bit0 of `flags` = mute and bits 1-7 reserved. Values stay in the document's 0..=127 scale
 * (C0.4): the conversion to linear gain belongs to the engine, which is where `lane_mix`
 * already does it for a compiled schedule.
 *
 * `solo` never travels: `buildMixRows` has already folded the global solo rule into
 * `muted`, so there is exactly one definition of the rule and it is the tested one.
 *
 * A row whose slot is >= 128 or whose values exceed 127 would be rejected by the engine as
 * E_RANGE; the port refuses them here instead so the error surfaces at the edit that caused
 * it rather than as an opaque engine failure one frame later.
 */
export function encodeMixRows(rows: readonly MixRow[]): Uint8Array {
  const bytes = new Uint8Array(rows.length * MIX_RECORD_BYTES);
  const view = new DataView(bytes.buffer);
  for (let i = 0; i < rows.length; i += 1) {
    const row = rows[i];
    const at = i * MIX_RECORD_BYTES;
    if (row.lane < 0 || row.lane > 127) {
      throw new RangeError(`mix slot ${row.lane} is outside 0..=127`);
    }
    view.setUint16(at, row.lane, true);
    view.setUint8(at + 2, clamp127(row.volume, 'volume'));
    view.setUint8(at + 3, clamp127(row.pan, 'pan'));
    view.setUint8(at + 4, clamp127(row.send, 'reverb_send'));
    view.setUint8(at + 5, row.muted ? 1 : 0);
  }
  return bytes;
}

function clamp127(value: number, field: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 127) {
    throw new RangeError(`mix ${field} ${value} is outside 0..=127`);
  }
  return value;
}