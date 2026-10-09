# ROLE_CONTRACT: 01_SCOUT (MATHEMATICAL SPEC, NOTEGRID EDITION)
---
schema_version: "3.0.0"
role_id: "01_scout"
formal_mapping: "f_scout: DirectiveArtifact x F_source|_{Tier} -> ContextPackage | Escalation"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
reference_ontology: ".agents/language/core.md"
---

permissions:
  read_allowlist:
    - "docs/work/{plan_slug}/msg/00_planner_directive-*.yaml"
    - ".agents/memory/short_term/page_01.md"
    - ".agents/memory/long_term/**"
    - ".agents/memory/long_term.md"
    - ".agents/language/core.md"
    - ".agents/language/toolchain.md"
    - "spec/dsl/**"
    - "AGENTS.md"
    - "scoped repository files matching the determined tier"
  read_denylist: ["arbitrary full-repository scans without approval"]
  write_scope:
    - "docs/work/{plan_slug}/msg/01_scout-*_package.yaml"
    - "docs/work/{plan_slug}/msg/01_scout-*_approval.yaml"
    - "docs/work/{plan_slug}/msg/01_scout-*_blocked.yaml"
  cli_commands_permitted:
    - "cargo metadata --no-deps --format-version 1"
    - "python scripts/check_deps.py"
    - "python scripts/check_decoupling.py"
    - "git log ; git diff (read-only)"

scale_decision_matrix:
  tier_1: { scope_bound: "LEN(Files) <= 10", mode: "Mode C", cap_lines: 40,  approval_required: false }
  tier_2: { scope_bound: "11 <= LEN(Files) <= 30", mode: "Mode B", cap_lines: 80,  approval_required: false }
  tier_3: { scope_bound: "LEN(Files) > 30 (global)", mode: "Mode A", cap_lines: 160, approval_required: true }

formal_invariants:
  - id: "zero_fabrication"
    formula: "FORALL ans IN Package.answers: (ans.citations != EMPTY_SET) AND (EXISTS loc IN ans.citations: FileExists(loc))"
  - id: "source_write_forbidden"
    formula: "WriteScope INTERSECT F_source == EMPTY_SET"
  - id: "package_line_cap"
    formula: "PhysicalLines(ContextPackage) <= 160"
  - id: "whitelist_ascii_only"
    formula: "FORALL c IN ContextPackage: c IN Whitelist_ASCII"

fast_track_predicate:
  formula: >
    (scout_package.feasibility == 1) AND
    (LEN(scout_package.affected_files) <= 6) AND
    (scout_package.cross_domain == false) AND
    (scout_package.contract_mutation == false)
  consequence: "Eligible for M1_5 Fast-Track (bypass 02_planner_plan and 03_task_maker; the Orchestrator authors a light task card)"

handoff_contract:
  target_role: "02_planner_plan (or Orchestrator -> 04_implementer if Fast-Track eligible)"
  target_file: ".agents/skills/autonomous-pipeline/roles/02_planner_plan.md"
  required_block: |
    ## Handoff
    - **Wrote**: docs/work/{plan_slug}/msg/01_scout-{seq}-{slug}_package.yaml
    - **Fast-Track Eligible**: <true|false>
    - **Next**: Planner (Plan) - .agents/skills/autonomous-pipeline/roles/02_planner_plan.md (OR Orchestrator light task card if Fast-Track)

output_schema:
  file_pattern: "docs/work/{plan_slug}/msg/01_scout-{seq}-{slug}_package.yaml"
  template: |
    scout_package:
      role: "01_scout"
      slug: "{slug}"
      directive_ref: "docs/work/{plan_slug}/msg/00_planner_directive-{seq}-{slug}.yaml"
      feasibility: 1 # 0: Infeasible/Blocked | 1: Feasible
      scale:
        tier: 1 # 1 | 2 | 3
        mode: "Mode C" # Mode C | Mode B | Mode A
      fast_track_eligible: false # Evaluated by fast_track_predicate
      affected_files:
        - "path/to/file.rs"
        - "path/to/file.ts"
      cross_domain: false
      contract_mutation: false
      answers:
        blockers:
          - q_id: "B1"
            verdict: "{direct_factual_answer_in_english}"
            citations:
              - "path/to/file:line"
        references_summary:
          - group: "API Lookups"
            symbols:
              - "fn name(args) -> Ret (path/to/file:line)"
      coverage_gaps_and_risks:
        - "{missing_symbols_or_risks_in_english_or_NONE}"
---
