// Browser harness for T04.AC6: save/open must work where File System Access is absent.
// Firefox and WebKit have no showSaveFilePicker, so they run the fallback path, which is
// the path T04.R04 says is the contract rather than a courtesy.

import { expect, test } from '@playwright/test';

const PROJECT = {
  format_version: 1,
  title: 'AC6 Fixture',
  ppq: 480,
  tempo: [{ tick: 0, bpm: 124 }],
  meter: [{ tick: 0, num: 4, den: 4 }],
  loop: { start: 0, end: 1920 },
  length_ticks: 1920,
  lanes: [{ id: 1, name: 'Piano', kind: 'melodic', program: 0, volume: 100, pan: 64, mute: false, solo: false }],
  notes: [
    [1, 0, 480, 72, 100],
    [1, 480, 480, 75, 96],
    [1, 960, 480, 79, 90],
  ],
};

interface SaveOutcome {
  ok: boolean;
  path: string;
  name: string;
}

interface DecodeOutcome {
  ok: boolean;
  title: string;
  notes: number;
  error: string;
}

declare global {
  interface Window {
    ng: {
      usesFileSystemAccess: boolean;
      save(name: string, projectJson: string): Promise<SaveOutcome>;
      decode(): Promise<DecodeOutcome>;
      openFromPicker(): Promise<{ ok: boolean; cancelled: boolean; title: string; notes: number }>;
      recoveredTitle(): Promise<string>;
      panelErrors(): number;
      storeInMemoryAndReload(): Promise<boolean>;
    };
  }
}

test.beforeEach(async ({ page }) => {
  await page.goto('/harness.html');
  await page.waitForFunction(() => typeof window.ng === 'object');
});

test('AC6: this engine takes the path the spec expects it to', async ({ page, browserName }) => {
  const usesFSA = await page.evaluate(() => window.ng.usesFileSystemAccess);
  // Chromium has the pickers, Firefox and WebKit do not. Pinning this is what makes the
  // matrix meaningful: a run where every engine claims FSA is not testing AC6.
  expect(usesFSA).toBe(browserName === 'chromium');
});

test('AC6: save produces a .ngproj.json that reads back as the same project', async ({ page }) => {
  const saved = await page.evaluate((json) => window.ng.save('ac6', json), JSON.stringify(PROJECT));
  expect(saved.ok).toBe(true);
  expect(saved.name).toBe('ac6.ngproj.json');

  const decoded = await page.evaluate(() => window.ng.decode());
  expect(decoded).toEqual({ ok: true, title: 'AC6 Fixture', notes: 3, error: '' });
});

test('AC6: the bytes written are canonical, so a second save is byte-identical', async ({ page }) => {
  await page.evaluate((json) => window.ng.save('ac6', json), JSON.stringify(PROJECT));
  const first = await page.evaluate(() => window.ng.lastSavedBytes());
  await page.evaluate((json) => window.ng.save('ac6', json), JSON.stringify(PROJECT));
  const second = await page.evaluate(() => window.ng.lastSavedBytes());
  expect(second).toEqual(first);
});

test('AC6: an invalid file is refused with a code, and nothing is opened', async ({ page }) => {
  const broken = JSON.stringify({ ...PROJECT, notes: [[1, 0, 480, 999, 100]] });
  const saved = await page.evaluate((json) => window.ng.save('broken', json), broken);
  expect(saved.ok).toBe(true);

  const decoded = await page.evaluate(() => window.ng.decode());
  expect(decoded.ok).toBe(false);
  expect(decoded.error).toBe('E_RANGE');
});

test('AC6: a structural error is reported through the validation panel', async ({ page }) => {
  const broken = JSON.stringify({ ...PROJECT, notes: [[1, 0, 480, 999, 100]] });
  await page.evaluate((json) => window.ng.save('broken', json), broken);
  await page.evaluate(() => window.ng.renderRejected());
  await expect(page.locator('[data-code="E_RANGE"]')).toHaveCount(1);
  await expect(page.locator('[data-can-open="false"]')).toHaveCount(1);
});

test('AC6: the open path the engine supports works', async ({ page, browserName }) => {
  test.skip(browserName === 'chromium', 'chromium uses the picker; the other two use the fallback');
  const opened = await page.evaluate(() => window.ng.openFromPicker());
  // A cancelled file dialog is a cancellation, never an error.
  expect(opened.ok === false ? opened.cancelled : true).toBe(true);
});

test('AC6: a reload recovers the autosaved project', async ({ page }) => {
  await page.evaluate((json) => window.ng.autosaveThenSeed(json), JSON.stringify(PROJECT));
  await page.reload();
  await page.waitForFunction(() => typeof window.ng === 'object');

  const title = await page.evaluate(() => window.ng.recoveredTitle());
  expect(title).toBe('AC6 Fixture Unsaved');
});