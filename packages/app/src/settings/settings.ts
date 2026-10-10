// <META - FILE SUMMARY - Settings: export sample rate, metronome, theme, export destination, with a tolerant parser.>

/** C11 browsers report the display theme; `system` follows it. */
export type Theme = 'system' | 'light' | 'dark';

/**
 * Where an export lands.
 *
 * `directory` is only reachable where File System Access exists, so the choice is stored but
 * never assumed: the picker decides whether the destination is usable at the moment of the
 * export, and the app falls back to a download when it is not (T06.AC5).
 */
export type ExportDestination = 'download' | 'directory';

export interface Settings {
  /** C5 sample rate for the offline render. Only the two rates the engine tests exist for. */
  exportSampleRate: number;
  metronome: boolean;
  theme: Theme;
  exportDestination: ExportDestination;
}

export const SUPPORTED_SAMPLE_RATES: readonly number[] = [44100, 48000];

export const DEFAULT_SETTINGS: Settings = {
  exportSampleRate: 44100,
  metronome: false,
  theme: 'system',
  exportDestination: 'download',
};

/** Persisted by preference, never by handle: the directory handle does not survive a reload. */
export const SETTINGS_STORAGE_KEY = 'ng:settings';

/**
 * Reads settings out of whatever was in storage.
 *
 * Every field falls back independently, because the failure mode this guards against is a
 * settings blob written by an older build: dropping the whole object would reset a
 * user's metronome preference over an unrecognised theme value.
 */
export function parseSettings(json: string | null | undefined): Settings {
  if (typeof json !== 'string' || json.length === 0) {
    return { ...DEFAULT_SETTINGS };
  }
  let raw: unknown;
  try {
    raw = JSON.parse(json);
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
  if (typeof raw !== 'object' || raw === null) {
    return { ...DEFAULT_SETTINGS };
  }
  const input = raw as Record<string, unknown>;
  const rate = input.exportSampleRate;
  const theme = input.theme;
  const destination = input.exportDestination;
  return {
    exportSampleRate:
      typeof rate === 'number' && SUPPORTED_SAMPLE_RATES.includes(rate)
        ? rate
        : DEFAULT_SETTINGS.exportSampleRate,
    metronome: typeof input.metronome === 'boolean' ? input.metronome : DEFAULT_SETTINGS.metronome,
    theme: theme === 'light' || theme === 'dark' ? theme : DEFAULT_SETTINGS.theme,
    exportDestination:
      destination === 'directory' ? 'directory' : DEFAULT_SETTINGS.exportDestination,
  };
}

export function serializeSettings(settings: Settings): string {
  return JSON.stringify({
    exportSampleRate: settings.exportSampleRate,
    metronome: settings.metronome,
    theme: settings.theme,
    exportDestination: settings.exportDestination,
  });
}

/** The one setting the app refuses rather than coerces: a rate the engine has no vector for. */
export function isSupportedSampleRate(rate: number): boolean {
  return SUPPORTED_SAMPLE_RATES.includes(rate);
}

export interface SettingsStorage {
  read(key: string): string | null;
  write(key: string, value: string): void;
}

/**
 * Settings over an injected storage seam.
 *
 * `localStorage` is a browser global with a quota, a privacy mode and a corrupt-value
 * case; all three are reasons the app should not touch it directly. Passing the two
 * functions in also means the settings path is testable in Node, where the same seam is a
 * plain map.
 */
export class SettingsStore {
  private readonly storage: SettingsStorage;
  private current: Settings;

  constructor(storage: SettingsStorage, initial?: Settings) {
    this.storage = storage;
    this.current = initial ?? parseSettings(storage.read(SETTINGS_STORAGE_KEY));
  }

  read(): Settings {
    return { ...this.current };
  }

  /** Merges a patch, ignoring fields the caller left `undefined`, and persists the result. */
  update(patch: Partial<Settings>): Settings {
    const next: Settings = { ...this.current };
    if (patch.exportSampleRate !== undefined) {
      if (!isSupportedSampleRate(patch.exportSampleRate)) {
        throw new RangeError(`export sample rate ${patch.exportSampleRate} is not supported`);
      }
      next.exportSampleRate = patch.exportSampleRate;
    }
    if (patch.metronome !== undefined) next.metronome = patch.metronome;
    if (patch.theme !== undefined) next.theme = patch.theme;
    if (patch.exportDestination !== undefined) next.exportDestination = patch.exportDestination;
    this.current = next;
    this.storage.write(SETTINGS_STORAGE_KEY, serializeSettings(next));
    return this.read();
  }

  reset(): Settings {
    this.current = { ...DEFAULT_SETTINGS };
    this.storage.write(SETTINGS_STORAGE_KEY, serializeSettings(this.current));
    return this.read();
  }
}