# <META - FILE SUMMARY - Project module implementation>
"""
Stop Gate: Enforces completion rituals for 04_implementer (V_GATE -> Memory -> Git).
"""
from __future__ import annotations
import sys
from pathlib import Path

HOOKS_DIR = Path(__file__).resolve().parent
if str(HOOKS_DIR) not in sys.path:
    sys.path.insert(0, str(HOOKS_DIR))

try:
    from session_state import get_or_create_state, save_cache
except ImportError:
    from hooks.session_state import get_or_create_state, save_cache

# <META - ROLE : Execute check stop | L19-80>
def check_stop(payload: dict) -> dict:
    conv_id = payload.get("conversationId", "default")
    transcript_p = payload.get("transcriptPath", "")

    cache, state = get_or_create_state(conv_id, transcript_p)
    role = state.get("role", "orchestrator")

    if role != "04_implementer":
        mod_files = state.get("modified_production_files", [])
        if mod_files and not state.get("vgate_passed", False):
            retries = state.get("stop_retries", 0)
            if retries < 3:
                state["stop_retries"] = retries + 1
                save_cache(cache)
                return {
                    "decision": "continue",
                    "reason": (
                        f"[STOP GATE: V_GATE Incomplete] Orchestrator modified production files {mod_files} but V_GATE verification is missing.\n"
                        f"Action: Execute: python -m dev.tools quality cross-domain ; python -m dev.tools quality runtime-risk ; python -m dev.tools code meta-sync --all"
                    )
                }
        return {"decision": "allow"}

    retries = state.get("stop_retries", 0)

    if retries >= 3:
        return {
            "decision": "allow",
            "reason": "[STOP GATE WARN] Max ritual retries (3) reached. Proceeding to stop for escalation."
        }

    workspace = payload.get("workspacePaths", ["."])[0] if payload.get("workspacePaths") else "."
    msg_dir = Path(workspace) / "docs" / "40_work"
    if msg_dir.exists():
        has_blocked_msg = any(
            f.name.startswith("04_implementer-") and ("blocked" in f.name or "failed" in f.name)
            for f in msg_dir.rglob("*.yaml")
        )
        if has_blocked_msg:
            return {"decision": "allow"}

    if not state.get("vgate_passed", False):
        state["stop_retries"] = retries + 1
        save_cache(cache)
        return {
            "decision": "continue",
            "reason": (
                "[STOP GATE 1/3: V_GATE Incomplete] Role '04_implementer' must pass V_GATE verification before finishing.\n"
                "Action: Execute: python -m dev.tools quality cross-domain && python -m dev.tools quality runtime-risk && python -m dev.tools code meta-sync --all"
            )
        }

    if not state.get("memory_committed", False):
        state["stop_retries"] = retries + 1
        save_cache(cache)
        return {
            "decision": "continue",
            "reason": (
                "[STOP GATE 2/3: Memory Commit Pending] Role '04_implementer' must commit memory BEFORE git commit to avoid residual dirty files.\n"
                "Action: Execute: python -m dev.tools docs memory --commit <task_file_path>"
            )
        }

    if not state.get("git_committed", False):
        state["stop_retries"] = retries + 1
        save_cache(cache)
        return {
            "decision": "continue",
            "reason": (
                "[STOP GATE 3/3: Git Commit Pending] Role '04_implementer' must commit source modifications and memory updates via Git.\n"
                "Action: Stage and commit all changes: git add -A && git commit -m \"...\""
            )
        }

    return {"decision": "allow"}
