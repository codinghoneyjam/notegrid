# AGENT_CONSTITUTION: NOTEGrid
---
project_identity:
  name: "NoteGrid"
  summary: "Offline browser composer for game music"
  stack: ["rust_wasm_core", "typescript_ui"]
  spec_package: "spec/dsl/"

language_axioms:
  chat_interface: "Korean (ko-KR) mandatory for user conversation"
  artifacts_and_vcs: "100% ASCII-English mandatory for agent-facing docs, Git, Code, and Comments"
  whitelist_regex: "^[\x20-\x7E\r\n\t]*$"
  ontology_ref: ".agents/language/core.md"

symbol_lexicon_core:
  set_ops: [IN, NOT_IN, INTERSECT, UNION, DIFF, SUBSET, EMPTY_SET, LEN]
  logic_ops: [IMPLIES, EQUALS, NOT_EQUALS, AND, OR, NOT, FORALL, EXISTS]

single_sources_of_truth:
  task_state:
    location: "spec/dsl/03_roadmap.dsl STATE lines"
    formula: "FORALL t IN Tasks: STATE(t) IN TASK_FLOW UNION {NOT_STARTED}"
    generated_views: ["docs/tasks/board.md"]
    generator: "python scripts/check_task_state.py"
    axiom: "STATE moves ONLY via scripts/agent_queue.py or the orchestrator; board.md is generated, never hand-edited"
  contracts: "spec/dsl/02_contracts.dsl (C0..C12)"
  gates: "spec/dsl/00_dsl.dsl GATES"
  crate_graph: "spec/dsl/01_system.dsl (enforced by gate g03)"
  decisions: "docs/tasks/DECISIONS.md"
  blockers: "docs/tasks/BLOCKERS.md"
  goldens: "docs/golden_changes.md"

task_flow:
  ref: "spec/dsl/00_dsl.dsl FSM TASK_FLOW"
  states: [NOT_STARTED, ORIENT, TEST_FIRST, IMPLEMENT, VERIFY, DONE, BLOCKED]
  entry: "ORIENT"
  terminal: [DONE, BLOCKED]

evidence_axioms:
  done_claim: "STATE(DONE, t) IMPLIES (FORALL item IN DoneWhen(t): Evidence(item) != EMPTY_SET)"
  done_validation: "python scripts/check_task_state.py --check validates every DONE claim against the live workspace"
  measurement: "A number enters docs/budgets.md ONLY from a script or test run; a copied target is not a measurement"
  generated_files: "FORALL f IN GeneratedFiles: NOT HandEdit(f)"
  second_opinion: "Expected values come from an implementation outside the crate under test (the gen_tempo_vectors.py pattern)"

scope_fence_invariants:
  worker_write: "FORALL f IN FilesWritten(worker): f IN SCOPE(card(worker))"
  parallel_disjoint: "FORALL c1, c2 IN ActiveCards, c1 != c2: ScopeUnits(c1) INTERSECT ScopeUnits(c2) == EMPTY_SET"
  phase_gate: "Dispatchable(t) IMPLIES Phase(t) EQUALS PhaseInForce()"
  reads_exist: "FORALL u IN Reads(t): ExistsInWorkspace(u)"
  override: "NOT P_ready(t) IMPLIES (Dispatch(t) IMPLIES DECIDE entry IN docs/tasks/DECISIONS.md)"

quality_commandments:
  typing: "Full parameter and return static typing in Rust and TS"
  hotpath: "SET HOTPATH = {ng-dsp, ng-synth, ng-render}; deny {unwrap_used, expect_used, indexing_slicing, panic}; crate-wide via [lints] workspace = true; no local allow"
  zero_dep_core: "ng-core has zero third-party deps (R01.R04); any new dep anywhere needs a DECISIONS.md entry"
  determinism: "Transcendentals via libm (pinned version); native == wasm byte-identical (gate g05)"
  test_doubles: "A crate's tests never name another implementation crate (TestSynth pattern)"

support_layer:
  ontology: ".agents/language/ (operators, architecture, toolchain, code metrics, task slicing)"
  memory: ".agents/memory/ ; page mechanics and procedure in .agents/memory/README.md"
  rules: ".agents/rules/basic.md (invariants, evidence, scope fences, runtime traps)"
  template: ".agents/templates/task_spec_template.md"
  hooks: ".agents/hooks.json (read-gate: MUST-read enforcement + f_modify scope check; for hosts with hook support)"
  permissions: ".agents/permissions.yaml (trusted-commands reference: allow / ask / deny)"

workflow_modes:
  M_solo:
    condition: "N_sessions == 1"
    actor: "one session follows TASK_FLOW directly on the task it owns"
  M_sequential:
    condition: "default for multi-session work"
    actor: "one worker at a time in the main tree on branch agent/{TID}; orchestrator fast-forwards; no worktree, no merge conflict"
  M_parallel:
    condition: "P_independent AND implementation-heavy tasks"
    actor: ".agents/ORCHESTRATOR.md dispatch FSM with one worktree per card; orchestrator merges in task-id order"

pipeline_role_invariants:
  orchestrator: "Sole writer of spec/dsl/03_roadmap.dsl STATE, docs/tasks/*.md, .agents/queue/**; never writes product code"
  worker: "Owns exactly one card and one worktree"
  cardinality: "MUST assign exactly ONE role per worker session"
  role_file_invariant: "MUST pass the role contract file path in the subagent prompt; NEVER copy text sections"
  self_heal_limit: 3
  subagent_halts: "A subagent halt is a dispatch trigger, not a stopping point; end turn with artifact and handoff block"
  zero_fabrication: "Never substitute a guess for a fact; a contract gap means BLOCKED + a BLOCKERS.md entry"
  planning_pipeline: "Work not yet in the roadmap goes through .agents/skills/autonomous-pipeline/ (directive -> scout -> plan -> task -> implement; transplanted 2026-10-07)"
  coexistence: "Pipeline scope units never overlap in-flight queue cards; roadmap tasks execute under .agents/queue (see .agents/dispatch_readme.md)"

runtime_traps:
  powershell_5_1: "No '&&' chaining; use ';' or separate commands"
  console_encoding: "cp949 console cannot print em-dash or arrows; agent-facing scripts print ASCII only"
  line_endings: "Working tree uses CRLF; preserve original line endings"
  set_content: "PowerShell Set-Content re-encodes UTF-8 to ANSI; do not use it for file writes"

cli_quick_reference:
  board: "python scripts/check_task_state.py [--check]"
  queue_ready: "python scripts/agent_queue.py ready"
  queue_claim: "python scripts/agent_queue.py claim TASK_ID [ROLE]"
  queue_release: "python scripts/agent_queue.py release TASK_ID"
  queue_complete: "python scripts/agent_queue.py complete TASK_ID"
  queue_block: "python scripts/agent_queue.py block TASK_ID \"REASON\""
  queue_status: "python scripts/agent_queue.py status"
  gates:
    - "python scripts/check_deps.py"
    - "python scripts/check_hotpath_lints.py"
    - "python scripts/check_decoupling.py"
    - "python scripts/check_determinism.py"
  cargo: "cargo fmt --check ; cargo clippy --workspace --all-targets -- -D warnings ; cargo test --workspace"
  ts: "pnpm -r check ; pnpm -r test"
---
