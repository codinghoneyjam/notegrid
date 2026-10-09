# ROLE_CONTRACT: 02_PLANNER_PLAN (MATHEMATICAL SPEC, NOTEGRID EDITION)
---
schema_version: "3.0.0"
role_id: "02_planner_plan"
formal_mapping: "f_plan: ContextPackage -> PlanArtifact"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
reference_ontology: ".agents/language/core.md"
---

permissions:
  read_allowlist:
    - "docs/work/{plan_slug}/msg/*.yaml"
    - ".agents/skills/autonomous-pipeline/template/plan/_template_plan.yaml"
    - ".agents/language/core.md"
    - ".agents/language/operators.md"
    - ".agents/language/architecture.md"
    - ".agents/language/task_slicing.md"
    - "spec/dsl/00_dsl.dsl"
    - "spec/dsl/01_system.dsl"
    - "spec/dsl/02_contracts.dsl"
    - "files cited with symbol or path:line in the Scout Context Package"
  read_denylist: ["uncited source files across crates/ and packages/"]
  write_scope:
    - "docs/work/{plan_slug}/plan/*.yaml"
    - "docs/work/{plan_slug}/msg/02_planner_plan-*_insufficient.yaml"
  cli_commands_permitted:
    - "cargo metadata --no-deps --format-version 1"

preconditions:
  - "ContextPackage.feasibility == 1"
  - "LEN(ActivePlanDirectories('docs/work/')) <= 3"

scope_confinement_axioms:
  - id: "confinement_to_citations"
    formula: "ReadScope SUBSET (Citations(ContextPackage) UNION {ContextPackage})"
  - id: "uncited_read_forbidden"
    formula: "ReadScope INTERSECT (F_source DIFF Citations(ContextPackage)) == EMPTY_SET"
  - id: "source_edit_forbidden"
    formula: "WriteScope INTERSECT F_source == EMPTY_SET"
  - id: "whitelist_ascii_only"
    formula: "FORALL c IN PlanArtifact: c IN Whitelist_ASCII"

handoff_contract:
  target_gate: "HUMAN_PLAN_APPROVAL"
  required_block: |
    ## Human Gate (User Approval Required)
    - **Plan Path**: docs/work/{plan_slug}/plan/{slug}.yaml
    - **Status**: PENDING (Awaiting explicit human approval)
    - **Handoff Target**: Orchestrator (single-slice M2) OR Task Maker (multi-slice M3)

output_schema:
  file_pattern: "docs/work/{plan_slug}/plan/{slug}.yaml"
  template_ssot: ".agents/skills/autonomous-pipeline/template/plan/_template_plan.yaml"
  instantiation_rule: "Populate fields matching template_ssot: id, status='PENDING', title, evidence_ref, architecture, slicing_budget, slices, verifications."
---
