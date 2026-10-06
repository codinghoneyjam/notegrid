# TASK_PLAN_P1.md — NoteGrid Phase P1 "Sounding vertical slice"

Status: ACTIVE, no task started  | Date: 2026-10-06  | Owner: TBD
Source of truth for scope: `03_roadmap.dsl` PHASE P1, `01_system.dsl`, `02_contracts.dsl`, `module/R0{2,4,5,6,7}*.dsl`, `module/T0{3,5}*.dsl`, `00_dsl.dsl`.

## 0. Executive summary

P1 delivers MS-A: a headless sounding engine. Given a validated project, the native CLI and the wasm-in-Node build produce a byte-identical WAV, with zero UI editor (T03 minimal only). Scope = crates `ng-timeline`, `ng-dsp`, `ng-synth(osc)`, `ng-render`, `ng-export(wav)`, `ng-wasm`, `ng-cli`, packages `abi`, `playback`, `export-service(wav)`.

## 1. Prerequisite check (BLOCKER until cleared)

- [x] P0 artifacts exist — DONE 2026-10-05 (see `docs/p0_completion.md`). All P1 task READS on `ng-core`/`ng-score`/`abi` are satisfiable. `cargo xtask test-all` green; gates active at P0 pass.
- [x] Repo layout settled — DONE 2026-10-06. Spec package at `spec/dsl/`, machine-readable artefacts at `spec/schema` + `spec/fixture`; tooling paths in `scripts/paths.py`. P1 SCOPE paths in 03_roadmap.dsl are written against this layout.
- [ ] SP-1 engine matrix deferred — PASS at mechanism level in Node; the chromium/firefox/webkit matrix needs the T1.7 worklet to exist. Not a blocker for T1.1..T1.6.

Entry state, verified 2026-10-06: no crate or package named in the §3 scope table exists yet
(`ng-timeline`, `ng-dsp`, `ng-synth`, `ng-render`, `ng-export`, `packages/playback`,
`packages/export-service/src/wav`), so no task can be past ORIENT. `spec/fixture/golden/`
holds only a `.gitkeep`, so g12 has nothing to compare against until T1.1 writes bytes.
`docs/budgets.md` carries real numbers only for `wasm_size`.

## 2. Exit criteria (inherited, verbatim from roadmap)

- MS-A: engine complete (sound + WAV without UI); native == wasm bit-exact.
- gates g01..g15 that are ACTIVE_FROM P1 pass: g04 (wasm size), g05 (native vs wasm), g10 (e2e smoke), g12 (golden reason), g14 (engine identical), g15 (RT rules), plus P0 gates.
- budgets measured and fixed (BUDGET.*), ac:R05.AC1, ac:R05.AC2, ac:R05.AC5.

## 3. Task graph (order = critical path first)

```
T1.1 ng-timeline ──┐
T1.2 ng-dsp ───────┼─> T1.3 ng-synth(osc) ──┐
                   │                          ├─> T1.4 ng-render ─┐
                   └──────────────────────────┘                    ├─> T1.6 CLI + determinism ─> T1.8 budgets
T1.5 ng-export(wav) ──────────────────────────────────────────────┤
T0.4 ABI skeleton extended ───────────────────────────────────────┼─> T1.7 engine ABI/worklet/AudioEngine
```

Each task below is `PENDING` until its state machine (00_dsl TASK_FLOW) reaches DONE. The
`done when` column is copied from 03_roadmap.dsl and is the only definition of finished;
the notes name the specific checks each AC expands to. A number in the notes is a design
target from the module spec until a test actually measures it.

| task | scope | state | done when (roadmap) | notes on the checks |
|---|---|---|---|---|
| T1.1 TempoMap, compile, schedule bytes | `crates/ng-timeline`, `spec/fixture/golden/schedule` | DONE (2026-10-06) | ac:R02.AC1..AC5 | AC1 124bpm -> 341419 @44100 / 371613 @48000; AC2 tempo-change points vs exact-rational reference (`scripts/gen_tempo_vectors.py` + in-crate integer reference); AC3 property bpm∈[20,400], sr∈{44100,48000}, round trip + strict monotonicity; AC4 slot/mute/solo/order == golden bytes; AC5 truncation/tampering/bitflips rejected, 0 panics. R02.R03 (100k notes ≤ 100 ms) NOT measured — deferred with reason |
| T1.2 DSP parts | `crates/ng-dsp` | PENDING | parts unit-tested ; ac:R04.R04 ; libm for all transcendentals ; denormal test passes | Envelope, Ramp (5ms), Lcg, SoftLimiter, Reverb skeleton; R04.R04 = reverb flushes to < -120 dBFS |
| T1.3 OscSynth + synth_contract | `crates/ng-synth/src/osc`, `crates/ng-synth/tests/synth_contract.rs` | PENDING | ac:R04.AC1 ; ac:R04.AC2 ; gate:g15 | AC1 full synth_contract, R-RT flavoured; AC2 128 programs x 3 keys, non-silent, no NaN |
| T1.4 Renderer + render_offline | `crates/ng-render` | PENDING | ac:R05.AC1..AC6 ; R05.R04 | AC1 block sizes {1,7,64,128,4096} identical; AC2 looped file length == E; AC3 `set_schedule` tick within ±1; AC4 queue flood; AC5 click-free mute/solo; AC6 offline cancel. Also R05.R05 parity: ng-render never names OscSynth/SoundFontSynth |
| T1.5 WavEncoder | `crates/ng-export/src/wav`, `crates/ng-export/tests/wav*` | PENDING | ac:R06.AC1 | 16/24/float read-back within ±1 LSB; smpl/INFO parse |
| T1.6 CLI render + determinism | `crates/ng-cli`, `spec/fixture/golden` | PENDING | ng-cli render writes WAV into music_output via FsSink(C12) ; g05, g12, g15 | g05 currently covers canonical only; it gains the render path here |
| T1.7 engine ABI, worklet, AudioEngine | `crates/ng-wasm`, `packages/playback`, `packages/export-service/src/wav` | PENDING | ac:R07.AC1 ; ac:R07.AC3 ; ac:T03.AC1 ; ac:T05.AC1 ; g10, g14, g04 | AC1 Node drives example project -> schedule -> offline WAV == native; AC3 worklet renders 128-frame blocks on chromium/firefox/webkit with byte-compile fallback; AC1 T05 example WAV == ng-cli bytes. g10/g14 cannot pass before T1.6 |
| T1.8 measure budgets | `docs/budgets.md`, `tools/perf` | PENDING | every BUDGET.* [MEASURE] recorded, confirmed or amended by DECIDE | tools/perf does not exist yet; last task by design |

Parallelization: T1.1 and T1.2 are independent; T1.5 can start as soon as ng-core is frozen and run parallel with T1.3/T1.4. T1.8 is last.

## 4. Contract anchors (read before each task)

- C0 (units: PPQ=480, mpq, sample formula, lane kinds), C1 (project JSON + invariants), C2 (traits, Renderer FSM, render_offline/tail_fold), C3 (schedule binary), C4 (wasm C ABI), C5 (worklet protocol), C7 (R-RT-1..5 determinism/RT), C8 WAV rules, C12 (output safety), C2.8 (lane reorder -> recompile), C4.2/E_* handling.

## 5. Budgets to MEASURE in T1.8 (become hard)

wasm_size <= 1 MB ; worklet_cost <= 30% of 2.9 ms block @64 voices ; offline_speed >= 20x realtime ; ci_time full <= 15 min, local <= 5 min. Edit latency etc. is a P2 item (BUDGET.edit_latency).

Measured values live in `docs/budgets.md` (one row per item, `??` = not yet measured). Do not
record a number in that table before a test or script produced it; a target copied here is not
a measurement. `wasm_size` is the only item with a real figure (278830 B, P0 skeleton) and even
that must be rechecked once ng-render/ng-synth land.

## 6. Risks / spikes

- SP-2 **PASS** (`docs/spikes/SP-2.md`): native == wasm-in-Node, libm pinned to a single version across both targets. Precondition for trusting T1.6 is satisfied; no own-transcendental fallback needed.
- SP-1 **PASS at Node mechanism level, engine matrix pending** (`docs/spikes/SP-1.md`). Gates T1.7; the matrix cannot be run until the worklet exists, and a FAIL there switches to the armed FALLBACK (wasm-bindgen path) before T1.7 starts.
- HOTPATH lints (g01) are active from P0 — do not weaken; reverb skeleton in T1.2 must already respect R-RT-3.

## 7. Definition of done per task

TASK_FLOW state machine (00_dsl): ORIENT -> TEST_FIRST (tests failing) -> IMPLEMENT -> VERIFY (all phase-active gates + done-when) -> DONE. Any scope/contract gap -> BLOCKED, report; never widen SCOPE. Every ac:* either holds or is deferred with a recorded reason + the matching DECIDE entry.

## 8. Out of scope for P1 (NOT_DO)

SoundFont/banks (P3), editor UI / grid / persistence / document package (P2), MIDI & OGG export (P4), PWA/hardening (P5), stems/manifest (P6).

## 9. Next step

T1.2 (`ng-dsp`), which is the first task that writes HOTPATH code — see §9.2 before starting it.

## 9.1 T1.1 as built (2026-10-06)

DONE. `crates/ng-timeline`, 32 tests green. Notes on what the contract left open:

- **R02.R02 is enforced by type, not by grep.** The crate never sees a float bpm: `TempoMap`
  takes `(Tick, mpq)` and `from_project` routes bpm→mpq through `ng_score::TempoItem::mpq()`, so
  C0.2 keeps one definition and no other crate can form a conversion.
- **`sample_to_tick` is a binary search over `tick_to_sample`**, defined as the largest tick whose
  sample does not exceed the input. The round trip holds because the forward function is
  monotonic by construction, not because both directions were written to agree. Not hot: display
  and C2.6 tick preservation only.
- **AC2's second opinion lives outside the crate** — `scripts/gen_tempo_vectors.py` computes
  expected positions with Python `Fraction` and writes `spec/fixture/tempo/*.json`; CI runs it
  with `--check`. The in-crate test carries a second integer-only implementation as well, so a
  mistake in the fixture generator alone does not pass unnoticed. `scripts/` is T0.6 scope, which
  is why this needed a note rather than silence.
- **Golden**: `spec/fixture/golden/schedule/example_44100.ngsc`, recorded in
  `docs/golden_changes.md`. The test creates it on first run and fails that run deliberately.
- **Segment boundary semantics were the one real bug found**: the stretch ending at tempo change
  *i+1* is priced with segment *i*'s mpq, not *i+1*'s. The AC2 fixtures caught it; a
  single-tempo test would not have.

Not done, and deliberately: R02.R03 (100k notes ≤ 100 ms) is not measured — see §9.2.

## 9.2 Before T1.2: the g01 hotpath lint gap

`00_dsl.dsl` declares g01 as `cargo fmt --check ; cargo clippy -D warnings ; hotpath lints deny
{ unwrap_used, expect_used, indexing_slicing, panic }`. The third clause has no implementation
anywhere: no `[lints]` in any `Cargo.toml`, no `clippy.toml`, no HOTPATH marker in the tree. Those
clippy lints are in the restriction group and are off unless enabled. g01 passes today because
fmt and clippy pass, not because the deny list is in force.

This is a P0 residual, not a T1.2 problem — but T1.2 is where it first bites, since `ng-dsp` is
the HOTPATH crate that GLOBAL_DENY protects with exactly those four lints. Owner decision needed
on the HOTPATH boundary (crate-wide vs marker-driven), plus the `ng-wasm` exemption (`unsafe`
and slice indexing are unavoidable at a C ABI). Record the outcome as a DECIDE entry.

Recorded by T1.1 because the crate was the first to satisfy `MODULE.DEPENDS_ON` without any
g03 edit, which is what made the gap visible.
