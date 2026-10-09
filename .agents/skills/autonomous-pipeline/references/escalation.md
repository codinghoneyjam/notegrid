# Escalation Protocol (Shared by Every Role, NOTEGRID EDITION)

Any role that cannot proceed without guessing MUST stop immediately.
Never fabricate a fix, never silently reinterpret a spec, and never narrow a contract to make it fit.

A fabricated-but-well-typed implementation passes cargo test, clippy, and the gates alike.
Fabrication is the one failure that quality gates cannot catch, which is why the constitution
forbids it outright.

## When to Escalate

You MUST escalate when:
- A plan assumption is contradicted by real code in the repository.
- A required reusable API turns out not to exist or has an incompatible signature.
- Preconditions conflict with what visible callers require.
- A user request is ambiguous enough that even a goal sentence cannot be written.
- The spec package (`spec/dsl/`) and the memory files contradict each other unresolvedly.
- An implementation contract cannot be satisfied within the function line bound
  (`.agents/language/code_metrics.md`).
- A spec, fixture, or premise handed to you turns out to be defective.

## Where to Write

All escalations MUST be written to `docs/work/{plan_slug}/msg/` in 100% ASCII-English YAML DSL:

| Role | Escalation Artifact |
| :--- | :--- |
| 00 Planner (Directive) | `docs/work/{plan_slug}/msg/00_planner_directive-{seq}-{slug}_ambiguous.yaml` |
| 01 Scout (Surveyor) | `docs/work/{plan_slug}/msg/01_scout-{seq}-{slug}_blocked.yaml` |
| 01 Scout (Approval) | `docs/work/{plan_slug}/msg/01_scout-{seq}-{slug}_approval.yaml` (Tier 3 / Mode A) |
| 02 Planner (Plan) | `docs/work/{plan_slug}/msg/02_planner_plan-{seq}-{slug}_insufficient.yaml` |
| 03 Task Maker | `docs/work/{plan_slug}/msg/03_task_maker-{seq}-{slug}_blocked.yaml` |
| 04 Implementer (Infeasible) | `docs/work/{plan_slug}/msg/04_implementer-{seq}-{slug}_blocked.yaml` |
| 04 Implementer (Self-Heal Failed) | `docs/work/{plan_slug}/msg/04_implementer-{seq}-{slug}_failed.yaml` |

Every escalation file MUST state its role, sequential tag, and slug in the filename.

## Required DSL Schema (100% English)

```yaml
escalation:
  role: "{role_id}"
  type: "BLOCKED" # BLOCKED | APPROVAL | INSUFFICIENT | FAILED
  slug: "{slug}"
  expected: "{what_the_contract_assumed}"
  found:
    evidence: "{what_actually_exists}"
    citations:
      - "path/to/file:line"
  why_blocked: "{concrete_explanation_in_english}"
  what_was_tried:
    - "{attempt_1_log}"
```

## Behavioral Rules

- Escalations MUST be written in 100% English. Non-English (Korean) characters are strictly forbidden in DSL artifacts.
- Escalations MUST be directed to the Orchestrator, never sideways to peer roles.
- You MUST conclude your turn with the standardized `## Handoff` block referencing the escalation file.
- When Role 04 Implementer escalates, it MUST record the blocking package in
  `docs/work/{plan_slug}/msg/`.
- The Orchestrator MUST treat an escalation as a pause gate: halt the current task, present the escalation
  file to the user, and resume only after explicit user guidance. Independent tasks MAY continue.
- Escalation is not failure. Guessing is the failure.
