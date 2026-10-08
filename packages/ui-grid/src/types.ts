// <META - FILE SUMMARY - UI Grid types: Viewport, Culling, HitTest, RenderMode (T02).>

import type { Project, Lane, Note } from '@ng/document';

export type RenderMode = 'compact' | 'roll' | 'drum';

export interface Viewport {
  scrollX: number;
  scrollY: number;
  zoomX: number;
  zoomY: number;
  width: number;
  height: number;
}

export interface VisibleRange {
  startTick: number;
  endTick: number;
  startLane: number;
  endLane: number;
}

export interface HitTestResult {
  type: 'note' | 'lane' | 'empty';
  noteIndex?: number;
  laneId?: number;
  tick?: number;
}

export interface RenderContext {
  ctx: CanvasRenderingContext2D;
  viewport: Viewport;
  visible: VisibleRange;
  mode: RenderMode;
  dpr: number;
}

export interface CullingResult {
  visibleNotes: Note[];
  visibleLanes: Lane[];
  drawCallCount: number;
}
