"""
Dynamic Role Contract Parser (SSOT engine) for NoteGrid.
Parses role markdown files (.agents/.../roles/*.md) to extract MUST and ALLOW requirements.
"""
from __future__ import annotations
import os
import re
import json
from pathlib import Path

# Resolve the workspace root relative to the hooks directory
HOOKS_DIR = Path(__file__).resolve().parent
AGENTS_DIR = HOOKS_DIR.parent
WORKSPACE_ROOT = AGENTS_DIR.parent
CONTRACTS_DIR = AGENTS_DIR / "skills" / "autonomous-pipeline" / "roles"
CACHE_FILE = HOOKS_DIR / ".contract_cache.json"

def norm(path_str: str) -> str:
    if not path_str:
        return ""
    try:
        p = Path(path_str)
        if not p.is_absolute():
            abs_p = (WORKSPACE_ROOT / p).resolve()
        else:
            abs_p = p.resolve()
        return os.path.normcase(str(abs_p)).replace("\\", "/")
    except Exception:
        return path_str.replace("\\", "/").lower()

def parse_role_markdown(file_path: Path) -> dict:
    text = file_path.read_text(encoding="utf-8")

    # 1. reference_ontology
    ref_ont = None
    m_ont = re.search(r'reference_ontology:\s*["\']?([^"\']+)["\']?', text)
    if m_ont:
        ref_ont = m_ont.group(1).strip()

    # 2. read_allowlist (block list form)
    read_allowlist = []
    m_allow = re.search(r'read_allowlist:\s*\n((?:\s*-\s*["\']?[^\n]+["\']?\n?)+)', text)
    if m_allow:
        for line in m_allow.group(1).splitlines():
            item = re.sub(r'^\s*-\s*["\']?', '', line).strip().strip("'\"")
            if item:
                read_allowlist.append(item)

    # 3. read_denylist (inline list form)
    read_denylist = []
    m_deny = re.search(r'read_denylist:\s*\[(.*?)\]', text)
    if m_deny:
        for it in m_deny.group(1).split(","):
            clean = it.strip().strip("'\"")
            if clean:
                read_denylist.append(clean)

    # 4. write_scope (block list form)
    write_scope = []
    m_write = re.search(r'write_scope:\s*\n((?:\s*-\s*["\']?[^\n]+["\']?\n?)+)', text)
    if m_write:
        for line in m_write.group(1).splitlines():
            item = re.sub(r'^\s*-\s*["\']?', '', line).strip().strip("'\"")
            if item:
                write_scope.append(item)

    # ---------------------------------------------------------------------
    # Classify into MUST (static prerequisite documents) and ALLOW (dynamic patterns)
    # ---------------------------------------------------------------------
    must_docs = [norm(str(file_path))]  # Step 0: the role document itself
    if ref_ont:
        must_docs.append(norm(ref_ont))

    allow_patterns = []
    for item in read_allowlist:
        is_fixed_file = ("." in item and not any(c in item for c in ["*", "{", "}"])) and item.endswith((".md", ".yaml", ".txt"))
        if is_fixed_file:
            must_docs.append(norm(item))
        else:
            pat = item.replace("{plan_slug}", "[^/]+").replace("**", ".*").replace("*", "[^/]*")
            allow_patterns.append(pat)

    must_docs.append(norm("AGENTS.md"))

    return {
        "mtime": file_path.stat().st_mtime,
        "must": list(set(must_docs)),
        "allow_patterns": allow_patterns,
        "read_denylist": read_denylist,
        "write_scope": write_scope,
    }

def get_role_contract(role_id: str) -> dict:
    role_file = CONTRACTS_DIR / f"{role_id}.md"
    if not role_file.exists():
        return {
            "must": [norm("AGENTS.md")],
            "allow_patterns": [],
            "read_denylist": [],
            "write_scope": []
        }

    cache = {}
    if CACHE_FILE.exists():
        try:
            cache = json.loads(CACHE_FILE.read_text(encoding="utf-8"))
        except Exception:
            cache = {}

    current_mtime = role_file.stat().st_mtime
    cached_data = cache.get(role_id)

    if cached_data and cached_data.get("mtime") == current_mtime:
        return cached_data

    parsed = parse_role_markdown(role_file)
    cache[role_id] = parsed
    CACHE_FILE.parent.mkdir(parents=True, exist_ok=True)
    CACHE_FILE.write_text(json.dumps(cache, indent=2, ensure_ascii=False), encoding="utf-8")
    return parsed
