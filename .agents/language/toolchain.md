# ONTOLOGY: TOOLCHAIN REGISTRY & EXECUTION MAP (NOTEGRID EDITION)
---
schema_version: "4.0.0"
spec_type: "TOOLCHAIN_ONTOLOGY"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
canonical_constitution: "AGENTS.md"
---

## 1. COMMAND REGISTRY (TOOL_MAP)
TOOL_MAP:
  GATES:
    board:        "python scripts/check_task_state.py [--check]"
    deps:         "python scripts/check_deps.py"
    hotpath:      "python scripts/check_hotpath_lints.py"
    decoupling:   "python scripts/check_decoupling.py"
    determinism:  "python scripts/check_determinism.py"
    vectors:      "python scripts/gen_tempo_vectors.py --check"
  DISPATCH:
    queue:        "python scripts/agent_queue.py {ready, claim, release, complete, block, status}"
  BUILD:
    wasm:         "cargo xtask build-wasm"
    test_all:     "cargo xtask test-all"
  RUST_GATES:     "cargo fmt --check ; cargo clippy --workspace --all-targets -- -D warnings ; cargo test --workspace"
  TS_GATES:       "pnpm -r check ; pnpm -r test"
  WORKSPACE_GRAPH: "cargo metadata --no-deps --format-version 1"

## 2. COMPOSITE QUALITY GATE (V_GATE)
V_GATE := [ GATES.deps, GATES.hotpath, GATES.decoupling, GATES.determinism ]
Predicate_Pass(V_GATE) <=> (FORALL cmd IN V_GATE: ExitCode(cmd) == 0)
NOTE: a card's DONE WHEN adds the role-level test command (cargo -p <crate>, pnpm -r test, or e2e).

## 3. LEGACY TOOLCHAIN MAP (migration note, 2026-10-07)
The game repository drove its checks through `python -m dev.tools`. NoteGrid has no such facade;
the table below is the honest remap used by the transplanted pipeline:

| legacy tool | NoteGrid equivalent |
| :--- | :--- |
| QUALITY.cross_domain | GATES.deps ; GATES.decoupling |
| QUALITY.runtime_risk | GATES.hotpath |
| CODE.inspect | WORKSPACE_GRAPH (read-only) |
| CODE.index / CODE.skeleton | NO EQUIVALENT (use spec/dsl module files and ripgrep) |
| CODE.meta_sync | NO EQUIVALENT (META headers are a manual convention; see code_metrics.md) |
| DOCS.memory | NO EQUIVALENT (manual memory procedure; see .agents/memory) |
