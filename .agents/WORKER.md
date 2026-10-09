# WORKER CONTRACT (NoteGrid)
---
schema_version: "1.0.0"
spec_type: "WORKER_ROLE"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
canonical_constitution: "AGENTS.md"
---

## 1. ROLE

A worker owns exactly ONE task card and ONE worktree.
The worker follows TASK_FLOW: ORIENT -> TEST_FIRST -> IMPLEMENT -> VERIFY -> DONE.

## 2. TASK_FLOW DISCIPLINE

### 2.1 ORIENT
- Read the task card: `.agents/queue/active/<TASK_ID>.md`
- Read the role contract: `.agents/workers/<ROLE>.md`
- Read the spec: `spec/dsl/03_roadmap.dsl` (task entry)
- Read the contracts: `spec/dsl/02_contracts.dsl`
- Verify all READS exist in the workspace

### 2.2 TEST_FIRST
- Write tests before implementation
- Tests must fail before implementation passes
- Use the second-opinion pattern: expected values from outside the crate under test

### 2.3 IMPLEMENT
- Write code in the worktree only
- Respect SCOPE: `FORALL f IN FilesWritten(worker): f IN SCOPE(card(worker))`
- Follow quality commandments (typing, hotpath, zero-dep core, determinism)

### 2.4 VERIFY
- Run role-level gates: `cargo -p <crate>`, `pnpm -r test`, or e2e
- Run V_GATE: `python scripts/check_deps.py`, `python scripts/check_hotpath_lints.py`, etc.
- Collect evidence: command + real output or exit code

### 2.5 DONE
- All DONE WHEN items have evidence
- All gates pass
- No scope violations

## 3. EVIDENCE

Evidence is a command + real output or exit code.
A worker claim is never evidence; the gates are re-run before a DONE verdict lands.

## 4. SCOPE FENCE

`FORALL f IN FilesWritten(worker): f IN SCOPE(card(worker))`

If a change requires touching files outside SCOPE:
- Stop
- Write a DECISIONS.md entry
- Request orchestrator approval

## 5. BLOCKED

A contract or scope gap means:
- BLOCKED + BLOCKERS.md entry
- NEVER widen SCOPE
- NEVER substitute a guess for a fact

## 6. ROLE CONTRACTS

| role | file | scope |
|---|---|---|
| core-rust | `.agents/workers/core-rust.md` | ng-core, ng-score, ng-timeline |
| engine-ts | `.agents/workers/engine-ts.md` | ng-wasm, packages/playback, packages/export-service |
| app-ui | `.agents/workers/app-ui.md` | packages/app, packages/document, packages/editing, packages/ui-grid |
| verifier | `.agents/workers/verifier.md` | gates, fixtures, goldens, budgets |

## 7. REFERENCES
- AGENTS.md (root)
- .agents/ORCHESTRATOR.md
- .agents/workers/*.md
- spec/dsl/00_dsl.dsl (TASK_FLOW)
- spec/dsl/02_contracts.dsl (C0..C12)
