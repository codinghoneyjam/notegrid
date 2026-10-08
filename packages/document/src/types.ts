// <META - FILE SUMMARY - TypeScript document model types mirroring C1/C6 contracts.>

export type LaneKind = 'melodic' | 'drum';

export interface TempoItem {
  tick: number;
  bpm: number;
  ext?: Record<string, unknown>;
}

export interface MeterItem {
  tick: number;
  num: number;
  den: number;
  ext?: Record<string, unknown>;
}

export interface LoopRegion {
  start: number;
  end: number;
}

export interface Lane {
  id: number;
  name: string;
  kind: LaneKind;
  program?: number;
  kit?: number;
  volume?: number;
  pan?: number;
  reverb_send?: number;
  mute?: boolean;
  solo?: boolean;
  color?: string;
  ext?: Record<string, unknown>;
}

export interface Note {
  lane: number;
  start: number;
  duration: number;
  pitch: number;
  velocity: number;
}

export interface Project {
  format_version: number;
  title: string;
  ppq: number;
  tempo: TempoItem[];
  meter: MeterItem[];
  loop?: LoopRegion;
  length_ticks?: number;
  lanes: Lane[];
  notes: Note[];
  ext?: Record<string, unknown>;
}

export interface Report {
  errors: Array<{ code: string; message: string; path: string }>;
  warnings: Array<{ code: string; message: string; path: string }>;
}

export type Intent =
  | { type: 'addNote'; lane: number; start: number; pitch: number; duration?: number }
  | { type: 'moveNotes'; ids: number[]; dTick: number; dPitch: number; toLane?: number }
  | { type: 'resizeNotes'; ids: number[]; edge: 'start' | 'end'; dTicks: number }
  | { type: 'deleteNotes'; ids: number[] }
  | { type: 'setVelocity'; ids: number[]; vel: number }
  | { type: 'addLane'; kind: LaneKind; program: number }
  | { type: 'setLaneProgram'; lane: number; program: number }
  | { type: 'setTempo'; tick: number; bpm: number }
  | { type: 'setLoop'; range: { start: number; end: number } | null }
  | { type: 'paste' }
  | { type: 'duplicate' }
  | { type: 'quantize' }
  | { type: 'transpose' };

export interface Command {
  label: string;
  apply(doc: Project): Project;
}
