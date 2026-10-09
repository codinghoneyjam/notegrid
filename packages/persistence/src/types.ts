// <META - FILE SUMMARY - Storage and codec contracts for project persistence (T04).>

import type { Project } from '@ng/document';

export type StoreName = 'project' | 'autosave' | 'log';

export const STORE_NAMES: readonly StoreName[] = ['project', 'autosave', 'log'];

export interface StoredRecord {
  key: string;
  /** Human label for the project list. */
  name?: string;
  /** Epoch millis of the last write; drives recovery ordering. */
  updated: number;
  bytes: Uint8Array;
}

/**
 * The storage surface the store needs, and nothing more.
 *
 * IndexedDB is the browser implementation; the in-memory one is what Node tests and
 * browsers without IndexedDB get. Keeping this narrow is what lets T04.R03 ("everything
 * works offline") be a property of the store rather than of the browser.
 */
export interface StorageBackend {
  get(store: StoreName, key: string): Promise<StoredRecord | null>;
  put(store: StoreName, record: StoredRecord): Promise<void>;
  delete(store: StoreName, key: string): Promise<void>;
  list(store: StoreName): Promise<StoredRecord[]>;
  /** `navigator.storage.persist()`; false when the browser declines. */
  requestPersistent(): Promise<boolean>;
}

/** T04: every storage problem is E_STORAGE, and every one of them offers the file path. */
export class StorageError extends Error {
  code: string;
  quotaExceeded: boolean;

  constructor(code: string, message: string, quotaExceeded: boolean) {
    super(message);
    this.code = code;
    this.quotaExceeded = quotaExceeded;
  }
}

export interface Issue {
  code: string;
  message: string;
  path: string;
}

export interface Report {
  errors: Issue[];
  warnings: Issue[];
}

/** The Rust side owns format rules (T04 NOT_OWNS); this is only the door to it. */
export interface ScoreCodec {
  migrate(json: string): string;
  canonical(json: string): string;
  validate(json: string): Report;
}

export type SaveResult =
  | { ok: true; bytes: Uint8Array; updated: number }
  | { ok: false; code: string; message: string; quotaExceeded: boolean; offerSaveToFile: true };

export type OpenResult =
  | { ok: true; doc: Project; warnings: Issue[]; canonical: string }
  | { ok: false; errors: Issue[] };

export interface LogEntry {
  at: number;
  code: string;
  message: string;
}

export interface RecoveryCandidate {
  autosavedAt: number;
  savedAt: number | null;
  bytes: Uint8Array;
}

/** T04.R02: autosave failures are reported, never thrown at the editor. */
export interface AutosaveOutcome {
  ok: boolean;
  code?: string;
  message?: string;
  offerSaveToFile: boolean;
}

export const PROJECT_FILE_EXTENSION = '.ngproj.json';

/** T04: the error log keeps the last 200 entries. */
export const LOG_SIZE = 200;

/** T04: autosave coalesces edits for 5 s, and also fires when the tab hides. */
export const AUTOSAVE_DEBOUNCE_MS = 5000;