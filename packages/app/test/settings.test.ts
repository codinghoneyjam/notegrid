// <META - FILE SUMMARY - Settings, the local log and the layout model, tested as values rather than through a DOM.>

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  DEFAULT_SETTINGS,
  LAYOUT_REGIONS,
  SETTINGS_STORAGE_KEY,
  LocalLog,
  SettingsStore,
  buildLayout,
  parseSettings,
  serializeSettings,
  type SettingsStorage,
} from '../src/index.ts';
import type { LogEntry } from '@ng/persistence';

/** The buildLayout input, so a partial override in a test reads as one changed fact. */
type LayoutInput = Parameters<typeof buildLayout>[0];

function memoryStorage(seed: Record<string, string> = {}): SettingsStorage & { written: Map<string, string> } {
  const written = new Map<string, string>(Object.entries(seed));
  return {
    written,
    read: (key) => written.get(key) ?? null,
    write: (key, value) => {
      written.set(key, value);
    },
  };
}

// --- settings ------------------------------------------------------------------------------

test('settings: defaults are returned when storage is empty', () => {
  assert.deepEqual(parseSettings(null), DEFAULT_SETTINGS);
  assert.deepEqual(parseSettings(''), DEFAULT_SETTINGS);
  assert.deepEqual(parseSettings('{not json'), DEFAULT_SETTINGS);
  assert.deepEqual(parseSettings('42'), DEFAULT_SETTINGS);
});

test('settings: an unknown field from an older build is dropped, not fatal', () => {
  // The failure this guards is a settings blob written by a previous version: resetting the
  // whole object would lose a user's metronome preference over an unrecognised theme.
  const settings = parseSettings(
    JSON.stringify({ exportSampleRate: 48000, metronome: true, theme: 'solarized', futureFlag: 7 }),
  );
  assert.equal(settings.exportSampleRate, 48000, 'the values we understand survive');
  assert.equal(settings.metronome, true);
  assert.equal(settings.theme, 'system', 'the one we do not understand falls back');
});

test('settings: an unsupported sample rate falls back rather than being stored', () => {
  assert.equal(parseSettings(JSON.stringify({ exportSampleRate: 22050 })).exportSampleRate, 44100);
  assert.equal(parseSettings(JSON.stringify({ exportSampleRate: 44100 })).exportSampleRate, 44100);
});

test('settings: an update persists and reads back through a fresh store', () => {
  const storage = memoryStorage();
  const store = new SettingsStore(storage);
  assert.deepEqual(store.read(), DEFAULT_SETTINGS);

  const updated = store.update({ metronome: true, theme: 'dark', exportSampleRate: 48000 });
  assert.deepEqual(updated, { exportSampleRate: 48000, metronome: true, theme: 'dark', exportDestination: 'download' });

  // A second store over the same storage is what a page reload looks like.
  assert.deepEqual(new SettingsStore(storage).read(), updated, 'the choice survives a reload');
  assert.equal(storage.written.get(SETTINGS_STORAGE_KEY), serializeSettings(updated));
});

test('settings: an unsupported rate is refused instead of silently coerced', () => {
  const store = new SettingsStore(memoryStorage());
  assert.throws(() => store.update({ exportSampleRate: 22050 }), RangeError);
  assert.deepEqual(store.read(), DEFAULT_SETTINGS, 'a refused update changes nothing');
});

test('settings: a partial patch leaves the other fields alone', () => {
  const store = new SettingsStore(memoryStorage());
  store.update({ metronome: true });
  assert.deepEqual(store.update({ theme: 'light' }), {
    exportSampleRate: 44100,
    metronome: true,
    theme: 'light',
    exportDestination: 'download',
  });
});

test('settings: reset returns the defaults and writes them', () => {
  const storage = memoryStorage();
  const store = new SettingsStore(storage);
  store.update({ metronome: true, exportDestination: 'directory' });
  assert.deepEqual(store.reset(), DEFAULT_SETTINGS);
  assert.deepEqual(new SettingsStore(storage).read(), DEFAULT_SETTINGS);
});

// --- layout -------------------------------------------------------------------------------

const LAYOUT_INPUT: LayoutInput = {
  lanes: 4,
  notes: 23,
  canUndo: true,
  canRedo: false,
  playing: false,
  bpm: 124,
  meterNum: 4,
  meterDen: 4,
  loop: { start: 0, end: 7680 },
  errorCount: 2,
  warningCount: 1,
  logCount: 5,
  dirty: false,
};

test('layout: the five regions are the T06 arrangement, in order', () => {
  assert.deepEqual(LAYOUT_REGIONS, ['top-bar', 'palette', 'grid', 'inspector', 'bottom']);
  assert.deepEqual(buildLayout(LAYOUT_INPUT).regions, LAYOUT_REGIONS);
});

test('layout: the top bar carries file, history, transport, score and export', () => {
  const ids = buildLayout(LAYOUT_INPUT).topBar.map((item) => item.id);
  assert.deepEqual(ids, [
    'open',
    'save',
    'save-as',
    'undo',
    'redo',
    'play',
    'stop',
    'tempo',
    'meter',
    'loop',
    'export',
  ]);
});

test('layout: history and transport reflect what the app can actually do', () => {
  const enabled = (id: string, state: Partial<LayoutInput>): boolean => {
    const item = buildLayout({ ...LAYOUT_INPUT, ...state }).topBar.find((i) => i.id === id);
    return (item as { enabled?: boolean }).enabled ?? false;
  };
  assert.equal(enabled('undo', { canUndo: true }), true);
  assert.equal(enabled('undo', { canUndo: false }), false);
  assert.equal(enabled('redo', { canRedo: false }), false);

  const label = (state: Partial<LayoutInput>): string => {
    const item = buildLayout({ ...LAYOUT_INPUT, ...state }).topBar.find((i) => i.id === 'play');
    return (item as { label: string }).label;
  };
  assert.equal(label({ playing: false }), 'Play');
  assert.equal(label({ playing: true }), 'Pause');
});

test('layout: an unsaved edit marks Save, and the loop reads as a tick range', () => {
  const find = (state: Partial<LayoutInput>, id: string) =>
    buildLayout({ ...LAYOUT_INPUT, ...state }).topBar.find((i) => i.id === id) as { label: string };

  assert.equal(find({}, 'save').label, 'Save');
  assert.equal(find({ dirty: true }, 'save').label, 'Save*');
  assert.equal(find({}, 'tempo').label, '124 BPM');
  assert.equal(find({ bpm: 124.5 }, 'tempo').label, '124.50 BPM');
  assert.equal(find({}, 'meter').label, '4/4');
  assert.equal(find({}, 'loop').label, '0-7680');
  assert.equal(find({ loop: null }, 'loop').label, 'no loop');
});

test('layout: the bottom pane carries validation counts and the log count', () => {
  const bottom = buildLayout(LAYOUT_INPUT).bottom;
  assert.deepEqual(bottom, [
    { id: 'validation', label: 'Validation', errors: 2, warnings: 1 },
    { id: 'log', label: 'Log', entries: 5 },
  ]);
});

test('layout: lane and note counts are carried for the palette and the grid', () => {
  const layout = buildLayout(LAYOUT_INPUT);
  assert.equal(layout.lanes, 4);
  assert.equal(layout.notes, 23);
});

// --- local log ----------------------------------------------------------------------------

class RecordingSink {
  entries: Array<{ code: string; message: string }> = [];
  failNext = false;

  async log(code: string, message: string): Promise<void> {
    if (this.failNext) {
      this.failNext = false;
      throw new Error('storage is gone');
    }
    this.entries.push({ code, message });
  }

  async recentErrors(limit = 200): Promise<LogEntry[]> {
    return this.entries.slice(0, limit).map((e, i) => ({ at: 1000 - i, ...e }));
  }
}

function makeLog(sink: RecordingSink) {
  let now = 5000;
  const log = new LocalLog(sink, { now: () => now });
  return { log, tick: () => (now += 1) };
}

test('log: every entry carries its code and lands in the ring newest first', async () => {
  const sink = new RecordingSink();
  const { log, tick } = makeLog(sink);
  log.record('E_RANGE', 'pitch 0..=127');
  tick();
  log.record('E_COMPILE', 'model parse failed');

  assert.deepEqual(
    log.entries().map((e) => e.code),
    ['E_COMPILE', 'E_RANGE'],
    'newest first, the order the pane shows',
  );
  await log.settled();
  assert.equal(sink.entries.length, 2, 'and durable, in the same order');
});

test('log: a sink that refuses the write does not take the caller down', async () => {
  // T04.R02 one layer up: a failure must never make the next keystroke wait or throw.
  const sink = new RecordingSink();
  sink.failNext = true;
  const { log } = makeLog(sink);
  log.record('E_STORAGE', 'quota exceeded');
  assert.equal(log.entries().length, 1, 'the user still sees it');
  await log.settled();
});

test('log: the ring keeps the newest entries only', async () => {
  const sink = new RecordingSink();
  const log = new LocalLog(sink, { now: () => 0 }, 3);
  for (let i = 0; i < 6; i += 1) {
    log.record('E_RANGE', `entry ${i}`);
  }
  assert.deepEqual(
    log.entries().map((e) => e.message),
    ['entry 5', 'entry 4', 'entry 3'],
  );
});

test('log: reload refills the ring from durable storage, so a reload is not an empty panel', async () => {
  const sink = new RecordingSink();
  const { log } = await makeLog(sink);
  log.record('E_RANGE', 'from this session');
  await log.settled();

  const reopened = new LocalLog(sink, { now: () => 0 });
  assert.equal(reopened.entries().length, 0, 'a fresh process starts empty');
  await reopened.reload();
  assert.deepEqual(
    reopened.entries().map((e) => e.message),
    ['from this session'],
  );
});

test('log: subscribers see new entries and can unsubscribe', async () => {
  const sink = new RecordingSink();
  const { log } = makeLog(sink);
  const seen: string[] = [];
  const off = log.onChange((entries) => seen.push(entries[0]?.code ?? ''));
  log.record('E_A', 'a');
  off();
  log.record('E_B', 'b');
  assert.deepEqual(seen, ['E_A'], 'a removed listener stops hearing the log');
});