# Domain Memory: Assembly and IO (ng-wasm, ng-export, ng-cli)

Status: 2 verified facts recorded

Scope:
- ng-wasm: C ABI facade and composition; the only crate allowed `extern "C"` / `unsafe`.
- ng-export: WavEncoder, MidiExporter, MidiImporter.
- ng-cli: native headless tool; FsSink limited to OWN_IO.
- ABI conventions of C4: i32 returns, RESULT/AUX buffers, f64 sample positions, no traps.

Rule: record only verified facts, each with a source (file:line, command output, or test).

## Facts

### A native test can never pass a real heap pointer to a ptr/len ABI function
`ng_alloc` returns `ptr as u32` (crates/ng-wasm/src/lib.rs), which truncates a 64-bit
native heap address. Verified: `cargo test -p ng-wasm` runs only the pointer-free
`parse_mix_records` cases plus the pointer-free error-table assertions; every pointer path
is tested in Node against the real wasm (packages/abi/test/abi.test.ts), where pointers are
32-bit.
Source: `cargo test -p ng-wasm` -> 11 passed; `pnpm -r test` -> abi 11/11.

### ENGINE is a process-global static: one fresh instance per test
`ENGINE: Mutex<EngineState>` (crates/ng-wasm/src/engine.rs) outlives every test and the
harness runs tests on parallel threads, so fresh-engine assertions must run before any
`ng_engine_init` inside a SINGLE test fn - see `abi_state_machine_error_tables`. On the TS
side the equivalent is one `loadNg()` per test: it instantiates a fresh wasm module, hence a
fresh ENGINE.
Source: crates/ng-wasm/src/engine.rs `mod tests`.
