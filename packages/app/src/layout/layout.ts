// <META - FILE SUMMARY - The shell layout: five regions, and what the top bar and bottom panes hold right now.>

/** T06 DESIGN: top bar | left program palette | centre grid | right inspector | bottom. */
export type LayoutRegion = 'top-bar' | 'palette' | 'grid' | 'inspector' | 'bottom';

export const LAYOUT_REGIONS: readonly LayoutRegion[] = [
  'top-bar',
  'palette',
  'grid',
  'inspector',
  'bottom',
];

export type TopBarItem =
  | { id: 'open'; label: string; group: 'file' }
  | { id: 'save'; label: string; group: 'file' }
  | { id: 'save-as'; label: string; group: 'file' }
  | { id: 'undo'; label: string; group: 'history'; enabled: boolean }
  | { id: 'redo'; label: string; group: 'history'; enabled: boolean }
  | { id: 'play'; label: string; group: 'transport'; playing: boolean }
  | { id: 'stop'; label: string; group: 'transport' }
  | { id: 'tempo'; label: string; group: 'score'; bpm: number }
  | { id: 'meter'; label: string; group: 'score'; num: number; den: number }
  | { id: 'loop'; label: string; group: 'score'; start: number; end: number }
  | { id: 'export'; label: string; group: 'export' };

export type BottomPaneItem =
  | { id: 'validation'; label: string; errors: number; warnings: number }
  | { id: 'log'; label: string; entries: number };

export interface LayoutModel {
  regions: readonly LayoutRegion[];
  topBar: readonly TopBarItem[];
  bottom: readonly BottomPaneItem[];
  /** Lane count drives the palette and the grid's vertical extent. */
  lanes: number;
  notes: number;
}

export interface LayoutState {
  lanes: number;
  notes: number;
  canUndo: boolean;
  canRedo: boolean;
  playing: boolean;
  bpm: number;
  meterNum: number;
  meterDen: number;
  loop: { start: number; end: number } | null;
  errorCount: number;
  warningCount: number;
  logCount: number;
  dirty: boolean;
}

/**
 * The layout as data.
 *
 * The shell is the one place that owns the arrangement, and keeping it as a value rather
 * than as DOM mutations means the arrangement is assertable in Node (T06.AC2) and a renderer
 * becomes a projection of this model instead of a second source of truth. `dirty` marks the
 * title with a marker rather than blocking a close: the app has no window to close.
 */
export function buildLayout(state: LayoutState): LayoutModel {
  const topBar: TopBarItem[] = [
    { id: 'open', label: 'Open', group: 'file' },
    { id: 'save', label: state.dirty ? 'Save*' : 'Save', group: 'file' },
    { id: 'save-as', label: 'Save as', group: 'file' },
    { id: 'undo', label: 'Undo', group: 'history', enabled: state.canUndo },
    { id: 'redo', label: 'Redo', group: 'history', enabled: state.canRedo },
    { id: 'play', label: state.playing ? 'Pause' : 'Play', group: 'transport', playing: state.playing },
    { id: 'stop', label: 'Stop', group: 'transport' },
    { id: 'tempo', label: `${formatBpm(state.bpm)} BPM`, group: 'score', bpm: state.bpm },
    { id: 'meter', label: `${state.meterNum}/${state.meterDen}`, group: 'score', num: state.meterNum, den: state.meterDen },
    {
      id: 'loop',
      label: state.loop ? `${state.loop.start}-${state.loop.end}` : 'no loop',
      group: 'score',
      start: state.loop?.start ?? 0,
      end: state.loop?.end ?? 0,
    },
    { id: 'export', label: 'Export', group: 'export' },
  ];

  return {
    regions: LAYOUT_REGIONS,
    topBar,
    bottom: [
      {
        id: 'validation',
        label: 'Validation',
        errors: state.errorCount,
        warnings: state.warningCount,
      },
      { id: 'log', label: 'Log', entries: state.logCount },
    ],
    lanes: state.lanes,
    notes: state.notes,
  };
}

function formatBpm(bpm: number): string {
  // Two decimals is where a score writer can tell 124 from 124.0 and both read the same.
  return Number.isInteger(bpm) ? String(bpm) : bpm.toFixed(2);
}