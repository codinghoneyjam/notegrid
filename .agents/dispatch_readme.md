# .agents/ - dispatch layer for parallel workers

Three layers, one truth:

| layer | what it is | where it lives |
|---|---|---|
| truth | task progress: `STATE` lines in `spec/dsl/03_roadmap.dsl`; the board is generated from it | `spec/`, `docs/tasks/` |
| dispatch | who works on what, in which worktree, with which evidence | `..agents/queue/` |
| support | language ontology, memory, rules, planning pipeline | `.agents/` |

Nothing in this folder is a second source of truth. Cards are
derived from the spec; `scripts/agent_queue.py` keeps the two in
sync and refuses to let them drift.

## Contracts

| artifact | what it is |
|---|---|
| `ORCHESTRATOR.md` | dispatch FSM automaton: survey -> claim -> isolate -> dispatch -> collect -> verify -> land |
| `WORKER.md` | worker role contract: TASK_FLOW discipline, evidence, scope fence |
| `workers/core-rust.md` | role contract for the Rust audio crates |
| `workers/engine-ts.md` | role contract for `ng-wasm` and the engine packages |
| `workers/app-ui.md` | role contract for the editor packages |
| `workers/verifier.md` | role contract for gates, fixtures, goldens, budgets |
| `queue/_template.md` | the shape of a task card (frontmatter + checklist) |
| `queue/active/` | cards for tasks in flight -- one git worktree each |
| `queue/done/` | archived cards of DONE tasks |
| `queue/blocked/` | parked cards; the reason lives in `docs/tasks/BLOCKERS.md` |

## Commands

```
python scripts/agent_queue.py ready                 # dispatchable now, and to whom
python scripts/agent_queue.py claim T1.7 engine-ts  # write a card, set STATE ORIENT
python scripts/agent_queue.py status                # cards vs spec consistency (CI runs this)
python scripts/agent_queue.py complete T1.7         # validate + archive + STATE DONE
python scripts/agent_queue.py block T1.7 "reason"   # STATE BLOCKED + park the card
python scripts/agent_queue.py release T1.7          # undo a claim
```

## How parallelism stays safe

A task is dispatchable only when:

1. its **phase is in force** (the roadmap PHASE FSM: a phase is
   entered only when the previous EXIT holds),
2. its **SCOPE paths are disjoint** from every other in-flight
   task (claimed cards included), and
3. everything it **READS already exists** in the workspace.

`ready` computes this; `claim` refuses overlapping claims;
`status` keeps cards honest against the spec. Overriding any of
these requires a `docs/tasks/DECISIONS.md` entry -- the same
discipline as every other exception in this repo.

## Entry point

`AGENTS.md` at the repo root. Every agent session -- human-driven
or orchestrated -- starts there.
