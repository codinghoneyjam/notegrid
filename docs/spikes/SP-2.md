# SP-2 spike report — wasm-in-Node byte-identical to native (libm pinned)

Date: 2026-10-05 | Owner: P0/T0.5

## Question
Is wasm-in-Node byte-identical to native for sine/noise renders with libm pinned?

## Method
`spikes/sp2` crate renders a 440 Hz sine (44100 frames, f32 LE) via `libm::sinf`.
- native: `cargo run -p ng-spike-sp2 --release` writes `spikes/sp2/native_sine.f32le`
- wasm:   `ng_spike_sp2.wasm` + `run_wasm.mjs` writes `spikes/sp2/wasm_sine.f32le`
- compare: byte equality

## Result
native_sine.f32le == wasm_sine.f32le (176400 bytes, byte-identical).

## Decision
**PASS**. libm 0.2.16 (workspace Cargo.lock pins a single version for both targets)
produces bit-identical f32 transcendentals on x86_64-pc-windows-msvc and
wasm32-unknown-unknown. Baseline for gate:g05 determinism.

## Follow-up
- gate:g05 compares full renders (ng-cli vs wasm-in-Node) from P1.
- If a future float op differs, isolate that function behind the same native/wasm harness.
