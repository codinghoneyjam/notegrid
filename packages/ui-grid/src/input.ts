// <META - FILE SUMMARY - Input handling: pointer/keyboard to Intent (T02).>

import type { Project, Intent } from '@ng/document';
import type { Viewport } from './types.js';
import { hitTest } from './culling.ts';

export interface InputState {
  isDragging: boolean;
  dragStartX: number;
  dragStartY: number;
  dragCurrentX: number;
  dragCurrentY: number;
  dragNoteIndex: number;
  dragLaneId: number;
  shiftKey: boolean;
  altKey: boolean;
  ctrlKey: boolean;
}

export function createInputState(): InputState {
  return {
    isDragging: false,
    dragStartX: 0,
    dragStartY: 0,
    dragCurrentX: 0,
    dragCurrentY: 0,
    dragNoteIndex: -1,
    dragLaneId: -1,
    shiftKey: false,
    altKey: false,
    ctrlKey: false,
  };
}

export function handlePointerDown(
  doc: Project,
  viewport: Viewport,
  state: InputState,
  x: number,
  y: number,
  shiftKey: boolean,
  altKey: boolean,
  ctrlKey: boolean
): InputState {
  const hit = hitTest(doc, viewport, x, y);
  return {
    ...state,
    isDragging: true,
    dragStartX: x,
    dragStartY: y,
    dragCurrentX: x,
    dragCurrentY: y,
    dragNoteIndex: hit.type === 'note' ? hit.noteIndex! : -1,
    dragLaneId: hit.type === 'note' ? hit.laneId! : -1,
    shiftKey,
    altKey,
    ctrlKey,
  };
}

export function handlePointerMove(
  state: InputState,
  x: number,
  y: number
): InputState {
  if (!state.isDragging) return state;
  return {
    ...state,
    dragCurrentX: x,
    dragCurrentY: y,
  };
}

export function handlePointerUp(
  doc: Project,
  viewport: Viewport,
  state: InputState
): { intent: Intent | null; state: InputState } {
  if (!state.isDragging) return { intent: null, state };

  const dx = state.dragCurrentX - state.dragStartX;
  const dy = state.dragCurrentY - state.dragStartY;
  const dTick = Math.round(dx / viewport.zoomX);
  const dLane = Math.round(dy / viewport.zoomY);

  let intent: Intent | null = null;

  if (state.dragNoteIndex >= 0) {
    if (state.altKey) {
      intent = { type: 'duplicate' };
    } else if (dLane !== 0) {
      const targetLane = state.dragLaneId + dLane;
      intent = { type: 'moveNotes', ids: [state.dragNoteIndex], dTick: 0, dPitch: 0, toLane: targetLane };
    } else {
      intent = { type: 'moveNotes', ids: [state.dragNoteIndex], dTick, dPitch: 0 };
    }
  } else {
    const hit = hitTest(doc, viewport, state.dragStartX, state.dragStartY);
    if (hit.type === 'empty' && hit.tick !== undefined && hit.laneId !== undefined) {
      intent = { type: 'addNote', lane: hit.laneId, start: hit.tick, pitch: 60 };
    }
  }

  return {
    intent,
    state: {
      ...state,
      isDragging: false,
      dragNoteIndex: -1,
      dragLaneId: -1,
    },
  };
}

export function handleKeyDown(
  doc: Project,
  state: InputState,
  key: string,
  shiftKey: boolean,
  ctrlKey: boolean
): Intent | null {
  if (ctrlKey) {
    switch (key) {
      case 'z':
        return { type: 'quantize' };
      case 'Z':
        return { type: 'transpose' };
      case 'c':
        return { type: 'paste' };
      case 'x':
        return { type: 'deleteNotes', ids: [] };
      case 'v':
        return { type: 'paste' };
      case 'd':
        return { type: 'duplicate' };
    }
  }

  switch (key) {
    case 'Delete':
      return { type: 'deleteNotes', ids: [] };
    case 'ArrowUp':
      return { type: 'moveNotes', ids: [], dTick: 0, dPitch: shiftKey ? 12 : 1 };
    case 'ArrowDown':
      return { type: 'moveNotes', ids: [], dTick: 0, dPitch: shiftKey ? -12 : -1 };
    case 'ArrowLeft':
      return { type: 'moveNotes', ids: [], dTick: shiftKey ? -480 : -1, dPitch: 0 };
    case 'ArrowRight':
      return { type: 'moveNotes', ids: [], dTick: shiftKey ? 480 : 1, dPitch: 0 };
    case ' ':
      return { type: 'addNote', lane: 1, start: 0, pitch: 60 };
  }

  return null;
}
