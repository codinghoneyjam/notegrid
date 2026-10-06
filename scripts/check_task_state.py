#!/usr/bin/env python3
# <META - FILE SUMMARY - g14a: task STATE in 03_roadmap.dsl is valid, consistent with the workspace, and docs/tasks/board.md matches it.>
"""Render and verify task progress.

`STATE` inside a TASK block in `spec/dsl/03_roadmap.dsl` is the single source of truth for where
a task stands. This script does three things:

  1. render  -- write `docs/tasks/board.md` from the spec (the human-facing view)
  2. check   -- validate every STATE, and cross-check the ones that claim completion
  3. --check -- verify `board.md` is up to date, so a stale board fails CI

The cross-check exists because this repo has been wrong about its own progress twice: g01 declared a
hotpath lint table that was never implemented, and TASK_PLAN_P1.md recorded T1.1..T1.8 as DONE while
none of their crates existed. A progress file that nobody can falsify is worse than none.

Status vocabulary is TASK_FLOW from 00_dsl.dsl, plus NOT_STARTED:
  NOT_STARTED  nobody has entered the task
  ORIENT      contracts and READS read, nothing written yet
  TEST_FIRST  failing tests written
  IMPLEMENT   production code being written
  VERIFY      implementation done, running the phase-active gates
  DONE        every item in DONE WHEN holds
  BLOCKED     a scope or contract gap; the task is stopped and reported (see BLOCKERS.md)
"""

import json
import re
import subprocess
import sys
from pathlib import Path

from paths import CARGO_BIN, ROOT, SPEC_DIR

ROADMAP = SPEC_DIR / "03_roadmap.dsl"
FLOW_FILE = SPEC_DIR / "00_dsl.dsl"
TASKS_DIR = ROOT / "docs" / "tasks"
BOARD = TASKS_DIR / "board.md"

# NOT_STARTED is ours; the rest come from the FSM in 00_dsl.dsl.
EXTRA_STATES = {"NOT_STARTED"}
STATE_ORDER = ["NOT_STARTED", "ORIENT", "TEST_FIRST", "IMPLEMENT", "VERIFY", "DONE", "BLOCKED"]

TASK_RE = re.compile(
    r'^TASK\s+(?P<id>T[\d.]+)\s+"(?P<title>[^"]*)"\s*$(?P<body>(?:\n(?!TASK |PHASE |## ).*)*)',
    re.MULTILINE,
)
FIELD_RE = re.compile(r"^\s*(?P<key>[A-Z ]+?)\s*\{(?P<body>[^}]*)\}", re.MULTILINE)


class SpecError(Exception):
    """The spec could not be read; the checker must fail rather than guess."""


def flow_states() -> set[str]:
    """The state list declared by the TASK_FLOW FSM, plus NOT_STARTED."""
    if not FLOW_FILE.exists():
        raise SpecError(f"{FLOW_FILE} not found")
    text = FLOW_FILE.read_text(encoding="utf-8")
    m = re.search(r"FSM\s+TASK_FLOW\s*\n\s*STATE\s+([^\n]+)", text)
    if not m:
        raise SpecError("00_dsl.dsl: no `FSM TASK_FLOW` with a STATE line")
    declared = {s.strip() for s in m.group(1).split(",") if s.strip()}
    return declared | EXTRA_STATES


def parse_tasks(path: Path) -> list[dict]:
    """Every TASK block with its title, STATE, SCOPE and DONE WHEN."""
    if not path.exists():
        raise SpecError(f"{path} not found")
    text = path.read_text(encoding="utf-8")

    tasks = []
    for m in TASK_RE.finditer(text):
        body = m.group("body")
        state = None
        sm = re.search(r"^\s*STATE\s+(\w+)\s*$", body, re.MULTILINE)
        if sm:
            state = sm.group(1)
        fields = {}
        for fm in FIELD_RE.finditer(body):
            fields[fm.group("key").strip()] = fm.group("body").strip()
        # A BLOCKED task states its reason in the body, either as `BLOCKED_REASON {...}` or as the
        # first sentence after the STATE line. Both are read here so the checker can require one.
        reason = fields.get("BLOCKED_REASON", "")
        if not reason and state == "BLOCKED":
            rm = re.search(r"^\s*#\s*BLOCKED\s+(.+?)\s*$", body, re.MULTILINE)
            reason = rm.group(1) if rm else ""
        tasks.append(
            {
                "id": m.group("id"),
                "title": m.group("title"),
                "state": state,
                "scope": fields.get("SCOPE", ""),
                "done_when": fields.get("DONE WHEN", ""),
                "blocked_reason": reason,
            }
        )
    if not tasks:
        raise SpecError(f"{path.name}: no TASK blocks found")
    return tasks


def phase_of(task_id: str) -> str:
    """T1.3 -> P1, so the board can group by phase."""
    parts = task_id.split(".")
    return "P" + parts[0][1:] if len(parts) > 1 else "?"


def gates_in(done_when: str) -> list[str]:
    """gate:gNN references named by DONE WHEN."""
    return re.findall(r"gate\s*:\s*(g\d\d\w*)", done_when)


def acs_in(done_when: str) -> list[str]:
    """ac:R02.AC1 style references named by DONE WHEN."""
    return re.findall(r"ac\s*:\s*([A-Z]\d\d\.[A-Z]+\d+)", done_when)


def workspace_members() -> set[str]:
    """Crate and package names cargo knows about, plus the TS workspace packages."""
    members = set()
    meta = subprocess.run(
        [CARGO_BIN, "metadata", "--format-version", "1"],
        cwd=ROOT, capture_output=True, encoding="utf-8", errors="replace", check=True,
    ).stdout
    for pkg in json.loads(meta)["packages"]:
        if pkg["id"] in json.loads(meta)["workspace_members"]:
            members.add(pkg["name"])
    for manifest in (ROOT / "packages").glob("*/package.json"):
        try:
            name = json.loads(manifest.read_text(encoding="utf-8"))["name"]
        except (json.JSONDecodeError, KeyError):
            continue
        # TS packages are namespaced (@ng/abi) while the spec writes them bare (abi), so register
        # both spellings rather than making every SCOPE carry the @.
        members.add(name)
        members.add(name.split("/")[-1])
    return members


def scope_units(scope: str) -> list[str]:
    """Unit names referenced by a SCOPE block: crates/ng-x, packages/y, docs/..., etc."""
    found = re.findall(r"(?:ROOT|REPO):?(?:[\w./*-]+/)*?([\w-]+)(?:/\*\*)?", scope)
    return [f for f in found if f and f not in ("ROOT", "REPO")]


def check(tasks: list[dict], members: set[str], valid_states: set[str]) -> list[str]:
    """Errors: a STATE that cannot be true, or that contradicts the workspace."""
    errors = []

    seen = set()
    for task in tasks:
        tid = task["id"]
        if tid in seen:
            errors.append(f"{tid}: declared twice in the roadmap")
        seen.add(tid)

        if task["state"] is None:
            # A task with no STATE has not been triaged. Not an error -- T2..T6 are all unstarted
            # and writing NOT_STARTED on 40 lines would be noise nobody maintains.
            continue

        if task["state"] not in valid_states:
            errors.append(f"{tid}: STATE {task['state']!r} is not a TASK_FLOW state")
            continue

        if task["state"] == "DONE":
            # The falsifiable part. A DONE task whose SCOPE names a crate that does not exist is
            # exactly the T1.1..T1.8 failure this script was written to prevent.
            for unit in scope_units(task["scope"]):
                if unit.startswith(("ng-", "abi", "packages/")) and unit not in members:
                    errors.append(
                        f"{tid}: STATE DONE but its SCOPE names '{unit}', which is not in the "
                        f"workspace (a DONE claim must be backed by code)"
                    )
            if not task["done_when"]:
                errors.append(f"{tid}: STATE DONE but DONE WHEN is empty, so nothing was verified")

        if task["state"] == "BLOCKED" and not task["blocked_reason"]:
            errors.append(
                f"{tid}: STATE BLOCKED without a reason; add `BLOCKED_REASON {{ ... }}` and an "
                f"entry in docs/tasks/BLOCKERS.md"
            )

    # Ordering: a task cannot be DONE while an earlier task in the same phase is not, unless the
    # earlier one is explicitly BLOCKED. This catches "finished T1.4 but T1.3 never started".
    by_phase: dict[str, list[dict]] = {}
    for task in tasks:
        by_phase.setdefault(phase_of(task["id"]), []).append(task)
    for phase, group in by_phase.items():
        group.sort(key=lambda t: [int(p) for p in t["id"].split(".")[1:]])
        first_unfinished = None
        for task in group:
            state = task["state"] or "NOT_STARTED"
            if state == "DONE":
                if first_unfinished and first_unfinished != task["id"]:
                    errors.append(
                        f"{phase}: {task['id']} is DONE but {first_unfinished} is not "
                        f"(a later task cannot finish before an earlier one)"
                    )
            elif state != "BLOCKED":
                first_unfinished = first_unfinished or task["id"]
    return errors


def render(tasks: list[dict]) -> str:
    """The human-facing board. Generated; edit the spec, not this file."""
    by_phase: dict[str, list[dict]] = {}
    for task in tasks:
        by_phase.setdefault(phase_of(task["id"]), []).append(task)

    counts = {s: 0 for s in STATE_ORDER}
    for task in tasks:
        counts[task["state"] or "NOT_STARTED"] += 1

    lines = [
        "# Task board",
        "",
        "Generated by `scripts/check_task_state.py` from `STATE` lines in `spec/dsl/03_roadmap.dsl`.",
        "Edit the spec, then run `python scripts/check_task_state.py` to regenerate. Do not hand-edit.",
        "",
        "## Summary",
        "",
        "| state | tasks |",
        "|---|---|",
    ]
    for state in STATE_ORDER:
        if counts[state]:
            lines.append(f"| {state} | {counts[state]} |")
    lines += ["", f"Total: {len(tasks)} tasks.", ""]

    for phase in sorted(by_phase, key=lambda p: (p == "?", int(p[1:]) if p[1:].isdigit() else 0)):
        group = sorted(by_phase[phase], key=lambda t: [int(p) for p in t["id"].split(".")[1:]])
        lines += [f"## {phase}", "", "| task | state | gates | done when |", "|---|---|---|---|"]
        for task in group:
            state = task["state"] or "NOT_STARTED"
            gates = ", ".join(gates_in(task["done_when"])) or "-"
            done = re.sub(r"\s+", " ", task["done_when"]) or "-"
            lines.append(f"| {task['id']} {task['title']} | **{state}** | {gates} | {done} |")
        lines.append("")

    return "\n".join(lines)


def main(argv: list[str]) -> int:
    check_only = "--check" in argv
    try:
        tasks = parse_tasks(ROADMAP)
        valid_states = flow_states()
    except SpecError as exc:
        print("task state check FAILED: cannot read the spec")
        print("   ", exc)
        return 1

    errors = check(tasks, workspace_members(), valid_states)

    rendered = render(tasks)
    if check_only:
        if not BOARD.exists():
            errors.append(f"{BOARD.relative_to(ROOT)} does not exist; run the script without --check")
        elif BOARD.read_text(encoding="utf-8") != rendered:
            errors.append(
                f"{BOARD.relative_to(ROOT)} is out of date; "
                f"run `python scripts/check_task_state.py` and commit the result"
            )
    else:
        TASKS_DIR.mkdir(parents=True, exist_ok=True)
        BOARD.write_text(rendered, encoding="utf-8")

    if errors:
        print("task state check FAILED:")
        for e in errors:
            print("  ", e)
        return 1

    done = sum(1 for t in tasks if t["state"] == "DONE")
    print(f"task state OK ({done}/{len(tasks)} DONE, {len(tasks)} tasks from the roadmap)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
