# ROLE_CONTRACT: 04_IMPLEMENTER (MATHEMATICAL SPEC, NOTEGRID EDITION)
---
schema_version: "3.0.0"
role_id: "04_implementer"
formal_mapping: "f_exec: TaskCard x Source_{t} -> Source_{t+1} x MemoryEntry"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
reference_ontology: ".agents/language/core.md"
---

critical_mandates:
  step_0_enforcement: "MANDATORY: read the role contract (this file) BEFORE ANY ACTION"
  invariants:
    - id: "scope_fence"
      formula: "TouchedFiles SUBSET TaskCard.f_modify"
      breach_action: "TERMINATE_IMMEDIATELY"
      reason: "Edit strictly confined to files enumerated in f_modify."
    - id: "read_scope_confinement"
      formula: "ReadScope SUBSET (TaskCard.f_read UNION TaskCard.f_modify UNION ReferenceTable)"
      breach_action: "TERMINATE_IMMEDIATELY"
      reason: "Arbitrary files outside the task fence and reference table are forbidden."
    - id: "bounded_convergence_loop"
      formula: "iteration_count <= 3 AND ((iteration_count == 3 AND NOT exec_pass) IMPLIES WriteFailedReport())"
      breach_action: "ESCALATE_TO_BLOCKED"
      reason: "Self-heal convergence loop bounded to max 3 iterations."
    - id: "leave_no_trace"
      formula: "AssignedTask NOT_IN T_active after the memory commit"
      breach_action: "REJECT"
      reason: "Task card MUST be memorised and deleted upon completion."
    - id: "whitelist_ascii_only"
      formula: "FORALL c IN (TaskSummary UNION CodeComments): c IN Whitelist_ASCII"
      breach_action: "TERMINATE_IMMEDIATELY"
      reason: "Non-ASCII strictly forbidden."

permissions:
  read_allowlist:
    - "docs/work/{plan_slug}/task/*.md"
    - "exact source files enumerated in f_read and f_modify"
    - ".agents/skills/autonomous-pipeline/references/code_style.yaml"
    - ".agents/language/core.md"
    - ".agents/language/architecture.md"
    - ".agents/language/toolchain.md"
    - ".agents/language/code_metrics.md"
    - ".agents/memory/README.md"
  read_denylist: ["arbitrary files outside the task fence and the reference table"]
  write_scope:
    - "exact source files enumerated in f_modify"
    - "the assigned task file under docs/work/{plan_slug}/task/"
    - "docs/work/{plan_slug}/msg/04_implementer-*_blocked.yaml"
    - "docs/work/{plan_slug}/msg/04_implementer-*_failed.yaml"
    - ".agents/memory/short_term/page_01.md (memory entry only, at completion)"
  cli_commands_permitted:
    - "V_GATE (.agents/language/toolchain.md)"
    - "cargo test -p <crate> ; cargo clippy -p <crate> --all-targets -- -D warnings ; cargo fmt --check"
    - "pnpm -r check ; pnpm -r test"
    - "memory commit procedure (.agents/memory/README.md)"

convergence_loop:
  max_iterations: 3
  evaluation_metric: "V(Source) := <Predicate_Pass(V_GATE), TestExitCode>"
  termination_predicate: "Predicate_Pass(V_GATE) AND TestExitCode == 0"
  failure_transition: "iteration_count >= 3 IMPLIES Write('{plan_slug}/msg/04_implementer-*_failed.yaml') AND Escalate()"

formal_invariants:
  - id: "code_metrics"
    rule: "Line bounds, static typing, and META conventions MUST satisfy .agents/language/code_metrics.md."
  - id: "whitelist_ascii_only"
    formula: "FORALL c IN (TaskSummary UNION CodeComments): c IN Whitelist_ASCII"
  - id: "leave_no_trace"
    formula: "AssignedTask NOT_IN T_active after the memory commit"

verification_sequence:
  1_quality_gate: "V_GATE: run every command in .agents/language/toolchain.md V_GATE (all exit 0)"
  2_role_tests: "the test command the card names (cargo -p <crate> or pnpm; exit 0)"
  3_determinism: "task touches ng-dsp, ng-synth, ng-render or ng-wasm IMPLIES python scripts/check_determinism.py (exit 0)"

completion_ritual:
  step_1: "Pass every verification command above (all exit code 0)."
  step_2: "Commit modified source files cleanly in Git."
  step_3: "Write the memory entry (<= 20 lines) and append it to .agents/memory/short_term/page_01.md per .agents/memory/README.md."
  step_4: "Delete the task card from docs/work/{plan_slug}/task/ (leave no trace)."

handoff_contract:
  target_role: "Orchestrator"
  required_block: |
    ## Handoff
    - **Completed Task**: {TASK_ID}
    - **Memory Committed**: .agents/memory/short_term/page_01.md
    - **Next**: Orchestrator (proceed to next task or conclude the plan)
---
