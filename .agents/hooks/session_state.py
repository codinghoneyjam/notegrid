# <META - FILE SUMMARY - Project module implementation>
"""
Session and Cache State Management for Lifecycle Hooks
Tracks per-conversation state, subagent roles, and task scopes.
"""
from __future__ import annotations
import json
import os
import re
from pathlib import Path

HOOKS_DIR = Path(__file__).resolve().parent
CACHE_FILE = HOOKS_DIR / ".read_cache.json"

try:
    from contract_parser import norm as norm_path
except ImportError:
    from hooks.contract_parser import norm as norm_path

# <META - ROLE : Execute load cache | L21-27>
def load_cache() -> dict:
    if CACHE_FILE.exists():
        try:
            return json.loads(CACHE_FILE.read_text(encoding="utf-8"))
        except Exception:
            return {}
    return {}

# <META - ROLE : Execute save cache | L30-32>
def save_cache(cache: dict) -> None:
    CACHE_FILE.parent.mkdir(parents=True, exist_ok=True)
    CACHE_FILE.write_text(json.dumps(cache, indent=2, ensure_ascii=False), encoding="utf-8")

# <META - ROLE : Check if is subagent session | L35-52>
def is_subagent_session(transcript_path: str) -> tuple[bool, str | None]:
    """Inspect transcript.jsonl to determine subagent session and bound role."""
    if not transcript_path or not Path(transcript_path).exists():
        return False, None
    try:
        with open(transcript_path, "r", encoding="utf-8") as f:
            first_line = f.readline()
            if '"SYSTEM_MESSAGE"' in first_line:
                m = re.search(r"roles/(0\d_[a-zA-Z_]+)\.md", first_line)
                if m:
                    return True, m.group(1)
                for rk in ["00_planner_directive", "01_scout", "02_planner_plan", "03_task_maker", "04_implementer"]:
                    if rk in first_line:
                        return True, rk
                return True, "subagent_unspecified"
    except Exception:
        pass
    return False, None

# <META - ROLE : Execute detect role from path | L55-57>
def detect_role_from_path(path_str: str) -> str | None:
    m = re.search(r"roles/(0\d_[a-zA-Z_]+)\.md", path_str.replace("\\", "/"))
    return m.group(1) if m else None

# <META - ROLE : Execute parse task card scopes | L60-68>
def parse_task_card_scopes(content: str) -> list[str]:
    scopes: list[str] = []
    m = re.search(r"f_modify:\s*\[(.*?)\]", content, re.DOTALL)
    if m:
        for item in m.group(1).split(","):
            clean = item.strip().strip("'\"").replace("res://", "")
            if clean:
                scopes.append(clean)
    return scopes

# <META - ROLE : Retrieve or create state | L71-96>
def get_or_create_state(conv_id: str, transcript_path: str = "") -> tuple[dict, dict]:
    """Load cache and return (cache, state) for the given conversation ID."""
    cache = load_cache()
    default_state = {
        "read_files": [],
        "role": None,
        "allow_scope": [],
        "stop_retries": 0,
        "vgate_passed": False,
        "memory_committed": False,
        "git_committed": False,
        "modified_production_files": []
    }
    state = cache.get(conv_id, default_state)

    for k, v in default_state.items():
        if k not in state:
            state[k] = v

    if not state.get("role"):
        is_sub, detected_role = is_subagent_session(transcript_path)
        if is_sub and detected_role:
            state["role"] = detected_role
        cache[conv_id] = state
        save_cache(cache)

    return cache, state
