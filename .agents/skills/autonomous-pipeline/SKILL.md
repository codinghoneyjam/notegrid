---
name: autonomous-pipeline
description: Exploration-first development pipeline for NoteGrid with formal FSM automaton routing, mathematical invariants, and adaptive Fast-Track bypass. Commands remapped to NoteGrid tooling (cargo/pnpm/check_*.py/agent_queue.py).
---

# SKILL_ROUTER: AUTONOMOUS_PIPELINE (FORMAL FSM SPECIFICATION, NOTEGRID EDITION)
---
schema_version: "3.0.0"
spec_type: "PIPELINE_AUTOMATON_AND_ROUTER"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
reference_ontology: ".agents/language/core.md"
---
 
## COEXISTENCE WITH THE DISPATCH LAYER (SSOT: .agents/dispatch_readme.md)
- `agents/` (root folder) = task EXECUTION dispatch for roadmap tasks: STATE lines,
  `scripts/agent_queue.py`, one card per in-flight task, ORCHESTRATOR.md / WORKER.md.
- `.agents/` (this tree) = constitution, language, memory, rules, and THIS planning pipeline
  for work that is not yet a roadmap task.
- Boundary rule: the two systems never claim the same scope units at the same time
  (scope_fence_invariants.parallel_disjoint in AGENTS.md). A planned slice that touches a
  unit owned by an active queue card waits, or is cancelled in favor of the queue card.
- The roadmap (`spec/dsl/03_roadmap.dsl`) stays the single source of truth for roadmap task
  STATE. This pipeline never edits STATE lines; promotion of planned work into the roadmap is
  an orchestrator/owner action.
 
pipeline_invariants:
  directory_limits:
    template_source: ".agents/skills/autonomous-pipeline/template/{msg, plan, task}"
    active_instances_max: 3
    instance_pattern: "docs/work/{plan_slug}/{msg, plan, task}"
    clone_protocol: "Copy directory '.agents/skills/autonomous-pipeline/template' into 'docs/work/{plan_slug}' upon pipeline entry"
    blocked_task_record: "Blocked tasks are recorded in {plan_slug}/msg/"
    memory_commit_mandatory: "All code-modifying workflows MUST conclude with a memory commit: append the task entry to .agents/memory/short_term/page_01.md (procedure SSOT: .agents/memory/README.md)"
    memory_commit_summary_limit: "The <= 29 line limit applies EXCLUSIVELY to the memory entry (the Implementer authoring budget of <= 20 lines is within it). Task card specs have NO 29-line limit."
  line_limits:
    ref: ".agents/language/code_metrics.md"
  channel_invariants:
    messenger_ssot: "docs/work/{plan_slug}/msg/ is the sole inter-role artifact channel."
    file_naming: "{role}-{seq}-{slug}.yaml"
    language_whitelist_axiom: "All DSL artifacts MUST satisfy Whitelist_ASCII (regex in .agents/language/core.md). Non-ASCII strictly forbidden."
  handoff_invariants:
    rule: "No role can invoke peers. A subagent HALT is a dispatch trigger for the Orchestrator."
    required_block: |
      ## Handoff
      - **Wrote**: <artifact_path>
      - **Next**: <role_name> - <role_file_path>

deterministic_lexer_algebra:
  alphabet: "Tokenize(Input) is intent-level: Korean or English synonyms of a listed token count as a match."
  ascii_note: "Romanized entries exist only to satisfy Whitelist_ASCII; user chat input stays Korean."
  lexicons:
    T_plan: {"plan", "roadmap", "blueprint", "architect", "design", "pipeline", "gyehoek", "peullaen", "seolgye"}
    T_accept: {"accept", "approve", "proceed", "go-ahead", "confirm", "execute", "seungin", "jinhaeng"}
    T_reject: {"reject", "revise", "modify", "cancel", "sujeong", "dasi"}
  predicates:
    P_lex: "P_lex(Input, T) <=> (Tokenize(Input) INTERSECT T) != EMPTY_SET"
    P_plan: "P_plan(Input, Gamma) <=> P_lex(Input, T_plan) AND (Gamma.N_files > 0 OR Gamma.C_cross OR Gamma.D_contract OR SlashCommand('autonomous-pipeline'))"
    P_accept: "P_accept(Input) <=> P_lex(Input, T_accept)"
    P_reject: "P_reject(Input) <=> P_lex(Input, T_reject)"

formal_fsm_automaton:
  states:
    S_TRIAGE: "Initial triage by Orchestrator based on metric vector Gamma and lexer token matching"
    S_M0: "Terminal state: read-only Q&A in chat (pure inquiry, N_files == 0, zero work folder creation)"
    S_M1: "Direct bounded edit: light plan -> edit -> V_GATE -> memory commit (<= 3 files)"
    S_DIRECTIVE: "Question-only directive by 00_planner_directive (no plan, no source reading)"
    S_SCOUT: "Exploration and evidence gathering by 01_scout"
    S_SCOUT_APPROVAL: "Human approval gate for Tier 3 / Mode A global scan (awaiting explicit confirmation)"
    S_PLAN: "Architectural plan formulation by 02_planner_plan under docs/work/{plan_slug}/plan/"
    S_GATE: "Human approval gate (awaiting explicit human confirmation)"
    S_TASK: "Task slicing by 03_task_maker under docs/work/{plan_slug}/task/"
    S_EXEC: "Bounded implementation and self-healing loop by 04_implementer"
    S_DONE: "Terminal state: memory entry committed (.agents/memory), task card deleted, {plan_slug} closed"
    S_BLOCKED: "Terminal state: escalation triggered, block package written to {plan_slug}/msg/. Resume = new S_TRIAGE entry carrying the user guidance."
 
  initial_state: "S_TRIAGE"
  terminal_states: ["S_M0", "S_DONE", "S_BLOCKED"]
 
  metric_vector:
    N_files: "Integer (count of touched files)"
    C_cross: "Boolean (crosses a declared dependency edge)"
    D_contract: "Boolean (mutates a public API, DTO schema, or a C0..C12 contract)"
    U_uncertain: "Boolean (requires exploration or missing symbol lookup)"
 
  derived_predicates:
    exec_pass: "Predicate_Pass(V_GATE) AND test_exit_code == 0"
 
  # Guards leaving the same state MUST be mutually exclusive. Evaluate top to bottom; first match wins.
  transition_matrix:
    # --- TRIAGE ---
    - from: "S_TRIAGE"
      to: "S_DIRECTIVE"
      guard: "P_plan(Input, Gamma) OR Gamma.U_uncertain OR Gamma.N_files > 3 OR Gamma.C_cross OR Gamma.D_contract"
      action: "ChatOutput(Input) == EMPTY_SET AND CloneTemplate({plan_slug}) AND Dispatch(00_planner_directive)"
 
    - from: "S_TRIAGE"
      to: "S_M0"
      guard: "Gamma.N_files == 0 AND NOT P_plan(Input, Gamma) AND NOT Gamma.U_uncertain AND NOT Gamma.C_cross AND NOT Gamma.D_contract"
      action: "Orchestrator answers directly in chat. Zero subagents, zero directory creation."
 
    - from: "S_TRIAGE"
      to: "S_M1"
      guard: "Gamma.N_files >= 1 AND Gamma.N_files <= 3 AND NOT Gamma.C_cross AND NOT Gamma.D_contract AND NOT Gamma.U_uncertain AND NOT P_plan(Input, Gamma)"
      action: "CloneTemplate({plan_slug}) -> author light plan -> edit source -> V_GATE -> memory commit -> S_DONE."
 
    # --- DIRECTIVE ---
    - from: "S_DIRECTIVE"
      to: "S_SCOUT"
      guard: "directive.status == 'WRITTEN'"
      action: "Dispatch 01_scout with the directive artifact path."
 
    - from: "S_DIRECTIVE"
      to: "S_BLOCKED"
      guard: "directive.status == 'AMBIGUOUS'"
      action: "Write {plan_slug}/msg/00_planner_directive-*_ambiguous.yaml and halt."
 
    # --- SCOUT ---
    - from: "S_SCOUT"
      to: "S_SCOUT_APPROVAL"
      guard: "scout.approval_file_written == 1"
      action: "Orchestrator surfaces {plan_slug}/msg/01_scout-*_approval.yaml to the user and awaits approval."
 
    - from: "S_SCOUT"
      to: "S_BLOCKED"
      guard: "scout.approval_file_written == 0 AND scout.feasibility == 0"
      action: "Write {plan_slug}/msg/01_scout-*_blocked.yaml and halt."
 
    - from: "S_SCOUT"
      to: "S_EXEC"
      guard: "scout.approval_file_written == 0 AND scout.feasibility == 1 AND scout.fast_track_eligible == true"
      action: "Fast-Track (M1_5): Orchestrator authors a light task card in {plan_slug}/task/ and dispatches 04_implementer. 02_planner_plan and 03_task_maker are bypassed."
 
    - from: "S_SCOUT"
      to: "S_PLAN"
      guard: "scout.approval_file_written == 0 AND scout.feasibility == 1 AND scout.fast_track_eligible == false"
      action: "Dispatch 02_planner_plan to author the Confirmed Plan under {plan_slug}/plan/."
 
    - from: "S_SCOUT_APPROVAL"
      to: "S_SCOUT"
      guard: "P_accept(Input) AND NOT P_reject(Input)"
      action: "Re-dispatch 01_scout with Mode A approval recorded."
 
    - from: "S_SCOUT_APPROVAL"
      to: "S_BLOCKED"
      guard: "P_reject(Input)"
      action: "Record the rejection in {plan_slug}/msg/ and halt."
 
    # --- PLAN & HUMAN GATE ---
    - from: "S_PLAN"
      to: "S_GATE"
      guard: "plan.status == 'PENDING'"
      action: "Orchestrator surfaces the plan summary to the user and awaits approval."
 
    - from: "S_PLAN"
      to: "S_BLOCKED"
      guard: "plan.status == 'INSUFFICIENT'"
      action: "02_planner_plan wrote {plan_slug}/msg/02_planner_plan-*_insufficient.yaml. Halt."
 
    - from: "S_GATE"
      to: "S_EXEC"
      guard: "P_accept(Input) AND NOT P_reject(Input) AND user_feedback == null AND plan.slices == 1"
      action: "ChatOutput(Input) == EMPTY_SET AND Orchestrator authors a single task card in {plan_slug}/task/ and dispatches 04_implementer."
 
    - from: "S_GATE"
      to: "S_TASK"
      guard: "P_accept(Input) AND NOT P_reject(Input) AND user_feedback == null AND plan.slices >= 2"
      action: "ChatOutput(Input) == EMPTY_SET AND Dispatch 03_task_maker to partition slices into bounded task cards."
 
    - from: "S_GATE"
      to: "S_PLAN"
      guard: "P_reject(Input) OR user_feedback != null"
      action: "Loopback to 02_planner_plan with user revision constraints."
 
    - from: "S_GATE"
      to: "S_GATE"
      guard: "NOT P_accept(Input) AND NOT P_reject(Input) AND user_feedback == null"
      action: "Orchestrator re-prompts for explicit approval or revision."
 
    # --- TASK MAKER ---
    - from: "S_TASK"
      to: "S_EXEC"
      guard: "task_queue != EMPTY_SET AND task_maker.status != 'BLOCKED'"
      action: "Sequentially dispatch 04_implementer per task card in topological order."
 
    - from: "S_TASK"
      to: "S_BLOCKED"
      guard: "task_maker.status == 'BLOCKED'"
      action: "03_task_maker wrote {plan_slug}/msg/03_task_maker-*_blocked.yaml. Halt."
 
    # --- EXECUTION & HEALING LOOP (heal_counter resets to 0 per task card) ---
    - from: "S_EXEC"
      to: "S_EXEC"
      guard: "exec_pass AND task_queue != EMPTY_SET"
      action: "Memory commit the current task (procedure: .agents/memory/README.md); dispatch 04_implementer for the next task; heal_counter := 0."
 
    - from: "S_EXEC"
      to: "S_DONE"
      guard: "exec_pass AND task_queue == EMPTY_SET"
      action: "Memory commit (procedure: .agents/memory/README.md), delete the task card, conclude."
 
    - from: "S_EXEC"
      to: "S_EXEC"
      guard: "NOT exec_pass AND implementer.status != 'INFEASIBLE' AND heal_counter < 3"
      action: "Self-heal loop iteration (heal_counter += 1)."
 
    - from: "S_EXEC"
      to: "S_BLOCKED"
      guard: "NOT exec_pass AND implementer.status != 'INFEASIBLE' AND heal_counter >= 3"
      action: "Write {plan_slug}/msg/04_implementer-*_failed.yaml and raise escalation."
 
    - from: "S_EXEC"
      to: "S_BLOCKED"
      guard: "implementer.status == 'INFEASIBLE'"
      action: "Write {plan_slug}/msg/04_implementer-*_blocked.yaml and raise escalation."
 
scout_scale_matrix:
  ref: ".agents/skills/autonomous-pipeline/roles/01_scout.md"  # scale_decision_matrix (SSOT)
 
orchestrator_commandments:
  - "DISPATCH_CONFINEMENT: P_plan(Input, Gamma) IMPLIES (ChatOutput(Orchestrator) == EMPTY_SET AND Dispatch(00_planner_directive))"
  - "SUBAGENT_TYPE_INVARIANT: FORALL role IN {00_dir, 01_scout, 02_plan, 03_task, 04_impl}: dispatch as a write-capable subagent; read-only research subagents lack write tools and violate the artifact generation contract"
  - "ROLE_FILE_INVARIANT: MUST pass the role file path (.agents/skills/autonomous-pipeline/roles/XX.md) in the subagent prompt; NEVER copy text sections."
  - "MANDATORY_FIRST_ACTION_ENFORCEMENT: Subagent prompt MUST prepend: 'MANDATORY STEP 1: read the role contract file <role_contract_file> before ANY other action. Strict adherence to invariants is mandatory.'"
  - "CARDINALITY_INVARIANT: MUST assign exactly ONE role per subagent invocation."
  - "TRIGGER_INVARIANT: MUST treat a subagent halt as a dispatch trigger; inspect the handoff block and advance the FSM transition."
  - "SCOPE_FENCE: MUST NOT edit source files under crates/ or packages/ in M2/M3 (delegate to the Implementer)."
  - "GIT_SAFETY: MUST NOT restore deleted files via git checkout (deletion is deliberate intent)."
  - "ASCII_AXIOM: MUST enforce Whitelist_ASCII (see .agents/language/core.md) on all generated pipeline DSL artifacts."
  - "MEMORY_COMMIT_MANDATORY: MUST execute the memory commit procedure (.agents/memory/README.md) upon every task completion."
  - "COEXISTENCE: MUST keep pipeline scope units disjoint from in-flight ..agents/queue cards (AGENTS.md scope_fence_invariants)."
