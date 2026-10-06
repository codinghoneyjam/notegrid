#!/usr/bin/env python3
# <META - FILE SUMMARY - g03: cargo metadata graph == the graph the spec declares; extern "C"/unsafe only in ng-wasm.>
"""g03: the allowed dependency graph comes from the spec, not from a hand-kept list.

The graph is derived from two spec sources that must agree (see spec_deps.py):
`01_system.dsl` ARCHITECTURE CRATE lines and `module/*.dsl` DEPENDS_ON. A workspace member
that neither source mentions is a FAILURE, not a skip -- the previous hardcoded table
skipped unknown crates silently, so adding one quietly removed it from inspection.
"""

import json
import re
import subprocess
import sys

from paths import CARGO_BIN, ROOT, CRATES_DIR, SPEC_DIR
from spec_deps import SpecError, TOOLING, combined, missing_from, resolve

try:
    ALLOWED_RAW, UNITS, ASSEMBLY = combined(SPEC_DIR)
except SpecError as exc:
    print("g03 FAILED: cannot read the dependency graph from the spec")
    print("   ", exc)
    sys.exit(1)

META = json.loads(
    subprocess.run(
        [CARGO_BIN, "metadata", "--format-version", "1"],
        cwd=ROOT, capture_output=True, encoding="utf-8", errors="replace", check=True,
    ).stdout
)

errors = []

workspace_members = {
    pkg["name"] for pkg in META["packages"] if pkg["id"] in META["workspace_members"]
}

# `ALL crates` resolves against what actually exists, so a crate that does not exist yet
# cannot be required before its task runs; as soon as it is added, every ALL crate must
# pick it up.
# `ALL crates` means "every crate the spec declares that actually exists". Build tooling
# (xtask, ng-spike-sp2) is not an architectural crate and is excluded from the expansion.
ALLOWED, OPEN_BOUND = resolve(
    ALLOWED_RAW,
    existing=set(ALLOWED_RAW) & workspace_members,
    assembly=ASSEMBLY & workspace_members,
)

# Coverage: every workspace member must be described by the spec or declared as tooling.
# This is the check that replaces the old silent skip.
undeclared = sorted(workspace_members - set(ALLOWED) - set(TOOLING))
for name in undeclared:
    errors.append(f"{name}: workspace member is not described by the spec (add a CRATE line "
                  f"in 01_system.dsl / a MODULE in module/*.dsl, or declare it in TOOLING)")

# A crate described by only one of the two sources is a warning, not a failure: the union
# still covers the workspace. It is reported because it usually means a MODULE file is
# missing (ng-cli has no owning module today).
for label, names in missing_from(SPEC_DIR).items():
    if names:
        print(f"  note -- spec gap, {label}: {', '.join(names)}")

# Deps naming something that does not exist at all (typo, or a unit renamed without the graph).
known_units = UNITS | set(TOOLING) | workspace_members
for name, allowed in sorted(ALLOWED.items()):
    for dep in sorted(allowed - known_units):
        errors.append(f"{name}: allowed dep '{dep}' is not a known crate or package")

# Cargo graph vs spec.
for pkg in META["packages"]:
    name = pkg["name"]
    if name not in workspace_members or name not in ALLOWED:
        continue
    actual = {
        dep["name"].replace("_", "-")
        for dep in pkg["dependencies"]
        if dep["name"].replace("_", "-") in (workspace_members | set(TOOLING))
    }
    # `ALL crates` is an upper bound (may depend on anything, is not required to depend on
    # everything); an explicit list is exact. Undeclared deps always fail either way.
    extra = sorted(actual - ALLOWED[name])
    missing = sorted(ALLOWED[name] - actual)
    detail = []
    if extra:
        detail.append(f"undeclared {extra}")
    if missing and name not in OPEN_BOUND:
        detail.append(f"declared but unused {missing}")
    if detail:
        errors.append(f"{name}: {'; '.join(detail)}")

# extern "C" / unsafe isolation check (only crates/* scanned; spikes/ and scripts/ exempt as throwaway).
# Test targets (crates/*/tests/**) may use `unsafe`: g15's counting allocator is an
# `unsafe impl GlobalAlloc`, and test code never ships in the wasm cdylib (that target
# compiles only src/). `unsafe` in src/ remains a failure outside ng-wasm.
for rs in CRATES_DIR.rglob("*.rs"):
    crate = rs.parts[rs.parts.index("crates") + 1]
    text = rs.read_text(encoding="utf-8", errors="replace")
    if crate != "ng-wasm":
        if 'extern "C"' in text:
            errors.append(f"{rs.relative_to(ROOT)}: extern \"C\" outside ng-wasm")
        is_test_target = "tests" in rs.parts
        if not is_test_target and re.search(r"\bunsafe\b", text):
            errors.append(f"{rs.relative_to(ROOT)}: unsafe outside ng-wasm")

if errors:
    print("g03 FAILED:")
    for e in errors:
        print("  ", e)
    sys.exit(1)
print(f"g03 OK ({len(workspace_members)} workspace members, graph from spec)")
