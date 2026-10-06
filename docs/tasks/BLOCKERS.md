# Blockers

A task in state `BLOCKED` per TASK_FLOW means it is **stopped and reported**, not worked around.
`00_dsl.dsl` puts it plainly: any scope or contract gap → `BLOCKED`; never widen SCOPE yourself.

Format, newest first:

```
## <task id> — <one line>
BLOCKED since: <date>
Wanted:        <what the task needed>
Gap:          <what is missing, and where the spec is silent or contradictory>
Options:      <2-3 concrete ways out>
Decision:      <who decides, and by when>
Unblocks:      <what becomes possible once decided>
```

---

## (none open)

Nothing is currently blocked. Two decisions are pending but neither stops work in progress; they
are recorded in `DECISIONS.md`.
