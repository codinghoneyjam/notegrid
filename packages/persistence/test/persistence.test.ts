// <META - FILE SUMMARY - T04 persistence acceptance tests: round trip, refusal, recovery, quota.>

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Ng } from '@ng/abi';
import type { Note, Project } from '@ng/document';
import { FileStore, MemoryBackend, ProjectStore, type Clock, type FilePlatform, type ScoreCodec } from '../src/index.ts';

// --- test doubles -------------------------------------------------------------------------------

class ManualClock implements Clock {
  private current = 0;
  private nextId = 1;
  private readonly timers = new Map<number, { at: number; fn: () => void }>();

  setTimeout(fn: () => void, ms: number): number {
    const id = this.nextId++;
    this.timers.set(id, { at: this.current + ms, fn });
    return id;
  }

  clearTimeout(id: number): void {
    this.timers.delete(id);
  }

  now(): number {
    return this.current;
  }

  advance(ms: number): void {
    const target = this.current + ms;
    for (;;) {
      const due = [...this.timers.entries()]
        .filter(([, t]) => t.at <= target)
        .sort((a, b) => a[1].at - b[1].at)[0];
      if (!due) break;
      this.timers.delete(due[0]);
      this.current = due[1].at;
      due[1].fn();
    }
    this.current = target;
  }
}

/** Drives the store against the real Rust canonical/migrate/validate (T04 NOT_OWNS format). */
class NgCodec implements ScoreCodec {
  private readonly ng: Ng;

  constructor(ng: Ng) {
    this.ng = ng;
  }

  migrate(json: string): string {
    return this.ng.migrate(json);
  }

  canonical(json: string): string {
    return this.ng.canonical(json);
  }

  validate(json: string): { errors: Report['errors']; warnings: Report['warnings'] } {
    return this.ng.validate(json);
  }
}

type Report = ReturnType<Ng['validate']>;

// --- fixtures -----------------------------------------------------------------------------------

/**
 * A project whose notes are already in canonical order: (start, lane, pitch).
 *
 * The fixture has to be canonical to begin with, because saving *does* reorder -- see
 * the "notes come back in canonical order" test for what that means for the editor.
 */
function makeProject(overrides: Partial<Project> = {}): Project {
  const base: Project = {
    format_version: 1,
    title: 'Persistence Test',
    ppq: 480,
    tempo: [{ tick: 0, bpm: 124 }],
    meter: [{ tick: 0, num: 4, den: 4 }],
    loop: { start: 0, end: 7680 },
    length_ticks: 7680,
    lanes: [
      { id: 1, name: 'Piano', kind: 'melodic', program: 0, volume: 100, pan: 64, reverb_send: 30, mute: false, solo: false },
      { id: 2, name: 'Drums', kind: 'drum', kit: 0, volume: 100, pan: 64, reverb_send: 10, mute: false, solo: false },
    ],
    notes: [
      { lane: 1, start: 0, duration: 480, pitch: 72, velocity: 100 },
      { lane: 2, start: 0, duration: 120, pitch: 36, velocity: 110 },
      { lane: 1, start: 480, duration: 480, pitch: 75, velocity: 96 },
    ],
  };
  return { ...base, ...overrides };
}

/** Drains the microtask queue so an awaited write inside the store can complete. */
function settle(): Promise<void> {
  return new Promise((resolve) => setImmediate(resolve));
}

function cmpNotes(a: Note, b: Note): number {
  return a.start - b.start || a.lane - b.lane || a.pitch - b.pitch;
}

async function makeStore(backend = new MemoryBackend()) {
  const ng = await loadFreshNg();
  const clock = new ManualClock();
  const store = new ProjectStore({ backend, codec: new NgCodec(ng), clock });
  return { store, backend, clock, ng };
}

let ngPromise: Promise<Ng> | null = null;

/** One wasm instance for the whole file: the engine statics are global per instance. */
async function loadFreshNg(): Promise<Ng> {
  if (ngPromise === null) {
    ngPromise = import('@ng/abi').then((m) => m.loadNg());
  }
  return ngPromise;
}

// --- T04.AC1 -------------------------------------------------------------------------------------

test('T04.AC1: save -> open round trip deep-equals the document', async () => {
  const { store } = await makeStore();
  const doc = makeProject();

  const saved = await store.save('p1', doc);
  assert.ok(saved.ok);
  const opened = await store.open('p1');
  assert.ok(opened.ok);
  assert.deepEqual(opened.doc, doc, 'the document survives the round trip unchanged');
});

test('T04.AC1: the saved bytes are exactly the canonical bytes', async () => {
  const { store, ng } = await makeStore();
  const doc = makeProject();

  const saved = await store.save('p1', doc);
  assert.ok(saved.ok);

  const canonical = ng.canonical(JSON.stringify({
    format_version: doc.format_version,
    title: doc.title,
    ppq: doc.ppq,
    tempo: doc.tempo,
    meter: doc.meter,
    loop: doc.loop,
    length_ticks: doc.length_ticks,
    lanes: doc.lanes,
    notes: doc.notes.map((n) => [n.lane, n.start, n.duration, n.pitch, n.velocity]),
  }));
  assert.equal(new TextDecoder().decode(saved.bytes), canonical);
});

test('T04.R01: save -> open -> save is byte-identical', async () => {
  const { store } = await makeStore();
  const first = await store.save('p1', makeProject());
  assert.ok(first.ok);

  const opened = await store.open('p1');
  assert.ok(opened.ok);
  const second = await store.save('p1', opened.doc);

  assert.ok(second.ok);
  assert.deepEqual(second.bytes, first.bytes, 'canonicalisation is a fixed point');
});

test('canonical output does not depend on the order notes were entered in', async () => {
  const { store } = await makeStore();
  const a = makeProject();
  const b = makeProject({ notes: [...a.notes].reverse() });

  const savedA = await store.save('a', a);
  const savedB = await store.save('b', b);
  assert.ok(savedA.ok && savedB.ok);
  assert.deepEqual(savedA.bytes, savedB.bytes);
});

test('notes come back in canonical (start, lane, pitch) order', async () => {
  const { store } = await makeStore();
  const shuffled = makeProject({
    notes: [
      { lane: 1, start: 480, duration: 480, pitch: 75, velocity: 96 },
      { lane: 2, start: 0, duration: 120, pitch: 36, velocity: 110 },
      { lane: 1, start: 0, duration: 480, pitch: 72, velocity: 100 },
    ],
  });

  await store.save('p1', shuffled);
  const opened = await store.open('p1');
  assert.ok(opened.ok);

  // The document set is identical; only the order is normalised.
  assert.deepEqual(opened.ok ? [...opened.doc.notes].sort(cmpNotes) : null,
    [...shuffled.notes].sort(cmpNotes), 'no note is lost or altered');
  assert.deepEqual(
    opened.ok ? opened.doc.notes.map((n) => [n.start, n.lane, n.pitch]) : null,
    [[0, 1, 72], [0, 2, 36], [480, 1, 75]],
    'reopened notes are sorted by start, then lane, then pitch',
  );
});

// --- T04.AC2 -------------------------------------------------------------------------------------

test('T04.AC2: an invalid file is refused with a code and a path for every error', async () => {
  const { store } = await makeStore();
  const before = makeProject();
  await store.save('p1', before);

  // A pitch outside 0..=127 and a zero-length note. Both are structural, so they are
  // reported together.
  const broken = new TextEncoder().encode(JSON.stringify({
    format_version: 1,
    title: 'Broken',
    ppq: 480,
    tempo: [{ tick: 0, bpm: 124 }],
    meter: [{ tick: 0, num: 4, den: 4 }],
    lanes: [{ id: 1, name: 'Piano', kind: 'melodic', program: 0 }],
    notes: [
      [1, 0, 480, 72, 100],
      [1, 480, 480, 999, 100],
      [1, 960, 0, 72, 100],
    ],
  }));

  const result = store.openBytes(broken);
  assert.equal(result.ok, false);
  assert.ok(!result.ok);
  assert.equal(result.errors.length, 2, `expected two errors, got ${result.errors.length}`);
  assert.deepEqual(result.errors.map((e) => e.code), ['E_RANGE', 'E_RANGE']);
  // The paths have to point at the offending field, not just at the notes array.
  assert.deepEqual(result.errors.map((e) => e.path), ['/notes/1/3', '/notes/2/2']);
  for (const issue of result.errors) {
    assert.ok(issue.code.length > 0, 'every error carries a code');
    assert.ok(issue.path.startsWith('/'), `every error carries a path, got '${issue.path}'`);
    assert.ok(issue.message.length > 0, 'every error carries a message');
  }

  // T04.AC2: the document the editor already had is untouched.
  const still = await store.open('p1');
  assert.ok(still.ok);
  assert.deepEqual(still.doc, before);
});

test('T04.AC2: a note pointing at a lane that does not exist is refused', async () => {
  const { store } = await makeStore();
  const result = store.openBytes(new TextEncoder().encode(JSON.stringify({
    format_version: 1,
    title: 'Dangling',
    ppq: 480,
    tempo: [{ tick: 0, bpm: 124 }],
    meter: [{ tick: 0, num: 4, den: 4 }],
    lanes: [{ id: 1, name: 'Piano', kind: 'melodic', program: 0 }],
    notes: [[1, 0, 480, 72, 100], [9, 0, 480, 72, 100]],
  })));
  assert.equal(result.ok, false);
  assert.ok(!result.ok);
  assert.deepEqual(result.errors.map((e) => e.code), ['E_LANE_MISSING']);
  assert.equal(result.errors[0].path, '/notes/1/0');
});

test('T04.AC2: a structural error hides the semantic ones behind it', async () => {
  // ng-score validates structure first and returns early, so a file with both a bad
  // pitch and a dangling lane reports only the pitch. The panel must present this as
  // "fix and try again" rather than as a complete error list, which is why the view
  // model is tested separately from the store.
  const { store } = await makeStore();
  const result = store.openBytes(new TextEncoder().encode(JSON.stringify({
    format_version: 1,
    title: 'Both',
    ppq: 480,
    tempo: [{ tick: 0, bpm: 124 }],
    meter: [{ tick: 0, num: 4, den: 4 }],
    lanes: [{ id: 1, name: 'Piano', kind: 'melodic', program: 0 }],
    notes: [[1, 0, 480, 999, 100], [9, 0, 480, 72, 100]],
  })));
  assert.equal(result.ok, false);
  assert.ok(!result.ok);
  assert.deepEqual(result.errors.map((e) => e.code), ['E_RANGE']);
});

test('T04.AC2: garbage bytes are refused rather than half-opened', async () => {
  const { store } = await makeStore();
  const result = store.openBytes(new TextEncoder().encode('{not json'));
  assert.equal(result.ok, false);
  assert.ok(!result.ok && result.errors[0].code.startsWith('E_'));
});

test('warnings open the file and come back with it', async () => {
  const { store, ng } = await makeStore();
  // A note reaching past length_ticks and a lane with nothing in it are both warnings.
  const payload = {
    format_version: 1,
    title: 'Warned',
    ppq: 480,
    tempo: [{ tick: 0, bpm: 124 }],
    meter: [{ tick: 0, num: 4, den: 4 }],
    length_ticks: 480,
    lanes: [
      { id: 1, name: 'Piano', kind: 'melodic', program: 0 },
      { id: 2, name: 'Unused', kind: 'melodic', program: 0 },
    ],
    notes: [[1, 0, 960, 72, 100]],
  };
  const report = ng.validate(JSON.stringify(payload));
  assert.deepEqual(
    report.warnings.map((w) => w.code).sort(),
    ['W_NOTE_PAST_LENGTH', 'W_UNUSED_LANE'],
    'fixture must actually produce the warnings it is testing',
  );
  assert.deepEqual(report.errors, [], 'and none of them are errors');

  const result = store.openBytes(new TextEncoder().encode(JSON.stringify(payload)));
  assert.ok(result.ok, 'a warning does not refuse the file');
  assert.ok(result.ok && result.warnings.length === 2);
  assert.equal(result.ok && result.doc.lanes.length, 2, 'the whole project still opened');
});

test('a loop past the project length is an error, not a warning', async () => {
  const { store } = await makeStore();
  // I-06: loop present -> start < end <= effective length. Anything else is E_LOOP.
  const result = store.openBytes(new TextEncoder().encode(JSON.stringify({
    format_version: 1,
    title: 'Bad loop',
    ppq: 480,
    tempo: [{ tick: 0, bpm: 124 }],
    meter: [{ tick: 0, num: 4, den: 4 }],
    length_ticks: 480,
    loop: { start: 0, end: 9600 },
    lanes: [{ id: 1, name: 'Piano', kind: 'melodic', program: 0 }],
    notes: [[1, 0, 480, 72, 100]],
  })));
  assert.equal(result.ok, false);
  assert.ok(!result.ok && result.errors.some((e) => e.code === 'E_LOOP'));
});

// --- T04.AC3 -------------------------------------------------------------------------------------

test('T04.AC3: after a reload the newer autosave is offered and recovery succeeds', async () => {
  const backend = new MemoryBackend();
  const { store, clock } = await makeStore(backend);

  // Session one: save, then keep editing without saving.
  const saved = makeProject({ title: 'Saved version' });
  await store.save('p1', saved);
  clock.advance(1000);

  const unsaved = makeProject({
    title: 'Unsaved version',
    notes: [
      ...saved.notes,
      { lane: 1, start: 960, duration: 480, pitch: 79, velocity: 90 },
    ],
  });
  store.autosave(unsaved);
  clock.advance(5000);

  // Session two: a brand new store over the same storage is the reload.
  const ng = await loadFreshNg();
  const reloaded = new ProjectStore({
    backend,
    codec: new NgCodec(ng),
    clock,
  });

  const candidate = await reloaded.recoveryCandidate('p1');
  assert.ok(candidate !== null, 'the newer autosave is offered');
  assert.ok(candidate.savedAt !== null);
  assert.ok(candidate.autosavedAt > candidate.savedAt!);

  const recovered = await reloaded.acceptRecovery('p1', candidate);
  assert.ok(recovered.ok);
  assert.equal(recovered.ok && recovered.doc.title, 'Unsaved version', 'the unsaved work came back');
  assert.deepEqual(recovered.ok && recovered.doc.notes, unsaved.notes);

  // Recovery ends durable: a further reload has nothing to offer.
  const settled = await reloaded.recoveryCandidate('p1');
  assert.equal(settled, null, 'recovery was promoted to a real save');
});

test('T04.AC3: no recovery is offered when the autosave is older than the save', async () => {
  const backend = new MemoryBackend();
  const { store, clock } = await makeStore(backend);

  store.autosave(makeProject({ title: 'Older autosave' }));
  clock.advance(5000);
  await store.save('p1', makeProject({ title: 'Newer save' }));
  clock.advance(1000);

  assert.equal(await store.recoveryCandidate('p1'), null);
});

test('T04.AC3: no recovery is offered when nothing was ever autosaved', async () => {
  const { store } = await makeStore();
  await store.save('p1', makeProject());
  assert.equal(await store.recoveryCandidate('p1'), null);
});

test('the autosave ring keeps five slots, newest first', async () => {
  const backend = new MemoryBackend();
  const { store, clock } = await makeStore(backend);

  for (let i = 0; i < 8; i += 1) {
    store.autosave(makeProject({ title: `pass ${i}` }));
    clock.advance(5000);
    // The debounce fires the write, but the write is async; let it land before the next
    // edit, exactly as it would with a real clock between two keystrokes.
    await settle();
  }

  const slots = await backend.list('autosave');
  assert.equal(slots.length, 5, `the ring is bounded, got ${slots.length}`);
  const titles: string[] = [];
  for (const slot of slots) {
    const opened = store.openBytes(slot.bytes);
    assert.ok(opened.ok);
    if (opened.ok) titles.push(opened.doc.title);
  }
  assert.deepEqual(titles, ['pass 7', 'pass 6', 'pass 5', 'pass 4', 'pass 3']);
});

test('autosave coalesces a burst into one write after 5 s', async () => {
  const backend = new MemoryBackend();
  const { store, clock } = await makeStore(backend);

  for (let i = 0; i < 10; i += 1) {
    store.autosave(makeProject({ title: `keystroke ${i}` }));
    clock.advance(300);
  }
  assert.equal((await backend.list('autosave')).length, 0, 'still inside the window');

  clock.advance(5000);
  await settle();
  const slots = await backend.list('autosave');
  assert.equal(slots.length, 1, 'one write for the whole burst');
  const opened = store.openBytes(slots[0].bytes);
  assert.equal(opened.ok && opened.doc.title, 'keystroke 9', 'the last edit is the one kept');
});

// --- T04.AC5 -------------------------------------------------------------------------------------

test('T04.AC5: a quota-exceeded save reports E_STORAGE and offers the file, editing is not blocked', async () => {
  const backend = new MemoryBackend();
  const { store, clock } = await makeStore(backend);

  await store.save('p1', makeProject());
  clock.advance(1000);

  backend.quotaExhaustedOn = 'project';
  const result = await store.save('p1', makeProject({ title: 'Bigger' }));

  assert.equal(result.ok, false);
  assert.ok(!result.ok);
  assert.equal(result.code, 'E_STORAGE');
  assert.equal(result.quotaExceeded, true);
  assert.equal(result.offerSaveToFile, true, 'T04.R06: the file path is always offered');

  // Editing continues: autosave still runs and still reports rather than throwing.
  store.autosave(makeProject({ title: 'still editing' }));
  clock.advance(5000);
  const outcome = await store.flushAutosave();
  assert.equal(outcome.ok, true, 'autosave uses its own store and is unaffected');

  // And the previous save is still intact.
  backend.quotaExhaustedOn = null;
  const opened = await store.open('p1');
  assert.ok(opened.ok);
  assert.equal(opened.ok && opened.doc.title, 'Persistence Test');
});

test('T04.AC5: a failed autosave is reported and never thrown at the editor', async () => {
  const backend = new MemoryBackend();
  const { store, clock } = await makeStore(backend);
  backend.quotaExhaustedOn = 'autosave';

  store.autosave(makeProject());
  clock.advance(5000);
  const outcome = await store.flushAutosave();

  assert.equal(outcome.ok, false);
  assert.ok(!outcome.ok && outcome.offerSaveToFile);
  assert.ok(!outcome.ok && outcome.code === 'E_STORAGE');

  const logged = await store.recentErrors();
  assert.ok(logged.some((e) => e.code === 'E_STORAGE'), 'the failure is in the log');
});

test('storage that is gone entirely surfaces as E_STORAGE with the file offered', async () => {
  const backend = new MemoryBackend();
  const { store } = await makeStore(backend);
  backend.failing = true;

  const result = await store.save('p1', makeProject());
  assert.equal(result.ok, false);
  assert.ok(!result.ok && result.code === 'E_STORAGE');
  assert.ok(!result.ok && result.offerSaveToFile);
});

// --- T04.R02 / R03 / R06 -------------------------------------------------------------------------

test('T04.R02: editing is never blocked, whatever storage does', async () => {
  const backend = new MemoryBackend();
  const { store, clock } = await makeStore(backend);
  backend.failing = true;

  for (let i = 0; i < 5; i += 1) {
    store.autosave(makeProject());
    clock.advance(5000);
    await store.flushAutosave();
  }
  const opened = store.openBytes(new TextEncoder().encode('{}'));
  assert.ok(!opened.ok, 'storage failures come back as results, not exceptions');
  assert.equal(await store.recoveryCandidate('p1'), null);
});

test('the error log keeps the newest 200 entries', async () => {
  const { store, clock } = await makeStore();
  for (let i = 0; i < 250; i += 1) {
    clock.advance(1);
    await store.log('E_TEST', `entry ${i}`);
  }
  const entries = await store.recentErrors();
  assert.equal(entries.length, 200);
  assert.equal(entries[0].message, 'entry 249', 'newest first');
});

test('T04: persist() is requested once and a refusal is survivable', async () => {
  const backend = new MemoryBackend();
  const { store } = await makeStore(backend);

  backend.persistGranted = true;
  assert.equal(await store.requestPersistent(), true);

  backend.persistGranted = false;
  assert.equal(await store.requestPersistent(), false);

  backend.failing = true;
  assert.equal(await store.requestPersistent(), false, 'a broken navigator is not fatal');
});

// --- T04.R04 / AC6 fallback path ------------------------------------------------------------------

class RecordingPlatform implements FilePlatform {
  saved: Array<{ name: string; bytes: Uint8Array }> = [];
  downloaded: Array<{ name: string; bytes: Uint8Array }> = [];
  picked: Uint8Array | null = null;
  private readonly fsa: boolean;

  constructor(fsa: boolean) {
    this.fsa = fsa;
  }

  hasFileSystemAccess(): boolean {
    return this.fsa;
  }

  async showSavePicker(suggestedName: string, bytes: Uint8Array): Promise<string | null> {
    if (!this.fsa) return null;
    this.saved.push({ name: suggestedName, bytes });
    return suggestedName;
  }

  async showOpenPicker(): Promise<Uint8Array | null> {
    return this.fsa ? this.picked : null;
  }

  async pickFile(): Promise<Uint8Array | null> {
    return this.fsa ? null : this.picked;
  }

  download(name: string, bytes: Uint8Array): void {
    this.downloaded.push({ name, bytes });
  }
}

test('T04.R04: without File System Access the save goes out as a download', async () => {
  const platform = new RecordingPlatform(false);
  const files = new FileStore({ platform });
  assert.equal(files.usesFileSystemAccess, false);

  const saved = await files.save('song', new Uint8Array([1, 2, 3]));
  assert.ok(saved.ok);
  assert.equal(saved.ok && saved.path, 'download');
  assert.equal(saved.ok && saved.name, 'song.ngproj.json', 'the extension is added');
  assert.deepEqual(platform.downloaded[0].bytes, new Uint8Array([1, 2, 3]));
  assert.equal(platform.saved.length, 0, 'the picker is never touched');
});

test('T04.R04: without File System Access the open goes through the file input', async () => {
  const platform = new RecordingPlatform(false);
  platform.picked = new Uint8Array([9, 9]);
  const files = new FileStore({ platform });

  const opened = await files.open();
  assert.ok(opened.ok);
  assert.equal(opened.ok && opened.path, 'input-file');
  assert.deepEqual(opened.ok && opened.bytes, new Uint8Array([9, 9]));
});

test('with File System Access the pickers are used instead', async () => {
  const platform = new RecordingPlatform(true);
  platform.picked = new Uint8Array([4]);
  const files = new FileStore({ platform });
  assert.equal(files.usesFileSystemAccess, true);

  const saved = await files.save('song', new Uint8Array([1]));
  assert.ok(saved.ok && saved.path === 'file-system-access');

  const opened = await files.open();
  assert.ok(opened.ok && opened.path === 'file-system-access');
  assert.equal(platform.downloaded.length, 0, 'no download fallback when the picker exists');
});

test('a cancelled dialog is a cancellation, not an error', async () => {
  // The pickers return null for "user pressed cancel"; they throw for a real failure.
  // Conflating the two would show the user an error for something they did on purpose.
  const cancelling: FilePlatform = {
    hasFileSystemAccess: () => true,
    showSavePicker: async () => null,
    showOpenPicker: async () => null,
    pickFile: async () => null,
    download: () => undefined,
  };
  const files = new FileStore({ platform: cancelling });
  const saved = await files.save('song', new Uint8Array([1]));
  assert.ok(!saved.ok && saved.reason === 'cancelled');
  const opened = await files.open();
  assert.ok(!opened.ok && opened.reason === 'cancelled');
});

test('a cancelled fallback open is a cancellation too', async () => {
  const cancelling: FilePlatform = {
    hasFileSystemAccess: () => false,
    showSavePicker: async () => null,
    showOpenPicker: async () => null,
    pickFile: async () => null,
    download: () => undefined,
  };
  const files = new FileStore({ platform: cancelling });
  const opened = await files.open();
  assert.ok(!opened.ok && opened.reason === 'cancelled');
  // A download cannot be cancelled once triggered, so saving always reports success here.
  const saved = await files.save('song', new Uint8Array([1]));
  assert.ok(saved.ok && saved.path === 'download');
});

test('a throwing picker is an error, not a cancellation', async () => {
  const throwing: FilePlatform = {
    hasFileSystemAccess: () => true,
    showSavePicker: async () => {
      throw new Error('permission denied');
    },
    showOpenPicker: async () => {
      throw new Error('permission denied');
    },
    pickFile: async () => null,
    download: () => undefined,
  };
  const files = new FileStore({ platform: throwing });
  const saved = await files.save('song', new Uint8Array([1]));
  assert.ok(!saved.ok && saved.reason === 'error');
  assert.ok(!saved.ok && saved.message === 'permission denied');
  const opened = await files.open();
  assert.ok(!opened.ok && opened.reason === 'error');
});

test('the extension is not doubled when the name already has it', () => {
  const files = new FileStore({ platform: new RecordingPlatform(false) });
  return files.save('song.ngproj.json', new Uint8Array([1])).then((saved) => {
    assert.equal(saved.ok && saved.name, 'song.ngproj.json');
  });
});

test('a file written through the fallback reopens to the same document', async () => {
  const { store } = await makeStore();
  const doc = makeProject();
  const saved = await store.save('p1', doc);
  assert.ok(saved.ok);

  // Exactly what a Firefox download gives back: bytes in, document out.
  const platform = new RecordingPlatform(false);
  platform.picked = saved.bytes;
  const files = new FileStore({ platform });
  const opened = await files.open();
  assert.ok(opened.ok);

  const decoded = store.openBytes(opened.ok ? opened.bytes : new Uint8Array());
  assert.ok(decoded.ok);
  assert.deepEqual(decoded.ok && decoded.doc, doc);
});