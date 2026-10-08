// <META - FILE SUMMARY - Lane header rendering helpers (T02).>

import type { Project, Lane } from '@ng/document';

export interface LaneHeaderInfo {
  lane: Lane;
  y: number;
  height: number;
  isMuted: boolean;
  isSolo: boolean;
  volume: number;
  pan: number;
}

export function getLaneHeaderInfo(doc: Project, laneId: number, zoomY: number, scrollY: number): LaneHeaderInfo | null {
  const lane = doc.lanes.find(l => l.id === laneId);
  if (!lane) return null;
  const index = doc.lanes.indexOf(lane);
  const y = index * zoomY - scrollY % zoomY;
  return {
    lane,
    y,
    height: zoomY,
    isMuted: lane.mute ?? false,
    isSolo: lane.solo ?? false,
    volume: lane.volume ?? 127,
    pan: lane.pan ?? 64,
  };
}

export function renderLaneHeader(ctx: CanvasRenderingContext2D, info: LaneHeaderInfo): void {
  const { lane, y, height, isMuted, isSolo, volume, pan } = info;
  ctx.fillStyle = isMuted ? '#666' : isSolo ? '#0066cc' : '#333';
  ctx.fillRect(0, y, 200, height);
  ctx.fillStyle = '#fff';
  ctx.font = '12px sans-serif';
  ctx.fillText(lane.name, 4, y + height / 2 + 4);
  ctx.fillText(`M:${isMuted ? 'Y' : 'N'} S:${isSolo ? 'Y' : 'N'} V:${volume} P:${pan}`, 100, y + height / 2 + 4);
}
