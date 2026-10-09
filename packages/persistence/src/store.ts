// <META - FILE SUMMARY - ProjectStore: save, open, autosave and recovery on top of a StorageBackend.>

import type { Project } from '@ng/document';
import { decodeProject, encodeProject } from './codec.ts';
import { StorageError } from './types.ts';
import {
  AUTOSAVE_DEBOUNCE_MS,
  LOG_SIZE,
  type AutosaveOutcome,
  type LogEntry,
  type OpenResult,
  type RecoveryCandidate,
  type SaveResult,
  type ScoreCodec,
  type StorageBackend,
} from './types.ts';

/** Timer surface, injected so the 5 s autosave debounce is testable without waiting. */
export interface Clock {
  setTimeout(fn: () => void, ms: number): number;
  clearTimeout(id: number): void;
  now(): number;
}

/** T04: the autosave ring holds five slots so a corrupted newest write is not fatal. */
export const AUTOSAVE_SLOTS = 5;

export const AUTOSAVE_PREFIX = 'auto-';

export interface ProjectStoreOptions {
  backend: StorageBackend;
  codec: ScoreCodec;
  clock: Clock;
  autosaveMs?: number;
  autosaveSlots?: number;
  logSize?: number;
}

/**
 * The single door between the editor and everything that survives a reload.
 *
 * Two rules shape it. A failed save is a result, not an exception, and every storage
 * failure carries `offerSaveToFile` because T04.R06 says the file is always offered.
 * And nothing here throws at the editing path: T04.R02 requires an autosave failure to
 * inform the user without blocking the next keystroke.
 */
export class ProjectStore {
  private readonly backend: StorageBackend;
  private readonly codec: ScoreCodec;
  private readonly clock: Clock;
  private readonly autosaveMs: number;
  private readonly autosaveSlots: number;
  private readonly logSize: number;

  private autosaveTimer: number | null = null;
  private autosaveSlot = 0;
  private pendingAutosave: Project | null = null;
  private autosaveInFlight: Promise<AutosaveOutcome> | null = null;

  constructor(options: ProjectStoreOptions) {
    this.backend = options.backend;
    this.codec = options.codec;
    this.clock = options.clock;
    this.autosaveMs = options.autosaveMs ?? AUTOSAVE_DEBOUNCE_MS;
    this.autosaveSlots = options.autosaveSlots ?? AUTOSAVE_SLOTS;
    this.logSize = options.logSize ?? LOG_SIZE;
  }

  // --- explicit save ------------------------------------------------------------------------

  /** Canonicalises and stores. The bytes written are exactly what `ng_project_canonical` produced. */
  async save(id: string, doc: Project): Promise<SaveResult> {
    let bytes: Uint8Array;
    try {
      bytes = this.encode(doc);
    } catch (cause) {
      return {
        ok: false,
        code: codeOf(cause),
        message: messageOf(cause),
        quotaExceeded: false,
        offerSaveToFile: true,
      };
    }
    const updated = this.clock.now();
    try {
      await this.backend.put('project', { key: id, name: doc.title, updated, bytes });
    } catch (cause) {
      await this.log(codeOf(cause), messageOf(cause));
      return {
        ok: false,
        code: codeOf(cause),
        message: messageOf(cause),
        quotaExceeded: cause instanceof StorageError && cause.quotaExceeded,
        offerSaveToFile: true,
      };
    }
    return { ok: true, bytes, updated };
  }

  // --- open ---------------------------------------------------------------------------------

  async open(id: string): Promise<OpenResult> {
    let record;
    try {
      record = await this.backend.get('project', id);
    } catch (cause) {
      await this.log(codeOf(cause), messageOf(cause));
      return { ok: false, errors: [issue(codeOf(cause), '/', messageOf(cause))] };
    }
    if (record === null) {
      return { ok: false, errors: [issue('E_NOT_FOUND', '/', `no saved project '${id}'`)] };
    }
    return this.openBytes(record.bytes);
  }

  /**
   * bytes -> validate -> migrate -> canonical -> document.
   *
   * T04 writes the pipeline as migrate then validate, but the order matters and the
   * spec's order loses information: migrate deserialises into the typed model, so any
   * value the type cannot hold comes back as "model parse failed" with no path, and a
   * structurally invalid file never reaches the validator that could say which field.
   * Validating first keeps the errors specific. Errors refuse the file and leave the
   * caller's document alone; warnings open it, because a warning is information the
   * editor shows after the fact (T04 DESIGN).
   */
  openBytes(bytes: Uint8Array): OpenResult {
    const text = new TextDecoder().decode(bytes);

    // Validate what the user handed us, not what migration produced.
    //
    // `ng_project_migrate` deserialises into the typed model, so a file with a pitch of
    // 999 fails there as "model parse failed" and never reaches the validator. Running
    // validation first is what turns that into the actionable "pitch 0..=127 at
    // /notes/0/3" that the panel can show.
    let report;
    try {
      report = this.codec.validate(text);
    } catch (cause) {
      return { ok: false, errors: [issue(codeOf(cause), '/', messageOf(cause))] };
    }

    if (report.errors.length > 0) {
      return { ok: false, errors: report.errors };
    }

    // A file can be schema-valid and still need migrating: that is the whole point of
    // format_version. Only now is it safe to let the typed path see it.
    let migrated: string;
    try {
      migrated = this.codec.migrate(text);
    } catch (cause) {
      return { ok: false, errors: [issue(codeOf(cause), '/', messageOf(cause))] };
    }

    let canonical: string;
    try {
      canonical = this.codec.canonical(migrated);
    } catch (cause) {
      return { ok: false, errors: [issue(codeOf(cause), '/', messageOf(cause))] };
    }

    return { ok: true, doc: decodeProject(canonical), warnings: report.warnings, canonical };
  }

  // --- autosave ----------------------------------------------------------------------------

  /**
   * Coalesces edits for the debounce window. T04.R02: the result is reported, and a
   * caller that keeps editing is never blocked by a failed write.
   */
  autosave(doc: Project): void {
    this.pendingAutosave = doc;
    if (this.autosaveTimer !== null) {
      this.clock.clearTimeout(this.autosaveTimer);
    }
    this.autosaveTimer = this.clock.setTimeout(() => {
      this.autosaveTimer = null;
      void this.writeAutosave();
    }, this.autosaveMs);
  }

  /**
   * T04: the tab hiding flushes immediately rather than waiting out the window.
   *
   * A no-op flush still waits for the write already in flight, so a caller that flushes
   * on hide can trust that everything up to the hide is on disk when it resolves.
   */
  async flushAutosave(): Promise<AutosaveOutcome> {
    if (this.autosaveTimer !== null) {
      this.clock.clearTimeout(this.autosaveTimer);
      this.autosaveTimer = null;
    }
    const inFlight = this.autosaveInFlight;
    const outcome = await this.writeAutosave();
    if (outcome.ok && inFlight) {
      // Nothing new to write, so the answer the caller needs is how the write already
      // running went. Reporting "ok" here would hide a quota failure from the hide handler.
      const settled = await inFlight;
      return settled.ok ? { ok: true, offerSaveToFile: false } : settled;
    }
    return outcome;
  }

  /**
   * Writes one autosave, coalescing onto the write already in flight.
   *
   * The pending document is claimed *before* awaiting the previous write. Reading it
   * after the await loses it: the in-flight write nulls the slot as it starts, so a
   * document that arrived during the write would be dropped and never autosaved at all.
   */
  private async writeAutosave(): Promise<AutosaveOutcome> {
    const doc = this.pendingAutosave;
    if (doc === null) {
      return { ok: true, offerSaveToFile: false };
    }
    this.pendingAutosave = null;

    // Claim the slot synchronously. If it were claimed after the first await, several
    // writes kicked off in the same tick would all read the same slot and overwrite each
    // other, and the ring would stay at one entry.
    const key = `${AUTOSAVE_PREFIX}${this.autosaveSlot}`;
    this.autosaveSlot = (this.autosaveSlot + 1) % this.autosaveSlots;

    const previous = this.autosaveInFlight;
    if (previous) {
      await previous;
    }

    const write = (async (): Promise<AutosaveOutcome> => {
      try {
        const bytes = this.encode(doc);
        await this.backend.put('autosave', { key, updated: this.clock.now(), bytes });
        return { ok: true, offerSaveToFile: false };
      } catch (cause) {
        await this.log(codeOf(cause), messageOf(cause));
        return {
          ok: false,
          code: codeOf(cause),
          message: messageOf(cause),
          offerSaveToFile: true,
        };
      }
    })();

    this.autosaveInFlight = write;
    try {
      return await write;
    } finally {
      this.autosaveInFlight = null;
    }
  }

  // --- recovery -----------------------------------------------------------------------------

  /** The newest autosave, but only when it is newer than what was last saved (T04.AC3). */
  async recoveryCandidate(id: string): Promise<RecoveryCandidate | null> {
    let autosaves;
    let saved;
    try {
      autosaves = await this.backend.list('autosave');
      saved = await this.backend.get('project', id);
    } catch (cause) {
      await this.log(codeOf(cause), messageOf(cause));
      return null;
    }
    if (autosaves.length === 0) return null;
    const newest = autosaves[0];
    if (saved !== null && newest.updated <= saved.updated) return null;
    return {
      autosavedAt: newest.updated,
      savedAt: saved === null ? null : saved.updated,
      bytes: newest.bytes,
    };
  }

  /** Opens the candidate and promotes it to a real save, so recovery ends in a durable state. */
  async acceptRecovery(id: string, candidate: RecoveryCandidate): Promise<OpenResult> {
    const opened = this.openBytes(candidate.bytes);
    if (!opened.ok) return opened;
    await this.save(id, opened.doc);
    return opened;
  }

  // --- log ----------------------------------------------------------------------------------

  async log(code: string, message: string): Promise<void> {
    const entry: LogEntry = { at: this.clock.now(), code, message };
    let existing: Array<{ key: string; updated: number; bytes: Uint8Array }> = [];
    try {
      existing = await this.backend.list('log');
    } catch {
      return; // a broken log must never take the caller down with it
    }
    try {
      await this.backend.put('log', {
        key: `log-${entry.at}`,
        updated: entry.at,
        bytes: new TextEncoder().encode(JSON.stringify(entry)),
      });
    } catch {
      return;
    }
    // `list` is newest-first, so the tail is what falls off the end.
    const overflow = existing.length + 1 - this.logSize;
    if (overflow > 0) {
      for (const victim of existing.slice(this.logSize - 1)) {
        try {
          await this.backend.delete('log', victim.key);
        } catch {
          return;
        }
      }
    }
  }

  async recentErrors(limit = this.logSize): Promise<LogEntry[]> {
    let records;
    try {
      records = await this.backend.list('log');
    } catch {
      return [];
    }
    const out: LogEntry[] = [];
    for (const record of records.slice(0, limit)) {
      try {
        out.push(JSON.parse(new TextDecoder().decode(record.bytes)) as LogEntry);
      } catch {
        // A log entry we cannot read is dropped, not thrown: the log is a diagnostic.
      }
    }
    return out;
  }

  // --- storage health -----------------------------------------------------------------------

  /** T04: ask once at startup; Safari usually declines and the UI says so. */
  async requestPersistent(): Promise<boolean> {
    try {
      return await this.backend.requestPersistent();
    } catch {
      return false;
    }
  }

  private encode(doc: Project): Uint8Array {
    const canonical = this.codec.canonical(encodeProject(doc));
    return new TextEncoder().encode(canonical);
  }
}

function issue(code: string, path: string, message: string): { code: string; path: string; message: string } {
  return { code, path, message };
}

function codeOf(cause: unknown): string {
  if (typeof cause === 'object' && cause !== null && 'code' in cause) {
    const code = (cause as { code: unknown }).code;
    if (typeof code === 'string') return code;
  }
  return 'E_STORAGE';
}

function messageOf(cause: unknown): string {
  if (cause instanceof Error) return cause.message;
  if (typeof cause === 'object' && cause !== null && 'message' in cause) {
    const message = (cause as { message: unknown }).message;
    if (typeof message === 'string') return message;
  }
  return String(cause);
}