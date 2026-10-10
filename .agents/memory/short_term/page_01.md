# Short-Term Working Memory: Page 01

> Capacity: 10 Tasks Max | Task Limit: <= 29 Lines | Page Limit: <= 300 Lines
> Status: ACTIVE (Receives new tasks. Shifts to Page 02 when 10 tasks are filled.)

## Tasks (Chronological, Newest at Bottom)

### task_01
- task_id: WASM-001-engine-entry-points (plan engine-entry-points, T1.7 follow-up for T2.8)
- status: DONE
- description: Implemented the three engine entry points that were E_NOT_IMPLEMENTED -
  new ng_engine_set_loop_ticks (tick -> sample via the schedule tempo map), real
  ng_engine_set_mix (6-byte LE per-lane record, gain = volume/127.0), real
  ng_engine_audition (Renderer::audition_on/off on AUDITION_SLOT). Contract edits in
  02_contracts.dsl C2/C4/C5 + one DECISIONS.md entry.
- touched_files: crates/ng-wasm/src/engine.rs, packages/abi/src/index.ts,
  packages/abi/test/abi.test.ts, spec/dsl/02_contracts.dsl, docs/tasks/DECISIONS.md
- verifications: cargo test -p ng-wasm -> 11 passed 0 failed; cargo test --workspace ->
  no failures; cargo fmt --check -> OK; cargo clippy --all-targets -D warnings -> clean;
  check_deps g03 OK; check_hotpath_lints g01 OK; check_decoupling g13 OK;
  check_determinism g05 OK (2447006 bytes native == wasm == golden);
  pnpm -r check clean; pnpm -r test all pass (abi 11/11).
- gotcha: the abi integration tests exercise the real wasm, and each loadNg() must be a
  fresh instance - ENGINE is a process-global static, so ordering matters (see the Rust
  `abi_state_machine_error_tables` test, which is ONE fn for the same reason).

---

## Memory Procedure Reference (Always at bottom of active page)
- Procedure SSOT: `.agents/memory/README.md`
- On page full: shift pages (01 -> 02, ...), clear page_01, promote durable facts to long_term/.
- Entry format: task_id, status, description, touched_files, verifications (command + real result).
