import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Project } from '@ng/document';
import {
  addNote,
  moveNotes,
  resizeNotes,
  deleteNotes,
  setVelocity,
  addLane,
  removeLane,
  setLaneProgram,
  setTempo,
  setMeter,
  setLoop,
  makeCommand,
} from '../src/commands.ts';
import { resolveOverlap } from '../src/overlap.ts';
import { HistoryImpl } from '../src/history.ts';

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

test('T01.AC1: random command sequences produce valid documents', () => {
  let doc = makeEmptyProject();
  const commands = [
    () => addNote(doc, 1, 0, 60, 480, 100),
    () => addNote(doc, 1, 480, 64, 480, 100),
    () => addNote(doc, 2, 0, 67, 480, 100),
    () => moveNotes(doc, [0], 240, 12),
    () => resizeNotes(doc, [1], 'end', 240),
    () => deleteNotes(doc, [2]),
    () => setVelocity(doc, [0], 80),
    () => addLane(doc, 'drum', 0),
    () => setLaneProgram(doc, 1, 42),
    () => setTempo(doc, 960, 140),
    () => setMeter(doc, 960, 3, 4),
    () => setLoop(doc, { start: 0, end: 1920 }),
  ];

  for (let i = 0; i < 100; i++) {
    const cmd = commands[Math.floor(Math.random() * commands.length)];
    doc = cmd();
    // I-01: lane ids unique, >= 1
    const laneIds = doc.lanes.map(l => l.id);
    assert.equal(new Set(laneIds).size, laneIds.length, 'I-01: lane ids must be unique');
    assert.ok(laneIds.every(id => id >= 1), 'I-01: lane ids must be >= 1');
    // I-02: every note.lane exists
    for (const note of doc.notes) {
      assert.ok(laneIds.includes(note.lane), 'I-02: every note.lane must exist');
    }
    // I-03: same lane + same pitch notes never overlap
    const byLanePitch = new Map<string, typeof doc.notes>();
    for (const note of doc.notes) {
      const key = `${note.lane}:${note.pitch}`;
      if (!byLanePitch.has(key)) byLanePitch.set(key, []);
      byLanePitch.get(key)!.push(note);
    }
    for (const [, notes] of byLanePitch) {
      const sorted = [...notes].sort((a, b) => a.start - b.start);
      for (let j = 1; j < sorted.length; j++) {
        assert.ok(sorted[j - 1].start + sorted[j - 1].duration <= sorted[j].start,
          'I-03: same lane + same pitch notes must not overlap');
      }
    }
  }
});

test('T01.AC2: undo-all == initial document, redo-all == final', () => {
  const initial = makeEmptyProject();
  const history = new HistoryImpl(initial);

  const cmd1 = makeCommand('addNote', (doc: Project) => addNote(doc, 1, 0, 60, 480, 100));
  const cmd2 = makeCommand('addNote', (doc: Project) => addNote(doc, 1, 480, 64, 480, 100));
  const cmd3 = makeCommand('addNote', (doc: Project) => addNote(doc, 2, 0, 67, 480, 100));

  history.execute(cmd1);
  history.execute(cmd2);
  history.execute(cmd3);

  const finalDoc = history.getCurrent();
  assert.equal(finalDoc.notes.length, 3);

  // Undo all
  while (history.canUndo()) {
    history.undo();
  }
  assert.deepEqual(history.getCurrent(), initial);

  // Redo all
  while (history.canRedo()) {
    history.redo();
  }
  assert.deepEqual(history.getCurrent(), finalDoc);
});

test('T01.AC3: overlap policy - trim / delete / replace', () => {
  // Trim: existing note's end trimmed to new start
  let doc = makeEmptyProject();
  doc = addNote(doc, 1, 0, 60, 480, 100);
  doc = addNote(doc, 1, 240, 60, 480, 100);
  assert.equal(doc.notes.length, 2);
  assert.equal(doc.notes[0].duration, 240);
  assert.equal(doc.notes[1].start, 240);

  // Delete: fully covered -> delete existing
  doc = makeEmptyProject();
  doc = addNote(doc, 1, 0, 60, 480, 100);
  doc = addNote(doc, 1, 0, 60, 960, 100);
  assert.equal(doc.notes.length, 1);
  assert.equal(doc.notes[0].duration, 960);

  // Replace: same start -> replace
  doc = makeEmptyProject();
  doc = addNote(doc, 1, 0, 60, 480, 100);
  doc = addNote(doc, 1, 0, 60, 960, 100);
  assert.equal(doc.notes.length, 1);
  assert.equal(doc.notes[0].duration, 960);
});

test('T01.AC4: undo of lane removal restores notes, ids and order', () => {
  const initial = makeEmptyProject();
  const history = new HistoryImpl(initial);

  const addCmd = makeCommand('addNote', (doc: Project) => addNote(doc, 1, 0, 60, 480, 100));
  const removeCmd = makeCommand('removeLane', (doc: Project) => removeLane(doc, 1));

  history.execute(addCmd);
  history.execute(removeCmd);

  assert.equal(history.getCurrent().lanes.length, 1);
  assert.equal(history.getCurrent().notes.length, 0);

  // Undo removeLane -> restores lane 1 and its note
  history.undo();
  assert.equal(history.getCurrent().lanes.length, 2);
  assert.equal(history.getCurrent().notes.length, 1);
  assert.equal(history.getCurrent().notes[0].pitch, 60);

  // Undo addNote -> back to initial
  history.undo();
  assert.deepEqual(history.getCurrent(), initial);

  // Redo addNote
  history.redo();
  assert.equal(history.getCurrent().lanes.length, 2);
  assert.equal(history.getCurrent().notes.length, 1);

  // Redo removeLane
  history.redo();
  assert.equal(history.getCurrent().lanes.length, 1);
  assert.equal(history.getCurrent().notes.length, 0);
});

test('T01.AC5: meter change snaps to bar boundary', () => {
  // 4/4 at 480 PPQ: bar = 1920 ticks
  let doc = makeEmptyProject();
  doc = setMeter(doc, 1920, 3, 4);
  assert.equal(doc.meter.length, 2);
  assert.equal(doc.meter[1].tick, 1920);
  assert.equal(doc.meter[1].num, 3);
  assert.equal(doc.meter[1].den, 4);

  // I-05: meter[0].tick == 0
  assert.equal(doc.meter[0].tick, 0);
});
