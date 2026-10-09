# Short-Term Working Memory: Page 01

> Capacity: 10 Tasks Max | Task Limit: <= 29 Lines | Page Limit: <= 300 Lines
> Status: ACTIVE (Receives new tasks. Shifts to Page 02 when 10 tasks are filled.)

## Tasks (Chronological, Newest at Bottom)

(empty -- no tasks recorded yet; entries are appended on memory commit)

---

## Memory Procedure Reference (Always at bottom of active page)
- Procedure SSOT: `.agents/memory/README.md`
- On page full: shift pages (01 -> 02, ...), clear page_01, promote durable facts to long_term/.
- Entry format: task_id, status, description, touched_files, verifications (command + real result).
