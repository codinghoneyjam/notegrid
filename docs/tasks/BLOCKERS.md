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

## T2.6 - `ng_engine_set_mix` and `ng_engine_audition` are still `E_NOT_IMPLEMENTED`

BLOCKED since: 2026-10-09 (T2.6 scope: `packages/playback/src/sync/**` only)
Wanted: T03 SYNC requires "lane volume/pan/send/mute/solo -> setMix immediately"
and "note add / pitch change while editing -> audition on the lane's instrument".
Both need an engine entry point that does not exist yet.

Gap: `crates/ng-wasm/src/engine.rs` returns `-6` with `E_NOT_IMPLEMENTED` from both
`ng_engine_set_mix` and `ng_engine_audition`. `Renderer::set_mix(&[LaneMix])` exists in
`ng-render` (C2) but is not reachable through the ABI, and there is no audition voice
path at all. T03.AC2/AC3/AC4 did not need them, so T2.6 landed; the *behaviour* does not
work end to end until they are implemented.

Options:
1. Extend the T1.7 follow-up to implement both in `crates/ng-wasm` + `ng-render`
   (mix is nearly free: `Renderer::set_mix` is already there; audition needs a voice
   in the synth that is not driven by the schedule).
2. Cut audition for P2 and keep the policy in place until P3 has a real bank, where
   auditioning a program is more valuable anyway.
3. Leave both unimplemented and drop them from T03 SYNC.

Decision: orchestrator, at the start of T2.8 (app composition). The policy side is
done and tested, so this is pure engine work with a written target.

Unblocks: audible fader moves while playing, and audible note entry in the editor.

---

## T2.6 - the document loop is in ticks, `ng_engine_set_loop` takes samples

BLOCKED since: 2026-10-09
Wanted: T03 SYNC requires "loop range -> setLoop immediately", and the editor's loop is
a document field in ticks (`/loop/start`, `/loop/end`, validated as plain non-negative
integers against `length_ticks`).

Gap: the renderer loop is in samples -- C2 `set_loop(Option<(u64,u64)>)`, and
`ng_engine_set_loop` compares its argument against `pos_sample`. The ABI exposes no
tick -> sample conversion, and the tempo map that would provide it lives inside
`ng-timeline` behind `ng_compile`. Passing the document loop straight through sets the
loop end to 7680 *samples*, which wraps the playhead every 174 ms at 124 bpm.

The T2.6 policy therefore emits the loop in document ticks and leaves the conversion to
`PlaybackPort`; `T2.6 unit boundary: the loop range` in `packages/playback/test/sync.test.ts`
pins that boundary so whoever implements the port cannot miss it.

Options:
1. Add `ng_engine_set_loop_ticks(start_tick, end_tick, enabled)` that converts inside
   `ng-wasm` via the schedule's tempo map. Smallest change; keeps the conversion in
   the crate that owns tempo (R02.R02).
2. Export a tick -> sample pair with `ng_compile` so the worklet can convert.
3. State the unit in the `MSG main->worklet {t:'loop'}` line of `02_contracts.dsl`
   and let the worklet own it. Weakest: it spreads tempo math into TS.

Decision: orchestrator, before T2.8. Option 1 is the one that keeps R02.R02 intact.

Unblocks: a loop range that survives the trip from the document to the renderer.

---

## (none blocking a task in flight)

Two decisions are pending in `DECISIONS.md`; neither stops work in progress.
