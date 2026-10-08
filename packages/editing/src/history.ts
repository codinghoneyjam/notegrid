// <META - FILE SUMMARY - History implementation with undo/redo (T01).>

import type { Project } from '@ng/document';
import type { Command, History } from './types.js';

const HISTORY_DEPTH = 200;

export class HistoryImpl implements History {
  private undoStack: Array<{ command: Command; before: Project; after: Project }> = [];
  private redoStack: Array<{ command: Command; before: Project; after: Project }> = [];
  private currentDoc: Project;

  constructor(initialDoc: Project) {
    this.currentDoc = initialDoc;
  }

  execute(command: Command): Project {
    const after = command.apply(this.currentDoc);
    this.undoStack.push({ command, before: this.currentDoc, after });
    if (this.undoStack.length > HISTORY_DEPTH) {
      this.undoStack.shift();
    }
    this.redoStack = [];
    this.currentDoc = after;
    return after;
  }

  undo(): Project {
    const entry = this.undoStack.pop();
    if (!entry) return this.currentDoc;
    this.redoStack.push(entry);
    this.currentDoc = entry.before;
    return this.currentDoc;
  }

  redo(): Project {
    const entry = this.redoStack.pop();
    if (!entry) return this.currentDoc;
    this.undoStack.push(entry);
    this.currentDoc = entry.after;
    return this.currentDoc;
  }

  canUndo(): boolean {
    return this.undoStack.length > 0;
  }

  canRedo(): boolean {
    return this.redoStack.length > 0;
  }

  getCurrent(): Project {
    return this.currentDoc;
  }
}
