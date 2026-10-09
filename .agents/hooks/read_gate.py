"""
Two-tier read & scope enforcement hook for NoteGrid (transplanted 2026-10-07).
Splits MUST (role contract, ontology, AGENTS.md) from ALLOW (task f_modify scope).
Escalates mismatches to the user / parent runner via 'decision: ask'.
"""
from __future__ import annotations
import sys
import json
import os
import re
from pathlib import Path

# Force UTF-8 I/O for Windows consoles
sys.stdin.reconfigure(encoding="utf-8")
sys.stdout.reconfigure(encoding="utf-8")

HOOKS_DIR = Path(__file__).resolve().parent
CACHE_FILE = HOOKS_DIR / ".read_cache.json"

sys.path.insert(0, str(HOOKS_DIR))
try:
    from contract_parser import get_role_contract, norm as norm_path
except ImportError:
    from hooks.contract_parser import get_role_contract, norm as norm_path

def load_cache() -> dict:
    if CACHE_FILE.exists():
        try:
            return json.loads(CACHE_FILE.read_text(encoding="utf-8"))
        except Exception:
            return {}
    return {}

def save_cache(cache: dict) -> None:
    CACHE_FILE.parent.mkdir(parents=True, exist_ok=True)
    CACHE_FILE.write_text(json.dumps(cache, indent=2, ensure_ascii=False), encoding="utf-8")

def extract_role_from_transcript(transcript_path: str) -> str | None:
    """Parse the role id from transcript lines carrying SYSTEM_MESSAGE or USER_INPUT."""
    if not transcript_path or not Path(transcript_path).exists():
        return None
    try:
        with open(transcript_path, "r", encoding="utf-8") as f:
            for line in f:
                # Support both subagent (SYSTEM_MESSAGE) and plain user (USER_INPUT) entries
                if '"USER_INPUT"' in line or '"SYSTEM_MESSAGE"' in line:
                    m = re.search(r"roles/(0\d_[a-zA-Z_]+)\.md", line)
                    if m:
                        return m.group(1)
                    for rk in ["00_planner_directive", "01_scout", "02_planner_plan", "03_task_maker", "04_implementer"]:
                        if rk in line:
                            return rk
                    break
    except Exception:
        pass
    return None

def detect_role_from_path(path_str: str) -> str | None:
    """Identify the role contract from the path of the file that was just read."""
    m = re.search(r"roles/(0\d_[a-zA-Z_]+)\.md", path_str.replace("\\", "/"))
    return m.group(1) if m else None

def parse_task_card_scopes(content: str) -> list[str]:
    scopes = []
    m = re.search(r"f_modify:\s*\[(.*?)\]", content, re.DOTALL)
    if m:
        for item in m.group(1).split(","):
            clean = item.strip().strip("'\"")
            if clean:
                scopes.append(clean)
    return scopes

# ---------------------------------------------------------------------------
# PreToolUse: MUST check & ALLOW scope cross-check
# ---------------------------------------------------------------------------
def check_pre_tool(payload: dict) -> dict:
    tool_call = payload.get("toolCall", {})
    tool_name = tool_call.get("name", "")
    args = tool_call.get("args", {})
    conv_id = payload.get("conversationId", "default")
    transcript_p = payload.get("transcriptPath", "")

    cache = load_cache()
    state = cache.get(conv_id, {"read_files": [], "role": None, "allow_scope": []})

    # Detect the role from the transcript when not yet identified
    if not state.get("role"):
        detected_role = extract_role_from_transcript(transcript_p)
        if detected_role:
            state["role"] = detected_role
            cache[conv_id] = state
            save_cache(cache)

    role = state.get("role", "general")
    contract = get_role_contract(role)
    must_list = [norm_path(d) for d in contract.get("must", [])]
    read_files = {norm_path(f) for f in state.get("read_files", [])}

    # [1] MUST tier check
    missing_must = [doc for doc in must_list if not any(doc in f for f in read_files)]
    if missing_must:
        return {
            "decision": "deny",
            "reason": (
                f"[MUST REQUIREMENT UNMET] Role '{role}' has NOT inspected mandatory foundation documents.\n"
                f"Missing: {missing_must}\n"
                f"Action: read each missing document before taking action."
            )
        }

    # [2] ALLOW tier check
    if tool_name in ("write_to_file", "replace_file_content"):
        target_file = norm_path(args.get("TargetFile", ""))
        file_exists = os.path.exists(target_file)

        is_safe_plan_or_spike = "docs/work/" in target_file or "spikes/" in target_file

        if role == "04_implementer" and not is_safe_plan_or_spike:
            allow_scope = [norm_path(s) for s in state.get("allow_scope", [])]
            is_in_allow = any(scope in target_file for scope in allow_scope) if allow_scope else False

            if not is_in_allow:
                return {
                    "decision": "ask",
                    "reason": (
                        f"[SCOPE MISMATCH ESCALATION] Target file '{target_file}' is NOT in the Task Card 'f_modify' scope (ALLOW: {allow_scope}).\n"
                        f"Requires explicit approval from the User or Orchestrator to proceed."
                    )
                }

        # [3] Existing files must be inspected before modification
        if file_exists and target_file not in read_files:
            return {
                "decision": "deny",
                "reason": (
                    f"[TARGET UNINSPECTED] You cannot modify '{target_file}' without inspecting its current contents first. "
                    f"Read '{target_file}' first."
                )
            }

    return {"decision": "allow"}

# ---------------------------------------------------------------------------
# PostToolUse: record reads, bind the role, parse task card scopes
# ---------------------------------------------------------------------------
def record_post_tool(payload: dict) -> dict:
    tool_call = payload.get("toolCall", {})
    args = tool_call.get("args", {})
    conv_id = payload.get("conversationId", "default")
    transcript_p = payload.get("transcriptPath", "")

    file_path = args.get("AbsolutePath", "")
    if file_path:
        cache = load_cache()
        state = cache.get(conv_id, {"read_files": [], "role": None, "allow_scope": []})
        norm_p = norm_path(file_path)

        if norm_p not in state["read_files"]:
            state["read_files"].append(norm_p)

        # [1] If the read file is a role contract, bind the role immediately
        if not state.get("role"):
            role_from_doc = detect_role_from_path(file_path)
            if role_from_doc:
                state["role"] = role_from_doc
            else:
                # [2] Otherwise parse the role from a SYSTEM_MESSAGE transcript
                detected_role = extract_role_from_transcript(transcript_p)
                if detected_role:
                    state["role"] = detected_role

        # Extract f_modify when a task card is read
        if "task/" in norm_p and norm_p.endswith(".md"):
            try:
                card_content = Path(file_path).read_text(encoding="utf-8")
                extracted = parse_task_card_scopes(card_content)
                for item in extracted:
                    n_item = norm_path(item)
                    if n_item not in state["allow_scope"]:
                        state["allow_scope"].append(n_item)
            except Exception:
                pass

        cache[conv_id] = state
        save_cache(cache)

    return {}

def main() -> None:
    mode = sys.argv[1] if len(sys.argv) > 1 else "pre"
    try:
        raw_in = sys.stdin.read().strip()
        payload = json.loads(raw_in) if raw_in else {}
        if mode == "pre":
            result = check_pre_tool(payload)
        elif mode == "post":
            result = record_post_tool(payload)
        else:
            result = {"decision": "allow"}
        json.dump(result, sys.stdout)
    except Exception as e:
        json.dump({"decision": "allow", "reason": f"Fallback on error: {str(e)}"}, sys.stdout)

if __name__ == "__main__":
    main()
