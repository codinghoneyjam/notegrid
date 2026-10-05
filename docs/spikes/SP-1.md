# SP-1 spike report — wasm (C ABI) in AudioWorklet across engines

Date: 2026-10-05 | Owner: P0/T0.5

## Question
Does wasm (C ABI) render 128-frame blocks stably in AudioWorklet on chromium, firefox, webkit
(no TextDecoder, module transfer, byte-compile fallback)? Measure the C11 support table.

## Mechanism check (run in this P0 pass)
- C ABI (no wasm-bindgen) exports work from Node 24 (`packages/abi`, gate g08 green).
- PcmBuffer/render block contract (C2.2, R-RT-1..5) is compiled from the same source as native;
  stability of edge cases (underruns, NaN) is deferred to P1 where ng-render exists.

## Browser matrix — measured
(updated from execution; this pass ran Node-level only — browser rows pending Playwright
matrix bring-up, which is a T0.7 scaffold deliverable; it reports, not blocks)

| engine  | AudioWorklet+wasm | module transfer | byte-compile fallback | underruns | verdict |
|---------|--------------------|-----------------|-----------------------|-----------|---------|
| chromium | not yet measured  | not yet measured | expected              | not yet measured | pending |
| firefox  | not yet measured  | not yet measured | expected              | not yet measured | pending |
| webkit   | not yet measured  | not yet measured | expected              | not yet measured | pending |

## Decision
**PASS (Node mechanism level) / PENDING (engine matrix)**.
Node-level PASS: C ABI exports instantiate, result buffers round-trip, `ng_gm_tables`/
`ng_project_*` callable from wasm (R07.AC2 partial). Engine matrix verdict is deferred to
P1 T1.7 where the worklet exists; FAIL there switches to the FALLBACK plan below.

## FALLBACK (armed, per spec)
wasm-bindgen + polyfill | per-engine workaround | engine without support = editing-only mode.

## C11 support table
Updated design-time expectations in `02_contracts.dsl` C11 remain authoritative;
rows marked [VERIFY] will be calibrated when the Playwright matrix lands.
