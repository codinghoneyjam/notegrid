// The page under test for e2e/tests/persistence.spec.ts.
//
// It imports the real package sources (bundled by e2e/build.mjs), so renaming anything in
// packages/persistence or packages/app breaks this harness instead of leaving a green
// spec behind. What it adds on top is only the seam a test needs: captured bytes, a
// seeded storage layer, and the panel mounted where Playwright can see it.

import { Ng } from '@ng/abi';
import { buildValidationView, renderValidationPanel } from '@ng/app';
import {
  browserPlatform,
  FileStore,
  MemoryBackend,
  ProjectStore,
  type OpenResult,
} from '@ng/persistence';

interface HarnessApi {
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

async function loadNg(): Promise<Ng> {
  // Served by e2e/build.mjs, so the browser gets the same artifact the Node tests do.
  const response = await fetch('/ng-wasm.wasm');
  const bytes = await response.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return new Ng(instance.exports as never);
}

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

const ng = await loadNg();
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

window.ng = {
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

