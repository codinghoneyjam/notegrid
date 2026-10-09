// <META - FILE SUMMARY - Injectable clock and a trailing-edge debouncer, so timing policy is testable without real timers.>

/** Timer surface the sync policy needs. Injected everywhere so tests can drive time by hand. */
export interface Clock {
  setTimeout(fn: () => void, ms: number): number;
  clearTimeout(id: number): void;
  now(): number;
}

export const systemClock: Clock = {
  setTimeout(fn: () => void, ms: number): number {
    return globalThis.setTimeout(fn, ms) as unknown as number;
  },
  clearTimeout(id: number): void {
    globalThis.clearTimeout(id);
  },
  now(): number {
    return globalThis.performance.now();
  },
};

/**
 * Trailing-edge debounce: every `schedule()` restarts the delay, the callback runs
 * `delayMs` after the last call. That is what turns a drag into one compile instead
 * of one compile per mouse-move (T03 SYNC, 50 ms).
 */
export class Debouncer {
  private readonly delayMs: number;
  private readonly clock: Clock;
  private timer: number | null = null;
  private pendingFn: (() => void) | null = null;

  constructor(delayMs: number, clock: Clock) {
    this.delayMs = delayMs;
    this.clock = clock;
  }

  schedule(fn: () => void): void {
    this.pendingFn = fn;
    if (this.timer !== null) {
      this.clock.clearTimeout(this.timer);
    }
    this.timer = this.clock.setTimeout(() => {
      this.timer = null;
      const run = this.pendingFn;
      this.pendingFn = null;
      if (run) run();
    }, this.delayMs);
  }

  /** Runs a pending callback right now. Used by tests and by edits that cannot wait. */
  flush(): void {
    if (this.timer !== null) {
      this.clock.clearTimeout(this.timer);
      this.timer = null;
    }
    const run = this.pendingFn;
    this.pendingFn = null;
    if (run) run();
  }

  cancel(): void {
    if (this.timer !== null) {
      this.clock.clearTimeout(this.timer);
      this.timer = null;
    }
    this.pendingFn = null;
  }

  get pending(): boolean {
    return this.pendingFn !== null;
  }
}