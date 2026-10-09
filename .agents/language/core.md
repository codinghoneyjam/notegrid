# ONTOLOGY: CORE & UNIVERSE MANIFEST (NOTEGRID EDITION)
---
schema_version: "4.0.0"
spec_type: "LANGUAGE_CORE"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
canonical_constitution: "AGENTS.md"
---

## 1. CHARACTER SET WHITELIST
Constraint: FORALL doc IN PipelineArtifacts: FORALL char IN doc: char IN Whitelist_ASCII
Regex_Validator: "^[\x20-\x7E\r\n\t]*$" # 100% Printable ASCII mandatory
Note: Korean (ko-KR) is mandatory for user conversation only; it NEVER enters artifacts.

## 2. UNIVERSE DOMAINS
U_repo      := Set of all git-tracked files in the repository
F_source    := { f IN U_repo | ext(f) IN ['rs', 'ts', 'tsx', 'py', 'json', 'toml', 'yaml'] }
D_docs      := { f IN U_repo | path(f) starts_with 'docs/' }
D_spec      := { f IN U_repo | path(f) starts_with 'spec/' }
L_layers    := ['core', 'dsp', 'synth', 'render', 'export', 'assembly']   # crate/package taxonomy, ref spec/dsl/01_system.dsl
T_flow      := ['ORIENT', 'TEST_FIRST', 'IMPLEMENT', 'VERIFY', 'DONE', 'BLOCKED']  # ref spec/dsl/00_dsl.dsl FSM TASK_FLOW

## 3. SPECIALIZED ONTOLOGY MANIFEST
modules:
  operators:    ".agents/language/operators.md"    # Mathematical & set theory DSL
  architecture: ".agents/language/architecture.md" # Crate/package topology & composition terms
  toolchain:    ".agents/language/toolchain.md"    # CLI registry, TOOL_MAP, and V_GATE
  code_metrics: ".agents/language/code_metrics.md" # Typing rules, line bounds, and META headers
  task_slicing: ".agents/language/task_slicing.md" # Slicing budget formulas and task tokens
