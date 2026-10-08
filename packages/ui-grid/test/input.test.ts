import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Project } from '@ng/document';
import {
  createInputState,
  handlePointerDown,
  handlePointerMove,
  handlePointerUp,
  handleKeyDown,
} from '../src/input.ts';
import { findKeymapEntry, KEYMAP } from '../src/keymap.ts';
import { copyNotes, pasteNotes, canPaste } from '../src/clipboard.ts';
import type { Viewport } from '../src/types.ts';
import { readFileSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

function makeProject(): Project {
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
    notes: [
      { lane: 1, start: 0, duration: 480, pitch: 60, velocity: 100 },
      { lane: 1, start: 480, duration: 480, pitch: 64, velocity: 100 },
    ],
  };
}

function makeViewport(): Viewport {
  return {
    scrollX: 0,
    scrollY: 0,
    zoomX: 1,
    zoomY: 24,
    width: 800,
    height: 600,
  };
}

test('T02.AC2: one drag == one undo step', () => {
  const doc = makeProject();
  const viewport = makeViewport();
  let state = createInputState();

  // Start drag on first note (lane 1, y=24 is lane 1's row)
  state = handlePointerDown(doc, viewport, state, 100, 24, false, false, false);
  assert.equal(state.isDragging, true);
  assert.equal(state.dragNoteIndex, 0);

  // Move drag
  state = handlePointerMove(state, 200, 24);
  assert.equal(state.dragCurrentX, 200);

  // End drag -> should produce moveNotes intent
  const { intent, state: newState } = handlePointerUp(doc, viewport, state);
  assert.ok(intent);
  assert.equal(intent!.type, 'moveNotes');
  assert.equal((intent as any).ids.length, 1);
  assert.equal((intent as any).dTick, 100);
  assert.equal(newState.isDragging, false);
});

test('T02.AC4: add/move/delete/copy/paste by keyboard only', () => {
  const doc = makeProject();

  // Delete
  const deleteIntent = handleKeyDown(doc, createInputState(), 'Delete', false, false);
  assert.ok(deleteIntent);
  assert.equal(deleteIntent!.type, 'deleteNotes');

  // Copy
  const copyIntent = handleKeyDown(doc, createInputState(), 'c', false, true);
  assert.ok(copyIntent);
  assert.equal(copyIntent!.type, 'paste');

  // Paste
  const pasteIntent = handleKeyDown(doc, createInputState(), 'v', false, true);
  assert.ok(pasteIntent);
  assert.equal(pasteIntent!.type, 'paste');

  // Move up
  const moveUpIntent = handleKeyDown(doc, createInputState(), 'ArrowUp', false, false);
  assert.ok(moveUpIntent);
  assert.equal(moveUpIntent!.type, 'moveNotes');

  // Move down
  const moveDownIntent = handleKeyDown(doc, createInputState(), 'ArrowDown', false, false);
  assert.ok(moveDownIntent);
  assert.equal(moveDownIntent!.type, 'moveNotes');

  // Move left
  const moveLeftIntent = handleKeyDown(doc, createInputState(), 'ArrowLeft', false, false);
  assert.ok(moveLeftIntent);
  assert.equal(moveLeftIntent!.type, 'moveNotes');

  // Move right
  const moveRightIntent = handleKeyDown(doc, createInputState(), 'ArrowRight', false, false);
  assert.ok(moveRightIntent);
  assert.equal(moveRightIntent!.type, 'moveNotes');
});

test('T02.AC5: ui-grid bundle contains neither editing nor playback', () => {
  // This is a structural test - verify that ui-grid imports only from document
  const __dirname = dirname(fileURLToPath(import.meta.url));
  const srcDir = join(__dirname, '..', 'src');
  const files = readdirSync(srcDir).filter((f: string) => f.endsWith('.ts'));
  for (const file of files) {
    const content = readFileSync(join(srcDir, file), 'utf8');
    assert.ok(!content.includes('@ng/editing'), `${file} must not import @ng/editing`);
    assert.ok(!content.includes('@ng/playback'), `${file} must not import @ng/playback`);
  }
});

test('keymap: findKeymapEntry returns correct entry', () => {
  const entry = findKeymapEntry('Delete', false, false, false);
  assert.ok(entry);
  assert.equal(entry!.action, 'deleteNotes');

  const copyEntry = findKeymapEntry('c', true, false, false);
  assert.ok(copyEntry);
  assert.equal(copyEntry!.action, 'copy');

  const undoEntry = findKeymapEntry('z', true, false, false);
  assert.ok(undoEntry);
  assert.equal(undoEntry!.action, 'undo');
});

test('clipboard: copyNotes and pasteNotes round-trip', () => {
  const doc = makeProject();
  const clipboard = copyNotes(doc, [0, 1]);
  assert.equal(clipboard.items.length, 2);
  assert.equal(clipboard.refTick, 0);

  const newDoc = pasteNotes(doc, clipboard, 960, 2);
  assert.equal(newDoc.notes.length, 4);
  assert.equal(newDoc.notes[2].lane, 2);
  assert.equal(newDoc.notes[2].start, 960);
  assert.equal(newDoc.notes[3].start, 1440);
});

test('clipboard: canPaste checks lane kind', () => {
  const doc = makeProject();
  const clipboard = copyNotes(doc, [0, 1]);
  assert.ok(canPaste(clipboard, 'melodic'));
  assert.ok(!canPaste(clipboard, 'drum'));
});
