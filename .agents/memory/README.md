# Agent Memory (NoteGrid)

Layout:
- `short_term/page_01.md` .. `page_10.md`: rolling task journal. page_01 is the active page.
- `long_term/`: scoped domain memory; verified facts only.
- `long_term.md`: the index for the domain files.

Page mechanics:
- A task entry is a YAML block of <= 20 lines (hard limit 29).
- Max 10 tasks per page; page limit 300 lines.
- When the active page is full: shift pages outward (page_01 -> page_02, ..., page_09 -> page_10),
  clear page_01 as the new active page, and promote any durable fact to long_term before a page
  leaves the window.

Memory commit (manual; NoteGrid has no memory tool):
1. Append the completed task entry to the active page (newest at bottom).
2. Entry format: task_id, status, description, touched_files, verifications (command + real
   output or exit code).
3. If a durable, verified fact emerged, write it into the matching long_term domain file with
   its source (file:line or command output).

This README is the SSOT for the memory procedure. A future TOOL task may automate it.
