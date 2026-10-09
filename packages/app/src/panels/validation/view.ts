// <META - FILE SUMMARY - Pure view model for the validation panel: what to show and which buttons to offer.>

import type { Issue, OpenResult } from '@ng/persistence';

export type ValidationSeverity = 'error' | 'warning';

export interface ValidationRow {
  severity: ValidationSeverity;
  code: string;
  path: string;
  message: string;
}

export interface ValidationView {
  rows: ValidationRow[];
  errorCount: number;
  warningCount: number;
  /** T04.AC2: errors refuse the file, so the open action is not offered. */
  canOpen: boolean;
  /** T04.R06: a storage problem always leaves the file path available. */
  canSaveToFile: boolean;
  title: string;
}

export type ValidationPanelState =
  | { kind: 'idle' }
  | { kind: 'validating' }
  | { kind: 'opened'; warnings: Issue[] }
  | { kind: 'rejected'; errors: Issue[] }
  | { kind: 'storage-error'; code: string; message: string };

/**
 * Turns a store result into exactly what the panel has to render.
 *
 * Pure on purpose: every rule T04 asks for -- errors refuse, warnings open, a storage
 * failure offers the file -- is a decision, and decisions tested through a real DOM are
 * decisions nobody runs. The renderer below does no thinking at all.
 */
export function buildValidationView(
  state: ValidationPanelState,
  options: { offerSaveToFile?: boolean } = {},
): ValidationView {
  const offerSaveToFile = options.offerSaveToFile ?? false;

  switch (state.kind) {
    case 'idle':
      return {
        rows: [],
        errorCount: 0,
        warningCount: 0,
        canOpen: false,
        canSaveToFile: offerSaveToFile,
        title: 'No file loaded',
      };

    case 'validating':
      return {
        rows: [],
        errorCount: 0,
        warningCount: 0,
        canOpen: false,
        canSaveToFile: offerSaveToFile,
        title: 'Checking...',
      };

    case 'opened': {
      const rows = state.warnings.map(toRow('warning'));
      return {
        rows,
        errorCount: 0,
        warningCount: rows.length,
        canOpen: true,
        canSaveToFile: offerSaveToFile,
        title: rows.length === 0 ? 'Opened, no problems' : `Opened with ${rows.length} warning(s)`,
      };
    }

    case 'rejected': {
      const rows = state.errors.map(toRow('error'));
      return {
        rows,
        errorCount: rows.length,
        warningCount: 0,
        canOpen: false,
        canSaveToFile: offerSaveToFile,
        title: `Cannot open: ${rows.length} error(s). Your current project is unchanged.`,
      };
    }

    case 'storage-error':
      return {
        rows: [{ severity: 'error', code: state.code, path: '/', message: state.message }],
        errorCount: 1,
        warningCount: 0,
        canOpen: false,
        // T04.R06: every storage problem routes the user to the file.
        canSaveToFile: true,
        title: 'Storage problem. Save to a file to keep your work safe.',
      };
  }
}

/** One entry point for every result the store can hand back. */
export function viewForResult(result: OpenResult): ValidationView {
  if (result.ok) {
    return buildValidationView({ kind: 'opened', warnings: result.warnings });
  }
  return buildValidationView({ kind: 'rejected', errors: result.errors });
}

/** Groups by path so a document with 200 note errors reads as one list, not 200 lines. */
export function groupByPath(rows: ValidationRow[]): Array<{ path: string; rows: ValidationRow[] }> {
  const order: string[] = [];
  const byPath = new Map<string, ValidationRow[]>();
  for (const row of rows) {
    const bucket = byPath.get(row.path);
    if (bucket) {
      bucket.push(row);
    } else {
      byPath.set(row.path, [row]);
      order.push(row.path);
    }
  }
  return order.map((path) => ({ path, rows: byPath.get(path) ?? [] }));
}

function toRow(severity: ValidationSeverity) {
  return (issue: Issue): ValidationRow => ({
    severity,
    code: issue.code,
    path: issue.path,
    message: issue.message,
  });
}