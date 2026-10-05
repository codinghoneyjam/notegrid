# Budgets

All items measured and hardened in P1 (T1.8). Values here are design targets until then.
`measured` must be produced by a script or test; a target is not a measurement. Tooling for
T1.8 lands in `tools/perf/`, which does not exist yet.

| budget | target | measured | status |
|---|---|---|---|
| wasm_size | <= 1 MB (wasm-opt -O3, bank excluded) | 278830 B, `cargo xtask build-wasm`, P0 skeleton, no wasm-opt step | measured 2026-10-06 (prior record 278584 B from 2026-10-05 was stale); recheck after T1.4/T1.7 add render+synth |
| worklet_cost | <= 30% of 2.9 ms block @64 voices | — | P1 |
| offline_speed | >= 20x realtime | — | P1 |
| edit_latency | undo/redo <= 50 ms @5000 notes | — | P2 |
| bank_load_cached | <= 1 s @ 64 MB | — | P3 |
| ci_time | full CI <= 15 min, local <= 5 min | — | P1 |
