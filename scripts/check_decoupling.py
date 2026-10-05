#!/usr/bin/env python3
# <META - FILE SUMMARY - Tooling script; see docstring below.>
"""g13: DR-1, DR-2, DR-6, DR-8 checks + C12 name/path vector predicate (sink dry-run)."""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
REPO = ROOT.parent.parent  # repository root holding tools/notegrid
GAME_GLOBS = ["**/*.gd", "**/*.tscn", "**/project.godot"]
GAME_DIRS = ["core", "entity", "ui", "world", "presentation", "assetdb", "main", "features"]
GAME_PATHS = re.compile(r"res://|core/|entity/|ui/|world/|project\.godot")

errors = []

# DR-1: game sources never mention ROOT
for base in GAME_DIRS:
    for gd_dir in [REPO / base]:
        if not gd_dir.exists():
            continue
        for f in gd_dir.rglob("*"):
            if f.suffix in (".gd", ".tscn", ".godot") or f.name == "project.godot":
                try:
                    text = f.read_text(encoding="utf-8", errors="replace")
                except OSError:
                    continue
                if "notegrid" in text or "tools/notegrid" in text:
                    errors.append(f"DR-1: {f.relative_to(REPO)} mentions notegrid")
if (REPO / "project.godot").exists():
    t = (REPO / "project.godot").read_text(encoding="utf-8", errors="replace")
    if "notegrid" in t:
        errors.append("DR-1: project.godot mentions notegrid")

# DR-2: ROOT sources never mention game paths
for f in ROOT.rglob("*"):
    if not f.is_file():
        continue
    rel = f.relative_to(ROOT)
    if any(part in ("node_modules", "target", "dist", ".git") for part in rel.parts):
        continue
    if f.suffix not in (".rs", ".ts", ".tsx", ".js", ".mjs", ".gd", ".toml", ".yaml", ".yml", ".json", ".md"):
        continue
    try:
        text = f.read_text(encoding="utf-8", errors="replace")
    except OSError:
        continue
    # docs may discuss the copy-out path; rule targets source-of-code coupling. We allow
    # the literal mention inside docs/ and README/CHANGELOG (contact surface note).
    if rel.parts[0] in ("docs",) or rel.name in ("README.md", "CHANGELOG.md"):
        continue
    for m in GAME_PATHS.finditer(text):
        errors.append(f"DR-2: {rel}: mentions game path '{m.group(0)}'")
        break

# DR-6: no relative path in imports/code leaves ROOT
for f in ROOT.rglob("*"):
    if not f.is_file() or f.suffix not in (".rs", ".ts", ".tsx", ".js", ".mjs"):
        continue
    rel = f.relative_to(ROOT)
    if any(part in ("node_modules", "target", "dist", ".git") for part in rel.parts):
        continue
    text = f.read_text(encoding="utf-8", errors="replace")
    for m in re.finditer(r"['\"]((?:\.\./)+[^'\"]+)['\"]", text):
        target = (f.parent / m.group(1)).resolve()
        try:
            target.relative_to(ROOT.resolve())
        except ValueError:
            errors.append(f"DR-6: {rel}: relative path '{m.group(1)}' escapes ROOT")

# DR-8 / C12 name predicate (sink dry-run): vectors must be rejected
RESERVED = {"CON", "PRN", "AUX", "NUL", *{f"COM{i}" for i in range(1, 10)}, *{f"LPT{i}" for i in range(1, 10)}}

def valid_name(name: str) -> bool:
    if not name or len(name) > 64:
        return False
    if re.search(r"[\\/:*?\"<>|\x00-\x1f]", name):
        return False
    if ".." in name:
        return False
    if name.endswith(".") or name.endswith(" "):
        return False
    stem = name.split(".")[0].upper()
    if stem in RESERVED:
        return False
    return True

MALICIOUS = ["../x", "C:\\x", "CON", "a" * 300, "..\\evil", "a/b", "x:y", "name.", "COM1.wav"]
for m in MALICIOUS:
    if valid_name(m):
        errors.append(f"C12 predicate ACCEPTED malicious name {m!r}")

if errors:
    print("g13 FAILED:")
    for e in errors:
        print("  ", e)
    sys.exit(1)
print("g13 OK")