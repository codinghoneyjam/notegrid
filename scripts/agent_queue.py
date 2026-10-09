#!/usr/bin/env python3
# <META - FILE SUMMARY - dispatch layer for parallel AI workers: claims roadmap tasks, keeps .agents/queue cards in sync with the spec.>
"""Dispatch roadmap tasks to parallel AI workers without a second source of truth.

Task progress lives in `STATE` lines in `spec/dsl/03_roadmap.dsl` and nowhere
else (see check_task_state.py). This script is the orchestration layer on top:

  ready              tasks dispatchable right now, each with its inferred worker
                     role -- parallel-safe by construction (phase in force, SCOPE
                     disjoint from every other in-flight task, READS that exist)
  claim TASK [ROLE]  write .agents/queue/active/TASK.md and set STATE ORIENT
  release TASK       undo a claim (STATE line removed = NOT_STARTED)
  complete TASK      archive the card to done/ and set STATE DONE -- only after
                     the card's checklist is fully checked and the DONE claim
                     validates against the workspace
  block TASK REASON  set STATE BLOCKED with the reason, park the card in blocked/
  status [--check]   cross-check every card against the spec (CI runs this)

Every mutation rewrites docs/tasks/board.md from the spec, so the generated
board can never drift from the STATE lines it comes from.
"""

import re
import sys
from datetime import date
from pathlib import Path

from check_task_state import (
    BOARD,
    ROADMAP,
    TASK_RE,
    check,
    flow_states,
    parse_tasks,
    phase_of,
    render,
    scope_units,
    workspace_members,
)
from paths import ROOT

QUEUE = ROOT / ".agents" / "queue"
ACTIVE = QUEUE / "active"
DONE_DIR = QUEUE / "done"
BLOCKED_DIR = QUEUE / "blocked"

# States a claimed card may be in while work is in flight.
FLOW_MID = ("ORIENT", "TEST_FIRST", "IMPLEMENT", "VERIFY")
ROLES = ("core-rust", "engine-ts", "app-ui", "verifier", "orchestrator")

FRONTMATTER_RE = re.compile(r"^---\n(.*?)\n---\n", re.DOTALL)

# Like check_task_state.FIELD_RE, but not anchored to the line start:
# the roadmap puts SCOPE and READS on the same line, and a mid-line
# field is still a field.
FIELD_ANY_RE = re.compile(r"(?P<key>[A-Z ]+?)\s*\{(?P<body>[^}]*)\}")


class QueueError(Exception):
    """A dispatch command cannot be carried out; the message is the whole answer."""


def workspace_dirs() -> tuple[set[str], set[str]]:
    """(crate names, package names) as bare words, from the live tree."""
    crates = {p.name for p in (ROOT / "crates").iterdir() if p.is_dir()}
    packages = {p.name for p in (ROOT / "packages").iterdir() if p.is_dir()}
    return crates, packages


def classify(unit: str, crates: set[str], packages: set[str]) -> str | None:
    """Which worker role owns a unit named in a SCOPE block."""
    if unit in crates:
        return "engine-ts" if unit == "ng-wasm" else "core-rust"
    if unit in packages:
        return "engine-ts" if unit in ("abi", "playback", "export-service") else "app-ui"
    if unit in ("docs", "scripts", "tools", "spec", "xtask", "spikes", "vendor", "fixture", "schema"):
        return "verifier"
    return None


def task_role(task: dict, crates: set[str], packages: set[str]) -> str:
    found = {classify(u, crates, packages) for u in scope_units(task["scope"])}
    found.discard(None)
    if len(found) == 1:
        return found.pop()
    if not found:
        return "verifier"  # docs/tools/spike work: whoever measures and records
    return "orchestrator"  # mixed scope: the orchestrator picks the role at claim time


def task_fields(tid: str) -> dict[str, str]:
    """Every `KEY { ... }` field of a TASK block, READS included."""
    text = ROADMAP.read_text(encoding="utf-8")
    for m in TASK_RE.finditer(text):
        if m.group("id") == tid:
            return {
                fm.group("key").strip(): fm.group("body").strip()
                for fm in FIELD_ANY_RE.finditer(m.group("body"))
            }
    raise QueueError(f"{tid}: no TASK block in {ROADMAP.relative_to(ROOT)}")


def read_units(fields: dict[str, str], known: set[str]) -> list[str]:
    """Workspace units named by READS -- the things a worker must be able to open."""
    return [tok for tok in read_tokens(fields) if tok in known]


def crate_deps() -> dict[str, set[str]]:
    """Workspace-internal dependency edges from each crate's Cargo.toml.

    P_independent (the parallel-worktree predicate) is checked against
    these edges; 01_system.dsl is the declared graph, this is the
    implemented one.
    """
    edges: dict[str, set[str]] = {}
    for manifest in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        name = manifest.parent.name
        text = manifest.read_text(encoding="utf-8", errors="replace")
        deps: set[str] = set()
        in_deps = False
        for line in text.splitlines():
            if line.startswith("["):
                header = line.strip()
                in_deps = header.startswith(("[dependencies", "[dev-dependencies"))
                if in_deps and "." in header:
                    dep = header.split(".", 1)[1].rstrip("]")
                    if dep:
                        deps.add(dep)
                continue
            if in_deps and line.strip() and not line.lstrip().startswith("#"):
                dep = line.split("=", 1)[0].strip()
                if dep:
                    deps.add(dep)
        edges[name] = deps
    return edges


def scope_crates(task: dict, crates: set[str]) -> set[str]:
    """The workspace crates a task's SCOPE names."""
    return {u for u in scope_units(task["scope"]) if u in crates}


def dep_edge(t1: dict, t2: dict, crates: set[str], deps: dict[str, set[str]]) -> list[str]:
    """Workspace-internal edges between two tasks' scopes."""
    found = set()
    for a in scope_crates(t1, crates):
        for b in scope_crates(t2, crates):
            if a != b and (b in deps.get(a, set()) or a in deps.get(b, set())):
                found.add(f"{a} <-> {b}")
    return sorted(found)


def phase_in_force(tasks: list[dict]) -> str | None:
    """The first phase with an unfinished task; later phases are not entered yet."""
    for phase in sorted({phase_of(t["id"]) for t in tasks}, key=lambda p: int(p[1:])):
        group = [t for t in tasks if phase_of(t["id"]) == phase]
        if any((t["state"] or "NOT_STARTED") not in ("DONE", "BLOCKED") for t in group):
            return phase
    return None


def earlier_in_phase(task: dict, tasks: list[dict]) -> list[dict]:
    pn = phase_of(task["id"])
    mine = [int(p) for p in task["id"].split(".")[1:]]
    return [
        t
        for t in tasks
        if t["id"] != task["id"]
        and phase_of(t["id"]) == pn
        and [int(p) for p in t["id"].split(".")[1:]] < mine
    ]


def load_cards(directory: Path) -> list[dict]:
    cards = []
    for path in sorted(directory.glob("*.md")):
        text = path.read_text(encoding="utf-8")
        cards.append({"file": path, "text": text, **parse_frontmatter(text)})
    return cards


def parse_frontmatter(text: str) -> dict[str, str]:
    """`key: value` lines between the leading `---` fences."""
    m = FRONTMATTER_RE.match(text)
    meta: dict[str, str] = {}
    if not m:
        return meta
    for line in m.group(1).splitlines():
        if ":" in line:
            key, value = line.split(":", 1)
            meta[key.strip()] = value.strip()
    return meta


def done_when_items(done_when: str) -> list[str]:
    return [i.strip() for i in done_when.split(";") if i.strip()]


def read_tokens(fields: dict[str, str]) -> list[str]:
    """READS entries as display tokens for the card."""
    reads = re.sub(r"\([^)]*\)", "", fields.get("READS", ""))
    return [t.strip().strip("`") for t in re.split(r"[;,]| and ", reads) if t.strip()]


def scope_entries(scope: str) -> list[str]:
    """Raw SCOPE entries, kept verbatim for the card's write fence."""
    return [e.strip() for e in scope.split(",") if e.strip()]


def card_checklist(text: str) -> list[tuple[bool, str]]:
    """The Done-when checkboxes of a card, in order."""
    m = re.search(r"^## Done when[^\n]*\n(.*?)(?=^## |\Z)", text, re.MULTILINE | re.DOTALL)
    out = []
    for line in (m.group(1) if m else "").splitlines():
        cm = re.match(r"^\s*- \[([ xX])\]\s+(.*?)\s*$", line)
        if cm:
            out.append((cm.group(1).lower() == "x", cm.group(2)))
    return out


def render_card(task: dict, role: str) -> str:
    tid = task["id"]
    fields = task_fields(tid)
    checks = "\n".join(f"- [ ] {i}" for i in done_when_items(task["done_when"])) or "- (no acceptance items listed)"
    reads = ", ".join(read_tokens(fields))
    writes = ", ".join(f'"{e}"' for e in scope_entries(fields.get("SCOPE", "")))
    return f"""---
task: {tid}
role: {role}
branch: agent/{tid}
worktree: ../notegrid-w-{tid}
state: ORIENT
claimed: {date.today().isoformat()}
scope_read: [{reads}]
scope_write: [{writes}]
---

# {tid} - {task['title']}

`state` mirrors `STATE` in `spec/dsl/03_roadmap.dsl`: the worker
moves it along TASK_FLOW, the orchestrator syncs the spec.
Never the reverse.

## Done when - copied from 03_roadmap.dsl, the only definition of finished

{checks}

## Evidence - every checked box above needs the command and its real output

- (empty)

## Notes

- (empty)

## Blocked - on a scope or contract gap: fill this in, then run
## `python scripts/agent_queue.py block {tid} "<reason>"`

- (empty)
"""


def set_state(tid: str, state: str | None, reason: str = "") -> None:
    """Write (state=None removes) the STATE line of a TASK block. Nothing else moves."""
    text = ROADMAP.read_text(encoding="utf-8")
    m = re.search(rf'^TASK\s+{re.escape(tid)}\s+"[^"]*"\s*$', text, re.MULTILINE)
    if not m:
        raise QueueError(f"{tid}: no TASK block in {ROADMAP.relative_to(ROOT)}")
    nxt = re.search(r"^(?:TASK |PHASE |## )", text[m.end():], re.MULTILINE)
    end = m.end() + (nxt.start() if nxt else len(text) - m.end())
    block = text[m.start():end]
    new_block = re.sub(r"^[ \t]*STATE\s+\w+[ \t]*\n", "", block, flags=re.MULTILINE)
    new_block = re.sub(r"^[ \t]*BLOCKED_REASON\s*\{[^}]*\}[ \t]*\n", "", new_block, flags=re.MULTILINE)
    if state is not None:
        if state == "BLOCKED":
            if not reason:
                raise QueueError("BLOCKED needs a reason; it becomes the BLOCKED_REASON in the spec")
            reason = reason.replace("{", "(").replace("}", ")")
        first_nl = new_block.index("\n")
        insert = f"\n  STATE {state}"
        if state == "BLOCKED":
            insert += f"\n  BLOCKED_REASON {{ {reason} }}"
        new_block = new_block[:first_nl] + insert + new_block[first_nl:]
    ROADMAP.write_text(text[: m.start()] + new_block + text[end:], encoding="utf-8")


def write_board() -> None:
    BOARD.write_text(render(parse_tasks(ROADMAP)), encoding="utf-8")


def cmd_ready() -> int:
    tasks = parse_tasks(ROADMAP)
    phase_now = phase_in_force(tasks)
    crates, packages = workspace_dirs()
    static_units = crates | packages
    try:
        live = workspace_members()
    except Exception as exc:  # no cargo: degrade, skip the READS-existence checks
        print(f"warning: cannot enumerate the workspace ({exc}); READS checks skipped\n")
        live = static_units
    active = load_cards(ACTIVE)
    active_units = {}
    for c in active:
        t = next((t for t in tasks if t["id"] == c["task"]), None)
        if t:
            active_units[c["task"]] = set(scope_units(t["scope"]))

    ready, waiting = [], []
    for task in tasks:
        tid = task["id"]
        state = task["state"] or "NOT_STARTED"
        if state in ("DONE", "BLOCKED"):
            continue
        reasons = []
        if state != "NOT_STARTED":
            reasons.append(f"in flight (STATE {state})")
        if phase_of(tid) != phase_now:
            reasons.append(f"phase {phase_of(tid)} not entered (exit of {phase_now} not met)")
        mine = set(scope_units(task["scope"]))
        for prev in earlier_in_phase(task, tasks):
            ps = prev["state"] or "NOT_STARTED"
            if ps in ("DONE", "BLOCKED"):
                continue
            overlap = mine & set(scope_units(prev["scope"]))
            if overlap:
                reasons.append(f"SCOPE overlaps {prev['id']} ({ps}): {', '.join(sorted(overlap))}")
        for owner, units in active_units.items():
            overlap = mine & units
            if overlap:
                reasons.append(f"claimed on card {owner}: {', '.join(sorted(overlap))}")
        for tok in read_units(task_fields(tid), static_units):
            if tok not in live:
                reasons.append(f"READS {tok}, which does not exist in the workspace yet")
        (ready if not reasons else waiting).append((task, reasons))

    print(
        f"phase in force: {phase_now or 'SHIPPED'}    board: "
        f"{sum(1 for t in tasks if t['state'] == 'DONE')}/{len(tasks)} DONE\n"
    )
    print("READY -- dispatch now (scopes are pairwise disjoint):")
    for task, _ in ready:
        print(f"  {task['id']}  {task['title']}  ->  role: {task_role(task, crates, packages)}")
    if not ready:
        print("  (nothing)")
    if len(ready) >= 2:
        edges = [
            (ready[i][0]["id"], ready[j][0]["id"], e)
            for i in range(len(ready))
            for j in range(i + 1, len(ready))
            for e in dep_edge(ready[i][0], ready[j][0], crates, crate_deps())
        ]
        print("\nparallel check (P_independent):")
        if edges:
            for a, b, e in edges:
                print(f"  {a} <-> {b}: dep edge {e} -- sequential mode, or a DECIDE entry for worktrees")
        else:
            print("  READY tasks are dependency-independent (worktree mode allowed)")
    print("\nWAITING -- with the reason:")
    for task, reasons in waiting:
        print(f"  {task['id']}  {task['title']}")
        for r in reasons:
            print(f"      - {r}")
    return 0


def cmd_claim(tid: str, role: str | None) -> int:
    tasks = parse_tasks(ROADMAP)
    by_id = {t["id"]: t for t in tasks}
    if tid not in by_id:
        raise QueueError(f"{tid}: no such task in 03_roadmap.dsl")
    task = by_id[tid]
    state = task["state"] or "NOT_STARTED"
    if state != "NOT_STARTED":
        raise QueueError(f"{tid}: STATE is {state}; only a NOT_STARTED task can be claimed")
    if (ACTIVE / f"{tid}.md").exists():
        raise QueueError(f"{tid}: a card already exists in .agents/queue/active/")
    crates, packages = workspace_dirs()
    chosen = role or task_role(task, crates, packages)
    if chosen not in ROLES:
        raise QueueError(f"unknown role {chosen!r}; one of: {', '.join(ROLES)}")
    mine = set(scope_units(task["scope"]))
    for c in load_cards(ACTIVE):
        other = by_id.get(c["task"])
        overlap = mine & set(scope_units(other["scope"])) if other else set()
        if overlap:
            raise QueueError(
                f"{tid}: SCOPE overlaps active card {c['task']} ({', '.join(sorted(overlap))}); "
                "parallel workers may not touch the same paths"
            )
    for c in load_cards(ACTIVE):
        other = by_id.get(c["task"])
        if other:
            for edge in dep_edge(task, other, crates, crate_deps()):
                print(
                    f"warning: {tid} <-> {c['task']} share dep edge {edge}; "
                    "parallel worktrees need P_independent or a DECIDE entry"
                )
    card = ACTIVE / f"{tid}.md"
    ACTIVE.mkdir(parents=True, exist_ok=True)
    card.write_text(render_card(task, chosen), encoding="utf-8")
    set_state(tid, "ORIENT")
    write_board()
    print(f"claimed {tid} for role {chosen}")
    print(f"  card:     .agents/queue/active/{tid}.md")
    print(f"  branch:   agent/{tid}")
    print(f"  worktree: ../notegrid-w-{tid}")
    print("  STATE set to ORIENT; docs/tasks/board.md regenerated")
    print(f"next: commit spec+board+card, then `git worktree add -b agent/{tid} ../notegrid-w-{tid}`")
    return 0


def cmd_release(tid: str) -> int:
    card = ACTIVE / f"{tid}.md"
    if not card.exists():
        raise QueueError(f"{tid}: no active card to release")
    card.unlink()
    set_state(tid, None)
    write_board()
    print(f"{tid}: released; STATE removed (NOT_STARTED), board regenerated")
    return 0


def cmd_complete(tid: str) -> int:
    card = ACTIVE / f"{tid}.md"
    if not card.exists():
        raise QueueError(f"{tid}: no active card to complete")
    checklist = card_checklist(card.read_text(encoding="utf-8"))
    unchecked = [item for checked, item in checklist if not checked]
    if unchecked:
        raise QueueError(
            f"{tid}: {len(unchecked)} acceptance item(s) still unchecked on the card:\n"
            + "\n".join(f"  - {i}" for i in unchecked)
        )
    tasks = parse_tasks(ROADMAP)
    by_id = {t["id"]: t for t in tasks}
    if tid not in by_id:
        raise QueueError(f"{tid}: no such task")
    by_id[tid]["state"] = "DONE"
    try:
        errors = [e for e in check(tasks, workspace_members(), flow_states()) if tid in e]
    except Exception as exc:
        print(f"warning: cannot validate the DONE claim against the workspace ({exc})")
        errors = []
    if errors:
        raise QueueError(
            f"{tid}: the DONE claim does not hold:\n" + "\n".join(f"  - {e}" for e in errors)
        )
    set_state(tid, "DONE")
    DONE_DIR.mkdir(parents=True, exist_ok=True)
    card.replace(DONE_DIR / f"{tid}.md")
    write_board()
    print(f"{tid}: STATE DONE, card archived to .agents/queue/done/{tid}.md, board regenerated")
    print(f"land it: merge branch agent/{tid}, then commit spec + board + card together")
    return 0


def cmd_block(tid: str, reason: str) -> int:
    card = ACTIVE / f"{tid}.md"
    if not card.exists():
        raise QueueError(f"{tid}: no active card; claim the task before blocking it")
    set_state(tid, "BLOCKED", reason)
    BLOCKED_DIR.mkdir(parents=True, exist_ok=True)
    card.replace(BLOCKED_DIR / f"{tid}.md")
    write_board()
    print(f"{tid}: STATE BLOCKED, card parked in .agents/queue/blocked/{tid}.md, board regenerated")
    print("add this entry to docs/tasks/BLOCKERS.md (newest first):\n")
    print(
        f"## {tid} - <one line>\n"
        f"BLOCKED since: {date.today().isoformat()}\n"
        f"Wanted:        <what the task needed>\n"
        f"Gap:          {reason}\n"
        f"Options:      <2-3 concrete ways out>\n"
        f"Decision:      <who decides, and by when>\n"
        f"Unblocks:      <what becomes possible once decided>"
    )
    return 0


def cmd_status() -> int:
    tasks = parse_tasks(ROADMAP)
    by_id = {t["id"]: t for t in tasks}
    phase_now = phase_in_force(tasks)
    problems: list[str] = []
    print(
        f"phase in force: {phase_now or 'SHIPPED'}    board: "
        f"{sum(1 for t in tasks if t['state'] == 'DONE')}/{len(tasks)} DONE\n"
    )

    active = load_cards(ACTIVE)
    print(f"active cards: {len(active)}")
    seen_units: dict[str, set[str]] = {}
    for c in active:
        tid = c["task"]
        task = by_id.get(tid)
        if task is None:
            problems.append(f"{c['file'].name}: task {tid} does not exist in the roadmap")
            continue
        spec_state = task["state"] or "NOT_STARTED"
        if spec_state not in FLOW_MID:
            problems.append(f"{tid}: card active but STATE is {spec_state}")
        elif c["state"] != spec_state:
            problems.append(f"{tid}: card state {c['state']} != spec STATE {spec_state}")
        want = [re.sub(r"\s+", " ", i) for i in done_when_items(task["done_when"])]
        got = [re.sub(r"\s+", " ", i) for _, i in card_checklist(c["text"])]
        if want != got:
            problems.append(f"{tid}: card checklist does not match DONE WHEN (edit the spec, re-claim)")
        units_ = set(scope_units(task["scope"]))
        for other, other_units in seen_units.items():
            overlap = units_ & other_units
            if overlap:
                problems.append(f"{tid}: SCOPE overlaps active card {other}: {', '.join(sorted(overlap))}")
        seen_units[tid] = units_
        print(f"  {tid}  role {c['role']}  branch {c['branch']}  card {c['state']} / spec {spec_state}")
    for directory, want_state in ((DONE_DIR, "DONE"), (BLOCKED_DIR, "BLOCKED")):
        for c in load_cards(directory):
            tid = c["task"]
            task = by_id.get(tid)
            if task is None:
                problems.append(f"{c['file'].name}: task {tid} does not exist in the roadmap")
            elif (task["state"] or "NOT_STARTED") != want_state:
                problems.append(
                    f"{tid}: card parked in {directory.name}/ but STATE is {task['state'] or 'NOT_STARTED'}"
                )
    if problems:
        print("\nproblems:")
        for p in problems:
            print(f"  - {p}")
        return 1
    print("\nqueue consistent with the spec")
    return 0


def main(argv: list[str]) -> int:
    cmd = argv[0] if argv else "status"
    try:
        if cmd == "ready":
            return cmd_ready()
        if cmd == "claim":
            if len(argv) < 2:
                raise QueueError("usage: agent_queue.py claim TASK_ID [ROLE]")
            return cmd_claim(argv[1], argv[2] if len(argv) > 2 else None)
        if cmd == "release":
            if len(argv) < 2:
                raise QueueError("usage: agent_queue.py release TASK_ID")
            return cmd_release(argv[1])
        if cmd == "complete":
            if len(argv) < 2:
                raise QueueError("usage: agent_queue.py complete TASK_ID")
            return cmd_complete(argv[1])
        if cmd == "block":
            if len(argv) < 3:
                raise QueueError('usage: agent_queue.py block TASK_ID "REASON"')
            return cmd_block(argv[1], " ".join(argv[2:]))
        if cmd == "status":
            return cmd_status()
        raise QueueError(
            f"unknown command {cmd!r}\n"
            "usage: agent_queue.py {ready|claim|release|complete|block|status}"
        )
    except QueueError as exc:
        print(f"agent_queue FAILED: {exc}")
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
