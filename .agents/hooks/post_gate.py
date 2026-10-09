# <META - FILE SUMMARY - Project module implementation>
"""
PostToolUse Gate: Records file reads, auto-binds roles, and tracks CLI ritual progress.
"""
from __future__ import annotations
import sys
from pathlib import Path

HOOKS_DIR = Path(__file__).resolve().parent
if str(HOOKS_DIR) not in sys.path:
    sys.path.insert(0, str(HOOKS_DIR))

try:
    from contract_parser import norm as norm_path
    from session_state import (
        get_or_create_state,
        save_cache,
        detect_role_from_path,
        parse_task_card_scopes,
        is_subagent_session
    )
except ImportError:
    from hooks.contract_parser import norm as norm_path
    from hooks.session_state import (
        get_or_create_state,
        save_cache,
        detect_role_from_path,
        parse_task_card_scopes,
        is_subagent_session
    )

# <META - ROLE : Execute record post tool | L33-85>
def record_post_tool(payload: dict) -> dict:
    tool_call = payload.get("toolCall", {})
    tool_name = tool_call.get("name", "")
    args = tool_call.get("args", {})
    conv_id = payload.get("conversationId", "default")
    transcript_p = payload.get("transcriptPath", "")

    cache, state = get_or_create_state(conv_id, transcript_p)

    if tool_name == "view_file":
        file_path = args.get("AbsolutePath", "")
        if file_path:
            norm_p = norm_path(file_path)
            if norm_p not in state["read_files"]:
                state["read_files"].append(norm_p)

            role_from_doc = detect_role_from_path(file_path)
            if role_from_doc:
                state["role"] = role_from_doc
            elif not state.get("role"):
                is_sub, detected_role = is_subagent_session(transcript_p)
                state["role"] = detected_role if is_sub and detected_role else "orchestrator"

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

    elif tool_name == "run_command":
        cmd = args.get("CommandLine", "")
        error = payload.get("error")
        is_success = not error

        if any(token in cmd for token in ["quality cross-domain", "quality runtime-risk", "code meta-sync", "dev.tools quality"]):
            state["vgate_passed"] = is_success

        if "docs memory" in cmd and "--commit" in cmd:
            if is_success:
                state["memory_committed"] = True

        if "git commit" in cmd:
            if is_success:
                state["git_committed"] = True

    elif tool_name in ("write_to_file", "replace_file_content"):
        target_file = norm_path(args.get("TargetFile", ""))
        is_safe = any(token in target_file for token in ["docs/40_work/", "scratch/", "tests/", ".agents/"])
        if target_file and not is_safe:
            mod_list = state.get("modified_production_files", [])
            if target_file not in mod_list:
                mod_list.append(target_file)
                state["modified_production_files"] = mod_list

    cache[conv_id] = state
    save_cache(cache)
    return {}
