import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Project } from '@ng/document';
import { computeVisibleRange, cullNotes, hitTest } from '../src/culling.ts';
import type { Viewport } from '../src/types.ts';

function makeProjectWithLanesAndNotes(laneCount: number, notesPerLane: number): Project {
  const lanes = Array.from({ length: laneCount }, (_, i) => ({
    id: i + 1,
    name: `Lane ${i + 1}`,
    kind: 'melodic' as const,
    program: 0,
  }));
  const notes = Array.from({ length: laneCount * notesPerLane }, (_, i) => ({
    lane: Math.floor(i / notesPerLane) + 1,
    start: (i % notesPerLane) * 480,
    duration: 480,
    pitch: 60,
    velocity: 100,
  }));
  return {
    format_version: 1,
    title: 'Test Project',
    ppq: 480,
    tempo: [{ tick: 0, bpm: 120 }],
    meter: [{ tick: 0, num: 4, den: 4 }],
    lanes,
    notes,
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

test('culling: computeVisibleRange returns correct range', () => {
  const doc = makeProjectWithLanesAndNotes(10, 10);
  const viewport = makeViewport();
  const visible = computeVisibleRange(doc, viewport);
  assert.equal(visible.startTick, 0);
  assert.equal(visible.endTick, 800);
  assert.equal(visible.startLane, 0);
  assert.equal(visible.endLane, 25);
});

test('culling: cullNotes returns only visible notes and lanes', () => {
  const doc = makeProjectWithLanesAndNotes(128, 10);
  const viewport = makeViewport();
  const result = cullNotes(doc, viewport);
  assert.ok(result.visibleLanes.length <= 25);
  assert.ok(result.visibleNotes.length <= 250);
  assert.ok(result.drawCallCount > 0);
});

test('culling: draw-call count proportional to visible lanes', () => {
  const doc = makeProjectWithLanesAndNotes(128, 10);
  const viewport = makeViewport();
  const result = cullNotes(doc, viewport);
  assert.equal(result.drawCallCount, result.visibleLanes.length + result.visibleNotes.length);
});

test('culling: 128-lane document scroll fps within budget', () => {
  const doc = makeProjectWithLanesAndNotes(128, 10);
  const viewport = makeViewport();
  const start = performance.now();
  for (let i = 0; i < 100; i++) {
    cullNotes(doc, viewport);
  }
  const elapsed = performance.now() - start;
  const avgFps = 1000 / (elapsed / 100);
  assert.ok(avgFps >= 30, `Expected >= 30 fps, got ${avgFps.toFixed(2)} fps`);
});

test('hit-test: hitTest returns correct type and indices', () => {
  const doc = makeProjectWithLanesAndNotes(10, 10);
  const viewport = makeViewport();

  // Hit a note
  const noteResult = hitTest(doc, viewport, 100, 24);
  assert.equal(noteResult.type, 'note');
  assert.ok(noteResult.noteIndex !== undefined);
  assert.ok(noteResult.laneId !== undefined);

  // Hit empty space
  const emptyResult = hitTest(doc, viewport, 100, 1000);
  assert.equal(emptyResult.type, 'empty');

  // Hit a lane
  const laneResult = hitTest(doc, viewport, 100, 24);
  assert.equal(laneResult.type, 'note');
  assert.ok(laneResult.laneId !== undefined);
});

test('hit-test: hitTest with scrolled viewport', () => {
  const doc = makeProjectWithLanesAndNotes(10, 10);
  const viewport = makeViewport();
  viewport.scrollX = 480;
  viewport.scrollY = 24;

  const result = hitTest(doc, viewport, 100, 24);
  assert.equal(result.type, 'note');
  assert.ok(result.tick !== undefined);
  assert.ok(result.tick! >= 480);
});
