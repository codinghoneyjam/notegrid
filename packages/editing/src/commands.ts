// <META - FILE SUMMARY - Command implementations (T01).>

import type { Project, LaneKind } from '@ng/document';
import type { Command } from './types.js';
import { resolveOverlap } from './overlap.ts';

export function addNote(doc: Project, laneId: number, start: number, pitch: number, duration = 480, velocity = 100): Project {
  const resolved = resolveOverlap(doc, laneId, start, start + duration, pitch);
  const newNote = { lane: laneId, start, duration, pitch, velocity };
  return {
    ...resolved,
    notes: [...resolved.notes, newNote],
  };
}

export function moveNotes(doc: Project, indices: number[], dTick: number, dPitch: number, toLane?: number): Project {
  const newNotes = doc.notes.map((n, i) => {
    if (!indices.includes(i)) return n;
    return {
      ...n,
      start: Math.max(0, n.start + dTick),
      pitch: Math.max(0, Math.min(127, n.pitch + dPitch)),
      lane: toLane !== undefined ? toLane : n.lane,
    };
  });
  return { ...doc, notes: newNotes };
}

export function resizeNotes(doc: Project, indices: number[], edge: 'start' | 'end', dTicks: number): Project {
  const newNotes = doc.notes.map((n, i) => {
    if (!indices.includes(i)) return n;
    if (edge === 'end') {
      return { ...n, duration: Math.max(1, n.duration + dTicks) };
    } else {
      const newStart = Math.max(0, n.start + dTicks);
      const delta = newStart - n.start;
      return { ...n, start: newStart, duration: Math.max(1, n.duration - delta) };
    }
  });
  return { ...doc, notes: newNotes };
}

export function deleteNotes(doc: Project, indices: number[]): Project {
  const newNotes = doc.notes.filter((_, i) => !indices.includes(i));
  return { ...doc, notes: newNotes };
}

export function setVelocity(doc: Project, indices: number[], vel: number): Project {
  const newNotes = doc.notes.map((n, i) => {
    if (!indices.includes(i)) return n;
    return { ...n, velocity: Math.max(1, Math.min(127, vel)) };
  });
  return { ...doc, notes: newNotes };
}

export function addLane(doc: Project, kind: LaneKind, program: number): Project {
  const maxId = doc.lanes.reduce((max, l) => Math.max(max, l.id), 0);
  const newLane = {
    id: maxId + 1,
    name: `Lane ${maxId + 1}`,
    kind,
    program,
  };
  return { ...doc, lanes: [...doc.lanes, newLane] };
}

export function removeLane(doc: Project, laneId: number): Project {
  return {
    ...doc,
    lanes: doc.lanes.filter(l => l.id !== laneId),
    notes: doc.notes.filter(n => n.lane !== laneId),
  };
}

export function setLaneProgram(doc: Project, laneId: number, program: number): Project {
  const newLanes = doc.lanes.map(l => {
    if (l.id !== laneId) return l;
    return { ...l, program };
  });
  return { ...doc, lanes: newLanes };
}

export function setTempo(doc: Project, tick: number, bpm: number): Project {
  const newTempo = [...doc.tempo];
  const idx = newTempo.findIndex(t => t.tick === tick);
  if (idx >= 0) {
    newTempo[idx] = { ...newTempo[idx], bpm };
  } else {
    newTempo.push({ tick, bpm });
    newTempo.sort((a, b) => a.tick - b.tick);
  }
  return { ...doc, tempo: newTempo };
}

export function setMeter(doc: Project, tick: number, num: number, den: number): Project {
  const newMeter = [...doc.meter];
  const idx = newMeter.findIndex(m => m.tick === tick);
  if (idx >= 0) {
    newMeter[idx] = { ...newMeter[idx], num, den };
  } else {
    newMeter.push({ tick, num, den });
    newMeter.sort((a, b) => a.tick - b.tick);
  }
  return { ...doc, meter: newMeter };
}

export function setLoop(doc: Project, range: { start: number; end: number } | null): Project {
  if (range === null) {
    const { loop: _loop, ...rest } = doc;
    return rest as Project;
  }
  return { ...doc, loop: range };
}

export function makeCommand(label: string, apply: (doc: Project) => Project): Command {
  return { label, apply };
}
