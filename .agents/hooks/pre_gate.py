# <META - FILE SUMMARY - Project module implementation>
"""
PreToolUse Gate: Enforces read-before-write, must-read contracts, and write scopes.
"""
from __future__ import annotations
import os
import re
import sys
from pathlib import Path

HOOKS_DIR = Path(__file__).resolve().parent
if str(HOOKS_DIR) not in sys.path:
    sys.path.insert(0, str(HOOKS_DIR))

try:
    from contract_parser import get_role_contract, norm as norm_path
    from session_state import get_or_create_state
except ImportError:
    from hooks.contract_parser import get_role_contract, norm as norm_path
    from hooks.session_state import get_or_create_state

# <META - ROLE : Execute check pre tool | L23-117>
def check_pre_tool(payload: dict) -> dict:
    tool_call = payload.get("toolCall", {})
    tool_name = tool_call.get("name", "")
    args = tool_call.get("args", {})
    conv_id = payload.get("conversationId", "default")
    transcript_p = payload.get("transcriptPath", "")

    cache, state = get_or_create_state(conv_id, transcript_p)
    role = state.get("role", "orchestrator")
    read_files = {norm_path(f) for f in state.get("read_files", [])}

    if role == "orchestrator":
        if tool_name == "view_file":
            return {"decision": "allow"}

        if tool_name in ("write_to_file", "replace_file_content"):
            target_file = norm_path(args.get("TargetFile", ""))
            file_exists = os.path.exists(target_file)

            agents_md = norm_path("AGENTS.md")
            if not any(agents_md in f for f in read_files):
                return {
                    "decision": "deny",
                    "reason": "[READ GATE BLOCKED: AGENTS.md Missing] Orchestrator must call 'view_file' on 'AGENTS.md' before authoring files."
                }

            if file_exists and target_file not in read_files:
                return {
                    "decision": "deny",
                    "reason": f"[TARGET UNINSPECTED] Call 'view_file' on '{target_file}' to review contents before modifying."
                }

            is_safe_artifact = any(
                token in target_file for token in ["docs/40_work/", "scratch/", "tests/", ".agents/"]
            )

            if not is_safe_artifact:
                already_modified = set(state.get("modified_production_files", []))
                if target_file not in already_modified and len(already_modified) >= 3:
                    return {
                        "decision": "deny",
                        "reason": (
                            f"[M1 VIOLATION BLOCKED: Bulk Edit Prohibited] Orchestrator cannot modify more than 3 production files (Already modified: {list(already_modified)}).\n"
                            f"Action: You MUST dispatch '04_implementer' via autonomous-pipeline to modify multiple files."
                        )
                    }

                return {
                    "decision": "ask",
                    "reason": (
                        f"[PRODUCTION WRITE ESCALATION] Orchestrator is attempting direct edit on production file:\n"
                        f"Target: '{target_file}' (Session production edits: {len(already_modified) + 1}/3)\n"
                        f"Confirm to proceed with this edit, or deny to enforce subagent dispatch."
                    )
                }

            return {"decision": "allow"}

    contract = get_role_contract(role)
    must_list = [norm_path(d) for d in contract.get("must", [])]
    read_denylist = contract.get("read_denylist", [])

    if tool_name == "view_file":
        target_read = norm_path(args.get("AbsolutePath", ""))
        for denied in read_denylist:
            if denied in target_read:
                return {
                    "decision": "deny",
                    "reason": f"[ROLE POLICY BLOCKED: Forbidden Read] Role '{role}' cannot read '{target_read}' (Denylist)."
                }
        return {"decision": "allow"}

    if tool_name in ("write_to_file", "replace_file_content"):
        target_file = norm_path(args.get("TargetFile", ""))
        file_exists = os.path.exists(target_file)

        missing_must = [doc for doc in must_list if not any(doc in f for f in read_files)]
        if missing_must:
            return {
                "decision": "deny",
                "reason": (
                    f"[MUST REQUIREMENT UNMET] Role '{role}' has NOT inspected mandatory foundation documents.\n"
                    f"Missing: {missing_must}\n"
                    f"Action: Call 'view_file' on each missing document before taking action."
                )
            }

        is_safe_pipeline_artifact = "docs/40_work/" in target_file or "scratch/" in target_file
        allowed_write_patterns = contract.get("write_scope", [])
        matches_write_scope = any(
            re.search(pat.replace("{plan_slug}", "[^/]+").replace("**", ".*").replace("*", "[^/]*"), target_file)
            for pat in allowed_write_patterns
        )

        if role == "04_implementer" and not is_safe_pipeline_artifact:
            allow_scope = [norm_path(s) for s in state.get("allow_scope", [])]
            is_in_task_allow = any(scope in target_file for scope in allow_scope) if allow_scope else False

            if not is_in_task_allow and not matches_write_scope:
                return {
                    "decision": "ask",
                    "reason": (
                        f"[SCOPE MISMATCH ESCALATION] Target file '{target_file}' is NOT in Task Card 'f_modify' scope (ALLOW: {allow_scope}).\n"
                        f"Requires explicit approval from User or Orchestrator to proceed."
                    )
                }
        elif not matches_write_scope and not is_safe_pipeline_artifact:
            return {
                "decision": "ask",
                "reason": f"[SCOPE MISMATCH ESCALATION] Role '{role}' is attempting to write out-of-scope file '{target_file}'. Confirm to proceed."
            }

        if file_exists and target_file not in read_files:
            return {
                "decision": "deny",
                "reason": f"[TARGET UNINSPECTED] Call 'view_file' on '{target_file}' to review contents before modifying."
            }

    return {"decision": "allow"}
