# docs/work

Plan workspaces for the `.agents` planning pipeline
(SSOT: `.agents/skills/autonomous-pipeline/SKILL.md`).

On pipeline entry, the directory `.agents/skills/autonomous-pipeline/template`
is cloned into `docs/work/{plan_slug}` (at most 3 active plan workspaces):

- `{plan_slug}/msg/` - inter-role artifacts: directive, scout package, approvals,
  escalations (YAML, 100% ASCII English)
- `{plan_slug}/plan/` - confirmed plan artifacts (YAML)
- `{plan_slug}/task/` - generated task cards (Markdown; deleted after the memory commit)

Rules:

- all artifacts are 100% ASCII English (`.agents/language/core.md` whitelist)
- pipeline scope units never overlap in-flight `.agents/queue` cards
  (`scope_fence_invariants` in `AGENTS.md`)
- roadmap task state stays in `spec/dsl/03_roadmap.dsl`; this folder never
  carries progress state

This folder starts empty (migration 2026-10-07).
