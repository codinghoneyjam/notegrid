// <META - FILE SUMMARY - The local error log: what T06.R03 shows and what it writes, with nothing leaving the machine.>

import type { LogEntry } from '@ng/persistence';

/**
 * Where a log entry can be written.
 *
 * `ProjectStore` already implements this for storage-backed errors; the app uses the same
 * seam for the errors that never reach the store (a compile failure, a dead transport), so
 * the log has exactly one reader and one retention rule no matter who wrote the entry.
 */
export interface LogSink {
  log(code: string, message: string): Promise<void>;
  recentErrors(limit?: number): Promise<LogEntry[]>;
}

export interface Clock {
  now(): number;
}

/** T04: the log keeps the newest 200 entries; the on-screen ring follows the same number. */
export const LOCAL_LOG_SIZE = 200;

export type LogListener = (entries: readonly LogEntry[]) => void;

/**
 * The log the bottom pane reads.
 *
 * Every failure the editor can survive goes through here with its code, and every entry
 * also goes to the sink, so T06.R03 ("logged locally; nothing leaves the machine") is a
 * property of this class rather than of each call site that remembered to log.
 *
 * The in-memory ring is a cache of the durable log, not a second store: it is what the UI
 * reads synchronously, and `reload()` refills it from the sink so a reload does not start
 * with an empty panel.
 */
export class LocalLog {
  private readonly sink: LogSink;
  private readonly clock: Clock;
  private readonly size: number;
  private ring: LogEntry[] = [];
  private readonly listeners = new Set<LogListener>();
  private writing: Promise<void> = Promise.resolve();

  constructor(sink: LogSink, clock: Clock, size: number = LOCAL_LOG_SIZE) {
    this.sink = sink;
    this.clock = clock;
    this.size = size;
  }

  /** Newest first, which is the order the pane shows and the order `recentErrors` returns. */
  entries(): readonly LogEntry[] {
    return this.ring;
  }

  /**
   * Records an error.
   *
   * Returns once the entry is on screen, not once it is on disk: a failure must never make
   * the next keystroke wait for storage (T04.R02, which is the same rule one layer up).
   */
  record(code: string, message: string): void {
    const entry: LogEntry = { at: this.clock.now(), code, message };
    this.ring = [entry, ...this.ring].slice(0, this.size);
    this.writing = this.writing
      .then(() => this.sink.log(code, message))
      // A log that cannot be written is not a second failure to report: the entry is
      // already on screen, and the durable copy is a diagnostic, not the contract.
      .catch(() => undefined);
    this.emit();
  }

  /** Resolves when every queued write has settled; a test awaits this instead of sleeping. */
  async settled(): Promise<void> {
    await this.writing;
  }

  /** Refills the ring from the durable log, newest first. */
  async reload(limit: number = this.size): Promise<readonly LogEntry[]> {
    try {
      this.ring = (await this.sink.recentErrors(limit)).slice(0, this.size);
    } catch {
      // Storage that is gone leaves the panel showing what it already had.
    }
    this.emit();
    return this.entries();
  }

  onChange(listener: LogListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private emit(): void {
    for (const listener of [...this.listeners]) {
      listener(this.entries());
    }
  }
}