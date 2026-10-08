// <META - FILE SUMMARY - Keymap table (T02).>

export interface KeymapEntry {
  key: string;
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  action: string;
  description: string;
}

export const KEYMAP: KeymapEntry[] = [
  { key: 'Delete', action: 'deleteNotes', description: 'Delete selected notes' },
  { key: 'c', ctrl: true, action: 'copy', description: 'Copy selected notes' },
  { key: 'x', ctrl: true, action: 'cut', description: 'Cut selected notes' },
  { key: 'v', ctrl: true, action: 'paste', description: 'Paste from clipboard' },
  { key: 'd', ctrl: true, action: 'duplicate', description: 'Duplicate selected notes' },
  { key: 'z', ctrl: true, action: 'undo', description: 'Undo last action' },
  { key: 'Z', ctrl: true, shift: true, action: 'redo', description: 'Redo last action' },
  { key: 'a', ctrl: true, action: 'selectAll', description: 'Select all notes' },
  { key: 'ArrowUp', action: 'moveUp', description: 'Move notes up' },
  { key: 'ArrowDown', action: 'moveDown', description: 'Move notes down' },
  { key: 'ArrowLeft', action: 'moveLeft', description: 'Move notes left' },
  { key: 'ArrowRight', action: 'moveRight', description: 'Move notes right' },
  { key: 'ArrowUp', shift: true, action: 'moveOctaveUp', description: 'Move notes up one octave' },
  { key: 'ArrowDown', shift: true, action: 'moveOctaveDown', description: 'Move notes down one octave' },
  { key: 'ArrowLeft', shift: true, action: 'moveBarLeft', description: 'Move notes left one bar' },
  { key: 'ArrowRight', shift: true, action: 'moveBarRight', description: 'Move notes right one bar' },
  { key: ' ', action: 'playPause', description: 'Play or pause' },
  { key: 'Escape', action: 'cancel', description: 'Cancel current gesture' },
  { key: '?', action: 'help', description: 'Show help overlay' },
];

export function findKeymapEntry(
  key: string,
  ctrl: boolean,
  shift: boolean,
  alt: boolean
): KeymapEntry | undefined {
  return KEYMAP.find(
    e =>
      e.key === key &&
      (e.ctrl ?? false) === ctrl &&
      (e.shift ?? false) === shift &&
      (e.alt ?? false) === alt
  );
}
