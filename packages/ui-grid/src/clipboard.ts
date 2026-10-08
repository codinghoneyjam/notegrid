// <META - FILE SUMMARY - Clipboard handling (T02).>

import type { Project, LaneKind } from '@ng/document';

export interface ClipboardItem {
  laneKind: LaneKind;
  dLane: number;
  dTick: number;
  dur: number;
  pitch: number;
  vel: number;
}

export interface Clipboard {
  refTick: number;
  items: ClipboardItem[];
}

export function copyNotes(doc: Project, noteIndices: number[]): Clipboard {
  const refTick = noteIndices.length > 0 ? doc.notes[noteIndices[0]].start : 0;
  const items: ClipboardItem[] = noteIndices.map(i => {
    const note = doc.notes[i];
    const lane = doc.lanes.find(l => l.id === note.lane);
    return {
      laneKind: lane?.kind ?? 'melodic',
      dLane: note.lane - (lane?.id ?? 0),
      dTick: note.start - refTick,
      dur: note.duration,
      pitch: note.pitch,
      vel: note.velocity,
    };
  });
  return { refTick, items };
}

export function pasteNotes(doc: Project, clipboard: Clipboard, targetTick: number, targetLane: number): Project {
  const newNotes = clipboard.items.map(item => ({
    lane: targetLane,
    start: targetTick + item.dTick,
    duration: item.dur,
    pitch: item.pitch,
    velocity: item.vel,
  }));
  return {
    ...doc,
    notes: [...doc.notes, ...newNotes],
  };
}

export function canPaste(clipboard: Clipboard, targetLaneKind: LaneKind): boolean {
  return clipboard.items.every(item => item.laneKind === targetLaneKind);
}
