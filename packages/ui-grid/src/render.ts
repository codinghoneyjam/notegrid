// <META - FILE SUMMARY - Grid rendering functions (T02).>

import type { Project } from '@ng/document';
import type { Viewport, RenderMode, RenderContext } from './types.js';
import { cullNotes } from './culling.js';

const COMPACT_ROW_HEIGHT = 24;
const ROLL_ROW_HEIGHT = 20;
const DRUM_ROW_HEIGHT = 20;

export function renderGrid(ctx: RenderContext, doc: Project): void {
  const { ctx: c, viewport, visible, mode, dpr } = ctx;
  c.save();
  c.scale(dpr, dpr);
  c.clearRect(0, 0, viewport.width, viewport.height);

  const result = cullNotes(doc, viewport);

  // Draw lanes
  for (const lane of result.visibleLanes) {
    const y = (lane.id - visible.startLane) * viewport.zoomY - viewport.scrollY % viewport.zoomY;
    c.fillStyle = lane.id % 2 === 0 ? '#f0f0f0' : '#e0e0e0';
    c.fillRect(0, y, viewport.width, viewport.zoomY);
    c.strokeStyle = '#ccc';
    c.strokeRect(0, y, viewport.width, viewport.zoomY);
  }

  // Draw notes
  for (const note of result.visibleNotes) {
    const x = (note.start - visible.startTick) * viewport.zoomX;
    const y = (note.lane - visible.startLane) * viewport.zoomY - viewport.scrollY % viewport.zoomY;
    const w = note.duration * viewport.zoomX;
    const h = mode === 'compact' ? COMPACT_ROW_HEIGHT : ROLL_ROW_HEIGHT;
    c.fillStyle = `hsl(${note.pitch * 2}, 70%, 50%)`;
    c.fillRect(x, y, w, h);
  }

  c.restore();
}

export function renderRuler(ctx: RenderContext, doc: Project): void {
  const { ctx: c, viewport, visible, dpr } = ctx;
  c.save();
  c.scale(dpr, dpr);
  c.clearRect(0, 0, viewport.width, 30);

  const ppq = doc.ppq;
  const barLength = ppq * 4; // 4/4
  const startBar = Math.floor(visible.startTick / barLength);
  const endBar = Math.ceil(visible.endTick / barLength);

  c.fillStyle = '#333';
  c.font = '12px sans-serif';
  for (let bar = startBar; bar <= endBar; bar++) {
    const x = (bar * barLength - visible.startTick) * viewport.zoomX;
    c.fillText(`Bar ${bar + 1}`, x + 2, 20);
    c.strokeStyle = '#999';
    c.beginPath();
    c.moveTo(x, 0);
    c.lineTo(x, 30);
    c.stroke();
  }

  c.restore();
}

export function renderPlayhead(ctx: RenderContext, doc: Project, playheadTick: number): void {
  const { ctx: c, viewport, visible, dpr } = ctx;
  c.save();
  c.scale(dpr, dpr);

  const x = (playheadTick - visible.startTick) * viewport.zoomX;
  c.strokeStyle = '#f00';
  c.lineWidth = 2;
  c.beginPath();
  c.moveTo(x, 0);
  c.lineTo(x, viewport.height);
  c.stroke();

  c.restore();
}
