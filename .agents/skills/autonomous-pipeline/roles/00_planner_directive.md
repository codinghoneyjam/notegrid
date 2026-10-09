# ROLE_CONTRACT: 00_PLANNER_DIRECTIVE (MATHEMATICAL SPEC, NOTEGRID EDITION)
---
schema_version: "3.0.0"
role_id: "00_planner_directive"
formal_mapping: "f_dir: UserRequest x D_docs -> DirectiveArtifact"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
reference_ontology: ".agents/language/core.md"
---

permissions:
  read_allowlist:
    - "USER_REQUEST"
    - "AGENTS.md"
    - ".agents/language/core.md"
    - "spec/dsl/00_dsl.dsl"
    - "spec/dsl/02_contracts.dsl"
    - "docs/tasks/DECISIONS.md"
    - "docs/tasks/BLOCKERS.md"
  read_denylist: ["*.rs", "*.ts", "*.tsx", "crates/**", "packages/**", "tests under crates or packages"]
  write_scope:
    - "docs/work/{plan_slug}/msg/00_planner_directive-*.yaml"
  cli_commands_permitted: []

isolation_axioms:
  - id: "zero_code_inspection"
    formula: "ReadScope INTERSECT F_source == EMPTY_SET"
    reason: "Reading implementation files now would bias the questions prematurely."
  - id: "zero_plan_drafting"
    formula: "Drafting architecture steps in this stage is strictly forbidden."
  - id: "single_artifact"
    formula: "LEN(WrittenArtifacts) == 1"
  - id: "whitelist_ascii_only"
    formula: "FORALL c IN DirectiveArtifact: c IN Whitelist_ASCII"

question_topology_bounds:
  tier_bounds:
    tier_1: { min_q: 1, max_q: 4,  scope: "Local (<= 10 files)" }
    tier_2: { min_q: 3, max_q: 8,  scope: "Domain (11-30 files)" }
    tier_3: { min_q: 4, max_q: 15, scope: "Cross-domain / global" }
  partition_rule: "Q_total := Q_blockers UNION Q_references"

handoff_contract:
  target_role: "01_scout"
  target_file: ".agents/skills/autonomous-pipeline/roles/01_scout.md"
  required_block: |
    ## Handoff
    - **Wrote**: docs/work/{plan_slug}/msg/00_planner_directive-{seq}-{slug}.yaml
    - **Next**: Scout (Broker & Surveyor) - .agents/skills/autonomous-pipeline/roles/01_scout.md

output_schema:
  file_pattern: "docs/work/{plan_slug}/msg/00_planner_directive-{seq}-{slug}.yaml"
  template: |
    directive:
      role: "00_planner_directive"
      slug: "{slug}"
      source_request: "{one_line_request_summary}"
      target_plan: "docs/work/{plan_slug}/plan/{slug}.yaml"
      goal: "{exact_goal_statement_in_english}"
      scale_hint:
        touched_domains: ["core", "audio", "asm", "ui", "tool"]
        estimated_tier: 1 # 1: Local | 2: Domain | 3: Cross-domain
      questions:
        blockers:
          - id: "B1"
            query: "{critical_path_question_in_english}"
            why_blocking: "{failure_impact_if_unanswered}"
        references:
          - id: "R1"
            query: "{signature_or_reusable_symbol_query}"
            target_scope: "{directory_or_type_hint}"
---
