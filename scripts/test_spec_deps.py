#!/usr/bin/env python3
# <META - FILE SUMMARY - Unit tests for the spec DSL parser behind g03 (scripts/spec_deps.py).>
"""Run with: python scripts/test_spec_deps.py  (or via `pnpm`/CI; no third-party deps)."""

import tempfile
import unittest
from pathlib import Path

import spec_deps
from spec_deps import ALL_MARKER, SpecError, global_sets, parse_system


def write_module(root: Path, name: str, body: str) -> None:
    d = root / "module"
    d.mkdir(parents=True, exist_ok=True)
    (d / name).write_text(body, encoding="utf-8")


class ParserTest(unittest.TestCase):
    def parse(self, modules: dict[str, str]) -> dict[str, set[str]]:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for name, body in modules.items():
                write_module(root, name, body)
            return spec_deps.resolve(spec_deps.parse(root))

    # --- the two DEPENDS_ON shapes that appear in the real spec ---

    def test_positional_means_every_crates_entry(self) -> None:
        got = self.parse(
            {
                "R02_timeline.dsl": """MODULE R02 timeline
  LANG rust ; PHASE P1 ; CRATES { ng-timeline }
  DEPENDS_ON { ng-core, ng-score } ; DEPENDED_BY { ng-render }
END
""",
            }
        )
        self.assertEqual(got, {"ng-timeline": {"ng-core", "ng-score"}})

    def test_keyed_assigns_per_crate(self) -> None:
        got = self.parse(
            {
                "R01_core_score.dsl": """MODULE R01 core_score
  LANG rust ; PHASE P0 ; CRATES { ng-core, ng-score }
  DEPENDS_ON { ng-core: {} ; ng-score: { ng-core } }
END
""",
            }
        )
        self.assertEqual(got, {"ng-core": set(), "ng-score": {"ng-core"}})

    def test_keyed_and_positional_mixed_in_one_module(self) -> None:
        got = self.parse(
            {
                "R04_synth_dsp.dsl": """MODULE R04 synth_dsp
  LANG rust ; CRATES { ng-dsp, ng-synth }
  DEPENDS_ON { ng-dsp: { ng-core } ; ng-synth: { ng-core, ng-dsp, ng-bank } }
END
""",
            }
        )
        self.assertEqual(got, {"ng-dsp": {"ng-core"}, "ng-synth": {"ng-core", "ng-dsp", "ng-bank"}})

    def test_all_crates_expands_to_every_other_crate(self) -> None:
        got = self.parse(
            {
                "R01_core.dsl": "MODULE R01 c\n  CRATES { ng-core }\n  DEPENDS_ON { }\nEND\n",
                "R02_timeline.dsl": "MODULE R02 t\n  CRATES { ng-timeline }\n  DEPENDS_ON { ng-core }\nEND\n",
                "R07_wasm_abi.dsl": "MODULE R07 w\n  CRATES { ng-wasm }\n  DEPENDS_ON { ng-wasm: ALL crates }\nEND\n",
            }
        )
        self.assertEqual(got["ng-wasm"], {"ng-timeline", "ng-core"})

    def test_all_crates_excludes_other_assembly_crates(self) -> None:
        """ng-cli composes every crate but must not depend on ng-wasm, and the reverse."""
        graph = {
            "ng-core": set(),
            "ng-timeline": {"ng-core"},
            "ng-wasm": {ALL_MARKER},
            "ng-cli": {ALL_MARKER},
        }
        got = spec_deps.resolve(graph, assembly={"ng-wasm", "ng-cli"})
        self.assertEqual(got["ng-wasm"], {"ng-core", "ng-timeline"})
        self.assertEqual(got["ng-cli"], {"ng-core", "ng-timeline"})

    def test_all_crates_resolves_only_against_crates_that_exist(self) -> None:
        """A crate not yet in the workspace must not be demanded as a dependency."""
        graph = {"ng-core": set(), "ng-wasm": {ALL_MARKER}}
        got = spec_deps.resolve(graph, existing={"ng-core", "ng-wasm"})
        self.assertEqual(got["ng-wasm"], {"ng-core"})

    def test_annotations_are_stripped(self) -> None:
        got = self.parse(
            {
                "T02_ui_grid.dsl": """MODULE T02 ui_grid
  LANG ts ; CRATES { ui-grid }
  DEPENDS_ON { document(readonly) } FORBIDDEN { editing, playback }
END
""",
            }
        )
        self.assertEqual(got, {"ui-grid": {"document"}})

    def test_packages_do_not_become_crates(self) -> None:
        got = self.parse(
            {
                "T01_editing.dsl": """MODULE T01 editing
  LANG ts ; PACKAGES { document, editing }
  DEPENDS_ON { document: { abi(types) } ; editing: { document } }
END
""",
            }
        )
        self.assertEqual(got, {})  # TS units are not cargo workspace members

    def test_cross_cutting_module_is_skipped(self) -> None:
        got = self.parse(
            {
                "X01_build_quality.dsl": """MODULE X01 build_quality
  LANG rust+ts+scripts ; PHASE P0 ; applies to ALL modules
  REQUIRE something
END
""",
            }
        )
        self.assertEqual(got, {})

    def test_determinism_is_stable_across_runs(self) -> None:
        modules = {
            "R01_core_score.dsl": "MODULE R01 c\n  CRATES { ng-core, ng-score }\n  DEPENDS_ON { ng-core: {} ; ng-score: { ng-core } }\nEND\n",
            "R02_timeline.dsl": "MODULE R02 t\n  CRATES { ng-timeline }\n  DEPENDS_ON { ng-core, ng-score }\nEND\n",
        }
        self.assertEqual(self.parse(modules), self.parse(modules))

    # --- failures must be loud, never a silent skip ---

    def test_missing_module_dir_raises(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(spec_deps.SpecError):
                spec_deps.parse(Path(tmp))

    def test_module_without_units_raises(self) -> None:
        with self.assertRaises(spec_deps.SpecError):
            self.parse({"X99.dsl": "MODULE X99 x\n  PHASE P9\nEND\n"})

    def test_module_without_depends_on_raises(self) -> None:
        with self.assertRaises(spec_deps.SpecError):
            self.parse({"R99_x.dsl": "MODULE R99 x\n  CRATES { ng-nope }\nEND\n"})

    def test_unbalanced_braces_raise(self) -> None:
        with self.assertRaises(spec_deps.SpecError):
            self.parse({"R98_x.dsl": "MODULE R98 x\n  CRATES { ng-a\n  DEPENDS_ON { ng-core }\nEND\n"})

    def test_unknown_dep_is_reported(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_module(root, "R02_timeline.dsl", "MODULE R02 t\n  CRATES { ng-timeline }\n  DEPENDS_ON { ng-core, ng-typo }\nEND\n")
            graph = spec_deps.resolve(spec_deps.parse(root))
            self.assertEqual(spec_deps.unknown_deps(graph, {"ng-timeline", "ng-core"}), ["ng-timeline -> ng-typo"])


class RealSpecTest(unittest.TestCase):
    """The committed spec must parse cleanly and cover the workspace."""

    def test_real_spec_parses_and_has_no_unknown_deps(self) -> None:
        from paths import SPEC_DIR

        graph, units, assembly = spec_deps.combined(SPEC_DIR)
        self.assertTrue(graph, "spec produced no crates")
        self.assertEqual(spec_deps.unknown_deps(spec_deps.resolve(graph), units), [])

    def test_set_crates_matches_the_crate_lines(self) -> None:
        """SET CRATES and the ARCHITECTURE CRATE lines must cover the same members."""
        from paths import SPEC_DIR

        graph, _ = parse_system(SPEC_DIR)
        self.assertEqual(set(graph), global_sets(SPEC_DIR)["CRATES"])

    def test_documented_crates_are_present(self) -> None:
        """If this fails, 01_system.dsl changed and the gate needs re-reading."""
        from paths import SPEC_DIR

        graph, _, _ = spec_deps.combined(SPEC_DIR)
        for name in ("ng-core", "ng-score", "ng-timeline", "ng-wasm", "ng-cli"):
            self.assertIn(name, graph)

    def test_workspace_crates_all_appear_in_the_spec(self) -> None:
        """The failure mode this replaces: a workspace member absent from ALLOWED was skipped silently."""
        from paths import CARGO_BIN, ROOT, SPEC_DIR
        import json
        import subprocess

        graph, _, _ = spec_deps.combined(SPEC_DIR)
        meta = json.loads(
            subprocess.run(
                [CARGO_BIN, "metadata", "--format-version", "1"],
                cwd=ROOT, capture_output=True, encoding="utf-8", check=True,
            ).stdout
        )
        members = {p["name"] for p in meta["packages"] if p["id"] in meta["workspace_members"]}
        missing = members - set(graph) - set(spec_deps.TOOLING)
        self.assertEqual(missing, set(), f"workspace members absent from the spec: {sorted(missing)}")


if __name__ == "__main__":
    unittest.main(verbosity=2)
