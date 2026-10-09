// <META - FILE SUMMARY - Validation panel tests: the decisions in view.ts and the DOM shape panel.ts produces.>

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  buildValidationView,
  groupByPath,
  renderValidationPanel,
  viewForResult,
  type PanelElement,
  type PanelHost,
} from '../src/index.ts';

// --- a DOM small enough to read ----------------------------------------------------------------

class FakeElement implements PanelElement {
  tagName: string;
  textContent = '';
  className = '';
  children: FakeElement[] = [];
  private readonly attrs = new Map<string, string>();

  constructor(tagName: string) {
    this.tagName = tagName;
  }

  setAttribute(name: string, value: string): void {
    this.attrs.set(name, value);
  }

  attribute(name: string): string | null {
    return this.attrs.get(name) ?? null;
  }

  appendChild(child: PanelElement): void {
    this.children.push(child as FakeElement);
  }

  remove(): void {
    /* nothing to detach from in a tree that is rebuilt wholesale */
  }

  /** Depth-first walk, which is how the assertions read the rendered tree. */
  all(): FakeElement[] {
    return [this, ...this.children.flatMap((c) => c.all())];
  }

  byClass(fragment: string): FakeElement[] {
    return this.all().filter((e) => e.className.includes(fragment));
  }
}

class FakeHost implements PanelHost {
  createElement(tag: string): PanelElement {
    return new FakeElement(tag);
  }
}

function render(state: Parameters<typeof buildValidationView>[0], options = {}) {
  const host = new FakeHost();
  const view = buildValidationView(state, options);
  const root = renderValidationPanel(host, view) as FakeElement;
  return { view, root };
}

// --- T04.AC2 -------------------------------------------------------------------------------------

test('T04.AC2: errors are listed with their code and path, and the file is not openable', () => {
  const { view, root } = render({
    kind: 'rejected',
    errors: [
      { code: 'E_RANGE', message: 'pitch 0..=127', path: '/notes/1/3' },
      { code: 'E_RANGE', message: 'duration in [1,u32::MAX]', path: '/notes/2/2' },
      { code: 'E_LANE_MISSING', message: 'note references unknown lane id', path: '/notes/3/0' },
    ],
  });

  assert.equal(view.canOpen, false, 'an invalid file must not be openable');
  assert.equal(view.errorCount, 3);
  assert.match(view.title, /current project is unchanged/);

  const rows = root.byClass('ng-validation__row');
  assert.equal(rows.length, 3);
  assert.deepEqual(
    rows.map((r) => r.attribute('data-code')),
    ['E_RANGE', 'E_RANGE', 'E_LANE_MISSING'],
  );
  assert.deepEqual(
    rows.map((r) => r.attribute('data-path')),
    ['/notes/1/3', '/notes/2/2', '/notes/3/0'],
  );
  assert.ok(rows.every((r) => r.attribute('data-severity') === 'error'));
  assert.ok(rows[0].textContent.includes('pitch 0..=127'), 'the message is readable, not just coded');
  assert.equal(root.attribute('data-errors'), '3');
});

test('errors are grouped by path so a note list reads as one group', () => {
  const grouped = groupByPath([
    { severity: 'error', code: 'E_RANGE', path: '/notes/1/3', message: 'pitch' },
    { severity: 'error', code: 'E_RANGE', path: '/notes/2/3', message: 'pitch' },
    { severity: 'error', code: 'E_LANE_MISSING', path: '/notes/3/0', message: 'lane' },
  ]);
  assert.deepEqual(grouped.map((g) => g.path), ['/notes/1/3', '/notes/2/3', '/notes/3/0']);
  assert.equal(grouped[0].rows.length, 1);

  const { root } = render({
    kind: 'rejected',
    errors: [
      { code: 'E_RANGE', message: 'pitch', path: '/notes' },
      { code: 'E_RANGE', message: 'pitch', path: '/notes' },
    ],
  });
  const groups = root.byClass('ng-validation__group');
  assert.equal(groups.length, 1);
  assert.equal(groups[0].attribute('data-path'), '/notes');
  assert.equal(groups[0].byClass('ng-validation__row').length, 2);
});

test('a viewForResult bridge maps both store outcomes', () => {
  const refused = viewForResult({
    ok: false,
    errors: [{ code: 'E_RANGE', message: 'pitch', path: '/notes/0/3' }],
  });
  assert.equal(refused.canOpen, false);
  assert.equal(refused.errorCount, 1);

  const opened = viewForResult({ ok: true, warnings: [], canonical: '', doc: {} as never });
  assert.equal(opened.canOpen, true);
  assert.equal(opened.title, 'Opened, no problems');
});

// --- T04.AC5 -------------------------------------------------------------------------------------

test('T04.AC5: a storage problem always offers save-to-file, whatever else is on screen', () => {
  const { view, root } = render({
    kind: 'storage-error',
    code: 'E_STORAGE',
    message: 'quota exceeded writing p1 to project',
  });

  assert.equal(view.canSaveToFile, true, 'T04.R06');
  assert.equal(view.canOpen, false);

  const offer = root.byClass('ng-validation__save-to-file');
  assert.equal(offer.length, 1);
  assert.equal(offer[0].attribute('data-action'), 'save-to-file');
  assert.match(offer[0].textContent, /Save to file/);
});

test('the save-to-file offer is absent when storage is healthy', () => {
  const { root } = render({
    kind: 'opened',
    warnings: [{ code: 'W_UNUSED_LANE', message: 'lane without notes', path: '/lanes/1' }],
  });
  assert.equal(root.byClass('ng-validation__save-to-file').length, 0);
  assert.ok(!render({ kind: 'idle' }).view.canSaveToFile);
});

// --- warnings -------------------------------------------------------------------------------------

test('warnings are shown as warnings and the file still opens', () => {
  const { view, root } = render({
    kind: 'opened',
    warnings: [
      { code: 'W_UNUSED_LANE', message: 'lane without notes', path: '/lanes/1' },
      { code: 'W_NOTE_PAST_LENGTH', message: 'notes extend past declared length_ticks', path: '/length_ticks' },
    ],
  });

  assert.equal(view.canOpen, true, 'a warning must not block opening');
  assert.equal(view.warningCount, 2);
  assert.equal(view.errorCount, 0);

  const rows = root.byClass('ng-validation__row');
  assert.equal(rows.length, 2);
  assert.ok(rows.every((r) => r.attribute('data-severity') === 'warning'));
  assert.equal(root.attribute('data-warnings'), '2');
  assert.equal(root.attribute('data-errors'), '0');
});

// --- lifecycle ------------------------------------------------------------------------------------

test('idle and validating show no rows and offer nothing', () => {
  for (const state of [{ kind: 'idle' } as const, { kind: 'validating' } as const]) {
    const { view, root } = render(state);
    assert.equal(view.rows.length, 0);
    assert.equal(view.canOpen, false);
    assert.equal(root.byClass('ng-validation__row').length, 0);
    assert.ok(root.byClass('ng-validation__title')[0].textContent.length > 0, 'always says what it is doing');
  }
});

test('a repaint replaces the previous list instead of stacking on it', () => {
  const host = new FakeHost();
  const first = renderValidationPanel(host, buildValidationView({
    kind: 'rejected',
    errors: [{ code: 'E_RANGE', message: 'pitch', path: '/notes/0/3' }],
  })) as FakeElement;
  const second = renderValidationPanel(host, buildValidationView({ kind: 'opened', warnings: [] })) as FakeElement;

  assert.equal(first.byClass('ng-validation__row').length, 1);
  assert.equal(second.byClass('ng-validation__row').length, 0, 'the new tree has no stale rows');
  assert.notEqual(first, second);
});