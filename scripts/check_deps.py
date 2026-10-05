#!/usr/bin/env python3
# <META - FILE SUMMARY - Tooling script; see docstring below.>
"""g03: cargo metadata graph == union of MODULE.DEPENDS_ON; extern "C" only in crates/ng-wasm."""
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
META = json.loads(
    subprocess.run(
        ["cargo", "metadata", "--format-version", "1"],
        cwd=ROOT, capture_output=True, encoding="utf-8", errors="replace", check=True,
    ).stdout
)

ALLOWED = {
    "ng-core": set(),
    "ng-score": {"ng-core"},
    "ng-wasm": {"ng-core", "ng-score"},
    "ng-cli": {"ng-core", "ng-score"},
    "xtask": set(),
    "ng-spike-sp2": set(),
}

errors = []
workspace_names = set(ALLOWED)
for pkg in META["packages"]:
    name = pkg["name"]
    if name not in workspace_names:
        continue
    actual = {
        dep["name"]
        for dep in pkg["dependencies"]
        if dep["name"] in workspace_names
    }
    # normalise - vs _
    actual = {a.replace("_", "-") for a in actual}
    if actual != ALLOWED[name]:
        errors.append(f"{name}: deps {sorted(actual)} != allowed {sorted(ALLOWED[name])}")

# extern "C" / unsafe isolation check (only crates/* scanned; spikes/ and scripts/ exempt as throwaway)
for rs in (ROOT / "crates").rglob("*.rs"):
    crate = rs.parts[rs.parts.index("crates") + 1]
    text = rs.read_text(encoding="utf-8", errors="replace")
    if crate != "ng-wasm":
        if 'extern "C"' in text:
            errors.append(f"{rs.relative_to(ROOT)}: extern \"C\" outside ng-wasm")
        if re.search(r"\bunsafe\b", text):
            errors.append(f"{rs.relative_to(ROOT)}: unsafe outside ng-wasm")

if errors:
    print("g03 FAILED:")
    for e in errors:
        print("  ", e)
    sys.exit(1)
print("g03 OK")