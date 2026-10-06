#!/usr/bin/env python3
# <META - FILE SUMMARY - Unit tests for the g01 hotpath lint verifier (scripts/check_hotpath_lints.py).>
"""Run with: python scripts/test_check_hotpath_lints.py"""

import tempfile
import unittest
from pathlib import Path

import check_hotpath_lints as target


class SilencedLintTest(unittest.TestCase):
    def scan(self, body: str) -> list[str]:
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / "crates" / "ng-dsp" / "src"
            src.mkdir(parents=True)
            (src / "part.rs").write_text(body, encoding="utf-8")
            target.ROOT = Path(tmp)
            try:
                return target.silenced_lints("ng-dsp")
            finally:
                target.ROOT = ORIGINAL_ROOT

    def test_plain_allow_is_not_a_weakening(self) -> None:
        self.assertEqual(self.scan("// #[allow(dead_code)]\nfn f() {}\n"), [])

    def test_unrelated_clippy_allow_is_fine(self) -> None:
        self.assertEqual(self.scan("#[allow(clippy::needless_range_loop)]\nfn f() {}\n"), [])

    def test_each_denied_lint_is_caught(self) -> None:
        for name in target.REQUIRED_LINTS:
            with self.subTest(lint=name):
                found = self.scan(f"#[allow(clippy::{name})]\nfn f() {{}}\n")
                self.assertEqual(len(found), 1, f"clippy::{name} not detected")
                self.assertIn(name, found[0])

    def test_scoped_allow_is_also_caught(self) -> None:
        found = self.scan("#[allow(clippy::unwrap_used)]\nmod inner {}\n")
        self.assertEqual(len(found), 1)

    def test_allow_with_other_lints_lists_is_caught(self) -> None:
        found = self.scan("#[allow(dead_code, clippy::panic)]\nfn f() {}\n")
        self.assertEqual(len(found), 1)


class CrateOptInTest(unittest.TestCase):
    def write_manifest(self, body: str) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            crate = Path(tmp) / "crates" / "ng-dsp"
            crate.mkdir(parents=True)
            (crate / "Cargo.toml").write_text(body, encoding="utf-8")
            target.ROOT = Path(tmp)
            try:
                self.result = target.crate_opts_in("ng-dsp")
            finally:
                target.ROOT = ORIGINAL_ROOT

    def test_workspace_true_opts_in(self) -> None:
        self.write_manifest('[package]\nname = "ng-dsp"\n\n[lints]\nworkspace = true\n')
        self.assertTrue(self.result)

    def test_workspace_false_does_not_opt_in(self) -> None:
        self.write_manifest('[package]\nname = "ng-dsp"\n\n[lints]\nworkspace = false\n')
        self.assertFalse(self.result)

    def test_absent_lints_section_does_not_opt_in(self) -> None:
        self.write_manifest('[package]\nname = "ng-dsp"\n')
        self.assertFalse(self.result)

    def test_missing_manifest_does_not_opt_in(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            target.ROOT = Path(tmp)
            try:
                self.assertFalse(target.crate_opts_in("ng-dsp"))
            finally:
                target.ROOT = ORIGINAL_ROOT


ORIGINAL_ROOT = target.ROOT


class WorkspaceLintTableTest(unittest.TestCase):
    def test_missing_block_raises(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
            target.CARGO_TOML = Path(tmp) / "Cargo.toml"
            try:
                with self.assertRaises(target.SpecError):
                    target.workspace_lints()
            finally:
                target.CARGO_TOML = ORIGINAL_CARGO

    def test_block_is_parsed(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "Cargo.toml").write_text(
                "[workspace.lints.clippy]\nunwrap_used = \"deny\"\nexpect_used = \"warn\"\n",
                encoding="utf-8",
            )
            target.CARGO_TOML = Path(tmp) / "Cargo.toml"
            try:
                self.assertEqual(
                    target.workspace_lints(), {"unwrap_used": "deny", "expect_used": "warn"}
                )
            finally:
                target.CARGO_TOML = ORIGINAL_CARGO

    def test_section_stops_at_the_next_table(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "Cargo.toml").write_text(
                "[workspace.lints.clippy]\nunwrap_used = \"deny\"\n\n[profile.release]\nlto = true\n",
                encoding="utf-8",
            )
            target.CARGO_TOML = Path(tmp) / "Cargo.toml"
            try:
                self.assertEqual(target.workspace_lints(), {"unwrap_used": "deny"})
            finally:
                target.CARGO_TOML = ORIGINAL_CARGO


ORIGINAL_CARGO = target.CARGO_TOML


if __name__ == "__main__":
    unittest.main(verbosity=2)
