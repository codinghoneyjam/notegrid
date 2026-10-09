# WORKER: engine-ts (NoteGrid)
---
schema_version: "1.0.0"
spec_type: "WORKER_ROLE_CONTRACT"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
canonical_constitution: "AGENTS.md"
---

## 1. SCOPE

Crates: `ng-wasm`
Packages: `packages/playback`, `packages/export-service`

## 2. GATES

| gate | command | what it checks |
|---|---|---|
| g04 | `cargo test -p ng-wasm` | wasm ABI stability |
| g05 | `python scripts/check_determinism.py` | native == wasm byte-identical |
| g08 | `cargo test -p ng-wasm` | ABI skeleton functions |
| g10 | `pnpm -r test` | engine packages |
| g14 | `pnpm -r test` | cross-browser harness |

## 3. QUALITY COMMANDMENTS

- Full parameter and return static typing in Rust and TS
- HOTPATH: ng-dsp, ng-synth, ng-render (deny unwrap_used, expect_used, indexing_slicing, panic)
- extern "C" only in ng-wasm (gate g03)
- AudioWorklet: 128-frame blocks, no allocation on audio thread
- Deterministic: no clock, no HashMap order in render/export

## 4. EVIDENCE

- `cargo test -p ng-wasm` output
- `pnpm -r test` output
- `python scripts/check_determinism.py` output
- `python scripts/check_hotpath_lints.py` output

## 5. REFERENCES
- spec/dsl/01_system.dsl (CRATE, EDGE)
- spec/dsl/02_contracts.dsl (C0..C12)
- .agents/language/architecture.md
- .agents/language/code_metrics.md
