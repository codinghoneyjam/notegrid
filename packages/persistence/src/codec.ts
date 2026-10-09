// <META - FILE SUMMARY - Project <-> score-format conversion; every rule here delegates to the Rust canonical/migrate/validate.>

import type { Lane, Note, Project } from '@ng/document';

export interface ScoreProject {
  format_version: number;
  title: string;
  ppq: number;
  tempo: Array<{ tick: number; bpm: number }>;
  meter: Array<{ tick: number; num: number; den: number }>;
  loop?: { start: number; end: number };
  length_ticks?: number;
  lanes: Lane[];
  notes: Array<[number, number, number, number, number]>;
  [ext: string]: unknown;
}

/** Extension fields are `x_`-prefixed and ordered by BTreeMap on the Rust side (C1). */
export const EXT_PREFIX = 'x_';

/**
 * Editor document -> score JSON, with notes as tuples.
 *
 * T04 owns the pipeline "document -> tuples -> ng_project_canonical -> bytes" but not
 * the format itself, so this only reshapes; every ordering and formatting decision is
 * left to `ng_project_canonical`.
 */
export function encodeProject(doc: Project): string {
  const payload: ScoreProject = {
    format_version: doc.format_version,
    title: doc.title,
    ppq: doc.ppq,
    tempo: doc.tempo.map((t) => ({ tick: t.tick, bpm: t.bpm })),
    meter: doc.meter.map((m) => ({ tick: m.tick, num: m.num, den: m.den })),
    ...(doc.loop ? { loop: { start: doc.loop.start, end: doc.loop.end } } : {}),
    ...(doc.length_ticks === undefined ? {} : { length_ticks: doc.length_ticks }),
    lanes: doc.lanes.map(encodeLane),
    notes: doc.notes.map(noteToTuple),
  };
  for (const [key, value] of Object.entries(doc.ext ?? {})) {
    payload[`${EXT_PREFIX}${key}`] = value;
  }
  return JSON.stringify(payload);
}

/** Score JSON (canonical or migrated) -> editor document. Notes go back to objects. */
export function decodeProject(json: string): Project {
  const raw = JSON.parse(json) as ScoreProject;
  const ext: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(raw)) {
    if (key.startsWith(EXT_PREFIX)) {
      ext[key.slice(EXT_PREFIX.length)] = value;
    }
  }
  const doc: Project = {
    format_version: raw.format_version,
    title: raw.title,
    ppq: raw.ppq,
    tempo: raw.tempo.map((t) => ({ tick: t.tick, bpm: t.bpm })),
    meter: raw.meter.map((m) => ({ tick: m.tick, num: m.num, den: m.den })),
    lanes: raw.lanes.map(decodeLane),
    notes: raw.notes.map(tupleToNote),
  };
  if (raw.loop) doc.loop = { start: raw.loop.start, end: raw.loop.end };
  if (raw.length_ticks !== undefined) doc.length_ticks = raw.length_ticks;
  if (Object.keys(ext).length > 0) doc.ext = ext;
  return doc;
}

function noteToTuple(note: Note): [number, number, number, number, number] {
  return [note.lane, note.start, note.duration, note.pitch, note.velocity];
}

function tupleToNote(tuple: [number, number, number, number, number]): Note {
  return {
    lane: tuple[0],
    start: tuple[1],
    duration: tuple[2],
    pitch: tuple[3],
    velocity: tuple[4],
  };
}

/** Optional fields stay absent rather than becoming `undefined`, so canonical bytes are stable. */
function encodeLane(lane: Lane): Lane {
  const out: Lane = {
    id: lane.id,
    name: lane.name,
    kind: lane.kind,
  };
  if (lane.program !== undefined) out.program = lane.program;
  if (lane.kit !== undefined) out.kit = lane.kit;
  if (lane.volume !== undefined) out.volume = lane.volume;
  if (lane.pan !== undefined) out.pan = lane.pan;
  if (lane.reverb_send !== undefined) out.reverb_send = lane.reverb_send;
  if (lane.mute !== undefined) out.mute = lane.mute;
  if (lane.solo !== undefined) out.solo = lane.solo;
  if (lane.color !== undefined) out.color = lane.color;
  return out;
}

function decodeLane(lane: Lane): Lane {
  return { ...lane };
}