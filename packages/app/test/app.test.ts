// <META - FILE SUMMARY - T06.AC2: the composition root runs edit -> schedule sync -> save in Node with fakes injected.>

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { loadNg, type Ng } from '@ng/abi';
import type { Project } from '@ng/document';
import type { ReplayOp, ScheduleCompiler, SyncDocument } from '@ng/playback';
import {
  MemoryBackend,
  ProjectStore,
  encodeProject,
  type Clock as StoreClock,
} from '@ng/persistence';
import { createApp, type TransportPort } from '../src/index.ts';

const EXAMPLE = new URL('../../../spec/fixture/example_project.json', import.meta.url);

const encoder = new TextEncoder();

/** A clock the test drives by hand; the watchdog's "2 s of silence" needs a real now(). */
function fakeClock(start = 1_700_000_000_000): { now(): number; advance(ms: number): void } {
  let current = start;
  return {
    now: () => current,
    advance: (ms) => {
      current += ms;
    },
  };
}

/** A store whose codec is the real wasm, so canonical bytes are the real canonical bytes. */
function realStore(ng: Ng, backend: MemoryBackend, clock = fakeClock()): ProjectStore {
  const storeClock: StoreClock = {
    now: () => clock.now(),
    setTimeout: (fn, ms) => setTimeout(fn, ms) as unknown as number,
    clearTimeout: (id) => clearTimeout(id),
  };
  return new ProjectStore({
    backend,
    clock: storeClock,
    codec: {
      migrate: (json) => ng.migrate(json),
      canonical: (json) => ng.canonical(json),
      validate: (json) => ng.validate(json),
    },
  });
}

/** Opens the fixture through the real store, so the document starts as typed notes. */
function exampleDoc(ng: Ng): Project {
  const opened = realStore(ng, new MemoryBackend()).openBytes(
    encoder.encode(readFileSync(EXAMPLE, 'utf8')),
  );
  assert.ok(opened.ok, `the fixture opens: ${opened.ok ? '' : opened.errors[0]?.code}`);
  return opened.doc;
}

/**
 * Note order on the way back from storage is the canonical order, not the insertion order.
 *
 * `ng_project_canonical` sorts by (start, lane, pitch) so two sessions that entered the
 * same notes in a different order produce the same bytes; the in-memory document keeps the
 * order they were entered in. Comparing the two directly would fail on a correct
 * implementation, so both sides are put in the same order first.
 */
function noteOrder(doc: Project): Array<[number, number, number]> {
  return doc.notes
    .map((n) => [n.lane, n.start, n.pitch] as [number, number, number])
    .sort((a, b) => a[1] - b[1] || a[0] - b[0] || a[2] - b[2]);
}

interface Harness {
  app: ReturnType<typeof createApp>;
  ops: ReplayOp[];
  transport: string[];
  closed: () => number;
  backend: MemoryBackend;
  store: ProjectStore;
  clock: ReturnType<typeof fakeClock>;
}

/**
 * The fakes T06.AC2 asks for: a port that records the FIFO instead of rendering, and
 * in-memory storage. The compiler is deliberately NOT faked -- "schedule sync" only means
 * something if the schedule is a real C3 schedule.
 */
function harness(ng: Ng, doc: Project, clock = fakeClock()): Harness {
  const ops: ReplayOp[] = [];
  const transport: string[] = [];
  let closedCount = 0;

  const port: TransportPort = {
    apply: (op) => {
      ops.push(op);
    },
    audition: () => undefined,
    play: () => transport.push('play'),
    pause: () => transport.push('pause'),
    stop: () => transport.push('stop'),
    close: () => {
      closedCount += 1;
    },
  };

  const compiler: ScheduleCompiler = {
    compile: (d: SyncDocument, sampleRate: number) =>
      ng.compile(encodeProject(d as unknown as Project), sampleRate),
  };

  const backend = new MemoryBackend();
  const store = realStore(ng, backend, clock);
  const app = createApp({
    document: doc,
    compiler,
    port,
    store,
    clock,
    sampleRate: 44100,
    debounceMs: 0,
  });

  return { app, ops, transport, backend, store, closed: () => closedCount, clock };
}

test('AC2: edit -> schedule sync -> save, with no browser in the loop', async () => {
  const ng = await loadNg();
  const doc = exampleDoc(ng);
  const h = harness(ng, doc);

  const before = h.ops.length;
  h.app.apply({ type: 'addNote', lane: 1, start: 960, pitch: 72, duration: 240 });
  assert.equal(h.app.document().notes.length, doc.notes.length + 1, 'the note was added');

  h.app.flush();
  const sent = h.ops.slice(before).map((op) => op.op);
  assert.ok(sent.includes('schedule'), `a note edit forces a recompile: ${sent.join(',')}`);
  // Mix rows come from lanes, not notes, so a note edit is only a
  // schedule change. The initial document carried its mix at creation,
  // together with the init that starts the port; its loop follows the
  // first schedule, because the engine attaches a loop to the loaded
  // schedule and refuses one before it.
  assert.deepEqual(
    h.ops.slice(0, before).map((op) => op.op),
    ['init', 'mix'],
    'the first document initialises the port, then states mix',
  );

  const scheduled = h.ops.filter((op) => op.op === 'schedule');
  assert.equal(scheduled.length, 1, 'one compile for one edit, not one per message');
  const bytes = (scheduled[0] as { bytes: Uint8Array }).bytes;
  assert.equal(String.fromCharCode(bytes[0], bytes[1], bytes[2], bytes[3]), 'NGSC', 'C3 schedule bytes');
  assert.equal(
    new DataView(bytes.buffer, bytes.byteOffset).getUint32(16, true),
    doc.lanes.length,
    'the compiled schedule carries the document lanes',
  );

  const saved = await h.app.save();
  assert.ok(saved.ok, `save succeeded: ${saved.ok ? '' : saved.code}`);

  const records = await h.backend.list('project');
  assert.equal(records.length, 1, 'one project record');
  const reopened = h.store.openBytes(records[0].bytes);
  assert.ok(reopened.ok, 'the saved bytes reopen');
  assert.deepEqual(
    noteOrder(reopened.doc),
    noteOrder(h.app.document()),
    'the durable copy is the edited document',
  );
  h.app.dispose();
});

test('AC2: the saved bytes are canonical, so a second save is byte-identical', async () => {
  const ng = await loadNg();
  const doc = exampleDoc(ng);
  const h = harness(ng, doc);
  h.app.apply({ type: 'addNote', lane: 2, start: 480, pitch: 67 });
  const first = await h.app.save();
  const second = await h.app.save();
  assert.ok(first.ok && second.ok);
  assert.deepEqual(second.bytes, first.bytes, 'canonical output does not depend on how it was produced');
  h.app.dispose();
});

test('AC2: undo re-syncs the engine, and the saved document follows', async () => {
  const ng = await loadNg();
  const doc = exampleDoc(ng);
  const h = harness(ng, doc);
  h.app.apply({ type: 'addNote', lane: 1, start: 1920, pitch: 79 });
  h.app.flush();
  assert.equal(h.app.document().notes.length, doc.notes.length + 1);

  const compilesBefore = h.ops.filter((op) => op.op === 'schedule').length;
  h.app.undo();
  h.app.flush();
  assert.equal(h.app.document().notes.length, doc.notes.length, 'undo restored the document');
  assert.ok(
    h.ops.filter((op) => op.op === 'schedule').length > compilesBefore,
    'the undo is a schedule change too, or the engine keeps a note the document dropped',
  );

  await h.app.save();
  const records = await h.backend.list('project');
  const reopened = h.store.openBytes(records[0].bytes);
  assert.ok(reopened.ok);
  assert.equal(reopened.doc.notes.length, doc.notes.length, 'the durable copy is the undone one');
  h.app.dispose();
});

test('AC2: transport reaches the port, and silence is a recovery not a hang', async () => {
  const ng = await loadNg();
  const h = harness(ng, exampleDoc(ng));
  assert.equal(h.app.togglePlay(), true);
  assert.equal(h.app.togglePlay(), false);
  assert.deepEqual(h.transport, ['play', 'pause'], `only the port hears transport: ${h.transport.join(',')}`);

  h.app.play();
  h.app.recordPosition(0);
  assert.equal(h.app.checkWatchdog(), false, 'a live position stream is not a recovery');

  // The watchdog only fires while playing, so a position stream that stops arriving has to
  // rebuild the port. With no factory the policy must say so rather than quietly retry.
  h.clock.advance(5000);
  const recovered = h.app.checkWatchdog();
  assert.equal(recovered, false, 'no portFactory means no silent retry');
  assert.ok(h.app.log().some((e) => e.code === 'E_NO_PORT_FACTORY'), 'and the reason is on the log');
  h.app.dispose();
});

test('AC2: a port factory recovers and replays the snapshot in order', async () => {
  const ng = await loadNg();
  const doc = exampleDoc(ng);
  const ops: ReplayOp[] = [];
  const makePort = (): TransportPort => ({
    apply: (op) => ops.push(op),
    audition: () => undefined,
    play: () => undefined,
    pause: () => undefined,
    stop: () => undefined,
    close: () => undefined,
  });
  const backend = new MemoryBackend();
  const clock = fakeClock();
  const store = realStore(ng, backend, clock);
  const app = createApp({
    document: doc,
    compiler: { compile: (d, sr) => ng.compile(encodeProject(d as unknown as Project), sr) },
    port: makePort(),
    portFactory: makePort,
    store,
    clock,
    sampleRate: 44100,
    debounceMs: 0,
  });

  app.apply({ type: 'addNote', lane: 1, start: 0, pitch: 60 });
  app.flush();
  app.play();
  app.recordPosition(0);
  clock.advance(5000);
  assert.equal(app.checkWatchdog(), true, 'a factory means the port is rebuilt');

  const replayed = ops.slice(-4).map((op) => op.op);
  assert.deepEqual(replayed, ['init', 'schedule', 'mix', 'loop'], 'T03.R05 replay order is fixed');
  app.dispose();
});

test('AC2: an intent the editor cannot perform yet is reported, not swallowed', async () => {
  const ng = await loadNg();
  const h = harness(ng, exampleDoc(ng));
  const notes = h.app.document().notes.length;
  const returned = h.app.apply({ type: 'quantize' });
  assert.equal(returned.notes.length, notes, 'the document is untouched');
  assert.equal(h.app.log()[0].code, 'E_UNSUPPORTED');
  h.app.dispose();
});

test('AC2: the layout is a value, so the arrangement is assertable without a DOM', async () => {
  const ng = await loadNg();
  const h = harness(ng, exampleDoc(ng));
  const layout = h.app.layout();
  assert.deepEqual(
    layout.regions,
    ['top-bar', 'palette', 'grid', 'inspector', 'bottom'],
    'T06 layout: top bar | palette | grid | inspector | bottom',
  );
  const undo = h.app.layout().topBar.find((item) => item.id === 'undo') as { enabled: boolean };
  assert.equal(undo.enabled, false, 'a fresh document has nothing to undo');
  assert.equal(h.app.layout().topBar.find((item) => item.id === 'save')?.label, 'Save');

  h.app.apply({ type: 'addNote', lane: 1, start: 0, pitch: 60 });
  const after = h.app.layout().topBar.find((item) => item.id === 'undo') as { enabled: boolean };
  assert.equal(after.enabled, true, 'undo lights up after an edit');
  assert.equal(h.app.layout().topBar.find((item) => item.id === 'save')?.label, 'Save*');
  h.app.dispose();
});