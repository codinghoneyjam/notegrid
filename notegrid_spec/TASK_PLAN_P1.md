# TASK_PLAN_P1.md — NoteGrid Phase P1 "Sounding vertical slice"

Status: DRAFT  | Date: 2026-10-05  | Owner: TBD
Source of truth for scope: `03_roadmap.dsl` PHASE P1, `01_system.dsl`, `02_contracts.dsl`, `module/R0{2,4,5,6,7}*.dsl`, `module/T0{3,5}*.dsl`, `00_dsl.dsl`.

## 0. Executive summary

P1 delivers MS-A: a headless sounding engine. Given a validated project, the native CLI and the wasm-in-Node build produce a byte-identical WAV, with zero UI editor (T03 minimal only). Scope = crates `ng-timeline`, `ng-dsp`, `ng-synth(osc)`, `ng-render`, `ng-export(wav)`, `ng-wasm`, `ng-cli`, packages `abi`, `playback`, `export-service(wav)`.

## 1. Prerequisite check (BLOCKER until cleared)

- [x] P0 artifacts exist — DONE 2026-10-05 (`tools/notegrid/` scaffolded; see `docs/p0_completion.md`). All P1 task READS on `ng-core`/`ng-score`/`abi` are now satisfiable. `cargo xtask test-all` green; gates g01..g15 active-at-P0 pass.

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

- T1.1  TempoMap, compile, Schedule bytes        | scope: crates/ng-timeline, fixture/golden/schedule
  DONE: R02.AC1..AC5, fixture: 124bpm -> 341419 @44100 / 371613 @48000; property bpm∈[20,400], sr∈{44100,48000}: sample_to_tick(tick_to_sample(t))==t; from_bytes fuzz 0 panics.
- T1.2  DSP parts (Envelope, Ramp 5ms, Lcg, SoftLimiter, Reverb skeleton, denormal-protected feedback)
  DONE: libm for all transcendentals; denormal test passes; ac:R04.R04 (reverb flushes to < -120 dBFS).
- T1.3  OscSynth + synth_contract
  DONE: R04.AC1 (full synth_contract<R-RT flavored>), R04.AC2 (128 programs x 3 keys, non-silent, no NaN), gate g15.
- T1.4  Renderer + render_offline
  DONE: R05.AC1 (block sizes {1,7,64,128,4096} identical), AC2 (looped file length == E), AC3 (set_schedule tick within +-1), AC4 (queue flood), AC5 (click-free mute/solo), AC6 (offline cancel); R05.R04 loop counters; R05.R05 parity: ng-render never references OscSynth/SoundFontSynth.
- T1.5  WavEncoder
  DONE: R06.AC1 (16/24/float read-back within +-1 LSB; smpl/INFO parse).
- T1.6  ng-cli render + cross-target determinism
  DONE: native render writes WAV via FsSink (C12) into music_output; gate g05, g12, g15.
- T1.7  engine ABI, worklet, minimal AudioEngine
  DONE: R07.AC1 (Node drives example project -> schedule -> offline WAV == native), R07.AC3 (worklet renders 128-frame blocks on chromium/firefox/webkit; byte-compile fallback), T03.AC1, T05.AC1 (example WAV == ng-cli bytes), gates g10, g14, g04.
- T1.8  measure budgets
  DONE: every BUDGET.* [MEASURE] has a measured value recorded and either confirmed or amended via a DECIDE entry (docs/budgets.md, tools/perf).

Parallelization: T1.1 and T1.2 are independent; T1.5 can start as soon as ng-core is frozen and run parallel with T1.3/T1.4. T1.8 is last.

## 4. Contract anchors (read before each task)

- C0 (units: PPQ=480, mpq, sample formula, lane kinds), C1 (project JSON + invariants), C2 (traits, Renderer FSM, render_offline/tail_fold), C3 (schedule binary), C4 (wasm C ABI), C5 (worklet protocol), C7 (R-RT-1..5 determinism/RT), C8 WAV rules, C12 (output safety), C2.8 (lane reorder -> recompile), C4.2/E_* handling.

## 5. Budgets to MEASURE in T1.8 (become hard)

wasm_size <= 1 MB ; worklet_cost <= 30% of 2.9 ms block @64 voices ; offline_speed >= 20x realtime ; ci_time full <= 15 min, local <= 5 min. Edit latency etc. is a P2 item (BUDGET.edit_latency).

## 6. Risks / spikes

- SP-2 result (native == wasm-in-Node for sine/noise) is a precondition for trusting T1.6 determinism; if FALLBACK, pin own transcendentals before T1.2 lands.
- AudioWorklet+C-ABI stability (SP-1) gates T1.7; FALLBACK => wasm-bindgen path amended before T1.7 starts.
- HOTPATH lints (g01) are active from P0 — do not weaken; reverb skeleton in T1.2 must already respect R-RT-3.

## 7. Definition of done per task

TASK_FLOW state machine (00_dsl): ORIENT -> TEST_FIRST (tests failing) -> IMPLEMENT -> VERIFY (all phase-active gates + done-when) -> DONE. Any scope/contract gap -> BLOCKED, report; never widen SCOPE. Every ac:* either holds or is deferred with a recorded reason + the matching DECIDE entry.

## 8. Out of scope for P1 (NOT_DO)

SoundFont/banks (P3), editor UI / grid / persistence / document package (P2), MIDI & OGG export (P4), PWA/hardening (P5), stems/manifest (P6).
