# Budgets

All items measured and hardened in P1 (T1.8). Values here are design targets until then.

| budget | target | measured | status |
|---|---|---|---|
| wasm_size | <= 1 MB (wasm-opt -O3, bank excluded) | 278584 B (ng-wasm debug release, P0 skeleton) | measured pending P1 recheck |
| worklet_cost | <= 30% of 2.9 ms block @64 voices | — | P1 |
| offline_speed | >= 20x realtime | — | P1 |
| edit_latency | undo/redo <= 50 ms @5000 notes | — | P2 |
| bank_load_cached | <= 1 s @ 64 MB | — | P3 |
| ci_time | full CI <= 15 min, local <= 5 min | — | P1 |
