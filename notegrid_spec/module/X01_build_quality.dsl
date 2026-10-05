MODULE X01 build_quality
  LANG rust+ts+scripts ; PHASE P0 ; applies to ALL modules
  PURPOSE reproducible build, test pyramid, gates ; rules checked by machines, not remembered by people
  DESIGN
    LAYOUT under ROOT : rust-toolchain.toml (stable pinned + wasm32-unknown-unknown) ; Cargo.toml workspace ; crates/* ; xtask/ ; packages/* (pnpm workspace) ; vendor/vorbis/ (pinned emscripten build) ;
                        schema/ ; fixture/{example_project.json, validation/*.json, golden/*} ; docs/ ; scripts/ ; README.md LICENSE CHANGELOG.md ; notegrid.root ; music_source/ ; music_output/
    tool versions pinned by files (rust-toolchain.toml, .nvmrc, pnpm lockfile, binaryen/emscripten versions in scripts)
    BUILD `cargo xtask build-wasm`: release profile -> wasm-opt -O3 -> size check against BUDGET.wasm_size -> copy to packages/abi ; TS via vite (bundle, worklet module, PWA) ;
          types generated from schema ; GM tables generated through ng_gm_tables ; cross-platform (Windows-safe) via xtask, no shell scripts required
    TEST PYRAMID
      rust unit+property (proptest) : R01-R06 invariants, time conversion, canonical round trip
      contract tests (shared generic fns) : synth_contract(R04), encoder_contract (PCM->bytes header/length), exporter_contract
      golden : schedule bytes, OscSynth WAV, MIDI bytes ; changes require docs/golden_changes.md entry (gate:g12)
      cross-target determinism : wasm in Node vs native CLI, byte-identical (gate:g05)
      realtime rules : counting allocator, clippy deny (gate:g15, g01)
      fuzz : bank parser, MIDI import, project JSON, schedule from_bytes, ABI invalid arguments
      ts unit+property : vitest, fast-check (commands, history, document queries, sync policy)
      e2e : playwright on ENGINES ; sound is verified by bytes/energy, never by ear
      perf : criterion (native), Node wasm bench, browser harness for grid fps ; feeds BUDGET.*
      manual checklists (results recorded in docs/): Godot import, external MIDI players, listening evaluation, real Safari
    CI = all GATES active for the phase ; merge blocked on any failure
  REQUIRE
    X01.R01 every new contract rule ships with an automated check in the same change ; a rule that cannot be checked is downgraded to guidance
    X01.R02 CI <= 15 min (cached), local test-all <= 5 min [MEASURE]
    X01.R03 golden updates are logged in docs/golden_changes.md (date, files, reason, impact)
    X01.R04 licences: dependencies pass the cargo-deny / license-checker allow-list ; third-party notices screen generated
    X01.R05 ROOT dependency/output directories (node_modules, target, dist, music_output) are excluded from any repo-wide static analysis (DR-7)
  ACCEPT
    X01.AC1 at end of P0 gates g01,g02,g03,g05,g06,g07,g08,g09,g11,g13 run and pass
    X01.AC2 adding a SoundFontSynth reference to ng-render fails g03
    X01.AC3 adding Vec::push inside render_block fails g15
    X01.AC4 exceeding the wasm size budget fails g04 and prints the top growth symbols
    X01.AC5 a `../../core` path inside ROOT, or `tools/notegrid` inside a game .gd, fails g13
    X01.AC6 hostile output names/paths fail g13 via sink test vectors
    X01.AC7 different WAV bytes on any engine fail g14 and print the first differing offset
END
