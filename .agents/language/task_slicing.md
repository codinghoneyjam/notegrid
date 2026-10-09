# ONTOLOGY: TASK SLICING & DOMAIN TAXONOMY (NOTEGRID EDITION)
---
schema_version: "4.0.0"
spec_type: "TASK_SLICING_ONTOLOGY"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
---

## 1. CAPACITY & SLICING BUDGET
Capacity_Formula: "2 * LEN(F_complex) + LEN(F_light) <= 8"

File_Classification:
  F_light:   "{ f | ModifiedSymbols(f) <= 1 function OR (const/var/schema adjustment only) }"
  F_complex: "{ f | ModifiedSymbols(f) >= 2 functions OR introduces new control flow/FSM/branching }"

Layer_Disjointness:
  Single_Subdomain:    "LEN({ Subdomain(f) | f IN ScopeWrite }) == 1"
  Scope_Disjointness:  "In-flight tasks keep SCOPE units disjoint (AGENTS.md scope_fence_invariants.parallel_disjoint); units are crate and package names plus declared path globs."

## 2. TASK NAMING CONVENTION & DOMAIN TOKENS
Naming_Format: "{DOMAIN}-{SUBDOMAIN}-{SLOT}-{name}.md"
Domain_Tokens:
  CORE:  "crates/ng-core, ng-score, ng-timeline (types, validation, time)"
  AUDIO: "crates/ng-dsp, ng-synth, ng-render (signal path)"
  ASM:   "crates/ng-wasm, ng-export, ng-cli (ABI, encoders, native tool)"
  UI:    "packages/abi, document, editing, ui-grid, playback, persistence, app"
  TOOL:  "scripts/, xtask/, e2e/, spec/, docs/"
