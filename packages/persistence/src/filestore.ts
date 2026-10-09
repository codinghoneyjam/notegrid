// <META - FILE SUMMARY - File save/open with File System Access where available and an input+download fallback everywhere else.>

import { PROJECT_FILE_EXTENSION } from './types.ts';

/**
 * The browser capabilities the file store needs, named as operations.
 *
 * Splitting them out is the whole point: Firefox and WebKit have no
 * `showSaveFilePicker`, so the fallback path is not a degraded mode of the same code,
 * it is a different branch that both browsers exercise on every save. If the decision
 * lived inline, the path AC6 tests would be the path nobody runs.
 */
export interface FilePlatform {
  hasFileSystemAccess(): boolean;
  /** Returns the written name, or null if the user cancelled. */
  showSavePicker(suggestedName: string, bytes: Uint8Array): Promise<string | null>;
  showOpenPicker(): Promise<Uint8Array | null>;
  /** The `<input type="file">` path: null means the user cancelled. */
  pickFile(): Promise<Uint8Array | null>;
  /** The `<a download>` path. */
  download(suggestedName: string, bytes: Uint8Array): void;
}

export type FileSaveOutcome =
  | { ok: true; path: 'file-system-access' | 'download'; name: string }
  | { ok: false; reason: 'cancelled' | 'unsupported' | 'error'; message?: string };

export type FileOpenOutcome =
  | { ok: true; bytes: Uint8Array; name: string | null; path: 'file-system-access' | 'input-file' }
  | { ok: false; reason: 'cancelled' | 'error'; message?: string };

export interface FileStoreOptions {
  platform: FilePlatform;
  extension?: string;
}

/** T04.R04: the fallback is the contract, not a courtesy. */
export class FileStore {
  private readonly platform: FilePlatform;
  private readonly extension: string;

  constructor(options: FileStoreOptions) {
    this.platform = options.platform;
    this.extension = options.extension ?? PROJECT_FILE_EXTENSION;
  }

  get usesFileSystemAccess(): boolean {
    return this.platform.hasFileSystemAccess();
  }

  async save(suggestedName: string, bytes: Uint8Array): Promise<FileSaveOutcome> {
    const name = withExtension(suggestedName, this.extension);
    if (this.platform.hasFileSystemAccess()) {
      try {
        const written = await this.platform.showSavePicker(name, bytes);
        if (written === null) return { ok: false, reason: 'cancelled' };
        return { ok: true, path: 'file-system-access', name: written };
      } catch (cause) {
        return { ok: false, reason: 'error', message: messageOf(cause) };
      }
    }
    try {
      this.platform.download(name, bytes);
      return { ok: true, path: 'download', name };
    } catch (cause) {
      return { ok: false, reason: 'error', message: messageOf(cause) };
    }
  }

  async open(): Promise<FileOpenOutcome> {
    if (this.platform.hasFileSystemAccess()) {
      try {
        const bytes = await this.platform.showOpenPicker();
        if (bytes === null) return { ok: false, reason: 'cancelled' };
        return { ok: true, bytes, name: null, path: 'file-system-access' };
      } catch (cause) {
        return { ok: false, reason: 'error', message: messageOf(cause) };
      }
    }
    try {
      const bytes = await this.platform.pickFile();
      if (bytes === null) return { ok: false, reason: 'cancelled' };
      return { ok: true, bytes, name: null, path: 'input-file' };
    } catch (cause) {
      return { ok: false, reason: 'error', message: messageOf(cause) };
    }
  }
}

export function withExtension(name: string, extension: string): string {
  return name.endsWith(extension) ? name : `${name}${extension}`;
}

function messageOf(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

// --- the real browser ---------------------------------------------------------------------------

interface FileSystemWritable {
  write(data: BufferSource): Promise<void>;
  close(): Promise<void>;
}

interface FileSystemFileHandle {
  name: string;
  createWritable(): Promise<FileSystemWritable>;
}

/**
 * `Uint8Array<ArrayBufferLike>` is not assignable to `BufferSource` under TS 5.7+.
 *
 * The wasm memory the bytes eventually live in may be shared, so the element type is
 * genuinely `ArrayBufferLike`. Copying into a plain `ArrayBuffer` here is the honest fix:
 * a Blob has to own its data anyway.
 */
function asBufferSource(bytes: Uint8Array): Uint8Array<ArrayBuffer> {
  const out = new Uint8Array(new ArrayBuffer(bytes.byteLength));
  out.set(bytes);
  return out;
}

interface WindowWithPicker {
  showSaveFilePicker?: (options: {
    suggestedName: string;
  }) => Promise<FileSystemFileHandle>;
  showOpenFilePicker?: (options: { types?: unknown[] }) => Promise<FileSystemFileHandle[]>;
}

/**
 * Builds the real platform from whatever the browser actually has.
 *
 * Every capability is probed rather than assumed, because the three engines differ:
 * Chromium has both pickers, Firefox and WebKit have neither, and a user can also
 * decline a permission inside a browser that has them.
 */
export function browserPlatform(
  win: WindowWithPicker = globalThis as unknown as WindowWithPicker,
  doc: Document = (globalThis as { document: Document }).document,
): FilePlatform {
  return {
    hasFileSystemAccess: () => typeof win.showSaveFilePicker === 'function',

    showSavePicker: async (suggestedName, bytes) => {
      const handle = await win.showSaveFilePicker?.({ suggestedName });
      if (!handle) return null;
      const writable = await handle.createWritable();
      await writable.write(asBufferSource(bytes));
      await writable.close();
      return handle.name;
    },

    showOpenPicker: async () => {
      const handles = await win.showOpenFilePicker?.({ types: [] });
      const handle = handles?.[0];
      if (!handle) return null;
      const file = await (handle as FileSystemFileHandle & {
        getFile(): Promise<File>;
      }).getFile();
      return new Uint8Array(await file.arrayBuffer());
    },

    pickFile: () => pickViaInput(doc),

    download: (suggestedName, bytes) => {
      const blob = new Blob([asBufferSource(bytes)], {
        type: 'application/json',
      });
      const url = URL.createObjectURL(blob);
      const anchor = doc.createElement('a');
      anchor.href = url;
      anchor.download = suggestedName;
      anchor.rel = 'noopener';
      doc.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      // Revoking immediately can cancel the download in WebKit; one turn is enough.
      setTimeout(() => URL.revokeObjectURL(url), 0);
    },
  };
}

function pickViaInput(doc: Document): Promise<Uint8Array | null> {
  return new Promise((resolve, reject) => {
    const input = doc.createElement('input');
    input.type = 'file';
    input.accept = '.json,application/json';
    let settled = false;

    const cleanup = (): void => {
      window.removeEventListener('focus', onFocus);
      input.remove();
    };

    const onFocus = (): void => {
      // A cancelled file dialog fires no event at all, so focus returning empty is
      // the only cancellation signal the fallback path gets.
      setTimeout(() => {
        if (settled) return;
        if (input.files && input.files.length > 0) {
          settled = true;
          cleanup();
          void input.files[0]
            .arrayBuffer()
            .then((buffer) => resolve(new Uint8Array(buffer)))
            .catch(reject);
        } else {
          settled = true;
          cleanup();
          resolve(null);
        }
      }, 300);
    };

    input.addEventListener('change', () => {
      if (settled) return;
      const file = input.files?.[0];
      if (!file) {
        settled = true;
        cleanup();
        resolve(null);
        return;
      }
      settled = true;
      cleanup();
      void file
        .arrayBuffer()
        .then((buffer) => resolve(new Uint8Array(buffer)))
        .catch(reject);
    });

    window.addEventListener('focus', onFocus, { once: true });
    doc.body.appendChild(input);
    input.click();
  });
}