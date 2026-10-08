import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Project, Note } from '../src/types.ts';
import {
  getLane,
  getNotesInLane,
  getNotesInRange,
  getEffectiveLength,
  getTempoAtTick,
  getMeterAtTick,
  findOverlappingNotes,
  getLaneCount,
  getNoteCount,
  getUnusedLanes,
} from '../src/queries.ts';
import {
  addNote,
  moveNotes,
  resizeNotes,
  deleteNotes,
  setVelocity,
  addLane,
  setLaneProgram,
  setTempo,
  setLoop,
} from '../src/updates.ts';
import {
  noteToTuple,
  tupleToNote,
  projectToTuples,
  tuplesToProject,
  projectToCanonicalJson,
  canonicalJsonToProject,
} from '../src/convert.ts';

function makeEmptyProject(): Project {
  return {
    format_version: 1,
    title: 'Test Project',
    ppq: 480,
    tempo: [{ tick: 0, bpm: 120 }],
    meter: [{ tick: 0, num: 4, den: 4 }],
    lanes: [
      { id: 1, name: 'Lane 1', kind: 'melodic', program: 0 },
      { id: 2, name: 'Lane 2', kind: 'melodic', program: 1 },
    ],
    notes: [],
  };
}

test('queries: getLane returns correct lane', () => {
  const doc = makeEmptyProject();
  const lane = getLane(doc, 1);
  assert.ok(lane);
  assert.equal(lane.name, 'Lane 1');
  assert.equal(getLane(doc, 999), undefined);
});

test('queries: getNotesInLane filters by lane', () => {
  const doc = makeEmptyProject();
  const withNotes = addNote(doc, 1, 0, 60, 480, 100);
  const withMore = addNote(withNotes, 2, 0, 64, 480, 100);
  const lane1Notes = getNotesInLane(withMore, 1);
  assert.equal(lane1Notes.length, 1);
  assert.equal(lane1Notes[0].pitch, 60);
  const lane2Notes = getNotesInLane(withMore, 2);
  assert.equal(lane2Notes.length, 1);
  assert.equal(lane2Notes[0].pitch, 64);
});

test('queries: getNotesInRange returns overlapping notes', () => {
  const doc = makeEmptyProject();
  const withNotes = addNote(doc, 1, 0, 60, 480, 100);
  const withMore = addNote(withNotes, 1, 960, 64, 480, 100);
  const inRange = getNotesInRange(withMore, 240, 720);
  assert.equal(inRange.length, 1);
  assert.equal(inRange[0].pitch, 60);
});

test('queries: getEffectiveLength returns max note end when no length_ticks', () => {
  const doc = makeEmptyProject();
  const withNotes = addNote(doc, 1, 0, 60, 480, 100);
  const withMore = addNote(withNotes, 1, 960, 64, 480, 100);
  assert.equal(getEffectiveLength(withMore), 1440);
});

test('queries: getEffectiveLength returns length_ticks when present', () => {
  const doc = makeEmptyProject();
  const withLength = { ...doc, length_ticks: 2000 };
  assert.equal(getEffectiveLength(withLength), 2000);
});

test('queries: getTempoAtTick returns correct tempo', () => {
  const doc = makeEmptyProject();
  const withTempo = setTempo(doc, 960, 140);
  assert.equal(getTempoAtTick(withTempo, 0), 120);
  assert.equal(getTempoAtTick(withTempo, 960), 140);
  assert.equal(getTempoAtTick(withTempo, 1920), 140);
});

test('queries: getMeterAtTick returns correct meter', () => {
  const doc = makeEmptyProject();
  const withMeter = { ...doc, meter: [{ tick: 0, num: 4, den: 4 }, { tick: 960, num: 3, den: 4 }] };
  assert.deepEqual(getMeterAtTick(withMeter, 0), { num: 4, den: 4 });
  assert.deepEqual(getMeterAtTick(withMeter, 960), { num: 3, den: 4 });
});

test('queries: findOverlappingNotes finds overlaps', () => {
  const doc = makeEmptyProject();
  const withNotes = addNote(doc, 1, 0, 60, 480, 100);
  const overlaps = findOverlappingNotes(withNotes, 1, 240, 720);
  assert.equal(overlaps.length, 1);
  const noOverlaps = findOverlappingNotes(withNotes, 1, 480, 960);
  assert.equal(noOverlaps.length, 0);
});

test('queries: getLaneCount and getNoteCount return correct counts', () => {
  const doc = makeEmptyProject();
  assert.equal(getLaneCount(doc), 2);
  assert.equal(getNoteCount(doc), 0);
  const withNotes = addNote(doc, 1, 0, 60, 480, 100);
  assert.equal(getNoteCount(withNotes), 1);
});

test('queries: getUnusedLanes returns lanes without notes', () => {
  const doc = makeEmptyProject();
  const withNotes = addNote(doc, 1, 0, 60, 480, 100);
  const unused = getUnusedLanes(withNotes);
  assert.equal(unused.length, 1);
  assert.equal(unused[0].id, 2);
});

test('updates: addNote creates new document with structural sharing', () => {
  const doc = makeEmptyProject();
  const newDoc = addNote(doc, 1, 0, 60, 480, 100);
  assert.equal(doc.notes.length, 0);
  assert.equal(newDoc.notes.length, 1);
  assert.equal(newDoc.notes[0].pitch, 60);
  assert.notEqual(doc, newDoc);
  assert.notEqual(doc.notes, newDoc.notes);
});

test('updates: moveNotes moves notes with structural sharing', () => {
  const doc = makeEmptyProject();
  const withNotes = addNote(doc, 1, 0, 60, 480, 100);
  const moved = moveNotes(withNotes, [0], 480, 12);
  assert.equal(withNotes.notes[0].start, 0);
  assert.equal(withNotes.notes[0].pitch, 60);
  assert.equal(moved.notes[0].start, 480);
  assert.equal(moved.notes[0].pitch, 72);
});

test('updates: resizeNotes resizes notes with structural sharing', () => {
  const doc = makeEmptyProject();
  const withNotes = addNote(doc, 1, 0, 60, 480, 100);
  const resized = resizeNotes(withNotes, [0], 'end', 240);
  assert.equal(withNotes.notes[0].duration, 480);
  assert.equal(resized.notes[0].duration, 720);
});

test('updates: deleteNotes removes notes with structural sharing', () => {
  const doc = makeEmptyProject();
  const withNotes = addNote(doc, 1, 0, 60, 480, 100);
  const withMore = addNote(withNotes, 1, 960, 64, 480, 100);
  const deleted = deleteNotes(withMore, [0]);
  assert.equal(withMore.notes.length, 2);
  assert.equal(deleted.notes.length, 1);
  assert.equal(deleted.notes[0].pitch, 64);
});

test('updates: setVelocity changes velocity with structural sharing', () => {
  const doc = makeEmptyProject();
  const withNotes = addNote(doc, 1, 0, 60, 480, 100);
  const changed = setVelocity(withNotes, [0], 80);
  assert.equal(withNotes.notes[0].velocity, 100);
  assert.equal(changed.notes[0].velocity, 80);
});

test('updates: addLane adds lane with structural sharing', () => {
  const doc = makeEmptyProject();
  const newDoc = addLane(doc, 'drum', 0);
  assert.equal(doc.lanes.length, 2);
  assert.equal(newDoc.lanes.length, 3);
  assert.equal(newDoc.lanes[2].id, 3);
  assert.equal(newDoc.lanes[2].kind, 'drum');
});

test('updates: setLaneProgram changes program with structural sharing', () => {
  const doc = makeEmptyProject();
  const newDoc = setLaneProgram(doc, 1, 42);
  assert.equal(doc.lanes[0].program, 0);
  assert.equal(newDoc.lanes[0].program, 42);
});

test('updates: setTempo adds or updates tempo with structural sharing', () => {
  const doc = makeEmptyProject();
  const newDoc = setTempo(doc, 960, 140);
  assert.equal(doc.tempo.length, 1);
  assert.equal(newDoc.tempo.length, 2);
  assert.equal(newDoc.tempo[1].bpm, 140);
});

test('updates: setLoop sets and clears loop with structural sharing', () => {
  const doc = makeEmptyProject();
  const withLoop = setLoop(doc, { start: 0, end: 1920 });
  assert.ok(withLoop.loop);
  assert.equal(withLoop.loop.start, 0);
  assert.equal(withLoop.loop.end, 1920);
  const cleared = setLoop(withLoop, null);
  assert.equal(cleared.loop, undefined);
});

test('convert: noteToTuple and tupleToNote round-trip', () => {
  const note: Note = { lane: 1, start: 480, duration: 240, pitch: 60, velocity: 100 };
  const tuple = noteToTuple(note);
  assert.deepEqual(tuple, [1, 480, 240, 60, 100]);
  const restored = tupleToNote(tuple);
  assert.deepEqual(restored, note);
});

test('convert: projectToTuples and tuplesToProject round-trip', () => {
  const doc = makeEmptyProject();
  const withNotes = addNote(doc, 1, 0, 60, 480, 100);
  const withMore = addNote(withNotes, 2, 960, 64, 480, 100);
  const tuples = projectToTuples(withMore);
  assert.equal(tuples.length, 2);
  const restored = tuplesToProject(tuples, withMore);
  assert.deepEqual(restored.notes, withMore.notes);
});

test('convert: projectToCanonicalJson and canonicalJsonToProject round-trip', () => {
  const doc = makeEmptyProject();
  const withNotes = addNote(doc, 1, 0, 60, 480, 100);
  const withMore = addNote(withNotes, 2, 960, 64, 480, 100);
  const json = projectToCanonicalJson(withMore);
  const restored = canonicalJsonToProject(json);
  assert.deepEqual(restored, withMore);
});

test('convert: canonical JSON sorts notes by (start, lane, pitch)', () => {
  const doc = makeEmptyProject();
  const withNotes = addNote(doc, 1, 960, 64, 480, 100);
  const withMore = addNote(withNotes, 1, 0, 60, 480, 100);
  const json = projectToCanonicalJson(withMore);
  const parsed = JSON.parse(json) as Project;
  assert.equal(parsed.notes[0].start, 0);
  assert.equal(parsed.notes[1].start, 960);
});
