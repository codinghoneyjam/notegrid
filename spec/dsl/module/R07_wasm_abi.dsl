MODULE R07 wasm_abi
  LANG rust(+ts wrapper) ; PHASE P0(skeleton) extended per phase ; CRATES { ng-wasm } PACKAGES { abi }
  DEPENDS_ON { ng-wasm: ALL crates ; abi: {} } ; DEPENDED_BY { every TS package }
  PURPOSE composition + C ABI (C4) ; typed TS wrapper hiding pointer management
  OWNS     { singleton instance state, extern "C" surface, result/aux/error buffers, TS loadNg wrapper }
  NOT_OWNS { any logic beyond wiring }
  DESIGN
    instance state = single singleton (wasm single-threaded) ; all `unsafe` isolated in one module with the invariant "one instance, one thread" documented
    no wasm-bindgen (D05) ; strings/JSON as UTF-8 bytes
    input buffers via ng_alloc/ng_free ; ng_engine_render fills caller-preallocated L/R buffers (no allocation)
    build: opt-level=3, lto=fat, codegen-units=1, panic=abort, wasm-opt -O3 ; optional +simd128 build selected by feature detection (P5)
    panic == trap == must never happen (R-RT-2)
    TS API
      loadNg(module?):Promise<Ng>   // rejects on abi_version mismatch
      Ng { validate, migrate, canonical, compile(json,sr), gmTables, bankParse, bankLoadImage, bankPresets, engine:EngineApi, offline:OfflineApi,
           encodeWav(pcm,opts), exportMidi(json,opts):{bytes,warnings}, importMidi(bytes):{json,warnings} }
      NgError { code, path? }
      every wrapper: try/finally ng_free, copy result buffer immediately
  REQUIRE
    R07.R01 abi_version mismatch fails loudly
    R07.R02 wasm export set == TS expected set (gate:g08)
    R07.R03 native CLI and wasm-in-Node give byte-identical compile/render/encodeWav/exportMidi results (gate:g05)
    R07.R04 invalid pointer/length -> error code, never a trap
    R07.R05 ng_engine_render contains no logic beyond calling ng-render
  ACCEPT
    R07.AC1 Node test drives the whole ABI: example project -> schedule -> offline WAV, equals native bytes
    R07.AC2 ABI fuzz (random pointers/lengths, truncated JSON): 0 traps
    R07.AC3 Chromium/Firefox/WebKit: AudioWorklet renders 128-frame blocks from the same wasm ; module-transfer failure falls back to byte compile (C11)
    R07.AC4 wasm size <= BUDGET.wasm_size [MEASURE]
END
