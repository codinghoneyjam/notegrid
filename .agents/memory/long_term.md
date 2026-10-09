# Long-Term Project Memory Index (NoteGrid)

> Target: AI Agents & Engineers
> Purpose: entry digest pointing to scoped domain memories; verified knowledge only.
> Budget: under 100 lines hard cap.

## Scoped Domain Memories (read on demand per task)
- Core platform (ng-core, ng-score, ng-timeline): `.agents/memory/long_term/core.md`
- Audio signal path (ng-dsp, ng-synth, ng-render): `.agents/memory/long_term/audio.md`
- Assembly and IO (ng-wasm, ng-export, ng-cli): `.agents/memory/long_term/assembly.md`
- UI packages (abi, document, editing, ui-grid, playback, persistence, app): `.agents/memory/long_term/ui.md`
- Tooling, gates and OS traps: `.agents/memory/long_term/tools.md`

Status: all domains start EMPTY (migration 2026-10-07). Write only verified facts,
each with a source (file:line, command output, or test result).

## Domain Overview
- Product identity: offline browser composer for game music; no server.
- Stack: Rust (WASM core) + TypeScript UI; tooling in Python 3.
- Spec package: `spec/dsl/` (00 grammar, 01 system, 02 contracts, 03 roadmap).

## How memory is written
- Procedure (SSOT): `.agents/memory/README.md`
- Short-term journal: `.agents/memory/short_term/page_01.md` (active page).
- Long-term: promote a short-term entry here when it is a durable, verified fact.
