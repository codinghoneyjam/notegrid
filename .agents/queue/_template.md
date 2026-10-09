# TASK CARD: <TASK_ID>
---
schema_version: "1.0.0"
spec_type: "TASK_CARD"
language_standard: "100%_ASCII_ENGLISH_MANDATORY"
canonical_constitution: "AGENTS.md"
task_id: "<TASK_ID>"
role: "<ROLE>"
state: "ORIENT"
created: "<YYYY-MM-DD>"
---

## 1. SCOPE

`FORALL f IN FilesWritten(worker): f IN SCOPE(card(worker))`

Files:
- <path/to/file1>
- <path/to/file2>

## 2. READS

- <path/to/read1>
- <path/to/read2>

## 3. DONE WHEN

- <criterion1>
- <criterion2>

## 4. GATES

- <gate1>: `<command>`
- <gate2>: `<command>`

## 5. EVIDENCE

| criterion | command | output | exit code |
|---|---|---|---|
| <criterion1> | `<command>` | <output> | 0 |
| <criterion2> | `<command>` | <output> | 0 |

## 6. SCOPE FENCE CHECK

- [ ] All written files are in SCOPE
- [ ] No files outside SCOPE were modified
- [ ] All gates pass
- [ ] All DONE WHEN items have evidence

## 7. NOTES

<worker notes, blockers, decisions>
