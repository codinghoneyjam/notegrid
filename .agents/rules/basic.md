# CORE_RULES_AND_RUNTIME_CHECKLIST (FORMAL MATHEMATICAL SPECIFICATION)
---
schema_version: "4.0.0"
spec_type: "INVARIANT_SET"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
ontology_ref: "AGENTS.md"
---

symbolic_domains:
  CRATES: "GLOBAL SETS in spec/dsl/00_dsl.dsl"
  PACKAGES: "GLOBAL SETS in spec/dsl/00_dsl.dsl"
  HOTPATH: "{ ng-dsp, ng-synth, ng-render }"
  Components: "Set of all crates, packages, gates and scripts"

quality_invariants:
  zero_dep_core:
    formula: "LEN(Deps(ng-core)) == 0 (R01.R04)"
  hotpath_deny:
    formula: "FORALL c IN HOTPATH: lints opt-in via [workspace.lints] AND no local allow (gate g01)"
  extern_c_isolation:
    formula: "crate uses {extern C, unsafe} IMPLIES crate EQUALS ng-wasm (gate g03)"
  determinism:
    formula: "libm for transcendentals AND no HashMap order AND no clock AND seeded noise AND native render == wasm render byte-identical (gate g05)"
  graph_equality:
    formula: "declared DEPENDS_ON == actual dependency graph, both directions (gate g03)"
  gate_ships_with_checker:
    formula: "FORALL g IN NewGates: EXISTS checker(g) AND EXISTS test(checker(g))"
  vacuous_gate_is_failure:
    formula: "Passes(g) AND ProtectsNothing(g) IMPLIES Failure"
  budget_measurement:
    formula: "number IN docs/budgets.md IMPLIES produced by a script or test run"
  generated_files:
    formula: "FORALL f IN GeneratedFiles: NOT HandEdit(f)"
  golden_change_logged:
    formula: "Change(golden) IMPLIES Entry(docs/golden_changes.md) WITH (date, reason)"

evidence_invariants:
  done_claim:
    formula: "STATE(DONE, t) IMPLIES (FORALL item IN DoneWhen(t): Evidence(item) != EMPTY_SET)"
  no_say_so:
    rule: "a worker claim is never evidence; the gates are re-run before a DONE verdict lands"
  blocked_is_reported:
    rule: "a contract or scope gap means BLOCKED plus a docs/tasks/BLOCKERS.md entry; never widen SCOPE"

scope_fences:
  worker_write:
    formula: "FORALL f IN FilesWritten(worker): f IN SCOPE(card(worker))"
  parallel_disjoint:
    formula: "FORALL c1, c2 IN ActiveCards: ScopeUnits(c1) INTERSECT ScopeUnits(c2) == EMPTY_SET"

runtime_traps:
  powershell_5_1: "no '&&' chaining; use ';' or separate commands"
  set_content: "PowerShell Set-Content re-encodes UTF-8; never use it for file writes"
  console_encoding: "cp949 console cannot print non-ASCII; agent-facing scripts print ASCII only"
  line_endings: "preserve each file's existing line-ending convention when editing"

pipeline_guard:
  ref: ".agents/skills/autonomous-pipeline/SKILL.md"
  note: "the deterministic lexer guards and the pipeline FSM live there (SSOT); this file does not duplicate them"
