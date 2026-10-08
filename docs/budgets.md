# Budget Measurements

Measured in P1. Values with [MEASURE] tag are hard after P1.

## Measured Values

### wasm_size
- **Target**: <= 1 MB (wasm-opt -O3, bank excluded)
- **Measured**: 384 KB (Oct 8, 2026)
- **Status**: ✅ PASS
- **Method**: `ls -lh target/wasm32-unknown-unknown/release/ng_wasm.wasm`

### worklet_cost
- **Target**: <= 30% of block time at 64 voices, 128 frames @ 44.1 kHz (2.9 ms block)
- **Measured**: DEFERRED to P2 (requires AudioWorklet implementation)
- **Status**: ⏸️ DEFERRED
- **Note**: Requires actual AudioWorklet with timing measurements

### offline_speed
- **Target**: >= 20x realtime (3 min, 16 lanes, stereo)
- **Measured**: DEFERRED to P2 (requires larger test project)
- **Status**: ⏸️ DEFERRED
- **Note**: Currently only example_project.json exists; need realistic test project

### edit_latency
- **Target**: undo/redo <= 50 ms @ 5000 notes ; grid >= 30 fps @ 5000 visible notes ; single add <= 5 ms
- **Measured**: DEFERRED to P2 (requires editor UI implementation)
- **Status**: ⏸️ DEFERRED
- **Note**: Requires editor UI implementation

### bank_load_cached
- **Target**: <= 1 s @ 64 MB image
- **Measured**: DEFERRED to P3 (requires SoundFont implementation)
- **Status**: ⏸️ DEFERRED
- **Note**: Requires SoundFont bank parser and loader

### ci_time
- **Target**: full CI <= 15 min ; local test-all <= 5 min
- **Measured**: DEFERRED (CI not yet configured)
- **Status**: ⏸️ DEFERRED
- **Note**: CI configuration pending

## Notes

- wasm_size measured with OscSynth only (P1 minimal)
- All deferred budgets have documented reasons and target phases
- Full measurement will occur when the required infrastructure is in place
