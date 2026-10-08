// <META - FILE SUMMARY - Editing types: Command, History, Selection, Clipboard (T01).>

import type { Project, LaneKind } from '@ng/document';

export interface Command {
  label: string;
  apply(doc: Project): Project;
}

export interface History {
  execute(c: Command): void;
  undo(): void;
  redo(): void;
  canUndo(): boolean;
  canRedo(): boolean;
}

export interface Selection {
  noteIds: Set<number>;
  laneIds: Set<number>;
}

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

export interface SnapGrid {
  division: number; // 1/1..1/64
  triplet: boolean;
  off: boolean;
}

export interface QuantizeOptions {
  grid: number;
  strength: number; // 0-100
  includeLength: boolean;
}
