# Domain Memory: Assembly and IO (ng-wasm, ng-export, ng-cli)

Status: EMPTY (starts blank after the 2026-10-07 migration)

Scope:
- ng-wasm: C ABI facade and composition; the only crate allowed `extern "C"` / `unsafe`.
- ng-export: WavEncoder, MidiExporter, MidiImporter.
- ng-cli: native headless tool; FsSink limited to OWN_IO.
- ABI conventions of C4: i32 returns, RESULT/AUX buffers, f64 sample positions, no traps.

Rule: record only verified facts, each with a source (file:line, command output, or test).
