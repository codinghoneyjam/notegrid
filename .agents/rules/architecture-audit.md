---
trigger: manual
---

# /architecture-audit (EMPTY SCAFFOLD)

Status: scaffold only. The game-specific procedure was removed during the 2026-10-07 migration
(see `.agents/README.md`); the slot is kept so NoteGrid can define its own structural audit here.

Skeleton to fill before enabling this rule:
1. Facts first, from checks that already exist:
   - python scripts/check_deps.py
   - python scripts/check_decoupling.py
   - cargo metadata --no-deps
2. Judgment questions (candidates):
   - does an edge exist that the declared DEPENDS_ON graph does not allow?
   - does a trait boundary sit with the consumer (RULE D)?
   - is a responsibility split across two crates or packages without a reason?
   - does the spec text and the implementation disagree about an invariant?
3. Report format: Fact / Judgment / Impact / Recommendation.
4. Out of scope: anything a dedicated gate already proves (g01..g16).

Do not enable this rule until the sections above carry verified NoteGrid procedures.
