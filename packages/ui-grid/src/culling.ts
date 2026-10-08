// <META - FILE SUMMARY - Culling and hit-test functions (T02).>

import type { Project, Note, Lane } from '@ng/document';
import type { Viewport, VisibleRange, HitTestResult, CullingResult } from './types.js';

export function computeVisibleRange(doc: Project, viewport: Viewport): VisibleRange {
  const ppq = doc.ppq;
  const startTick = Math.floor(viewport.scrollX / viewport.zoomX);
  const endTick = Math.ceil((viewport.scrollX + viewport.width) / viewport.zoomX);
  const startLane = Math.floor(viewport.scrollY / viewport.zoomY);
  const endLane = Math.ceil((viewport.scrollY + viewport.height) / viewport.zoomY);
  return { startTick, endTick, startLane, endLane };
}

export function cullNotes(doc: Project, viewport: Viewport): CullingResult {
  const visible = computeVisibleRange(doc, viewport);
  const visibleNotes: Note[] = [];
  const visibleLanes: Lane[] = [];
  let drawCallCount = 0;

  for (const lane of doc.lanes) {
    if (lane.id >= visible.startLane && lane.id <= visible.endLane) {
      visibleLanes.push(lane);
      drawCallCount++;
    }
  }

  for (const note of doc.notes) {
    if (note.lane >= visible.startLane && note.lane <= visible.endLane) {
      if (note.start < visible.endTick && note.start + note.duration > visible.startTick) {
        visibleNotes.push(note);
        drawCallCount++;
      }
    }
  }

  return { visibleNotes, visibleLanes, drawCallCount };
}

export function hitTest(doc: Project, viewport: Viewport, x: number, y: number): HitTestResult {
  const tick = Math.floor((x + viewport.scrollX) / viewport.zoomX);
  const laneId = Math.floor((y + viewport.scrollY) / viewport.zoomY);

  const lane = doc.lanes.find(l => l.id === laneId);
  if (!lane) {
    return { type: 'empty', tick, laneId };
  }

  const noteIndex = doc.notes.findIndex(n => n.lane === laneId && n.start <= tick && n.start + n.duration > tick);
  if (noteIndex >= 0) {
    return { type: 'note', noteIndex, laneId, tick };
  }

  return { type: 'lane', laneId, tick };
}
