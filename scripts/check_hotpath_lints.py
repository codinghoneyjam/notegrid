#!/usr/bin/env python3
# <META - FILE SUMMARY - g01 support: every SET HOTPATH crate must opt into the workspace clippy deny list, and the deny list must exist.>
"""Verify the g01 hotpath lint clause is actually wired up.

`00_dsl.dsl` declares g01 as `cargo fmt --check ; cargo clippy -D warnings ; hotpath lints deny
{ unwrap_used, expect_used, indexing_slicing, panic }`. The third clause was declared but never
implemented, so the gate passed while protecting nothing. This check makes the declaration
falsifiable in three directions:

  1. the deny list is missing from the workspace Cargo.toml   -> fail
  2. a crate in SET HOTPATH does not opt in via [lints]       -> fail
  3. a HOTPATH crate silences one of the lints locally       -> fail

(3) is the interesting one. A local `#[allow(clippy::unwrap_used)]` in a hot path compiles
cleanly, so nothing else in the toolchain would notice. GLOBAL_DENY already forbids weakening a
gate to make it pass; this is the mechanical check for that.

A crate only needs to exist to be checked; crates the spec declares but the workspace has not built
yet are reported, not failed, since their task has not run.
"""

import re
import sys

from paths import ROOT, SPEC_DIR
from spec_deps import SpecError, global_sets

REQUIRED_LINTS = ("unwrap_used", "expect_used", "indexing_slicing", "panic")
CARGO_TOML = ROOT / "Cargo.toml"
# A local allow for any of these, scoped or not, is a weakening of the gate.
ALLOW_RE = re.compile(r"#\s*\[\s*allow\s*\([^)]*clippy::(\w+)")


def silenced_lints(crate: str) -> list[str]:
    """HOTPATH crates that locally allow one of the denied lints."""
    found = []
    for rs in (ROOT / "crates" / crate / "src").rglob("*.rs"):
        text = rs.read_text(encoding="utf-8", errors="replace")
        for name in ALLOW_RE.findall(text):
            if name in REQUIRED_LINTS:
                found.append(f"{rs.relative_to(ROOT)} allows clippy::{name}")
    return found


def workspace_lints() -> dict[str, str]:
    if not CARGO_TOML.exists():
        raise SpecError(f"{CARGO_TOML} not found")
    text = CARGO_TOML.read_text(encoding="utf-8")
    m = re.search(r"^\[workspace\.lints\.clippy\]\s*$(.*?)(?=^\[|\Z)", text, re.MULTILINE | re.DOTALL)
    if not m:
        raise SpecError("Cargo.toml has no [workspace.lints.clippy] block")
    found = dict(re.findall(r"^\s*(\w+)\s*=\s*\"(\w+)\"", m.group(1), re.MULTILINE))
    return found


def crate_opts_in(crate: str) -> bool:
    """True if crates/<crate>/Cargo.toml declares `[lints]` with workspace = true."""
    manifest = ROOT / "crates" / crate / "Cargo.toml"
    if not manifest.exists():
        return False
    text = manifest.read_text(encoding="utf-8")
    m = re.search(r"^\[lints\]\s*$(.*?)(?=^\[|\Z)", text, re.MULTILINE | re.DOTALL)
    if not m:
        return False
    return re.search(r"^\s*workspace\s*=\s*true", m.group(1), re.MULTILINE) is not None


def main() -> int:
    try:
        hotpath = sorted(global_sets(SPEC_DIR)["HOTPATH"])
        lints = workspace_lints()
    except (SpecError, KeyError) as exc:
        print("g01 FAILED: hotpath lint declaration cannot be verified")
        print("   ", exc)
        return 1

    errors = []
    for name in REQUIRED_LINTS:
        if name not in lints:
            errors.append(f"Cargo.toml: [workspace.lints.clippy] is missing '{name}'")
        elif lints[name] != "deny":
            errors.append(f"Cargo.toml: '{name}' is '{lints[name]}', g01 requires 'deny'")

    pending = []
    for crate in hotpath:
        if not crate_opts_in(crate):
            if (ROOT / "crates" / crate / "Cargo.toml").exists():
                errors.append(
                    f"{crate}: in SET HOTPATH but its Cargo.toml has no `[lints] workspace = true` "
                    f"-- its code would not be denied unwrap/expect/indexing/panic"
                )
            else:
                pending.append(crate)
            continue
        errors.extend(silenced_lints(crate))

    if errors:
        print("g01 FAILED:")
        for e in errors:
            print("  ", e)
        return 1

    detail = f"{len(hotpath) - len(pending)}/{len(hotpath)} HOTPATH crates opted in"
    if pending:
        detail += f"; not built yet: {', '.join(pending)}"
    print(f"g01 hotpath lints OK ({detail}; deny {', '.join(REQUIRED_LINTS)})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
