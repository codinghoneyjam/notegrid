// <META - FILE SUMMARY - DOM renderer for the validation panel; all decisions live in view.ts.>

import { groupByPath, type ValidationView } from './view.ts';

/**
 * The minimum DOM surface the panel needs, so a fake element tree can drive it.
 *
 * `setAttribute` rather than a plain `attributes` record on purpose: a real element's
 * `attributes` is a live NamedNodeMap, and assigning into it as if it were an object
 * silently does nothing. The Node tests pass with the record, the browsers do not, and
 * the browser is the one that ships.
 */
export interface PanelElement {
  tagName: string;
  textContent: string;
  className: string;
  setAttribute(name: string, value: string): void;
  appendChild(child: PanelElement): void;
  remove(): void;
}

export interface PanelHost {
  createElement(tag: string): PanelElement;
}

/**
 * Paints a `ValidationView` into `host`.
 *
 * `data-code` and `data-path` go on every row because that is what a test -- and a
 * screen reader navigating a 200-row error list -- actually needs, and because the
 * Playwright spec for T04.AC6 reads them back off the DOM.
 */
export function renderValidationPanel(host: PanelHost, view: ValidationView): PanelElement {
  const root = element(host, 'section', 'ng-validation');
  root.setAttribute('data-errors', String(view.errorCount));
  root.setAttribute('data-warnings', String(view.warningCount));
  root.setAttribute('data-can-open', String(view.canOpen));

  const heading = element(host, 'h2', 'ng-validation__title');
  heading.textContent = view.title;
  root.appendChild(heading);

  for (const group of groupByPath(view.rows)) {
    const section = element(host, 'section', `ng-validation__group ng-validation__group--${severityOf(group.rows)}`);
    section.setAttribute('data-path', group.path);
    const pathLabel = element(host, 'h3', 'ng-validation__path');
    pathLabel.textContent = group.path;
    section.appendChild(pathLabel);

    for (const row of group.rows) {
      const item = element(host, 'li', `ng-validation__row ng-validation__row--${row.severity}`);
      item.setAttribute('data-code', row.code);
      item.setAttribute('data-path', row.path);
      item.setAttribute('data-severity', row.severity);
      item.textContent = `${row.code} ${row.path} ${row.message}`;
      section.appendChild(item);
    }
    root.appendChild(section);
  }

  if (view.canSaveToFile) {
    const offer = element(host, 'button', 'ng-validation__save-to-file');
    offer.setAttribute('type', 'button');
    offer.setAttribute('data-action', 'save-to-file');
    offer.textContent = 'Save to file';
    root.appendChild(offer);
  }

  return root;
}

/** Replaces the panel's contents in one step, so a repaint cannot leave two lists on screen. */
export function replacePanel(host: PanelHost, mount: PanelElement, view: ValidationView): PanelElement {
  mount.remove();
  return renderValidationPanel(host, view);
}

function severityOf(rows: Array<{ severity: string }>): string {
  return rows.some((r) => r.severity === 'error') ? 'error' : 'warning';
}

function element(host: PanelHost, tag: string, className: string): PanelElement {
  const node = host.createElement(tag);
  node.className = className;
  return node;
}