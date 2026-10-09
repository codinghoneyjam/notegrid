---
trigger: manual
---

# /naming-audit (EMPTY SCAFFOLD)

Status: scaffold only. The game-specific procedure was removed during the 2026-10-07 migration
(see `.agents/README.md`); the slot is kept so NoteGrid can define its own naming audit here.

Skeleton to fill before enabling this rule:
1. Collect names (candidate: ripgrep over `crates/` and `packages/`).
2. Judgment questions (candidates):
   - same concept under different names across crates or packages?
   - spec vocabulary drift (a different word than the TERMS block of the spec)?
   - casing convention breaks (snake_case, PascalCase, UPPER_SNAKE_CASE)?
   - synonyms for one operation in the same area?
3. Report format: Concept / Current Name / Judgment / Recommended Name / Impact.
4. Never edit code from this audit; propose the change scope instead.
