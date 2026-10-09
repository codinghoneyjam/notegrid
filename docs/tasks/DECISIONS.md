# Decisions taken outside the spec

Decisions made while implementing, that the spec does not (or did not) record. Each entry must
either be folded into `spec/dsl/` or stand with a stated reason.

A DECIDE entry in `01_system.dsl` is the canonical home; this file is the staging area for
decisions that have not been promoted there yet.

Format, newest first:

```
## <date> — <one line>
Affects:   <task id, crate, or gate>
Decision:  <what was chosen>
Because:   <why, and what the alternatives cost>
Status:    <promoted to spec / open>
```

---

## 2026-10-09 — T2.6 scope extended to the package scaffolding and `@ng/abi` exports

Affects: T2.6, `.agents/queue/active/T2.6.md`, `packages/playback/**`, `packages/abi/package.json`

Decision: T2.6 was carried out with SCOPE
`{ packages/playback/package.json, packages/playback/tsconfig.json,
packages/playback/test/**, packages/abi/package.json }` in addition to the
roadmap's `packages/playback/src/sync/**`, and that deviation is recorded here
rather than left implicit.

Because: the roadmap scope is the source tree, but a package with no `test`
script, no `check` script and no `tsconfig.json` cannot be tested or type
checked, and `pnpm -r test` never reaches it. The precedent is T2.3, which put
its tests in `packages/ui-grid/test/` although its SCOPE reads
`ui-grid/src/render/**`. The `@ng/abi` change is smaller still: its `exports`
pointed at `./src/index.js` while the file is `./src/index.ts`, so *no* other
package could import it at runtime. That is a defect, not a design choice.

Status: **open** — `03_roadmap.dsl` SCOPE blocks should say whether a package's
`test/`, `package.json` and `tsconfig.json` are inside a `src/**` slice's scope,
so the next worker does not have to decide it again.

## 2026-10-09 — the sync policy speaks document units, the port speaks engine units

Affects: T2.6, `packages/playback/src/sync/types.ts`, `spec/dsl/02_contracts.dsl` C2

Decision: `DocumentSync` emits the loop range in document **ticks** and mix rows
in the **0..=127** document scale (C0.4). `PlaybackPort.apply(op)` is the single
boundary where those become engine units: samples for `ng_engine_set_loop`,
gain/pan factors for `ng_engine_set_mix`.

Because: the policy is the only thing that knows the document, and the port is
the only thing that knows the engine. Putting the conversion anywhere else means
either a second tempo map in TypeScript (R02.R02 says the conversion lives in
`ng-timeline` and nowhere else) or a second policy in the worklet. Normal edits
and watchdog recovery both go through the same `apply`, which is what makes
T03.R05 an ordering property of one function instead of a rule about call sites.

Status: **open** — the tick -> sample half is blocked on an ABI entry point and is
recorded in `BLOCKERS.md`.

## 2026-10-09 — a one-tick position bound applies only across a schedule swap

Affects: T2.6, `spec/dsl/module/T03_playback.dsl` T03.AC2

Decision: `DocumentSync.recordPosition` enforces two separate rules. Every
report must be non-decreasing (the playhead only moves forward). The
one-tick bound is checked only on the first report after a schedule swap, which
is the one instant C2.6 says must not move the playhead.

Because: T03.AC2 says "position jump <= 1 tick, position sequence monotonic",
which reads as one rule but is two. Ordinary playback advances the tick by
several ticks per 128-frame block at any real tempo -- at 124 bpm and 480 ppq that
is 3 ticks per block. Applying the one-tick bound everywhere makes the check fire
on every report and therefore says nothing; applying it nowhere makes the
"jump" half vacuous. A mutation that leaves the swap window open is caught by
the AC2 test, which is what makes the split load bearing rather than convenient.

Status: **open** — T03.AC2 should be restated as two clauses.

## 2026-10-06 — g03 `unsafe` scan is limited to non-test targets

Affects: gate g03, `scripts/check_deps.py`, `crates/ng-synth/tests/rt_rules.rs`

Decision: in the extern-"C"/unsafe isolation scan, files under `crates/*/tests/` may contain
`unsafe`. `src/` outside `ng-wasm` is still a failure for both.

Because: g15's R-RT-1 evidence is a counting allocator, and `GlobalAlloc` requires
`unsafe impl`. Test targets only build for `cargo test` — never into the wasm cdylib, which
compiles `src/` alone — so widening the production rule would block the gate's own
evidence. The exemption stays narrow: `extern "C"` is still an error in test files, and no
`unsafe` in `src/` is excused.

Status: **open** — the rule text in `02_contracts.dsl` (R-RT-2 / R07) should name the test
target exemption explicitly.

## 2026-10-06 — ng-synth DEPENDS_ON drops ng-bank until P3

Affects: `01_system.dsl`, `module/R04_synth_dsp.dsl`, `crates/ng-synth`

Decision: `ng-synth`'s declared dependency set is `{ ng-core, ng-dsp }` for P1. `ng-bank`
is re-added to both sources of truth when T3.2 (SoundFontSynth) starts.

Because: an explicit `DEPENDS_ON` list is exact in both directions (see the ALL-is-a-ceiling
entry below), and g03 fails "declared but unused". `ng-bank` does not exist yet, so the
exact-match rule was unsatisfiable while T1.3's crate declared it. The Osc half of R04 is
self-contained — `OscSynth` never touches SF2 types.

Status: **open** — restore `ng-bank` in the same commit that creates `crates/ng-bank`.

## 2026-10-06 — g01 hotpath lint boundary is crate-wide

Affects: gate g01, `crates/ng-dsp`, `crates/ng-synth`, `crates/ng-render`

Decision: `SET HOTPATH = { ng-dsp, ng-synth, ng-render }` is exactly the set that opts into
`[lints] workspace = true`. No marker attribute, so a new hot-path function cannot opt itself out.

Because: the alternative — a marker attribute naming individual functions — has to be added before
each hot function is written, and forgetting it is silent. Crate-wide has no per-function bookkeeping.
`ng-wasm` needs no exemption: it is not in SET HOTPATH, and its `unsafe` / `extern "C"` isolation is
already enforced by g03.

Status: **open** — `00_dsl.dsl` declares the deny list but does not say how the boundary is drawn.
The implementation and its check are in `scripts/check_hotpath_lints.py`.

## 2026-10-06 — `ALL crates` is an upper bound, not an obligation

Affects: gate g03, `01_system.dsl` ARCHITECTURE

Decision: a crate declared `DEPENDS_ON { ALL crates }` may depend on any existing crate and on none
of them yet. An explicit dependency list is exact in both directions; `ALL` is a ceiling only.

Because: requiring `ng-cli` to depend on `ng-timeline` the day `ng-timeline` appears makes gate g03
unsatisfiable during the very task that creates it. Both directions still fail on *undeclared*
deps, which is the direction that matters for decoupling.

Status: **promoted** — implemented in `scripts/spec_deps.py`; the semantics live in its docstring.

## 2026-10-06 — R02.R02 is enforced by type, not by grep

Affects: `crates/ng-timeline`, `module/R02_timeline.dsl` R02.R02

Decision: `TempoMap` holds `mpq` values and never sees a float `bpm`; `from_project` routes the
bpm→mpq conversion through `ng_score::TempoItem::mpq()`. No other crate can assemble a
tick→sample conversion at all.

Because: the spec described R02.R02 as a "grep-deny" rule ("`* sample_rate` together with `bpm`"),
which a later rename of an intermediate variable defeats silently. Closing the path with types
means there is no path to defeat. C0.2 keeps a single definition of the bpm→mpq conversion as a
side effect.

Status: **open** — R02.R02 should be restated in the module file; the grep rule is superseded.

## 2026-10-06 — task state lives in the spec, not in a status file

Affects: `spec/dsl/03_roadmap.dsl`, `docs/tasks/`

Decision: `STATE` lines in the roadmap are the only record of task progress. `docs/tasks/board.md`
is generated, and `scripts/check_task_state.py --check` fails CI if it is stale or if a `DONE`
claim is not backed by the workspace.

Because: this repo has twice recorded progress that the code did not support (g01's unimplemented
lint clause; T1.1–T1.8 recorded `DONE` with no crates). See `docs/tasks/README.md`.

Status: **open** — the roadmap's own header documents the vocabulary; the gate is not yet listed
in `00_dsl.dsl` GATES.

## 2026-10-08 — P1 budget measurement partial, deferred to later phases

Affects: `docs/budgets.md`, `03_roadmap.dsl` T1.8

Decision: P1 measures only wasm_size (384KB < 1MB budget). Other budgets are deferred with documented
reasons: worklet_cost to P2 (AudioWorklet), offline_speed to P2 (larger test project), edit_latency to P2
(editor UI), bank_load_cached to P3 (SoundFont), ci_time (CI configuration).

Because: P1 is the "Sounding vertical slice" with minimal UI and no SoundFont. Measuring worklet_cost
requires AudioWorklet implementation; offline_speed needs a realistic test project; edit_latency needs the
editor; bank_load_cached needs SoundFont parsing; ci_time needs CI infrastructure. These are
properly scoped to their respective phases.

Status: **open** — this decision documents a legitimate phase-by-phase measurement strategy; no spec change needed.
