// <META - FILE SUMMARY - Classifies a document edit into schedule / mix / loop work (T03 SYNC change routing).>

import type { LoopRange, SyncDocument, SyncLane, SyncNote } from './types.ts';
import { DEFAULT_PAN, DEFAULT_SEND, DEFAULT_VOLUME } from './mix.ts';

export interface DocumentDiff {
  /** notes | tempo | meter | lane add/remove/reorder | program change -> recompile */
  schedule: boolean;
  /** lane volume/pan/send/mute/solo (or a slot remap, C2.8) -> setMix, never recompile */
  mix: boolean;
  /** loop range -> setLoop, never recompile */
  loop: boolean;
  /** notes present now and absent before; the audition candidates */
  addedNotes: SyncNote[];
  /** human readable reasons, in the order they were detected */
  reasons: string[];
}

const EMPTY: DocumentDiff = { schedule: false, mix: false, loop: false, addedNotes: [], reasons: [] };

function laneSig(lane: SyncLane): string {
  return [lane.id, lane.kind, lane.program ?? -1, lane.kit ?? -1].join('/');
}

function mixSig(lane: SyncLane): string {
  return [
    lane.volume ?? DEFAULT_VOLUME,
    lane.pan ?? DEFAULT_PAN,
    lane.reverb_send ?? DEFAULT_SEND,
    lane.mute ? 1 : 0,
    lane.solo ? 1 : 0,
  ].join('/');
}

function noteSig(note: SyncNote): string {
  return [note.lane, note.start, note.duration, note.pitch, note.velocity].join('/');
}

function sameLoop(a: LoopRange | null | undefined, b: LoopRange | null | undefined): boolean {
  const left = a ?? null;
  const right = b ?? null;
  if (left === null || right === null) return left === right;
  return left.start === right.start && left.end === right.end;
}

/**
 * Routes an edit to the cheapest engine call that is still correct.
 *
 * A lane reorder changes the slot mapping (C2.8), so it dirties both mix and schedule.
 * A `prev` of `null` means "first document": everything is dirty.
 */
export function diffDocuments(prev: SyncDocument | null, next: SyncDocument): DocumentDiff {
  const diff: DocumentDiff = { ...EMPTY, addedNotes: [] };
  if (prev === null) {
    diff.schedule = true;
    diff.mix = true;
    diff.loop = true;
    diff.reasons.push('initial document');
    return diff;
  }

  if (prev.ppq !== next.ppq) {
    diff.schedule = true;
    diff.reasons.push('ppq changed');
  }

  if (tempoSig(prev.tempo) !== tempoSig(next.tempo)) {
    diff.schedule = true;
    diff.reasons.push('tempo changed');
  }

  if (JSON.stringify(prev.meter) !== JSON.stringify(next.meter)) {
    diff.schedule = true;
    diff.reasons.push('meter changed');
  }

  if (prev.notes.length !== next.notes.length || noteSigsDiffer(prev.notes, next.notes)) {
    diff.schedule = true;
    diff.reasons.push('notes changed');
  }

  const added = addedNotes(prev.notes, next.notes);
  if (added.length > 0) {
    diff.addedNotes = added;
  }

  const laneTopologyChanged = laneSigsDiffer(prev.lanes, next.lanes);
  if (laneTopologyChanged) {
    diff.schedule = true;
    diff.reasons.push('lanes added/removed/reordered or program changed');
  }

  if (laneTopologyChanged || mixSigsDiffer(prev.lanes, next.lanes)) {
    diff.mix = true;
    if (!laneTopologyChanged) {
      diff.reasons.push('lane volume/pan/send/mute/solo changed');
    } else {
      diff.reasons.push('slot mapping changed (C2.8)');
    }
  }

  if (!sameLoop(prev.loop, next.loop)) {
    diff.loop = true;
    diff.reasons.push('loop range changed');
  }

  return diff;
}

function tempoSig(items: SyncDocument['tempo']): string {
  return items.map((t) => `${t.tick}:${t.bpm}`).join(',');
}

function noteSigsDiffer(a: SyncNote[], b: SyncNote[]): boolean {
  if (a.length !== b.length) return true;
  for (let i = 0; i < a.length; i += 1) {
    if (noteSig(a[i]) !== noteSig(b[i])) return true;
  }
  return false;
}

function laneSigsDiffer(a: SyncLane[], b: SyncLane[]): boolean {
  if (a.length !== b.length) return true;
  for (let i = 0; i < a.length; i += 1) {
    if (laneSig(a[i]) !== laneSig(b[i])) return true;
  }
  return false;
}

function mixSigsDiffer(a: SyncLane[], b: SyncLane[]): boolean {
  if (a.length !== b.length) return true;
  for (let i = 0; i < a.length; i += 1) {
    if (mixSig(a[i]) !== mixSig(b[i])) return true;
  }
  return false;
}

/** Multiset difference keyed on the full note signature, so a pitch or length edit is not an "add". */
function addedNotes(prev: SyncNote[], next: SyncNote[]): SyncNote[] {
  const counts = new Map<string, number>();
  for (const n of prev) {
    counts.set(noteSig(n), (counts.get(noteSig(n)) ?? 0) + 1);
  }
  const added: SyncNote[] = [];
  for (const n of next) {
    const key = noteSig(n);
    const left = counts.get(key) ?? 0;
    if (left > 0) {
      counts.set(key, left - 1);
    } else {
      added.push(n);
    }
  }
  return added;
}