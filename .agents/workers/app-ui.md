# WORKER: app-ui (NoteGrid)
---
schema_version: "1.0.0"
spec_type: "WORKER_ROLE_CONTRACT"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
canonical_constitution: "AGENTS.md"
---

## 1. SCOPE

Packages: `packages/app`, `packages/document`, `packages/editing`, `packages/ui-grid`, `packages/persistence`

## 2. GATES

| gate | command | what it checks |
|---|---|---|
| g10 | `pnpm -r test` | app composition |
| g13 | `python scripts/check_decoupling.py` | cross-package coupling |
| g14 | `pnpm -r test` | cross-browser harness |

## 3. QUALITY COMMANDMENTS

- Full parameter and return static typing in TS
- Abstractions live with the consumer (no shared UI kit)
- Document model: structural-sharing updates, load/save round-trip
- Grid rendering: culling, hit-test, fps budget
- No direct domain -> UI references (UI is subscriber-only)

## 4. EVIDENCE

- `pnpm -r test` output
- `pnpm -r check` output
- `python scripts/check_decoupling.py` output

## 5. REFERENCES
- spec/dsl/01_system.dsl (PACKAGE, EDGE)
- spec/dsl/02_contracts.dsl (C0..C12)
- .agents/language/architecture.md
- .agents/language/code_metrics.md
