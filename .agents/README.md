# .agents/ - constitution support layer (NoteGrid)

Three layers, one truth (SSOT: `AGENTS.md`):

| layer | what it is | where it lives |
|---|---|---|
| truth | behaviour and task progress: spec DSL, `STATE` lines | `spec/dsl/`, `docs/tasks/` |
| dispatch | who works on what: queue cards, orchestrator / worker contracts | `.agents/` (this folder) |
| support | language ontology, memory, rules, skills, templates | `.agents/` (this folder) |

Nothing in this folder is a second source of truth. The roadmap
(`spec/dsl/03_roadmap.dsl`) stays the only record of task progress; this layer
never writes `STATE` lines or `docs/tasks/board.md`.

## Contents

| artifact | what it is |
|---|---|
| `language/core.md` | ontology root: universe sets, whitelist, module manifest |
| `language/operators.md` | set theory and logic operators used by every spec |
| `language/architecture.md` | crate / package topology and composition vocabulary |
| `language/toolchain.md` | command registry (TOOL_MAP) and V_GATE |
| `language/code_metrics.md` | line bounds, typing axioms, META header convention |
| `language/task_slicing.md` | slicing budget and domain tokens |
| `memory/` | short-term journal (page_01..10) and long-term domain memory |
| `rules/basic.md` | invariant checklist: quality, evidence, scope fences, runtime traps |
| `rules/*audit*.md` | audit rule scaffolds (fill before enabling) |
| `skills/autonomous-pipeline/` | planning pipeline: directive -> scout -> plan -> task -> implement |
| `skills/autonomous-pipeline/template/` | artifact stubs ({msg, plan, task}) cloned into `docs/work/{plan_slug}/` on pipeline entry |
| `skills/codebase-analysis/` | Rust/TS codebase analysis tooling and workflows |
| `templates/task_spec_template.md` | the shape of a pipeline task card |
| `hooks.json` + `hooks/` | read-gate enforcement for hosts with hook support: MUST-read check + f_modify scope check |
| `permissions.yaml` | trusted-commands reference (allow / ask / deny) |

## How the pipeline coexists with dispatch

`.agents/queue` executes roadmap tasks (STATE, queue cards). The `.agents` planning
pipeline handles work that is **not yet** in the roadmap: it scouts, plans, and
slices, then executes bounded task cards. Boundary rule: the two never claim the
same scope units at the same time (`scope_fence_invariants` in `AGENTS.md`);
promotion of planned work into the roadmap is an owner / orchestrator action.

## Migration note (2026-10-07)

Transplanted from the "Your wor(l)d" game repository, then edited to fit
NoteGrid rules:

- `language/`, `rules/`, `skills/autonomous-pipeline/`, `templates/` were
  rewritten for NoteGrid; commands were remapped to real NoteGrid tooling
  (`cargo`, `pnpm`, `scripts/check_*.py`, `scripts/agent_queue.py`). No
  `dev.tools` facade exists here; the remap table is in
  `language/toolchain.md`.
- Memory started EMPTY: the game task history was not carried over. The memory
  procedure lives in `memory/README.md`.
- Game-only assets (Godot skills, game domain memories, game audits) were
  removed entirely; NoteGrid has no Godot dependency.
- History: the initial byte-identical copy was verified (45 files, SHA-256);
  a follow-up pass folded in the upstream deltas that landed during the
  migration (read-gate hooks, pipeline templates, role refinements), remapped
  to NoteGrid; this folder is the edited NoteGrid edition.
