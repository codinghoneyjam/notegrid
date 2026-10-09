# ONTOLOGY: CODE METRIC & TYPING FORMAL AXIOMS (NOTEGRID EDITION)
---
schema_version: "4.0.0"
spec_type: "CODE_METRICS_ONTOLOGY"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
---

## 1. PHYSICAL LINE LIMITS
Function_Max_Lines:     50
File_Recommended_Lines: 300
File_Max_Lines:         400
Tolerance_Buffer:       10   # a file violates only when PhysicalLines > 400 + 10
Axiom_Lines_Function: FORALL f IN Functions(F_source): PhysicalLines(f) <= Function_Max_Lines
Axiom_Lines_File:     FORALL doc IN Files(F_source): PhysicalLines(doc) <= File_Max_Lines + Tolerance_Buffer
Flow_Control:         Guard clauses preferred. Maximum nesting level <= 2.
Compression_Ban:      Inline compression to slip under the limits is forbidden.
NOTE: these bounds are initial defaults transplanted from the game repository; changing them is a
docs/tasks/DECISIONS.md entry, not a silent edit.

## 2. STRICT STATIC TYPING
Axiom_Static_Typing:
  FORALL fn IN Functions(F_source): Full parameter and return typing.
Rust:       type-checked by construction; HOTPATH crates additionally deny
            { unwrap_used, expect_used, indexing_slicing, panic } via [workspace.lints] (gate g01).
TypeScript: strict compiler options; `pnpm -r check` (tsc --noEmit) is the proof.
Python:     full annotations on functions and parameters (tooling scripts follow this file).

## 3. META COMMENT FORMAT
Meta_Format (the tooling scripts already in the tree follow this):
  FileHeader: "# <META - FILE SUMMARY - {summary}>"   # 1-3 lines, <= 100 chars
Rust and TypeScript files are not covered by this convention.
NOTE: NoteGrid has no META sync tool (see toolchain.md); the header is a convention, not a gate.
