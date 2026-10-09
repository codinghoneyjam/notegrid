# Domain Memory: Tooling, Gates and OS Traps

Status: EMPTY (starts blank after the 2026-10-07 migration)

Scope:
- Gate scripts under scripts/: check_task_state, check_deps, check_hotpath_lints,
  check_decoupling, check_determinism, gen_tempo_vectors.
- CI workflow: .github/workflows/ detects the gate set; a gate ships with its checker test.
- Agent dispatch: ..agents/queue cards, scripts/agent_queue.py, spec STATE lines.
- Runtime traps: PowerShell 5.1 has no '&&' chaining; cp949 console cannot print non-ASCII;
  working tree line endings; never use Set-Content for file writes.

Rule: record only verified facts, each with a source (file:line, command output, or test).
