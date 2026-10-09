# ONTOLOGY: MATHEMATICAL & LOGIC OPERATORS (NOTEGRID EDITION)
---
schema_version: "4.0.0"
spec_type: "OPERATOR_ONTOLOGY"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
---

## 1. SET THEORY OPERATORS
| Keyword | Semantics | Example |
| :--- | :--- | :--- |
| `IN` | Element membership | `file IN F_source` |
| `NOT_IN` | Non-membership | `type NOT_IN {Variant, untyped}` |
| `INTERSECT` | Set intersection | `WriteScope INTERSECT F_source == EMPTY_SET` |
| `UNION` | Set union | `AllDocs := D_spec UNION D_docs` |
| `DIFF` | Relative complement | `Forbidden := U_repo DIFF Allowlist` |
| `SUBSET` | Subset relation | `TouchedCrates SUBSET CRATES` |
| `EMPTY_SET` | Empty set | `Violations == EMPTY_SET` |
| `LEN` | Set cardinality | `LEN(ModifiedFiles) <= 6` |

## 2. LOGIC & RELATION OPERATORS
| Keyword | Semantics | Example |
| :--- | :--- | :--- |
| `IMPLIES` | Logical implication | `HOTPATH_crate IMPLIES lints_opt_in` |
| `EQUALS` | Strict equality | `AuditScore EQUALS 0` |
| `NOT_EQUALS`| Inequality | `ExitCode NOT_EQUALS 0` |
| `AND` | Conjunction | `exit_ok == 1 AND gates_pass == 1` |
| `OR` | Disjunction | `state EQUALS DONE OR state EQUALS BLOCKED` |
| `NOT` | Negation | `NOT has_contract_mutation` |
| `FORALL` | Universal quantifier | `FORALL f IN Files: Lines(f) <= 400` |
| `EXISTS` | Existential quantifier | `EXISTS g IN Gates: Protects(g)` |
