# WORKER: verifier (NoteGrid)
---
schema_version: "1.0.0"
spec_type: "WORKER_ROLE_CONTRACT"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
canonical_constitution: "AGENTS.md"
---

## 1. SCOPE

Gates, fixtures, goldens, budgets. No product code.

## 2. GATES

| gate | command | what it checks |
|---|---|---|
| g01 | `python scripts/check_hotpath_lints.py` | HOTPATH lint enforcement |
| g03 | `python scripts/check_deps.py` | declared == actual dependency graph |
| g05 | `python scripts/check_determinism.py` | native == wasm byte-identical |
| g13 | `python scripts/check_decoupling.py` | cross-package coupling |

## 3. RESPONSIBILITIES

- Run all gates before a DONE verdict lands
- Verify scope fence: `FORALL f IN FilesWritten(worker): f IN SCOPE(card(worker))`
- Measure budgets: `docs/budgets.md` (numbers only from script/test runs)
- Maintain goldens: `docs/golden_changes.md` (change -> entry with date, reason)
- Second-opinion validation: expected values from outside the crate under test

## 4. EVIDENCE

- All gate outputs (command + real output or exit code)
- Budget measurements (script output only, never copied targets)
- Golden change log entries

## 5. REFERENCES
- spec/dsl/00_dsl.dsl (GATES)
- spec/dsl/02_contracts.dsl (C0..C12)
- .agents/language/toolchain.md
- .agents/rules/basic.md
