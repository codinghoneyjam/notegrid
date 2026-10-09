// <META - FILE SUMMARY - In-memory storage backend: the Node test double and the fallback for browsers without IndexedDB.>

import { StorageError, type StorageBackend, type StoreName, type StoredRecord } from './types.ts';

/**
 * A backend with the same shape as IndexedDB and none of its failure modes.
 *
 * It is not only a test double: T04.R03 says the app works offline, and a browser with
 * IndexedDB disabled still has to be able to save somewhere. In-memory is that somewhere
 * -- the file store is the durable answer, autosave is only temporary protection, and
 * T04 says so out loud.
 */
export class MemoryBackend implements StorageBackend {
  private readonly data = new Map<StoreName, Map<string, StoredRecord>>();
  private persistent = false;

  /** When set, the next `put` to this store fails with a quota error (T04.AC5). */
  quotaExhaustedOn: StoreName | null = null;

  /** When set, every call fails with E_STORAGE, for the "storage is gone entirely" path. */
  failing = false;

  /** When false, `requestPersistent` declines, which is Safari's usual answer. */
  persistGranted = false;

  constructor() {
    for (const store of ['project', 'autosave', 'log'] as StoreName[]) {
      this.data.set(store, new Map());
    }
  }

  async get(store: StoreName, key: string): Promise<StoredRecord | null> {
    this.guard();
    const found = this.data.get(store)?.get(key) ?? null;
    return found === null ? null : copy(found);
  }

  async put(store: StoreName, record: StoredRecord): Promise<void> {
    this.guard();
    if (this.quotaExhaustedOn === store) {
      throw new StorageError(
        'E_STORAGE',
        `quota exceeded writing ${record.key} to ${store}`,
        true,
      );
    }
    this.data.get(store)?.set(record.key, copy(record));
  }

  async delete(store: StoreName, key: string): Promise<void> {
    this.guard();
    this.data.get(store)?.delete(key);
  }

  async list(store: StoreName): Promise<StoredRecord[]> {
    this.guard();
    return [...(this.data.get(store)?.values() ?? [])]
      .sort((a, b) => b.updated - a.updated)
      .map(copy);
  }

  async requestPersistent(): Promise<boolean> {
    this.guard();
    this.persistent = this.persistGranted;
    return this.persistent;
  }

  /** Not part of the interface; lets a test look at what the store believes it wrote. */
  isPersistent(): boolean {
    return this.persistent;
  }

  private guard(): void {
    if (this.failing) {
      throw new StorageError('E_STORAGE', 'storage backend is unavailable', false);
    }
  }
}

function copy(record: StoredRecord): StoredRecord {
  return {
    key: record.key,
    name: record.name,
    updated: record.updated,
    bytes: record.bytes.slice(),
  };
}