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

## 2026-10-10 -- T06.R01 is checkable, and T2.8 writes wider than packages/app

Affects:   T2.8, g13 (scripts/check_decoupling.py), packages/playback, packages/export-service
Decision:  T06.R01 / T06.AC4 ("zero cross-package implementation imports
            outside app") is checked by g13 with one mechanical rule: inside
            `packages/*/src/`, an import from another `@ng/*` package must be
            `import type`. `app` is exempt (composing implementations is its
            job) and so is `test/` (a test needs a real implementation to
            assert against, and a test is not composition). The check ships
            with detector vectors, so a pattern that stops matching the code
            fails the gate instead of silently passing.

            Consequence: the two cross-package VALUE imports that existed are
            gone. `packages/playback/src/index.ts` and
            `packages/export-service/src/wav.ts` now declare the shape they
            consume (`EngineAbi`, `WavRenderer`) and take it as a constructor
            argument, following the `SyncDocument` precedent the sync policy
            already set. The AudioWorkletProcessor lives in `packages/app`,
            because it is a composition root in a second realm and it is the
            only thing that may instantiate `@ng/abi`.

            T2.8's SCOPE widens from `ROOT/packages/app/**` to also cover
            `packages/playback/**` (the port implementation the composition
            root wires -- already named T2.8 work in the 2026-10-09 entry
            below), `packages/export-service/src/wav.ts`,
            `scripts/check_decoupling.py` (the AC4 check) and `e2e/**`
            (T06.AC5 + gate g10).
Because:   R01 had no mechanical reading. "Implementation" could mean the
            module, the class, or the instance, and each reading gives a
            different answer about `import { Ng }`. Keying on the erased
            edge rather than on the noun makes the rule decidable by a
            script: `import type` leaves no edge, so a package that only
            needs a shape never wires anything, and `app` becomes the one
            place where wiring happens. The alternative (leaving
            playback and export-service importing `Ng` and calling R01
            "satisfied by convention") is exactly the drift the T06.AC5 e2e
            matrix exists to catch later.
Status:    open -- the 01_system.dsl EDGE line already states the rule; the
            mechanical form above is not yet folded into the spec, and the
            three consumer seams (`EngineAbi`, `WavRenderer`,
            `ScheduleCompiler`) should become the declared interfaces that
            01_system.dsl names.

## 2026-10-09 -- mix wire format and conversion placement (engine-entry-points plan)

Affects:   T1.7 follow-up, crates/ng-wasm, packages/abi, 02_contracts.dsl C2/C4/C5
Decision:  (a) ng_engine_set_mix payload = fixed little-endian, packed
            6-byte per-lane record { slot u16, volume u8, pan u8,
            send u8, flags u8 with bit0 = mute }; bits 1-7 must be
            zero. len must be a multiple of 6; len == 0 is E_FORMAT
            (read_bytes rejects it) and the port skips the mix op for
            a zero-lane document.
            (b) The document->engine conversion (gain = volume / 127.0)
            lives in ng-wasm at the ABI boundary, mirroring lane_mix
            (crates/ng-render/src/slot_mix.rs:50-59); Renderer::set_mix
            stays engine-unit pure. The alternative (a
            LaneMix::from_document helper in ng-render) was rejected:
            it grows ng-render and duplicates the boundary ng-wasm
            already owns for set_schedule bytes.
            (c) solo never travels on the wire: the port policy folds the
            global solo rule into muted (packages/playback/src/sync/
            mix.ts:20-29) and every record carries solo = false, so
            Renderer::set_mix recomputes any_solo = false and the
            audible result equals the document's solo semantics. This
            keeps exactly one definition of the solo rule (the tested
            T2.6 policy).
            (d) ng_engine_set_loop_ticks with enabled != 0 and no schedule
            returns -3 E_ENGINE (never silently succeeds); clearing
            (enabled = 0) works without a schedule. A degenerate
            start >= end is dropped by Renderer::set_loop's filter
            (renderer.rs:176), twin behaviour of ng_engine_set_loop.
Because:   (a) C3's lane record is the only in-repo little-endian
            precedent and its field order matches; a JSON or
            variable-length payload would cost parsing on the worklet
            message path for no benefit.
            (b) ng-wasm already decodes ABI payloads (Schedule::from_bytes
            precedent, engine.rs:61) and already depends on
            ng-timeline and ng-render; zero new crate edges (g03).
            (c) The T2.6 policy is already tested; duplicating solo
            semantics in the wire would create a second source of
            truth.
            (d) Explicit errors beat silent success under the C4
            convention, and clearing must stay idempotent for
            recovery paths.
Status:    open -- C2/C4/C5 lines updated in the same change; the T2.8 port wiring (packages/playback) serializes MixRow into this record and calls engineSetLoopTicks.

## 2026-10-09 -- loop unit conversion is owned by ng-wasm (ng_engine_set_loop_ticks)

Affects:   T2.8, crates/ng-wasm, packages/playback, 02_contracts.dsl
Decision:  Add ng_engine_set_loop_ticks(start_tick, end_tick, enabled) to the
           engine ABI. Conversion from document ticks to renderer samples
           happens inside ng-wasm via the compiled schedule's tempo map.
           ng_engine_set_loop (samples) stays as the sample-domain primitive.
Because:   BLOCKERS.md option 1. R02.R02 (tempo math belongs to ng-timeline,
           surfaced through ng_compile) stays intact and the change is the
           smallest. Option 2 (export a tick->sample pair from ng_compile)
           grows the ABI for every consumer; option 3 (worklet owns the unit)
           spreads tempo math into TS.
Status:    open -- 02_contracts.dsl MSG main->worklet {t:'loop'} line should
           state the unit (ticks); packages/playback/test/sync.test.ts already
           pins the boundary.

## 2026-10-09 -- engine mix and audition entry points are implemented (T1.7 follow-up)

Affects:   T2.8, T1.7 follow-up, crates/ng-wasm, crates/ng-render, crates/ng-synth
Decision:  Implement both ng_engine_set_mix (ABI wiring to the existing
           Renderer::set_mix) and ng_engine_audition (a synth voice path that
           is not driven by the schedule), as a follow-up on T1.7 completed
           before T2.8 is dispatched.
Because:   T03 SYNC requires "lane volume/pan/send/mute/solo -> setMix
           immediately" and "note add / pitch change -> audition on the lane's
           instrument". The policy side landed and is tested in T2.6, so this
           is pure engine work with a written target. Cutting audition (option
           2) would leave P2 composing feedback silent; dropping both (option
           3) is a requirements regression.
Status:    open -- not yet a roadmap TASK; enters through the autonomous
           pipeline. Scope crates/ng-wasm, crates/ng-render, crates/ng-synth
           is disjoint from the in-flight T2.8 card (packages/app/**).

## 2026-10-09 — T2.7 scope extended to package scaffolding, the e2e harness, and `.gitignore`

Affects: T2.7, `packages/persistence/**`, `packages/app/**`, `e2e/**`, `.gitignore`

Decision: T2.7 was carried out with SCOPE
`{ packages/app/{package,tsconfig}.json, packages/persistence/{package,tsconfig}.json,
e2e/**, .gitignore }` in addition to the roadmap's `packages/persistence/**` and
`packages/app/src/panels/validation/**`.

Because: the roadmap scope is the source tree, but neither package existed, and a
package with no `test`/`check` script and no `tsconfig.json` is invisible to
`pnpm -r test` and `pnpm -r check`. T04.AC6 cannot be met without an e2e harness
at all: `@playwright/test` was not a dependency anywhere, no browsers were
installed, and `e2e/` was not even a package -- it was a `playwright.config.ts`
importing a library no manifest declared. The harness produces
`e2e/harness/harness.js` and `e2e/test-results/`, which have to be ignored or the
first run leaves build output in the tree.

This is the same pattern as the T2.6 entry, and it is now the second task in a row
to need it. That is the signal worth acting on: the roadmap's SCOPE lines describe
where product code goes, and say nothing about the scaffolding a new package
needs to be testable at all.

Status: **open** — `03_roadmap.dsl` should state whether a `src/**` slice's SCOPE
includes its package's `package.json`, `tsconfig.json` and `test/`, so the next
worker does not re-decide this.

## 2026-10-09 — the open pipeline validates before it migrates

Affects: T2.7, `spec/dsl/module/T04_persistence.dsl` DESIGN, `ProjectStore.openBytes`

Decision: opening a file runs `ng_project_validate` on the bytes as given, and
only then `ng_project_migrate`, then `ng_project_canonical`. T04 DESIGN writes it
as "migrate -> validate".

Because: `ng_project_migrate` deserialises into the typed model, so any value the
type cannot hold is rejected there as "model parse failed: invalid value:
integer `999`, expected u8" -- with no path, no note index, and nothing the editor
can highlight. The validator can say `E_RANGE /notes/2/3 pitch 0..=127`, which is
the difference between an error list and an apology. Migrate still runs and still
does its job: a file can be schema-valid and still need migrating, and that is
what `format_version` is for.

A second, smaller reason the written order loses information:
`validate_structure` returns `None` on the first structural failure, so
`validate_semantics` never runs and semantic errors (`E_LANE_MISSING`) are hidden
behind structural ones (`E_RANGE`). A user fixing errors one at a time will meet
those in waves. That is `ng-score`'s behaviour and T2.7 does not change it; two
tests in `packages/persistence/test/persistence.test.ts` pin it so it is a known
shape rather than a surprise.

Status: **open** — T04's DESIGN line should read validate -> migrate -> canonical.

## 2026-10-09 — a reopened project has canonically ordered notes, an edited one may not

Affects: T2.7, `packages/document` `addNote`, `packages/persistence` `decodeProject`

Decision: nothing enforces note order in the document model. `addNote` appends,
so an in-memory project holds notes in insertion order, while anything that has
been through `ng_project_canonical` comes back sorted by (start, lane, pitch).

Because: canonical order is a property of the file format (C1), not of the editor,
and the editor has no reason to pay for it. The consequence is that
`save -> open` is not identity on array order even though it is identity on
content. T04.AC1's "deep-equal document" therefore holds for a project that is
already canonical, which is what the AC1 fixture is, and the reordering is pinned
by its own test so it is documented behaviour rather than a surprise later.

The alternative -- keeping `notes` sorted on every insert -- would make every
update in `packages/document` O(n) and buy nothing the format does not already
give. The real question is whether anything downstream depends on note indices
being stable across a save, and that is a question for T2.8, when the app composes
undo, clipboard and hit-test against live documents.

Status: **open** — T2.8 must confirm that note indices are not load bearing across
a reload.

## 2026-10-09 — the DOM seam uses `setAttribute`, because assigning to `attributes` is a no-op

Affects: T2.7, `packages/app/src/panels/validation/panel.ts`

Decision: the panel's `PanelElement` seam exposes `setAttribute(name, value)`
rather than an `attributes` record for the renderer to write into.

Because: a real element's `attributes` is a live `NamedNodeMap`;
`el.attributes['data-code'] = x` compiles, type checks against a structural
stand-in, and does nothing at runtime. Every `data-*` hook the specs read back was
silently absent in all three browsers while the Node tests passed. Only running
the specs on real engines caught it, which is the argument for T04.AC6 being worth
its cost rather than being signed off from Node.

Status: **promoted** — the type says `setAttribute`, so the mistake is not available
to the next renderer.

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
