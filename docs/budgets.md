# Budgets

All items measured and hardened in P1 (T1.8). Values here are design targets until then.
`measured` must be produced by a script or test; a target is not a measurement. Tooling for
T1.8 lands in `tools/perf/`, which does not exist yet.

| budget | target | measured | status |
|---|---|---|---|
| wasm_size | <= 1 MB (wasm-opt -O3, bank excluded) | 351227 B, `cargo xtask build-wasm`, full render pipeline (T1.3–T1.5) | measured 2026-10-06 (T1.6; prior record 278830 B was P0 skeleton); recheck with wasm-opt -O3 and after T1.7 adds the engine ABI surface |
| worklet_cost | <= 30% of 2.9 ms block @64 voices | — | P1 |
| offline_speed | >= 20x realtime | — | P1 |
| edit_latency | undo/redo <= 50 ms @5000 notes | — | P2 |
| bank_load_cached | <= 1 s @ 64 MB | — | P3 |
| ci_time | full CI <= 15 min, local <= 5 min | — | P1 |
