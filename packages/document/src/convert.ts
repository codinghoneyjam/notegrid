// <META - FILE SUMMARY - Load/save conversion between Project and tuple encoding (C1 NOTE_ENCODING).>

import type { Project, Note } from './types.js';

export type NoteTuple = [number, number, number, number, number]; // [lane, start, duration, pitch, velocity]

export function noteToTuple(note: Note): NoteTuple {
  return [note.lane, note.start, note.duration, note.pitch, note.velocity];
}

export function tupleToNote(tuple: NoteTuple): Note {
  return {
    lane: tuple[0],
    start: tuple[1],
    duration: tuple[2],
    pitch: tuple[3],
    velocity: tuple[4],
  };
}

export function projectToTuples(doc: Project): NoteTuple[] {
  return doc.notes.map(noteToTuple);
}

export function tuplesToProject(tuples: NoteTuple[], base: Project): Project {
  return {
    ...base,
    notes: tuples.map(tupleToNote),
  };
}

export function projectToCanonicalJson(doc: Project): string {
  const sorted = {
    ...doc,
    notes: [...doc.notes].sort((a, b) => {
      if (a.start !== b.start) return a.start - b.start;
      if (a.lane !== b.lane) return a.lane - b.lane;
      return a.pitch - b.pitch;
    }),
  };
  return JSON.stringify(sorted, null, 2) + '\n';
}

export function canonicalJsonToProject(json: string): Project {
  return JSON.parse(json) as Project;
}
