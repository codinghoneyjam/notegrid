// <META - FILE SUMMARY - Audition policy: a note played while editing sounds on its own lane instrument, auto-released after a short hold.>

import type { Clock } from './debounce.ts';
import { systemClock } from './debounce.ts';
import type { SyncDocument, SyncLane } from './types.ts';

/** Numeric lane kinds as the compiled `LaneEntry.kind` defines them (ng-render `kind_of`). */
export const LANE_KIND_MELODIC = 0;
export const LANE_KIND_DRUM = 1;

/** T03 SYNC: audition sounds "~250 ms or while held". */
export const AUDITION_HOLD_MS = 250;

/** The one call the policy makes to make a sound without a schedule. */
export type AuditionSink = (kind: number, program: number, key: number, vel: number, on: boolean) => void;

export interface AuditionOptions {
  holdMs?: number;
  clock?: Clock;
}

interface Voice {
  lane: SyncLane;
  key: number;
  timer: number | null;
}

/**
 * One-shot audition voices for note entry and pitch edits.
 *
 * Each `(lane, key)` pair arms its own release timer, so dragging a pitch does not cut
 * the previous key off early and two lanes can audition at once. A caller that is still
 * holding the key calls `hold()` to re-arm instead of letting the timer fire.
 */
export class AuditionController {
  private readonly sink: AuditionSink;
  private readonly holdMs: number;
  private readonly clock: Clock;
  private readonly voices = new Map<string, Voice>();
  private document: SyncDocument | null = null;

  constructor(sink: AuditionSink, options: AuditionOptions = {}) {
    this.sink = sink;
    this.holdMs = options.holdMs ?? AUDITION_HOLD_MS;
    this.clock = options.clock ?? systemClock;
  }

  setDocument(doc: SyncDocument | null): void {
    this.document = doc;
  }

  /** Returns false when the lane is unknown, so the caller can report the edit instead of hearing nothing. */
  noteOn(laneId: number, key: number, velocity: number): boolean {
    const lane = this.document?.lanes.find((l) => l.id === laneId);
    if (!lane) return false;
    const id = voiceKey(laneId, key);
    const existing = this.voices.get(id);
    if (existing) {
      this.arm(existing);
      return true;
    }
    const voice: Voice = { lane, key, timer: null };
    this.voices.set(id, voice);
    this.send(lane, key, velocity, true);
    this.arm(voice);
    return true;
  }

  noteOff(laneId: number, key: number): void {
    const voice = this.voices.get(voiceKey(laneId, key));
    if (!voice) return;
    this.voices.delete(voiceKey(laneId, key));
    if (voice.timer !== null) {
      this.clock.clearTimeout(voice.timer);
    }
    this.send(voice.lane, key, 0, false);
  }

  /** Keeps a sounding audition alive while the caller still holds the key. */
  hold(laneId: number, key: number): void {
    const voice = this.voices.get(voiceKey(laneId, key));
    if (voice) this.arm(voice);
  }

  /** Silences every sounding voice, e.g. when a transport is replaced. */
  releaseAll(): void {
    for (const id of Array.from(this.voices.keys())) {
      const voice = this.voices.get(id);
      if (voice) this.noteOff(voice.lane.id, voice.key);
    }
  }

  get sounding(): number {
    return this.voices.size;
  }

  private arm(voice: Voice): void {
    if (voice.timer !== null) {
      this.clock.clearTimeout(voice.timer);
    }
    voice.timer = this.clock.setTimeout(() => {
      this.noteOff(voice.lane.id, voice.key);
    }, this.holdMs);
  }

  private send(lane: SyncLane, key: number, velocity: number, on: boolean): void {
    const kind = lane.kind === 'drum' ? LANE_KIND_DRUM : LANE_KIND_MELODIC;
    const program = lane.kind === 'drum' ? (lane.kit ?? 0) : (lane.program ?? 0);
    this.sink(kind, program, key, velocity, on);
  }
}

function voiceKey(laneId: number, key: number): string {
  return `${laneId}:${key}`;
}