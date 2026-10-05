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
| T1.1 TempoMap, compile, schedule bytes | `crates/ng-timeline`, `spec/fixture/golden/schedule` | PENDING | ac:R02.AC1..AC5 | AC1 124bpm -> 341419 @44100 / 371613 @48000; AC2 tempo-change points checked against an exact-rational reference; AC3 property bpm∈[20,400], sr∈{44100,48000}, `sample_to_tick(tick_to_sample(t))==t`; AC4 slot/mute/solo/order == golden bytes; AC5 truncated+tampered bytes rejected, fuzz 0 panics |
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

T1.1, from state ORIENT. Nothing blocks it: its READS (`ng-core`, `ng-score`) exist, its SCOPE
is empty, and the contracts it needs are already written — C0.1..C0.3 (tick, mpq, the single
u128 accumulate + round-once rule), C3 (schedule binary), C1 (project input). Read `00_dsl.dsl`
for the gates and GLOBAL_DENY, `module/R02_timeline.dsl` for compile order and the REQUIRE list,
then this §3 row. Start at TEST_FIRST with ac:R02.AC1 (124bpm -> 341419 @44100) as the first
failing test; AC2 needs a Python `fractions` reference script, AC4 needs the first golden bytes,
AC5 needs a fuzz target.

Adding `crates/ng-timeline` to the workspace makes g03 (`scripts/check_deps.py`) compare the
cargo graph against MODULE.DEPENDS_ON, so the crate must declare exactly `{ ng-core, ng-score }`.
Any further crate or a third-party dep is BLOCKED, not a judgement call.

Watch three traps that the P0 work already hit:
- GLOBAL_DENY forbids any conversion that touches `sample_rate` and `bpm` outside TempoMap
  (R02.R02 is a grep-deny rule, not a code comment).
- No `HashMap` iteration order, no wall clock, no unseeded randomness anywhere in compile or
  serialisation — R02.R01 requires same input to same bytes.
- `tools/perf/` (T1.8 scope) does not exist; create it then, not now.
