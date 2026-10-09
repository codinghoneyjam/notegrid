#!/usr/bin/env python3
# <META - FILE SUMMARY - Unit tests for the task-state checker (scripts/check_task_state.py).>
"""Run with: python scripts/test_check_task_state.py

The point of these tests is the drift cases, not the happy path: a checker that only proves it can
say "OK" is not protecting anything.
"""

import tempfile
import unittest
from pathlib import Path

import check_task_state as target

VALID = {"ORIENT", "TEST_FIRST", "IMPLEMENT", "VERIFY", "DONE", "BLOCKED", "NOT_STARTED"}


def roadmap(*tasks: str) -> str:
    return "\n".join(tasks) + "\n"


def task(tid: str, *, state: str | None = None, scope: str = "", done_when: str = "gate:g01") -> str:
    lines = [f'TASK {tid} "task {tid}"']
    if state:
        lines.append(f"  STATE {state}")
    if scope:
        lines.append(f"  SCOPE {{ {scope} }}")
    if done_when:
        lines.append(f"  DONE WHEN {{ {done_when} }}")
    return "\n".join(lines)


class ParseTest(unittest.TestCase):
    def parse(self, text: str) -> list[dict]:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "03_roadmap.dsl"
            path.write_text(text, encoding="utf-8")
            return target.parse_tasks(path)

    def test_parses_id_title_state_scope_and_done_when(self) -> None:
        tasks = self.parse(
            roadmap(
                "TASK T1.1 \"tempo\"",
                "  STATE DONE",
                "  SCOPE { ROOT/crates/ng-timeline/** }",
                "  DONE WHEN { ac:R02.AC1 ; gate:g12 }",
            )
        )
        self.assertEqual(len(tasks), 1)
        self.assertEqual(tasks[0]["id"], "T1.1")
        self.assertEqual(tasks[0]["title"], "tempo")
        self.assertEqual(tasks[0]["state"], "DONE")
        self.assertIn("ng-timeline", tasks[0]["scope"])
        self.assertIn("gate:g12", tasks[0]["done_when"])

    def test_task_without_state_parses_as_none(self) -> None:
        tasks = self.parse(roadmap(task("T2.1")))
        self.assertIsNone(tasks[0]["state"])

    def test_multiple_tasks_do_not_bleed_into_each_other(self) -> None:
        tasks = self.parse(
            roadmap(
                task("T1.1", state="DONE", scope="ROOT/crates/ng-a/**", done_when="gate:g01"),
                task("T1.2", state="ORIENT", scope="ROOT/crates/ng-b/**", done_when="gate:g02"),
            )
        )
        self.assertEqual([t["state"] for t in tasks], ["DONE", "ORIENT"])
        self.assertIn("ng-a", tasks[0]["scope"])
        self.assertNotIn("ng-b", tasks[0]["scope"])

    def test_duplicate_task_is_reported(self) -> None:
        tasks = self.parse(
            roadmap(
                task("T1.1", state="DONE", scope="ROOT/crates/ng-a/**"),
                task("T1.1", state="ORIENT", scope="ROOT/crates/ng-b/**"),
            )
        )
        errors = target.check(tasks, {"ng-a", "ng-b"}, VALID)
        self.assertTrue(any("declared twice" in e for e in errors))

    def test_empty_roadmap_raises(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "03_roadmap.dsl"
            path.write_text("## PHASE P2\nnothing here\n", encoding="utf-8")
            with self.assertRaises(target.SpecError):
                target.parse_tasks(path)


class StateValidityTest(unittest.TestCase):
    def check_one(self, **kwargs) -> list[str]:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "03_roadmap.dsl"
            path.write_text(roadmap(task("T1.1", **kwargs)), encoding="utf-8")
            tasks = target.parse_tasks(path)
        return target.check(tasks, {"ng-timeline", "ng-score"}, VALID)

    def test_unknown_state_is_rejected(self) -> None:
        errors = self.check_one(state="FINISHED", scope="ROOT/crates/ng-timeline/**")
        self.assertTrue(any("not a TASK_FLOW state" in e for e in errors), errors)

    def test_done_with_missing_crate_is_rejected(self) -> None:
        # The T1.1..T1.8 failure: DONE recorded while the crate does not exist.
        errors = self.check_one(state="DONE", scope="ROOT/crates/ng-render/**")
        self.assertTrue(any("not in the workspace" in e for e in errors), errors)

    def test_done_with_empty_done_when_is_rejected(self) -> None:
        errors = self.check_one(state="DONE", scope="ROOT/crates/ng-timeline/**", done_when="")
        self.assertTrue(any("DONE WHEN is empty" in e for e in errors), errors)

    def test_done_with_existing_crate_passes(self) -> None:
        errors = self.check_one(state="DONE", scope="ROOT/crates/ng-timeline/**")
        self.assertEqual(errors, [])

    def test_done_with_missing_package_is_rejected(self) -> None:
        # T1.7's case: `packages/playback` extracts to the bare unit
        # `playback`, which the name-prefix rule does not catch. The path
        # it was written with must still be checked.
        errors = self.check_one(state="DONE", scope="ROOT/packages/playback/**")
        self.assertTrue(any("not in the workspace" in e for e in errors), errors)

    def test_done_with_existing_package_passes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "03_roadmap.dsl"
            path.write_text(
                roadmap(task("T1.7", state="DONE", scope="ROOT/packages/abi/**")),
                encoding="utf-8",
            )
            tasks = target.parse_tasks(path)
        self.assertEqual(target.check(tasks, {"abi"}, VALID), [])

    def test_no_state_is_not_an_error(self) -> None:
        errors = self.check_one(scope="ROOT/crates/ng-timeline/**")
        self.assertEqual(errors, [])

    def test_blocked_without_reason_is_rejected(self) -> None:
        tasks = [
            {
                "id": "T1.1",
                "title": "t",
                "state": "BLOCKED",
                "scope": "",
                "done_when": "",
                "blocked_reason": "",
            }
        ]
        errors = target.check(tasks, set(), VALID)
        self.assertTrue(any("BLOCKED without a reason" in e for e in errors), errors)

    def test_blocked_reason_is_read_from_a_comment(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "03_roadmap.dsl"
            path.write_text(
                roadmap(
                    "TASK T1.1 \"t\"",
                    "  STATE BLOCKED",
                    "  # BLOCKED C3 does not define the note count limit",
                    "  DONE WHEN { gate:g01 }",
                ),
                encoding="utf-8",
            )
            tasks = target.parse_tasks(path)
        self.assertIn("C3 does not define", tasks[0]["blocked_reason"])
        self.assertEqual(target.check(tasks, set(), VALID), [])


class OrderingTest(unittest.TestCase):
    def check_phase(self, *states: str) -> list[str]:
        tasks = [
            {
                "id": f"T1.{i + 1}",
                "title": "t",
                "state": state,
                "scope": "ROOT/crates/ng-a/**",
                "done_when": "gate:g01",
            }
            for i, state in enumerate(states)
        ]
        return target.check(tasks, {"ng-a"}, VALID)

    def test_later_done_while_earlier_unfinished_is_rejected(self) -> None:
        errors = self.check_phase("ORIENT", "DONE")
        self.assertTrue(any("cannot finish before" in e for e in errors), errors)

    def test_blocked_earlier_does_not_block_a_later_done(self) -> None:
        # A blocked task carries its reason in the spec body, which parse_tasks exposes as
        # blocked_reason; the ordering rule must read it without demanding a second one.
        tasks = [
            {"id": "T1.1", "title": "t", "state": "BLOCKED", "scope": "", "done_when": "",
             "blocked_reason": "needs C3 clarification"},
            {"id": "T1.2", "title": "t", "state": "DONE", "scope": "", "done_when": "gate:g01"},
        ]
        self.assertEqual(target.check(tasks, set(), VALID), [])

    def test_sequential_progress_is_fine(self) -> None:
        self.assertEqual(self.check_phase("DONE", "DONE", "ORIENT"), [])

    def test_tasks_in_different_phases_are_independent(self) -> None:
        tasks = [
            {"id": "T0.9", "title": "t", "state": "DONE", "scope": "", "done_when": "gate:g01"},
            {"id": "T1.1", "title": "t", "state": "ORIENT", "scope": "", "done_when": "gate:g01"},
        ]
        self.assertEqual(target.check(tasks, set(), VALID), [])


class ExtractTest(unittest.TestCase):
    def test_gates_and_acs_are_found(self) -> None:
        done = "ac:R05.AC1 ; gate:g05 ; ac:R04.R04 ; gate:g12"
        self.assertEqual(target.gates_in(done), ["g05", "g12"])
        self.assertEqual(target.acs_in(done), ["R05.AC1", "R04.R04"])

    def test_phase_derivation(self) -> None:
        self.assertEqual(target.phase_of("T0.3"), "P0")
        self.assertEqual(target.phase_of("T1.8"), "P1")
        self.assertEqual(target.phase_of("T4.2"), "P4")

    def test_namespaced_ts_package_matches_the_bare_spec_name(self) -> None:
        # The spec writes `packages/abi`, package.json says `@ng/abi`.
        members = target.workspace_members()
        self.assertIn("abi", members)
        self.assertIn("@ng/abi", members)


class RenderTest(unittest.TestCase):
    def test_board_is_deterministic(self) -> None:
        tasks = [
            {"id": "T0.1", "title": "a", "state": "DONE", "scope": "", "done_when": "gate:g01"},
            {"id": "T1.1", "title": "b", "state": "ORIENT", "scope": "", "done_when": ""},
        ]
        self.assertEqual(target.render(tasks), target.render(tasks))

    def test_board_says_do_not_hand_edit(self) -> None:
        tasks = [{"id": "T0.1", "title": "a", "state": "DONE", "scope": "", "done_when": ""}]
        self.assertIn("Do not hand-edit", target.render(tasks))

    def test_board_groups_by_phase(self) -> None:
        tasks = [
            {"id": "T0.1", "title": "a", "state": "DONE", "scope": "", "done_when": ""},
            {"id": "T1.1", "title": "b", "state": "ORIENT", "scope": "", "done_when": ""},
        ]
        board = target.render(tasks)
        self.assertIn("## P0", board)
        self.assertIn("## P1", board)


if __name__ == "__main__":
    unittest.main(verbosity=2)
