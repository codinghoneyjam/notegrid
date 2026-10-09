# ORCHESTRATOR CONTRACT (NoteGrid)
---
schema_version: "1.0.0"
spec_type: "ORCHESTRATOR_ROLE"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
canonical_constitution: "AGENTS.md"
---

## 1. ROLE

The orchestrator is the sole writer of:
- `spec/dsl/03_roadmap.dsl` STATE lines
- `docs/tasks/*.md` (board, decisions, blockers)
- `.agents/queue/**` (task cards)

The orchestrator NEVER writes product code. It dispatches, collects, verifies, and lands.

## 2. DISPATCH FSM

```
SURVEY -> CLAIM -> ISOLATE -> DISPATCH -> COLLECT -> VERIFY -> LAND
```

### 2.1 SURVEY
- Run `python scripts/agent_queue.py ready`
- Identify dispatchable tasks (phase in force, scope disjoint, reads exist)
- Check `docs/tasks/BLOCKERS.md` for blockers

### 2.2 CLAIM
- Run `python scripts/agent_queue.py claim <TASK_ID> <ROLE>`
- Write task card to `.agents/queue/active/<TASK_ID>.md`
- Set STATE to ORIENT in `spec/dsl/03_roadmap.dsl`

### 2.3 ISOLATE
- Create git worktree: `git worktree add .agents/worktrees/<TASK_ID> -b agent/<TASK_ID>`
- Assign exactly ONE worker role per task

### 2.4 DISPATCH
- Pass the role contract file path in the subagent prompt
- NEVER copy text sections; reference the file
- Worker owns exactly one card and one worktree

### 2.5 COLLECT
- Worker completes task, writes evidence
- Orchestrator reviews evidence against DONE WHEN

### 2.6 VERIFY
- Re-run all gates named in DONE WHEN
- Run `python scripts/check_task_state.py --check`
- Verify scope fence: `FORALL f IN FilesWritten(worker): f IN SCOPE(card(worker))`

### 2.7 LAND
- Merge worktree: `git merge agent/<TASK_ID>`
- Remove worktree: `git worktree remove .agents/worktrees/<TASK_ID>`
- Archive card: move to `.agents/queue/done/<TASK_ID>.md`
- Set STATE to DONE in `spec/dsl/03_roadmap.dsl`
- Regenerate board: `python scripts/check_task_state.py`

## 3. SELF-HEAL LIMIT

Max 3 self-heal attempts per task. After 3 failures:
- Set STATE to BLOCKED
- Write `docs/tasks/BLOCKERS.md` entry
- Park card in `.agents/queue/blocked/`

## 4. ZERO FABRICATION

Never substitute a guess for a fact. A contract gap means:
- BLOCKED + BLOCKERS.md entry
- NOT a silent workaround

## 5. COEXISTENCE

Pipeline scope units never overlap in-flight queue cards.
Roadmap tasks execute under `.agents/queue`.
Work not yet in the roadmap goes through `.agents/skills/autonomous-pipeline/`.

## 6. COMMANDS

```powershell
# Survey
python scripts/agent_queue.py ready

# Claim
python scripts/agent_queue.py claim T1.7 engine-ts

# Status
python scripts/agent_queue.py status

# Complete
python scripts/agent_queue.py complete T1.7

# Block
python scripts/agent_queue.py block T1.7 "reason"

# Release
python scripts/agent_queue.py release T1.7
```

## 7. REFERENCES
- AGENTS.md (root)
- .agents/dispatch_readme.md
- .agents/WORKER.md
- .agents/workers/*.md
- spec/dsl/00_dsl.dsl (TASK_FLOW, PHASE FSM)
