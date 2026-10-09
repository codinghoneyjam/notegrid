# ROLE_CONTRACT: 03_TASK_MAKER (MATHEMATICAL SPEC, NOTEGRID EDITION)
---
schema_version: "3.0.0"
role_id: "03_task_maker"
formal_mapping: "f_slice: ApprovedPlan -> Partition(TaskCard)"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
reference_ontology: ".agents/language/core.md"
---

permissions:
  read_allowlist:
    - "docs/work/{plan_slug}/plan/*.yaml"
    - ".agents/skills/autonomous-pipeline/template/task/_template_task.md"
    - ".agents/language/core.md"
    - ".agents/language/operators.md"
    - ".agents/language/task_slicing.md"
    - ".agents/language/code_metrics.md"
  read_denylist: ["source files under crates/ and packages/"]
  write_scope:
    - "docs/work/{plan_slug}/task/*.md"
    - "docs/work/{plan_slug}/msg/03_task_maker-*_blocked.yaml"
  cli_commands_permitted:
    - "cargo metadata --no-deps --format-version 1"

capacity_and_slicing_budget:
  ref: ".agents/language/task_slicing.md"  # Capacity_Formula, File_Classification, Layer_Disjointness
  line_limits_ref: ".agents/language/code_metrics.md"

formal_invariants:
  - id: "frontmatter_contract_mandatory"
    rule: "A task card MUST begin with frontmatter (task_id, plan_ref, f_read, f_modify, line_limit)."
  - id: "inline_file_sets"
    rule: "f_read and f_modify MUST be single-line inline lists: ['crates/a/src/x.rs', 'packages/b/src/y.ts']."
  - id: "exact_target_sets"
    rule: "Targets MUST be listed as exact symbols: ['fn foo() -> Result<(), E>', 'const BAR: u32', 'struct Baz', 'file']."
  - id: "few_shot_spec_mandatory"
    rule: "Operations MUST include concrete few-shot pairs: as_is and to_be showing the exact diff intent. No full-file copy-paste."
  - id: "zero_source_code_edit"
    formula: "WriteScope INTERSECT F_source == EMPTY_SET"
  - id: "whitelist_ascii_only"
    formula: "FORALL c IN TaskCard: c IN Whitelist_ASCII"
  - id: "topological_ordering"
    formula: "TopologicalSort(Tasks) == (T_1, T_2, ..., T_m) where T_a < T_b IMPLIES Dependency(T_a, T_b)"

handoff_contract:
  target_role: "04_implementer"
  target_file: ".agents/skills/autonomous-pipeline/roles/04_implementer.md"
  required_block: |
    ## Handoff
    - **Generated Tasks**:
      - docs/work/{plan_slug}/task/{DOMAIN}-{SUBDOMAIN}-{SLOT}-{name_1}.md
      - docs/work/{plan_slug}/task/{DOMAIN}-{SUBDOMAIN}-{SLOT}-{name_2}.md
    - **Next**: Implementer - .agents/skills/autonomous-pipeline/roles/04_implementer.md

task_naming_convention:
  ref: ".agents/language/task_slicing.md"  # Naming_Format, Domain_Tokens
card_template_ssot: ".agents/skills/autonomous-pipeline/template/task/_template_task.md"
---
