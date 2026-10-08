// <META - FILE SUMMARY - Pure query functions over the document model.>

import type { Project, Lane, Note } from './types.js';

export function getLane(doc: Project, laneId: number): Lane | undefined {
  return doc.lanes.find(l => l.id === laneId);
}

export function getNotesInLane(doc: Project, laneId: number): Note[] {
  return doc.notes.filter(n => n.lane === laneId);
}

export function getNotesInRange(doc: Project, startTick: number, endTick: number): Note[] {
  return doc.notes.filter(n => n.start < endTick && n.start + n.duration > startTick);
}

export function getEffectiveLength(doc: Project): number {
  if (doc.length_ticks !== undefined) return doc.length_ticks;
  let maxEnd = 0;
  for (const note of doc.notes) {
    const end = note.start + note.duration;
    if (end > maxEnd) maxEnd = end;
  }
  return maxEnd;
}

export function getTempoAtTick(doc: Project, tick: number): number {
  let bpm = 120;
  for (const item of doc.tempo) {
    if (item.tick <= tick) bpm = item.bpm;
    else break;
  }
  return bpm;
}

export function getMeterAtTick(doc: Project, tick: number): { num: number; den: number } {
  let meter = { num: 4, den: 4 };
  for (const item of doc.meter) {
    if (item.tick <= tick) meter = { num: item.num, den: item.den };
    else break;
  }
  return meter;
}

export function findOverlappingNotes(doc: Project, laneId: number, start: number, end: number, excludeId?: number): Note[] {
  return doc.notes.filter(n => {
    if (n.lane !== laneId) return false;
    if (excludeId !== undefined && doc.notes.indexOf(n) === excludeId) return false;
    return n.start < end && n.start + n.duration > start;
  });
}

export function getLaneCount(doc: Project): number {
  return doc.lanes.length;
}

export function getNoteCount(doc: Project): number {
  return doc.notes.length;
}

export function getUnusedLanes(doc: Project): Lane[] {
  return doc.lanes.filter(l => !doc.notes.some(n => n.lane === l.id));
}
