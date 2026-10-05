# P0 completion record

Date: 2026-10-05 | Scope: TASK T0.1..T0.7

## Delivered
- T0.1 `cargo xtask test-all` green on the workspace; `tools/.gdignore`; `.gitignore` covers node_modules/target/dist/music_output.
- T0.2 ng-core: C2 traits/types compile; zero third-party deps.
- T0.3 ng-score: validate (I-01..I-10, W-1..W-3), canonical, migrate scaffold, GM tables; 46 validation vectors in `fixture/validation/`; proptest + fuzz-smoke; ac:R01.AC1..AC4 green; gate g09 green.
- T0.4 ng-wasm C ABI skeleton + `packages/abi` TS loader: abi_version/alloc/free/validate/migrate/canonical/gm_tables work from Node; gate g08 green; ac:R07.AC2 partial (random pointers/lengths, truncated JSON: 0 traps).
- T0.5 SPIKE SP-1/SP-2 reports in `docs/spikes/`; SP-2 PASS (native == wasm-in-Node, 176400 bytes); SP-1 mechanism-level PASS at Node, engine matrix PENDING until T1.7 worklet exists.
- T0.6 gates g01 g02 g03 g05 g06 g07(config) g09 g11(config) g13 pass; CI workflow `.github/workflows/notegrid-ci.yml` (root) + in-tree copy.
- T0.7 decoupling: DR-1, DR-2, DR-6, DR-8 checks pass; DR-7 target/node_modules excluded from repo analyses via gitignore; Playwright 3-engine matrix skeleton at `e2e/`.

## Gate results (2026-10-05)
- g01 fmt+clippy: PASS
- g02 cargo test (workspace): PASS (9 tests incl. 5 ng-score acceptance)
- g03 scripts/check_deps.py: PASS
- g05 scripts/check_determinism.py: PASS (1498 bytes, native == wasm)
- g06 tsc --noEmit + node --test: PASS (4 tests). NOTE: vitest/eslint/fast-check deferred — substituted with node:test; record as DECIDE entry for T1.8 review.
- g07 .dependency-cruiser.cjs: config committed; execution deferred (no cross-package deps yet)
- g08 abi export set == TS expected: PASS
- g09 schema vectors == Rust verdicts: PASS (46 vectors)
- g11 deny.toml committed; cargo-deny run deferred (binary not installed locally)
- g13 scripts/check_decoupling.py: PASS

## Residuals for P1
- full 1e6 AC3 fuzz run (NOTEGRID_FUZZ_N=1000000 cargo test -p ng-score ac3)
- SP-1 engine matrix after T1.7 worklet exists
