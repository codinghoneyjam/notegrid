// The page under test for e2e: the persistence matrix (T04.AC6) and the
// app composition matrix (T06.AC5 / gate g10).
//
// It imports the real package sources (bundled by e2e/build.mjs), so renaming
// anything in packages/persistence, packages/playback or packages/app breaks
// this harness instead of leaving a green spec behind. What it adds on top is
// only the seam a test needs: captured bytes, a seeded storage layer, and the
// validation panel and the app mounted where Playwright can see them.
//
// `?port=direct` mounts the composition root on the DirectPlaybackPort
// instead of the worklet, which is the T06.AC5 fallback path: the same
// app, the same engine calls, on the main thread.

import { Ng } from '@ng/abi';
import {
  buildValidationView,
  createApp,
  renderValidationPanel,
  type App,
} from '@ng/app';
import type { Project } from '@ng/document';
import {
  DirectPlaybackPort,
  createWorkletPort,
  loadWorkletModule,
  type ScheduleCompiler,
} from '@ng/playback';
import {
  browserPlatform,
  encodeProject,
  FileStore,
  MemoryBackend,
  ProjectStore,
  type OpenResult,
} from '@ng/persistence';

interface SmokeOutcome {
  ok: boolean;
  error: string;
}

interface SmokeApi {
  mounted: boolean;
  portKind(): 'worklet' | 'direct';
  audioState(): string;
  workletReady(): boolean;
  mountFixture(projectJson: string): Promise<SmokeOutcome>;
  noteCount(): number;
  placeNote(lane: number, start: number, pitch: number, duration: number): boolean;
  flush(): void;
  play(): void;
  pause(): void;
  stop(): void;
  playing(): boolean;
  blockPeaks(): number[];
  nonSilentBlocks(): number;
  lastPosition(): { sample: number; tick: number; playing: boolean } | null;
  errors(): string[];
  log(): Array<{ code: string; message: string }>;
  exportWav(): { ok: boolean; bytes: string; error: string };
  /** Renders a project string through the offline path, which needs no audio clock. */
  renderOffline(projectJson: string): { ok: boolean; bytes: string; error: string };
}

interface HarnessApi extends SmokeApi {
  usesFileSystemAccess: boolean;
  save(name: string, projectJson: string): Promise<{ ok: boolean; path: string; name: string }>;
  lastSavedBytes(): Uint8Array | null;
  decode(): Promise<{ ok: boolean; title: string; notes: number; error: string }>;
  renderRejected(): void;
  openFromPicker(): Promise<{ ok: boolean; cancelled: boolean; title: string; notes: number }>;
  autosaveThenSeed(projectJson: string): Promise<void>;
  recoveredTitle(): Promise<string>;
}

declare global {
  interface Window {
    ng: HarnessApi;
    __ngStorage?: Storage;
  }
}

const enc = new TextEncoder();
const dec = new TextDecoder();

const real = browserPlatform();

// Captured in memory instead of going to disk, so a test can read the bytes back and a
// run leaves nothing behind.
let captured: Uint8Array | null = null;
let picked: Uint8Array | null = null;

const platform = {
  hasFileSystemAccess: () => real.hasFileSystemAccess(),
  showSavePicker: async (name: string, bytes: Uint8Array) => {
    captured = bytes;
    return name;
  },
  showOpenPicker: async () => {
    if (real.hasFileSystemAccess()) return real.showOpenPicker();
    return picked;
  },
  pickFile: async () => picked,
  download: (name: string, bytes: Uint8Array) => {
    captured = bytes;
    void name;
  },
};

const files = new FileStore({ platform });

// One fetch serves both the main-thread Ng and the worklet's wasm module.
const response = await fetch('/ng-wasm.wasm');
const wasmBytes = new Uint8Array(await response.arrayBuffer());
const { instance } = await WebAssembly.instantiate(wasmBytes, {});
const ng = new Ng(instance.exports as never);

const backend = new MemoryBackend();
const clock = {
  now: () => Date.now(),
  setTimeout: (fn: () => void, ms: number) => setTimeout(fn, ms) as unknown as number,
  clearTimeout: (id: number) => clearTimeout(id),
};

const store = new ProjectStore({
  backend,
  clock,
  codec: {
    migrate: (json: string) => ng.migrate(json),
    canonical: (json: string) => ng.canonical(json),
    validate: (json: string) => ng.validate(json),
  },
});

const mount = document.getElementById('mount');
if (mount) {
  mount.appendChild(
    renderValidationPanel(
      { createElement: (tag: string) => document.createElement(tag) },
      buildValidationView({ kind: 'opened', warnings: [] }),
    ),
  );
}

// --- app composition (T06.AC5 / gate g10) ------------------------------------------------

/** The engine the spec is driving: worklet by default, direct under `?port=direct`. */
const portKind: 'worklet' | 'direct' =
  new URLSearchParams(window.location.search).get('port') === 'direct' ? 'direct' : 'worklet';

const peaks: number[] = [];
const positions: Array<{ sample: number; tick: number; playing: boolean }> = [];
const engineErrors: string[] = [];

let app: App | null = null;
let activeDirect: DirectPlaybackPort | null = null;
let directTimer: ReturnType<typeof setInterval> | null = null;
/** The module stays registered for the context's lifetime; loading it twice would re-register the name. */
let workletLoaded = false;
/** Set when the processor has answered `ready`; without it every port message is queued. */
let workletReady = false;

/**
 * The audio clock the composition root runs on; the spec resumes it with a key press.
 *
 * Resolved through `window` with a prefix fallback, and allowed to be absent: an engine
 * with no AudioContext still loads the document, edits and renders offline, which is the
 * editing-only degradation C11.3 describes. Constructing one unconditionally would throw at
 * module scope and take `window.ng` down with it, so the whole page would be unreachable
 * rather than merely audio-less.
 */
const AudioContextCtor: typeof AudioContext | undefined =
  (window as unknown as { AudioContext?: typeof AudioContext; webkitAudioContext?: typeof AudioContext })
    .AudioContext ??
  (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;

const audio: AudioContext | null = AudioContextCtor === undefined ? null : new AudioContextCtor();

const settingsStorage = {
  read: (key: string): string | null => window.localStorage.getItem(key),
  write: (key: string, value: string): void => {
    window.localStorage.setItem(key, value);
  },
};

/** The compiler the app uses: document -> real C3 schedule bytes, through the wasm. */
const compiler: ScheduleCompiler = {
  compile: (doc, sampleRate) => ng.compile(encodeProject(doc as unknown as Project), sampleRate),
};

function recordStatus(status: {
  sample: number;
  tick: number;
  playing: boolean;
  peak: number;
}): void {
  peaks.push(status.peak);
  positions.push({
    sample: status.sample,
    tick: status.tick,
    playing: status.playing,
  });
}

/** The direct port renders on demand; this timer is the main-thread stand-in for the audio clock. */
function startDirectTimer(): void {
  if (directTimer !== null) return;
  directTimer = setInterval(() => {
    if (app?.playing() && activeDirect) {
      activeDirect.renderBlock();
    }
  }, 4);
}

async function mountFixture(projectJson: string): Promise<SmokeOutcome> {
  try {
    const opened: OpenResult = store.openBytes(enc.encode(projectJson));
    if (!opened.ok) {
      return { ok: false, error: opened.errors[0]?.code ?? 'E_OPEN' };
    }

    // AudioContext starts suspended under autoplay policy; the spec presses a
    // key first, which is the user activation resume() needs.
    //
    // An engine with no AudioContext still opens, edits and renders; it simply cannot
    // play. `mountFixture` says so rather than throwing, so the page stays reachable and
    // the offline half of the scenario is still testable there.
    if (audio === null) {
      return { ok: false, error: 'NO_AUDIO_CONTEXT' };
    }
    await audio.resume();

    app?.dispose();
    if (directTimer !== null) {
      clearInterval(directTimer);
      directTimer = null;
    }
    peaks.length = 0;
    positions.length = 0;
    engineErrors.length = 0;

    const wasmModule = await WebAssembly.compile(wasmBytes);

    if (portKind === 'direct') {
      const makePort = (): DirectPlaybackPort => {
        activeDirect = new DirectPlaybackPort({ abi: ng, onStatus: recordStatus });
        return activeDirect;
      };
      app = createApp({
        document: opened.doc,
        compiler,
        port: makePort(),
        portFactory: makePort,
        store,
        sampleRate: audio.sampleRate,
        settingsStorage,
        projectId: 'e2e',
      });
      startDirectTimer();
    } else {
      if (!workletLoaded) {
        await loadWorkletModule(audio, '/worklet.js');
        workletLoaded = true;
      }
      const makePort = () =>
        createWorkletPort({
          context: audio,
          moduleUrl: '/worklet.js',
          wasmModule,
          onReady: () => {
            workletReady = true;
          },
          onBlock: (_frames, peak) => peaks.push(peak),
          onPosition: (sample, tick, playing) => positions.push({ sample, tick, playing }),
          onError: (code, message) => engineErrors.push(`${code}: ${message}`),
        });
      app = createApp({
        document: opened.doc,
        compiler,
        port: makePort(),
        portFactory: makePort,
        store,
        sampleRate: audio.sampleRate,
        settingsStorage,
        projectId: 'e2e',
      });
    }

    return { ok: true, error: '' };
  } catch (cause) {
    return { ok: false, error: cause instanceof Error ? cause.message : String(cause) };
  }
}

function toBase64(bytes: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(binary);
}

const smoke: SmokeApi = {
  get mounted() {
    return app !== null;
  },
  portKind: () => portKind,
  audioState: () => (audio === null ? 'unavailable' : audio.state),
  workletReady: () => workletReady,
  mountFixture,
  noteCount: () => app?.document().notes.length ?? -1,
  placeNote: (lane, start, pitch, duration) => {
    if (!app) return false;
    const before = app.document().notes.length;
    app.apply({ type: 'addNote', lane, start, pitch, duration });
    return app.document().notes.length === before + 1;
  },
  flush: () => app?.flush(),
  play: () => app?.play(),
  pause: () => app?.pause(),
  stop: () => app?.stop(),
  playing: () => app?.playing() ?? false,
  blockPeaks: () => [...peaks],
  nonSilentBlocks: () => peaks.filter((peak) => peak > 0).length,
  lastPosition: () => (positions.length > 0 ? positions[positions.length - 1] : null),
  errors: () => [...engineErrors],
  log: () => (app?.log() ?? []).map((entry) => ({ code: entry.code, message: entry.message })),
  exportWav: () => {
    if (!app) return { ok: false, bytes: '', error: 'no document mounted' };
    try {
      const json = encodeProject(app.document());
      const wav = ng.renderWav(json, app.settings().exportSampleRate, 0);
      return { ok: true, bytes: toBase64(wav), error: '' };
    } catch (cause) {
      return { ok: false, bytes: '', error: cause instanceof Error ? cause.message : String(cause) };
    }
  },
  renderOffline: (projectJson) => {
    // Deliberately independent of `app`: this is the path that must keep working on an
    // engine with no AudioContext, so it takes the JSON directly rather than the document
    // a mounted app would hold.
    try {
      const wav = ng.renderWav(projectJson, 44100, 0);
      return { ok: true, bytes: toBase64(wav), error: '' };
    } catch (cause) {
      return { ok: false, bytes: '', error: cause instanceof Error ? cause.message : String(cause) };
    }
  },
};

window.ng = {
  ...smoke,

  get usesFileSystemAccess() {
    return files.usesFileSystemAccess;
  },

  async save(name, projectJson) {
    const bytes = enc.encode(projectJson);
    const result = await files.save(name, bytes);
    return result.ok
      ? { ok: true, path: result.path, name: result.name }
      : { ok: false, path: result.reason, name };
  },

  lastSavedBytes() {
    return captured;
  },

  async decode() {
    if (captured === null) return { ok: false, title: '', notes: 0, error: 'nothing written' };
    const result: OpenResult = store.openBytes(captured);
    if (!result.ok) {
      return { ok: false, title: '', notes: 0, error: result.errors[0]?.code ?? 'E_UNKNOWN' };
    }
    return { ok: true, title: result.doc.title, notes: result.doc.notes.length, error: '' };
  },

  renderRejected() {
    const view = buildValidationView({
      kind: 'rejected',
      errors: [{ code: 'E_RANGE', message: 'pitch 0..=127', path: '/notes/0/3' }],
    });
    const host = { createElement: (tag: string) => document.createElement(tag) };
    const rendered = renderValidationPanel(host, view);
    const current = mount?.firstElementChild;
    if (current && mount) {
      mount.replaceChild(rendered, current);
    }
  },

  async openFromPicker() {
    picked = captured;
    const result = await files.open();
    if (!result.ok) {
      return { ok: false, cancelled: result.reason === 'cancelled', title: '', notes: 0 };
    }
    const opened = store.openBytes(result.bytes);
    return {
      ok: opened.ok,
      cancelled: false,
      title: opened.ok ? opened.doc.title : '',
      notes: opened.ok ? opened.doc.notes.length : 0,
    };
  },

  async autosaveThenSeed(projectJson) {
    // Go through open() so the document is a real Project (notes as objects). Handing
    // the store the raw score JSON would put tuple arrays where Note objects belong.
    const opened = store.openBytes(enc.encode(projectJson));
    if (!opened.ok) throw new Error(`fixture does not open: ${opened.errors[0]?.code}`);
    await store.save('ac6', opened.doc);

    store.autosave({ ...opened.doc, title: 'AC6 Fixture Unsaved' });
    const outcome = await store.flushAutosave();
    if (!outcome.ok) throw new Error(`autosave failed: ${outcome.code}`);

    // Mirror the ring into localStorage so it outlives the reload the spec performs.
    for (const record of await backend.list('autosave')) {
      window.localStorage.setItem(`ng:auto:${record.key}`, dec.decode(record.bytes));
    }
  },

  async recoveredTitle() {
    for (let i = 0; i < window.localStorage.length; i += 1) {
      const key = window.localStorage.key(i);
      if (key === null || !key.startsWith('ng:auto:')) continue;
      const bytes = enc.encode(window.localStorage.getItem(key) ?? '');
      const result = store.openBytes(bytes);
      if (result.ok) return result.doc.title;
    }
    return '';
  },
};
