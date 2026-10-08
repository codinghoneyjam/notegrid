// <META - FILE SUMMARY - Overlap policy (I-03): trim / delete / replace.>

import type { Project, Note } from '@ng/document';

export function resolveOverlap(doc: Project, laneId: number, start: number, end: number, pitch: number): Project {
  const sameLanePitch = doc.notes.filter(n => n.lane === laneId && n.pitch === pitch);
  let result = doc;

  for (const existing of sameLanePitch) {
    const existingEnd = existing.start + existing.duration;

    // Case 3: same start -> replace
    if (existing.start === start) {
      result = {
        ...result,
        notes: result.notes.filter(n => n !== existing),
      };
      continue;
    }

    // Case 2: fully covered -> delete existing
    if (start <= existing.start && end >= existingEnd) {
      result = {
        ...result,
        notes: result.notes.filter(n => n !== existing),
      };
      continue;
    }

    // Case 1: trim existing note's end to new start
    if (start > existing.start && start < existingEnd) {
      result = {
        ...result,
        notes: result.notes.map(n =>
          n === existing ? { ...n, duration: start - n.start } : n
        ),
      };
      continue;
    }

    // Case 1b: trim existing note's start to new end
    if (end > existing.start && end < existingEnd) {
      result = {
        ...result,
        notes: result.notes.map(n =>
          n === existing ? { ...n, start: end, duration: n.start + n.duration - end } : n
        ),
      };
      continue;
    }
  }

  return result;
}
