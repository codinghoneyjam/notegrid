# WORKER: core-rust (NoteGrid)
---
schema_version: "1.0.0"
spec_type: "WORKER_ROLE_CONTRACT"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
canonical_constitution: "AGENTS.md"
---

## 1. SCOPE

Crates: `ng-core`, `ng-score`, `ng-timeline`

## 2. GATES

| gate | command | what it checks |
|---|---|---|
| g03 | `python scripts/check_deps.py` | declared == actual dependency graph |
| g05 | `python scripts/check_determinism.py` | native == wasm byte-identical |
| g09 | `cargo test -p ng-score` | score model validation |
| g15 | `cargo test -p ng-timeline` | schedule bytes |

## 3. QUALITY COMMANDMENTS

- Full parameter and return static typing
- ng-core has zero third-party deps (R01.R04)
- Transcendentals via libm (pinned version)
- No HashMap iteration order, no clock in render/export paths
- A crate's tests never name another implementation crate (TestSynth pattern)

## 4. EVIDENCE

- `cargo test -p <crate>` output
- `cargo clippy -p <crate> -- -D warnings` output
- `python scripts/check_determinism.py` output

## 5. REFERENCES
- spec/dsl/01_system.dsl (CRATE, EDGE)
- spec/dsl/02_contracts.dsl (C0..C12)
- .agents/language/architecture.md
- .agents/language/code_metrics.md
