---
task_id: "WASM-001-engine-entry-points"
plan_ref: "docs/work/engine-entry-points/plan/engine-entry-points.yaml"
f_read:
  - "docs/work/engine-entry-points/plan/engine-entry-points.yaml"
  - "crates/ng-wasm/src/engine.rs"
  - "crates/ng-wasm/src/buffers.rs"
  - "crates/ng-wasm/src/lib.rs"
  - "crates/ng-render/src/renderer.rs"
  - "crates/ng-render/src/mix.rs"
  - "crates/ng-render/src/slot_mix.rs"
  - "crates/ng-timeline/src/tempo_map.rs"
  - "crates/ng-timeline/src/schedule.rs"
  - "crates/ng-core/src/lib.rs"
  - "crates/ng-synth/src/osc/mod.rs"
  - "packages/abi/src/index.ts"
  - "packages/abi/test/abi.test.ts"
  - "packages/playback/src/sync/types.ts"
  - "packages/playback/src/sync/mix.ts"
  - "packages/playback/src/sync/audition.ts"
  - "packages/playback/test/sync.test.ts"
  - "spec/dsl/02_contracts.dsl"
f_modify:
  - "crates/ng-wasm/src/engine.rs"
  - "packages/abi/src/index.ts"
  - "packages/abi/test/abi.test.ts"
  - "spec/dsl/02_contracts.dsl"
  - "docs/tasks/DECISIONS.md"
line_limit: "Recommended <= 300, Max <= 400 (+10 buffer allowed)"
---

# Operations

TEST_FIRST (TASK_FLOW): stub the new signature first so the tests compile and fail, then write the failing tests, then implement. The full semantics (algorithms, error tables, risks) live in the plan_ref file, which is in f_read.

- file: "crates/ng-wasm/src/engine.rs"
  action: modify
  target:
    - "fn ng_engine_set_loop_ticks(start_tick: u32, end_tick: u32, enabled: u32) -> i32 (new export)"
    - "fn ng_engine_set_mix (replaces the -6 E_NOT_IMPLEMENTED stub at engine.rs:85-91)"
    - "fn ng_engine_audition (replaces the -6 E_NOT_IMPLEMENTED stub at engine.rs:140-152)"
    - "enum MixError { Format(&'static str), Range(&'static str) }"
    - "fn parse_mix_records(bytes: &[u8]) -> Result<Vec<LaneMix>, MixError> (pure decode helper)"
    - "use ng_core::{LaneKind, Slot}; use ng_render::LaneMix; use ng_timeline::TempoMap"
    - "#[cfg(test)] mod tests (the first test module in ng-wasm)"
  spec:
    few_shot:
      - symbol: "fn ng_engine_set_loop_ticks(start_tick: u32, end_tick: u32, enabled: u32) -> i32"
        as_is: |
          # None (not declared)
        to_be: |
          pub extern "C" fn ng_engine_set_loop_ticks(start_tick: u32, end_tick: u32, enabled: u32) -> i32 {
              // lock ENGINE (Err -> bare -5)
              // renderer None -> set_error(E_ENGINE, "engine not initialized"), -2
              // enabled == 0 -> renderer.set_loop(None), 0  (clearing needs no schedule)
              // renderer.schedule() None -> set_error(E_ENGINE, "no schedule loaded"), -3
              //   (REQUIRED: a tick loop-set before any schedule must fail, never silently succeed)
              // pairs: Vec<(u32,u32)> = schedule.tempos.iter().map(|t| (t.tick, t.mpq)).collect()
              //   (owned Vec ends the &Schedule borrow before the &mut set_loop call)
              // TempoMap::from_mpq_segments(schedule.sample_rate, &pairs) Err(_) ->
              //   set_error(E_ENGINE, "tempo map build failed"), -4  (fixed message; TempoMapError has no Display)
              // s0 = tempo.tick_to_sample(start_tick); s1 = tempo.tick_to_sample(end_tick)
              //   (tick_to_sample takes NO sample-rate argument; sr is fixed at construction)
              // renderer.set_loop(Some((s0, s1))), 0
              // A degenerate start >= end is dropped by set_loop's own filter (twin of ng_engine_set_loop).
          }
      - symbol: "fn ng_engine_set_mix(ptr: u32, len: u32) -> i32"
        as_is: |
          pub extern "C" fn ng_engine_set_mix(_ptr: u32, _len: u32) -> i32 {
              set_error("E_NOT_IMPLEMENTED", "ng_engine_set_mix not implemented in P1");
              -6
          }
        to_be: |
          pub extern "C" fn ng_engine_set_mix(ptr: u32, len: u32) -> i32 {
              // read_bytes(ptr, len) None -> set_error(E_FORMAT, "invalid mix payload"), -2
              // parse_mix_records(bytes): MixError::Format(m) -> set_error(E_FORMAT, m), -2;
              //                           MixError::Range(m) -> set_error(E_RANGE, m), -3
              // lock ENGINE (Err -> bare -5)
              // renderer None -> set_error(E_ENGINE, "engine not initialized"), -4
              // renderer.set_mix(&lanes), 0
          }
      - symbol: "fn parse_mix_records(bytes: &[u8]) -> Result<Vec<LaneMix>, MixError>"
        as_is: |
          # None (not declared)
        to_be: |
          fn parse_mix_records(bytes: &[u8]) -> Result<Vec<LaneMix>, MixError> {
              // Wire format: repeated 6-byte packed little-endian record, no padding.
              // bytes.len() % 6 != 0 -> Err(MixError::Format)
              // Per record { slot u16, volume u8, pan u8, reverb_send u8, flags u8 }:
              //   slot >= 128            -> Err(MixError::Range)  (slot 128 is AUDITION_SLOT)
              //   volume > 127           -> Err(MixError::Range)
              //   pan > 127              -> Err(MixError::Range)
              //   reverb_send > 127      -> Err(MixError::Range)
              //   flags & 0b1111_1110 != 0 -> Err(MixError::Format)  (bits 1-7 reserved, must be zero)
              // LaneMix { slot: Slot(slot), gain: f32::from(volume) / 127.0, pan, reverb_send,
              //           mute: flags & 1 != 0, solo: false }
              //   (gain conversion mirrors lane_mix, crates/ng-render/src/slot_mix.rs:50-59;
              //    solo never travels on the wire: the T2.6 port policy folds the global solo
              //    rule into muted, so Renderer::set_mix recomputes any_solo = false)
          }
      - symbol: "fn ng_engine_audition(kind: u32, program: u32, key: u32, vel: u32, on: u32) -> i32"
        as_is: |
          pub extern "C" fn ng_engine_audition(_kind: u32, _program: u32, _key: u32, _vel: u32, _on: u32) -> i32 {
              set_error("E_NOT_IMPLEMENTED", "ng_engine_audition not implemented in P1");
              -6
          }
        to_be: |
          pub extern "C" fn ng_engine_audition(kind: u32, program: u32, key: u32, vel: u32, on: u32) -> i32 {
              // Validate arguments BEFORE locking (mirrors set_schedule's decode-before-lock):
              //   on != 0: kind > 1 -> E_RANGE; program > 127 -> E_RANGE; key > 127 -> E_RANGE;
              //              vel == 0 or vel > 127 -> E_RANGE  (set_error(E_RANGE, m), -2)
              //   on == 0: only key is validated (key > 127 -> E_RANGE, -2);
              //              kind / program / vel are ignored (the port sends vel 0 as its release sentinel)
              // lock ENGINE (Err -> bare -4)
              // renderer None -> set_error(E_ENGINE, "engine not initialized"), -3
              // on != 0: renderer.audition_on(kind_of(kind), program as u8, key as u8, vel as u8), 0
              //   (renderer.rs:223-227: synth.set_program(AUDITION_SLOT, ...) + synth.note_on(
              //    AUDITION_SLOT, ...) + saturating note_ons counter; AUDITION_SLOT = Slot(128),
              //    MAX_SLOTS = 129; Schedule::validate enforces note.slot < lane_count, so no
              //    scheduled note can ever target slot 128; voices render regardless of transport
              //    state, so the note sounds immediately even while stopped)
              // on == 0: renderer.audition_off(key as u8), 0  (renderer.rs:229-232)
          }

- file: "packages/abi/src/index.ts"
  action: modify
  target:
    - "interface WasmExports: ng_engine_set_loop_ticks(start_tick: number, end_tick: number, enabled: number): number (after ng_engine_set_loop)"
    - "class Ng: engineSetLoopTicks(startTick: number, endTick: number, enabled: boolean): void (after engineSetLoop; body mirrors engineSetLoop, forwarding enabled ? 1 : 0)"
  spec:
    few_shot: []

- file: "packages/abi/test/abi.test.ts"
  action: modify
  target:
    - "pinned export list (lines 10-20): add 'ng_engine_set_loop_ticks' (both sides are .sort()ed, so position is free) -- gate g08 passes only when the wasm export set and this list agree"
    - "new integration tests (wasm_integration cases below)"
  spec:
    few_shot: []

- file: "spec/dsl/02_contracts.dsl"
  action: modify
  target:
    - "C2 line 49: tick_to_sample(tick,sr)->u64 / sample_to_tick(sample,sr)->Tick  ->  drop the stale sr arguments (sample rate is fixed at construction, tempo_map.rs:47,109)"
    - "C4 line 85 EXPORT list: gain ng_engine_set_loop_ticks(start_tick:u32,end_tick:u32,enabled:u32)"
    - "C5 line 97 MSG main->worklet: {t:'mix', bytes} -> bytes(6B/lane packed LE: slot u16, volume u8, pan u8, send u8, flags u8 bit0=mute, bits1-7=0); {t:'loop', start, end, enabled} -> {t:'loop', start_tick, end_tick, enabled /*document ticks, C0.1; converted in the engine via ng_engine_set_loop_ticks*/}; {t:'audition', ...} -> {t:'audition', kind, program, key, vel, on}"
  spec:
    few_shot: []

- file: "docs/tasks/DECISIONS.md"
  action: add
  target:
    - "one entry, newest first, per the plan's decisions_required section (title: '2026-10-09 -- mix wire format and conversion placement (engine-entry-points plan)'; covers (a) the 6-byte LE wire record, (b) the volume/127.0 conversion placement in ng-wasm, (c) solo never on the wire, (d) the -3 E_ENGINE no-schedule rule and the degenerate-range twin behaviour)"
  spec:
    few_shot: []

## Tests

Two layers, because ng_alloc returns `ptr as u32` (lib.rs:17-22), which truncates a 64-bit native heap address: no native test may pass a real heap pointer to a ptr/len ABI function. The pure decode logic lives in parse_mix_records and is tested natively with byte slices; every pointer path is tested in Node against the real wasm module, where pointers are 32-bit and each loadNg() instantiates a fresh ENGINE static.

Rust unit tests (crates/ng-wasm/src/engine.rs, #[cfg(test)] mod tests; assert! / assert_eq! only; no unwrap/expect/indexing/panic; Schedule::new's Result is matched, never unwrapped):
- parse_mix_records: valid record [0,0,100,64,0,0] -> Ok(LaneMix { slot: Slot(0), gain: 100.0/127.0, pan: 64, reverb_send: 0, mute: false, solo: false }) (LaneMix derives PartialEq, mix.rs:5)
- parse_mix_records: mute flag set ([0,0,100,64,0,1]) -> mute == true
- parse_mix_records: slot 127 accepted; slot 128 (LE [0x80,0x00]) -> Err(MixError::Range)
- parse_mix_records: volume 128 / pan 128 / send 128 -> Err(MixError::Range)
- parse_mix_records: len % 6 != 0 (e.g. 7 bytes) -> Err(MixError::Format); flags with any of bits 1-7 set (e.g. 0b100) -> Err(MixError::Format)
- parse_mix_records: empty slice -> Ok(empty vec)
- set_loop_ticks state machine (single scripted test fn: the ENGINE static is process-global, so the fresh-state assertions must run before any init inside one fn): (0,7680,1) == -2 before init; after ng_engine_init(44100,0,64): (0,7680,1) == -3 (no schedule) and (0,0,0) == 0 (clear needs no schedule)
- set_mix pointer-less paths: (0,0) == -2 before and after init (read_bytes rejects ptr 0 and len 0 before any state is touched)
- audition: fresh engine (0,0,60,100,1) == -3; after init: kind 2 -> -2, program 128 -> -2, key 128 -> -2, vel 0 with on -> -2, vel 128 with on -> -2, key 128 with off -> -2, valid on (0,0,60,100,1) -> 0, valid off (0,0,60,0,0) -> 0

Wasm integration tests (packages/abi/test/abi.test.ts, Node against the real wasm; every test uses its own loadNg() instance = fresh ENGINE static):
- g08: the pinned export list gains 'ng_engine_set_loop_ticks'
- loop_ticks conversion: ng.compile(example_project, 44100) -> engineSetSchedule -> engineSetLoopTicks(0, 7680, true) must not throw; decode the C3 tempo table from the schedule bytes (header 32 B: magic 4, version u16, flags u16, sample_rate u32, note_count u32 at 12, lane_count u32 at 16, tempo_count u32 at 20, total_samples u64; then lane_count * 8 B of lanes; then 16 B tempo entries: tick u32, mpq u32, sample u64, all LE) and assert the first mpq == 483871 (124 bpm, pinning the fixture); engineTransport(0, 0) (play); render 128-frame blocks via engineRender until ng.enginePositionSample() wraps; assert the wrap point == round_half_up(44100 * 7680 * 483871 / 480_000_000) == 341419 and the position then reads 0 -- the C0.3 example value computed by an independent TS implementation of the C0.3 formula (second-opinion pattern), bounded by a max-iterations budget
- loop_ticks error path: fresh instance, engineSetLoopTicks(0, 7680, true) throws NgError with code E_ENGINE (rc -3 read through readError)
- mix end to end: compile + engineSetSchedule the example project; decode lane_count from the C3 header; build one 6-byte record per lane (volume 100, pan 64, send 0, flags 0) with DataView little-endian; engineSetMix(records) -> OK; engineTransport(0, 0); render until a block is non-silent (budgeted); engineSetMix(same records with flags = 1, mute); render 44100 samples; assert the tail is silent (5 ms gain ramp, renderer.rs:149-165) -- muting every lane is what silences the mix, since any_solo is recomputed false and unmuted lanes would keep sounding
- mix malformed payloads: engineSetMix with a 7-byte payload throws NgError E_FORMAT; a payload whose record has slot 128 throws E_RANGE
- audition: fresh instance, engineInit, engineAudition(0, 0, 60, 100, true) -> OK; engineRender(zeroed 128-frame buffers) returns a block whose samples are NOT all zero (the audition voice sounds with the transport stopped -- the core T03 SYNC behaviour); engineAudition(0, 0, 60, 0, false) -> OK; render 88200 samples and assert the last 44100 are silent (envelope release; if the release exceeds 1 s the implementer widens the budget -- envelope parameters live in ng-dsp, out of scope)
- wrapper: Ng.engineSetLoopTicks exists and forwards (startTick, endTick, enabled ? 1 : 0) to the wasm export

## Verification
- `V_GATE` (.agents/language/toolchain.md) -- includes rebuilding the wasm module so the Node tests exercise the new export
- `cargo test -p ng-wasm ; cargo test --workspace ; cargo fmt --check ; cargo clippy --workspace --all-targets -- -D warnings ; python scripts/check_deps.py`
- `python scripts/check_determinism.py` (ng-wasm touched)
- `pnpm -r check ; pnpm -r test`

## Completion Summary

Status: DONE. All f_modify files landed in one atomic slice (gate g08 pins the export set).

### What changed

- `crates/ng-wasm/src/engine.rs`
  - `ng_engine_set_loop_ticks(start_tick, end_tick, enabled) -> i32` (new export): converts
    document ticks to renderer samples through `TempoMap::from_mpq_segments` built from
    `Renderer::schedule()`. Returns -2 (no renderer), -3 (`E_ENGINE`, no schedule),
    -4 (`E_ENGINE`, tempo map build failed), 0 otherwise; `enabled == 0` clears without
    needing a schedule.
  - `ng_engine_set_mix`: replaced the -6 `E_NOT_IMPLEMENTED` stub with a real decode of the
    6-byte little-endian per-lane record via the new pure helper `parse_mix_records`,
    returning -2 `E_FORMAT`, -3 `E_RANGE`, -4 `E_ENGINE`, 0 on success.
  - `ng_engine_audition`: replaced the -6 stub; validates before locking (E_RANGE, -2), then
    calls `Renderer::audition_on` / `audition_off`. Uninitialized engine is -3 `E_ENGINE`.
  - New `enum MixError { Format, Range }` and `#[cfg(test)] mod tests` (the crate's first).
- `packages/abi/src/index.ts`: `WasmExports.ng_engine_set_loop_ticks` +
  `Ng.engineSetLoopTicks(startTick, endTick, enabled)`.
- `packages/abi/test/abi.test.ts`: `ng_engine_set_loop_ticks` added to the pinned g08 export
  list; 6 new wasm integration tests (C3 header decode helper + an independent TS
  implementation of the C0.3 formula as the second opinion).
- `spec/dsl/02_contracts.dsl`: C2 stale `tick_to_sample(tick, sr)` signature corrected,
  C4 EXPORT list gains `ng_engine_set_loop_ticks`, C5 documents the mix wire record, the
  loop unit (ticks) and the audition fields.
- `docs/tasks/DECISIONS.md`: one new entry (mix wire format and conversion placement).

### Deviations from the plan

None in the product code. Two test details were tightened during implementation:

- The loop-wrap assertion computes the expected sample from the C0.3 rule in TS
  (`c03Samples`) and asserts `== 341419` plus a one-block tolerance on the observed wrap,
  instead of the loose "near 341419" bound; `assert.equal(expected, 341419)` pins the
  contract example itself.
- The audition test renders with the transport **stopped** (the plan's T03 SYNC claim). The
  earlier draft started playback, which would have passed even if stopped-transport
  auditioning were broken.

### Verification (real output)

    cargo test -p ng-wasm                -> test result: ok. 11 passed; 0 failed
    cargo test --workspace               -> no failures
    cargo fmt --check                    -> FMT OK
    cargo clippy --workspace --all-targets -- -D warnings -> Finished, no diagnostics
    cargo xtask build-wasm               -> Finished release profile
    python scripts/check_deps.py         -> g03 OK (11 workspace members)
    python scripts/check_hotpath_lints.py-> g01 hotpath lints OK (3/3 HOTPATH crates)
    python scripts/check_decoupling.py   -> g13 OK
    python scripts/check_determinism.py  -> g05 OK (2447006 bytes native == wasm == golden)
    pnpm -r check                        -> all 7 packages tsc --noEmit clean
    pnpm -r test                         -> all suites pass, abi 11/11

### Unblocks

T2.8 / T03 SYNC can now wire `packages/playback`'s `PlaybackPort` to real engine calls:
`engineSetLoopTicks` for the document loop and `engineSetMix` for the fader/mute policy.
Both T2.6 blockers stay RESOLVED.
