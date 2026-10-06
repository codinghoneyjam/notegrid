#!/usr/bin/env python3
# <META - FILE SUMMARY - Parses MODULE.CRATES/PACKAGES/DEPENDS_ON out of spec/dsl/module/*.dsl into a crate -> allowed-deps map.>
"""Derive the allowed dependency graph from the spec package instead of hand-maintaining it.

Each module file declares its own units and edges:
  R02  CRATES { ng-timeline }        DEPENDS_ON { ng-core, ng-score }
  R01  CRATES { ng-core, ng-score } DEPENDS_ON { ng-core: {} ; ng-score: { ng-core } }

Both DEPENDS_ON shapes appear in the spec: keyed (`unit: { deps }`) and positional
(a bare list, meaning "every CRATES entry depends on these"). `ALL crates` / `ALL packages`
defer to whatever exists, which is how ng-wasm and app stay edge-agnostic.

This module is pure parsing: no I/O beyond reading the spec, no cargo. `cargo metadata`
lives in check_deps.py. Run this file directly to print what the spec currently claims.
"""

import pathlib
import re
import sys
import unittest

MODULE_GLOB = "module/*.dsl"
ALL_MARKER = "\x00ALL\x00"

# The authoritative, complete crate list lives in the ARCHITECTURE block of 01_system.dsl:
#   CRATE ng-core DEPENDS_ON { }  OWNS ...
# The module files repeat a subset of that (a crate with no owning MODULE, such as ng-cli,
# appears only here). Both are parsed and required to agree, so a drift in either is a
# failure rather than a silent override.
CRATE_RE = re.compile(r"^\s*CRATE\s+(\S+)\s+DEPENDS_ON\s*", re.MULTILINE)
SET_RE = re.compile(r"^\s*SET\s+(\S+)\s*=\s*\{([^}]*)\}", re.MULTILINE)

# Cross-cutting modules that describe rules rather than own units.
NON_UNIT_MODULES = {"X01"}

# Workspace members that are build tooling rather than spec'd units. They are not
# described by any MODULE, so they are listed here explicitly -- and only these.
# Anything else in the workspace must appear in the spec or g03 fails.
TOOLING = {
    "xtask": set(),
    "ng-spike-sp2": set(),
}


class SpecError(Exception):
    """The spec could not be parsed; g03 must fail rather than fall back."""


def balanced(text: str, start: int) -> tuple[str, int]:
    """Return (contents of the {...} beginning at `start`, index just past it)."""
    try:
        open_at = text.index("{", start)
    except ValueError:
        raise SpecError(f"expected '{{' at offset {start}: {text[start:start + 60]!r}")
    depth = 0
    for i in range(open_at, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[open_at + 1 : i], i + 1
    raise SpecError(f"unbalanced braces at offset {start}: {text[start:start + 60]!r}")


def split_top(inner: str, sep: str = ";") -> list[str]:
    """Split on `sep` at brace depth zero, so nested dep lists survive intact."""
    parts, buf, depth = [], [], 0
    for ch in inner:
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
        if ch == sep and depth == 0:
            parts.append("".join(buf))
            buf = []
            continue
        buf.append(ch)
    parts.append("".join(buf))
    return [p.strip() for p in parts if p.strip()]


def unit_name(token: str) -> str:
    """Strip annotations: `abi(types)` -> `abi`, `document(readonly)` -> `document`."""
    return re.split(r"[(\s]", token.strip(), maxsplit=1)[0].strip()


def unit_list(text: str, start: int) -> list[str]:
    inner, _ = balanced(text, start)
    return [unit_name(p) for p in split_top(inner, ",") if unit_name(p)]


def parse_dep_list(text: str, start: int) -> list[str]:
    """A dep list is either a braced list `{ a, b }` or `ALL crates` (with or without braces)."""
    tail = text[start:].lstrip()
    if tail.upper().startswith("ALL"):
        return [ALL_MARKER]
    inner, _ = balanced(text, start)
    if inner.strip().upper().startswith("ALL"):
        return [ALL_MARKER]
    return [unit_name(p) for p in split_top(inner, ",") if unit_name(p)]


def global_sets(spec_dir: pathlib.Path) -> dict[str, set[str]]:
    """Parse the `SET NAME = { a, b }` lines of 00_dsl.dsl."""
    path = spec_dir / "00_dsl.dsl"
    if not path.exists():
        raise SpecError(f"{path} not found")
    text = path.read_text(encoding="utf-8", errors="replace")
    sets: dict[str, set[str]] = {}
    for m in SET_RE.finditer(text):
        sets[m.group(1)] = {unit_name(p) for p in split_top(m.group(2), ",") if unit_name(p)}
    if not sets:
        raise SpecError(f"{path.name}: no SET lines found")
    return sets


def parse_system(spec_dir: pathlib.Path) -> tuple[dict[str, set[str]], set[str]]:
    """Parse the ARCHITECTURE CRATE lines of 01_system.dsl.

    Returns (crate -> deps with ALL_MARKER unexpanded, assembly crates).
    """
    path = spec_dir / "01_system.dsl"
    if not path.exists():
        raise SpecError(f"{path} not found")
    text = path.read_text(encoding="utf-8", errors="replace")

    graph: dict[str, set[str]] = {}
    for m in CRATE_RE.finditer(text):
        graph[m.group(1)] = set(parse_dep_list(text, m.end()))

    if not graph:
        raise SpecError(f"{path.name}: no CRATE lines found")

    # SET CRATES is the declared membership. A CRATE line for something outside it, or a
    # member with no CRATE line, is a spec defect. Both are failures: a CRATE line the SET
    # forgot means the set misreports coverage, and a member with no line means the gate
    # has nothing to check it against.
    sets = global_sets(spec_dir)
    declared = sets.get("CRATES", set())
    problems = [f"{path.name}: CRATE {n} is not in SET CRATES" for n in sorted(set(graph) - declared)]
    problems += [f"{path.name}: SET CRATES lists {n} but there is no CRATE line" for n in sorted(declared - set(graph))]
    if problems:
        raise SpecError("; ".join(problems))

    return graph, sets.get("ASSEMBLY", set())


def parse(spec_dir: pathlib.Path) -> dict[str, set[str]]:
    """Return crate -> allowed workspace deps from the module files, ALL_MARKER unexpanded."""
    graph: dict[str, set[str]] = {}
    units: set[str] = set()
    files = sorted(spec_dir.glob(MODULE_GLOB))
    if not files:
        raise SpecError(f"no module files found under {spec_dir}")

    for path in files:
        text = path.read_text(encoding="utf-8", errors="replace")
        header = re.match(r"MODULE\s+(\S+)", text)
        if not header:
            raise SpecError(f"{path.name}: no MODULE header")
        name = header.group(1)
        if name in NON_UNIT_MODULES:
            continue

        crates = unit_list(text, re.search(r"\bCRATES\s*", text).end()) if re.search(r"\bCRATES\s*", text) else []
        packages = unit_list(text, re.search(r"\bPACKAGES\s*", text).end()) if re.search(r"\bPACKAGES\s*", text) else []
        if not crates and not packages:
            raise SpecError(f"{path.name} ({name}): declares neither CRATES nor PACKAGES")

        units.update(crates)
        units.update(packages)
        for c in crates:
            graph.setdefault(c, set())

        m = re.search(r"\bDEPENDS_ON\s*", text)
        if not m:
            raise SpecError(f"{path.name} ({name}): no DEPENDS_ON")
        body = balanced(text, m.end())[0]

        if ":" in body:
            # keyed: unit: { deps } ; unit: ALL crates
            for clause in split_top(body, ";"):
                key, sep, _ = clause.partition(":")
                if not sep:
                    raise SpecError(f"{path.name} ({name}): malformed DEPENDS_ON clause {clause!r}")
                unit = unit_name(key)
                if unit not in graph:
                    continue  # a TS package: not a cargo workspace unit
                rhs = clause[len(key) + 1 :].lstrip()
                if rhs.upper().startswith("ALL"):
                    graph[unit].add(ALL_MARKER)
                else:
                    graph[unit].update(parse_clause(clause))
        else:
            # positional: every CRATES entry depends on the listed units
            deps = parse_dep_list(text, m.end())
            for c in crates:
                graph[c].update(deps)

    return graph


def parse_clause(clause: str) -> list[str]:
    """Parse the `{ deps }` of one `unit: { deps }` clause."""
    inner, _ = balanced(clause, 0)
    return [unit_name(p) for p in split_top(inner, ",") if unit_name(p)]


def combined(spec_dir: pathlib.Path) -> tuple[dict[str, set[str]], set[str], set[str]]:
    """Merge both spec sources.

    Returns (crate -> deps, all known unit names, assembly crates). A crate present in both
    sources must declare the same deps in both, otherwise the spec contradicts itself and
    this raises. A crate in only one source is reported by `missing_from` rather than
    silently resolved one way or the other.
    """
    system, assembly = parse_system(spec_dir)
    modules = parse(spec_dir)

    graph = dict(system)
    units: set[str] = set(system) | set(modules)
    contradictions: list[str] = []
    for name, deps in modules.items():
        if name not in system:
            graph[name] = deps
        elif deps != system[name] and ALL_MARKER not in system[name] and ALL_MARKER not in deps:
            contradictions.append(
                f"{name}: 01_system.dsl says {sorted(system[name])}, module says {sorted(deps)}"
            )
    if contradictions:
        raise SpecError("spec sources disagree: " + "; ".join(contradictions))
    return graph, units, assembly


def missing_from(spec_dir: pathlib.Path) -> dict[str, list[str]]:
    """Crates only one of the two sources covers. Empty entries mean full agreement."""
    system, _ = parse_system(spec_dir)
    modules = parse(spec_dir)
    return {
        "01_system.dsl only": sorted(set(system) - set(modules)),
        "module files only": sorted(set(modules) - set(system)),
    }


def resolve(
    graph: dict[str, set[str]],
    existing: set[str] | None = None,
    assembly: set[str] | None = None,
) -> tuple[dict[str, set[str]], set[str]]:
    """Expand ALL_MARKER into concrete edges. Returns (crate -> edges, open_bound crates).

    Two exclusions, both from the spec's own sets:

    - `existing`: ALL resolves against crates that actually exist (the cargo workspace), not
      against every crate the spec ever mentions. A crate that does not exist yet cannot be
      depended upon, so requiring it would make the gate unsatisfiable before its task runs.
    - `assembly`: SET ASSEMBLY marks the entry points (ng-wasm, ng-cli, app). "ALL crates"
      means every crate an entry point composes, which excludes the other entry points:
      ng-cli does not depend on ng-wasm, and vice versa.

    A crate written as `ALL crates` gets an upper bound, not an obligation: it MAY depend on any
    existing crate and is free to depend on none of them yet. An explicit list is an exact
    contract in both directions. This distinction matters in practice -- requiring ng-cli to
    depend on ng-timeline the day ng-timeline appears would make the gate unsatisfiable during
    the very task that creates it.
    """
    pool = set(graph) if existing is None else set(existing)
    assembly = assembly or set()
    edges: dict[str, set[str]] = {}
    open_bound: set[str] = set()
    for name, deps in graph.items():
        if ALL_MARKER in deps:
            edges[name] = pool - assembly - {name}
            open_bound.add(name)
        else:
            edges[name] = set(deps)
    return edges, open_bound


def unknown_deps(graph: dict[str, set[str]], units: set[str]) -> list[str]:
    """Deps naming something that is neither a spec unit nor declared tooling."""
    known = units | set(TOOLING)
    return [f"{name} -> {d}" for name, deps in sorted(graph.items()) for d in sorted(deps) if d not in known]


if __name__ == "__main__":
    from paths import SPEC_DIR

    try:
        raw, _, assembly = combined(SPEC_DIR)
        parsed = resolve(raw, assembly=assembly)
        gaps = missing_from(SPEC_DIR)
    except SpecError as exc:
        print("SPEC PARSE FAILED:", exc, file=sys.stderr)
        sys.exit(1)
    for label, names in gaps.items():
        if names:
            print(f"# {label}: {', '.join(names)}")
    for name in sorted(parsed):
        print(f"{name}: {sorted(parsed[name])}")
