# 03_roadmap.dsl — phases, tasks, spikes, risks. Tasks inherit 00_dsl.dsl TASK defaults and GLOBAL_DENY.
# A TASK lists only SCOPE, extra READS/ALLOW, and DONE WHEN. FLOW = TASK_FLOW unless stated.
# EFFORT: S = 1-2 days, M = 3-5 days, L = 1-2 weeks (rough; recalibrate after P0/P1).

## PHASE FSM
FSM PHASES
  STATE P0, P1, P2, P3, P4, P5, P6, SHIPPED
  START P0
  P0 --exit_met--> P1 ; P1 --exit_met--> P2 ; P2 --exit_met--> P3 ; P3 --exit_met--> P4 ; P4 --exit_met--> P5 ; P5 --exit_met--> SHIPPED ; P6 optional after P4
  PARALLEL_OK { R03 bank parser + SF synth (P3 Rust side) WITH P2 }
  CRITICAL_PATH P0 -> P1 -> P2 -> P4(UI wiring)
  RULE a phase is entered only when the previous EXIT holds ; a spike failure switches to its FALLBACK and amends the phase scope
END

## MILESTONES
MILESTONE MS-A end P1  engine complete: sound + WAV without UI ; native == wasm bit-exact
MILESTONE MS-B end P2  composing without touching JSON
MILESTONE MS-C end P3  128 programs + drum kit with real timbres
MILESTONE MS-D end P4  files ready for the game: WAV/OGG/MIDI export, MIDI import, seamless loop
MILESTONE MS-E end P5  budgets met, offline/PWA complete, cross-browser verified

## SPIKES
SPIKE SP-1  phase P0  timebox 1-2d
  QUESTION does wasm (C ABI) render 128-frame blocks stably in AudioWorklet on chromium, firefox, webkit (no TextDecoder; module transfer; byte-compile fallback) ; measure the C11 support table
  PASS     non-silent sine, no underruns, byte-identical output across engines
  FALLBACK wasm-bindgen + polyfill | per-engine workaround | engine without support = editing-only mode
SPIKE SP-2  phase P0  timebox 1d
  QUESTION is wasm-in-Node byte-identical to native for sine/noise renders with libm pinned
  PASS     identical bytes
  FALLBACK own transcendental implementations (tables/polynomials)
SPIKE SP-3  phase P3 (start)  timebox 2d
  QUESTION build SoundFontSynth or adopt an existing crate behind a Synth adapter (R04 criteria)
  PASS     all MUST criteria met -> adopt
  FALLBACK build
SPIKE SP-4  phase P3  timebox 1d
  QUESTION SF3 decoder choice and memory expansion
  PASS     64 MB-class SF3 within memory budget
  FALLBACK SF2 only ; SF3 users convert externally
SPIKE SP-5  phase P4 (start)  timebox 1-2d
  QUESTION Vorbis wasm: own Emscripten build vs ready-made package (streaming input, quality control, size, licence)
  PASS     criteria met
  FALLBACK own Emscripten build
SPIKE SP-6  phase P5  timebox 1d
  QUESTION single-file build working from file:// (wasm base64, worklet via Blob URL)
  PASS     double-click open runs audio
  FALLBACK PWA only (documented)

## PHASE P0  "Foundation and risk removal"   EFFORT M
PHASE P0
  MODULES { R01, R07(skeleton), T06(skeleton), X01 }
  NOT_DO  { synth, UI, export formats }
  EXIT    { gates active at P0 pass ; spike:SP-1 and spike:SP-2 results recorded in docs/ ; example project yields identical canonical output in Rust and in wasm-in-Node }

TASK T0.1 "workspaces and toolchain"
  SCOPE { ROOT/Cargo.toml, ROOT/rust-toolchain.toml, ROOT/xtask/**, ROOT/package.json, ROOT/pnpm-workspace.yaml, ROOT/.gitignore, ROOT/notegrid.root, ROOT/README.md, ROOT/LICENSE, ROOT/CHANGELOG.md, REPO:tools/.gdignore }
  DONE WHEN { cargo xtask test-all runs on an empty workspace ; tools/.gdignore exists ; .gitignore covers node_modules, target, dist, music_output }
TASK T0.2 "core types and traits"
  SCOPE { ROOT/crates/ng-core/** }   READS { ROOT/docs/02_contracts.dsl }
  DONE WHEN { traits and types compile exactly as C2 ; ng-core has zero third-party deps (R01.R04) }
TASK T0.3 "score model, validation, canonical, migrate scaffold"
  SCOPE { ROOT/crates/ng-score/**, ROOT/fixture/validation/**, ROOT/schema/** }   READS { ROOT/crates/ng-core/** }
  DONE WHEN { ac:R01.AC1 ; ac:R01.AC2 ; ac:R01.AC3 ; ac:R01.AC4 ; gate:g09 }
TASK T0.4 "ABI skeleton and TS loader"
  SCOPE { ROOT/crates/ng-wasm/**, ROOT/packages/abi/** }   READS { ng-core, ng-score }
  DONE WHEN { ng_abi_version, ng_alloc/free, ng_project_validate/migrate/canonical, ng_gm_tables work from Node ; gate:g08 ; ac:R07.AC2(partial: skeleton functions) }
TASK T0.5 "spikes SP-1 and SP-2"
  SCOPE { ROOT/docs/spikes/**, ROOT/spikes/** (throwaway code) }
  DONE WHEN { both spike reports written with PASS/FALLBACK decision ; C11 support table updated from measurements ; DECIDE amended if a FALLBACK is chosen }
TASK T0.6 "CI gates and dependency checks"
  SCOPE { ROOT/.github/** or CI config, ROOT/scripts/check_deps*, ROOT/scripts/check_decoupling*, ROOT/.dependency-cruiser.* }
  DONE WHEN { gates g01 g02 g03 g06 g07 g09 g11 g13 run in CI ; ac:X01.AC2(dry run) ; ac:X01.AC5 ; ac:X01.AC6 }
TASK T0.7 "decoupling scaffold"
  SCOPE { ROOT/scripts/**, ROOT/music_source/.gitkeep, ROOT/music_output/.gitignore, REPO:.gitignore(tools entries only) }
  DONE WHEN { DR-1,2,6,8 checks pass ; repo-wide analysis tools configured to exclude ROOT (DR-7) ; three-engine Playwright matrix skeleton exists }

## PHASE P1  "Sounding vertical slice"   EFFORT M-L
PHASE P1
  MODULES { R02, R04(Osc, DSP), R05, R06(WAV), T03(minimal) }
  NOT_DO  { SoundFont, editor UI, MIDI, OGG }
  EXIT    { MS-A ; gates g01..g15 active at P1 pass ; budgets measured and fixed (BUDGET.*) ; ac:R05.AC1 ; ac:R05.AC2 ; ac:R05.AC5 }

TASK T1.1 "TempoMap, compile, schedule bytes"
  SCOPE { ROOT/crates/ng-timeline/**, ROOT/fixture/golden/schedule/** }   READS { ng-core, ng-score }
  DONE WHEN { ac:R02.AC1 ; ac:R02.AC2 ; ac:R02.AC3 ; ac:R02.AC4 ; ac:R02.AC5 }
TASK T1.2 "DSP parts"
  SCOPE { ROOT/crates/ng-dsp/** }   READS { ng-core }
  DONE WHEN { Envelope, Ramp, Lcg, SoftLimiter, Reverb(skeleton) unit-tested ; ac:R04.R04 ; libm used for all transcendentals ; denormal test passes }
TASK T1.3 "OscSynth and synth_contract"
  SCOPE { ROOT/crates/ng-synth/src/osc/**, ROOT/crates/ng-synth/tests/synth_contract.rs }   READS { ng-core, ng-dsp }
  DONE WHEN { ac:R04.AC1 ; ac:R04.AC2 ; gate:g15 }
TASK T1.4 "Renderer and offline render"
  SCOPE { ROOT/crates/ng-render/** }   READS { ng-core, ng-timeline, ng-dsp }
  DONE WHEN { ac:R05.AC1 ; ac:R05.AC2 ; ac:R05.AC3 ; ac:R05.AC4 ; ac:R05.AC5 ; ac:R05.AC6 ; R05.R04 loop counters }
TASK T1.5 "WavEncoder"
  SCOPE { ROOT/crates/ng-export/src/wav/**, ROOT/crates/ng-export/tests/wav* }   READS { ng-core }
  DONE WHEN { ac:R06.AC1 }
TASK T1.6 "native CLI render and cross-target determinism"
  SCOPE { ROOT/crates/ng-cli/**, ROOT/fixture/golden/** }   READS { all crates }
  DONE WHEN { ng-cli render writes WAV into music_output via FsSink(C12) ; gate:g05 ; gate:g12 ; gate:g15 }
TASK T1.7 "engine ABI, worklet, minimal AudioEngine"
  SCOPE { ROOT/crates/ng-wasm/**, ROOT/packages/playback/**, ROOT/packages/export-service/src/wav/** }   READS { R07, T03 }
  DONE WHEN { ac:R07.AC1 ; ac:R07.AC3 ; ac:T03.AC1 ; ac:T05.AC1 ; gate:g10 ; gate:g14 ; gate:g04 }
TASK T1.8 "measure budgets"
  SCOPE { ROOT/docs/budgets.md, ROOT/tools/perf/** }
  DONE WHEN { every BUDGET.* [MEASURE] item has a measured value recorded and either confirmed or amended by a DECIDE entry }

## PHASE P2  "Editor"   EFFORT L
PHASE P2
  MODULES { T01, T02, T03(sync), T04, T06 }
  NOT_DO  { velocity-lane advanced editing, tempo curves, audio clips }
  EXIT    { MS-B ; scenario: 4 lanes x 16 bars composed without touching JSON, notes moved across lanes change the instrument ; ac:T01.AC1 ; ac:T01.AC2 ; BUDGET.edit_latency met ; ac:T06.AC1 }
  NOTE    timbres in this phase come from OscSynth (audible feedback only) ; real GM timbres arrive in P3

TASK T2.1 "document model and queries"
  SCOPE { ROOT/packages/document/** }   READS { abi types }
  DONE WHEN { queries and structural-sharing updates unit-tested ; load/save conversion to tuples round-trips }
TASK T2.2 "commands, history, overlap policy"
  SCOPE { ROOT/packages/editing/** }   READS { document }
  DONE WHEN { ac:T01.AC1 ; ac:T01.AC2 ; ac:T01.AC3 ; ac:T01.AC4 ; ac:T01.AC5 }
TASK T2.3 "grid rendering (Compact, Roll, Drum), ruler, playhead"
  SCOPE { ROOT/packages/ui-grid/src/render/** }   READS { document(readonly) }
  DONE WHEN { culling and hit-test tests ; ac:T02.AC3 ; fps budget on the browser harness }
TASK T2.4 "input to Intent, keymap, clipboard"
  SCOPE { ROOT/packages/ui-grid/src/input/**, ROOT/packages/editing/src/clipboard/** }   READS { editing(types of Intent only) }
  DONE WHEN { ac:T02.AC2 ; ac:T02.AC4 ; ac:T02.AC5 }
TASK T2.5 "lane header, program palette, all-programs view"
  SCOPE { ROOT/packages/ui-grid/src/palette/**, ROOT/packages/ui-grid/src/lane/** }   READS { generated GM JSON }
  DONE WHEN { palette lists 16 families / 128 programs ; EnsureProgramLanes creates missing lanes ; ac:T02.AC1(lane-move part) }
TASK T2.6 "document to schedule sync, audition"
  SCOPE { ROOT/packages/playback/src/sync/** }   READS { editing, playback interfaces }
  DONE WHEN { ac:T03.AC2 ; ac:T03.AC3 ; ac:T03.AC4 }
TASK T2.7 "save/open/autosave/recovery, validation panel"
  SCOPE { ROOT/packages/persistence/**, ROOT/packages/app/src/panels/validation/** }
  DONE WHEN { ac:T04.AC1 ; ac:T04.AC2 ; ac:T04.AC3 ; ac:T04.AC5 ; ac:T04.AC6 }
TASK T2.8 "app composition, layout, settings, local log"
  SCOPE { ROOT/packages/app/** }   READS { all package interfaces }
  DONE WHEN { ac:T06.AC2 ; ac:T06.AC4 ; ac:T06.AC5 ; gate:g10 }

## PHASE P3  "SoundFont (GM 128)"   EFFORT L
PHASE P3
  MODULES { R03, R04(SoundFont), T03(bank), T04(bank cache), T07 }
  EXIT    { MS-C ; real GM bank supplied by the user: 128 programs + Standard kit audible ; ac:R03.AC4 ; ac:R04.AC4 ; ac:R03.AC5 ; BUDGET.worklet_cost met or voice cap amended }
  NOTE    no bundled bank: before a bank is loaded OscSynth plays ; a non-CORS link falls back to LocalFileSource (R15)

TASK T3.0 "spikes SP-3 and SP-4, then fix the implementation route"
  SCOPE { ROOT/docs/spikes/**, ROOT/spikes/** }
  DONE WHEN { reports with PASS/FALLBACK ; DECIDE D09 annotated with the chosen route }
TASK T3.1 "SF2 parser, flattening, SoundbankImage, sf2_builder, fuzz"
  SCOPE { ROOT/crates/ng-bank/** }   READS { ng-core }
  DONE WHEN { ac:R03.AC1 ; ac:R03.AC2 ; ac:R03.AC3 }
TASK T3.2 "SoundFontSynth (or adapter) passing synth_contract"
  SCOPE { ROOT/crates/ng-synth/src/soundfont/**, ROOT/crates/ng-synth/tests/** }   READS { ng-bank, ng-dsp }
  DONE WHEN { ac:R04.AC3 ; gate:g15 ; gate:g05 }
TASK T3.3 "bank ABI, bank worker, stopped-state loading"
  SCOPE { ROOT/crates/ng-wasm/src/bank/**, ROOT/packages/playback/src/bank/** }
  DONE WHEN { ac:T03.AC5 ; ng_bank_presets reports coverage }
TASK T3.4 "SoundbankSource: UrlSource, LocalFileSource, link presets, bank manager UI, cache, offline restore"
  SCOPE { ROOT/packages/bank-source/**, ROOT/packages/persistence/src/bank/**, ROOT/packages/app/src/bank-manager/** }
  DONE WHEN { ac:T07.AC1 ; ac:T07.AC2 ; ac:T07.AC3 ; ac:T07.AC4 ; ac:T04.AC4 }
TASK T3.5 "128-program golden renders and listening notes"
  SCOPE { ROOT/fixture/golden/soundfont/**, ROOT/docs/listening_notes.md }
  DONE WHEN { every program + kit renders non-silent, no NaN (energy golden) ; listening evaluation recorded }

## PHASE P4  "Export and MIDI I/O"   EFFORT M-L
PHASE P4
  MODULES { R06(MIDI export + import), T05, T04(MIDI open), T06(export dialog) }
  EXIT    { MS-D ; WAV/OGG/MIDI exports from the example and from a real song ; ac:R06.AC4 ; ac:T05.AC2 ; ac:T05.AC3 ; ac:T05.AC4 ; ac:T05.AC5 ; ac:T05.AC6 ; gate:g13 }

TASK T4.1 "MidiExporter with channel plan and multi-port"
  SCOPE { ROOT/crates/ng-export/src/midi_export/** }   READS { ng-core, ng-score }
  DONE WHEN { ac:R06.AC2 ; ac:R06.AC3 }
TASK T4.2 "MidiImporter"
  SCOPE { ROOT/crates/ng-export/src/midi_import/** }
  DONE WHEN { ac:R06.AC4 ; imported results always pass validate ; W_IMPORT_TRIMMED reported }
TASK T4.3 "spike SP-5, then OGG encoder module and adapter"
  SCOPE { ROOT/vendor/vorbis/**, ROOT/packages/export-service/src/ogg/**, ROOT/docs/spikes/** }
  DONE WHEN { spike report ; OGG decodes in an independent decoder ; streaming encode keeps memory flat ; licence notice generated }
TASK T4.4 "export orchestration, encoder and sink registries"
  SCOPE { ROOT/packages/export-service/src/** except ogg/ }
  DONE WHEN { ac:T05.AC1 ; ac:T05.AC4 ; T05.R03 verified by adding a dummy encoder in a test }
TASK T4.5 "OutputSink: DownloadSink, DirectorySink, C12 vectors; FsSink in CLI"
  SCOPE { ROOT/packages/export-service/src/sink/**, ROOT/crates/ng-cli/src/sink/**, ROOT/fixture/validation/paths/** }
  DONE WHEN { ac:T05.AC6 ; ac:X01.AC6 ; atomic write verified (kill during write leaves no partial file) ; overwrite policy implemented in UI and CLI }
TASK T4.6 "loop export: tail_fold, sidecar, comments"
  SCOPE { ROOT/crates/ng-render/src/offline/**, ROOT/packages/export-service/src/loop/** }
  DONE WHEN { ac:R05.AC2 ; ac:T05.AC3 }
TASK T4.7 "export dialog and MIDI open UI"
  SCOPE { ROOT/packages/app/src/export/**, ROOT/packages/app/src/open/** }
  DONE WHEN { dialog offers formats, rate, depth/quality, loop, include-muted, destination sink, overwrite policy ; .mid opens through ng_import_midi with warnings shown }
TASK T4.8 "external verification"
  SCOPE { ROOT/docs/verification/** }
  DONE WHEN { checklist results recorded: independent decoders, external MIDI players, copy music_output -> core/assets/audio/music and Godot 4.8-dev6 import (Loop only; Loop Offset with intro; overwrite keeps .import) }

## PHASE P5  "Hardening"   EFFORT M
PHASE P5
  MODULES { R04(tuning), T06(PWA, single-file), T04, X01 }
  EXIT    { MS-E ; every BUDGET.* met or amended with a DECIDE ; offline scenarios pass on ENGINES ; real-Safari checklist recorded ; ac:T06.AC3 }

TASK T5.1 "reverb and limiter tuning as data tables"
  SCOPE { ROOT/crates/ng-dsp/src/reverb/**, ROOT/crates/ng-render/src/master/**, ROOT/fixture/golden/** }
  DONE WHEN { listening notes recorded ; golden changes logged ; ac:R04.R04 }
TASK T5.2 "PWA offline completion and update flow"
  SCOPE { ROOT/packages/app/src/pwa/** }
  DONE WHEN { ac:T06.AC1 ; ac:T06.AC3 }
TASK T5.3 "cross-browser final pass and storage-durability warnings"
  SCOPE { ROOT/packages/persistence/**, ROOT/packages/app/src/settings/**, ROOT/docs/verification/safari.md }
  DONE WHEN { warnings for >= 7 days since last file save shown ; gate:g14 ; real-Safari checklist recorded }
TASK T5.4 "spike SP-6 and optional single-file build"
  SCOPE { ROOT/xtask/**, ROOT/packages/app/build/** }
  DONE WHEN { spike report ; single-file build shipped or PWA-only documented }
TASK T5.5 "SIMD build selection, performance and size pass"
  SCOPE { ROOT/crates/ng-wasm/**, ROOT/crates/ng-dsp/**, ROOT/xtask/** }
  DONE WHEN { gate:g04 ; BUDGET.worklet_cost ; BUDGET.offline_speed ; byte-identical output between SIMD and non-SIMD builds or documented per-build goldens }
TASK T5.6 "accessibility, keyboard, open-source notices, user docs"
  SCOPE { ROOT/packages/app/**, ROOT/docs/user/** }
  DONE WHEN { ac:T02.AC4 ; notices screen lists libvorbis/libogg and Rust/TS dependencies ; user guide written }

## PHASE P6 (optional)  "Game hand-off helpers"   EFFORT S-M
PHASE P6
  MODULES { T05 }
  EXIT    { stems and optional manifest usable ; Godot runtime snippet verified }
TASK T6.1 "per-lane stem export (solo_lane repeated)"
  SCOPE { ROOT/packages/export-service/src/stems/** }
  DONE WHEN { stems export per lane ; sum of stems matches the mix before the limiter within 1e-5 }
TASK T6.2 "optional manifest and runtime loader snippet"
  SCOPE { ROOT/packages/export-service/src/manifest/**, ROOT/docs/godot_snippet.md }
  DONE WHEN { manifest {id,file,bpm,duration,source_hash} written next to exports ; snippet reading the sidecar and setting loop/loop_offset verified on Godot 4.8-dev6 }

## RISKS
RISK R1  AudioWorklet+wasm constraints                impact high  mitigation spike:SP-1, C ABI (D05)
RISK R2  native vs wasm byte differences              impact med   mitigation spike:SP-2, libm pinned, gate:g05
RISK R3  soundfont licence and size                   impact high  mitigation user supplies the bank ; notice in bank manager ; image <= 64 MB recommended
RISK R4  SoundFont synth effort                       impact med   mitigation spike:SP-3 gate ; v1/v2 generator staging
RISK R5  memory (image copies per thread, SF3 growth) impact med   mitigation spike:SP-4 ; only worklet and offline hold images
RISK R6  audio-thread allocation or panic             impact high  mitigation R-RT rules, gate:g15, gate:g01
RISK R7  Vorbis wasm build/licence/size               impact med   mitigation spike:SP-5, lazy loading
RISK R8  grid performance (thousands of notes, 128 lanes) impact med mitigation culling, layers, virtualisation, budget in CI harness
RISK R9  Rust toolchain/learning cost (solo)          impact med   mitigation crate boundaries, contract tests validate AI-written code
RISK R10 tooling outgrows real song production        impact med   mitigation NOT_DO lists ; compose one real song at MS-B and re-check requirements
RISK R11 browser differences (Safari/Firefox)         impact med   mitigation spike:SP-1 measurements, C11 feature detection, 3-engine CI, g14
RISK R12 browser storage purge (Safari inactivity)    impact high  mitigation file save as primary safety net, last-saved warning, PWA hint, persist() request
RISK R13 Godot 4.8 development snapshots change       impact low   mitigation depend only on import-dock Loop and the sidecar ; rerun checklist on each snapshot and on 4.8 stable
RISK R14 coupling/noise from living inside the game repo impact med mitigation DR-1..DR-8, .gdignore, g13, path-filtered CI
RISK R15 bank link host not CORS-enabled              impact med   mitigation clear error + LocalFileSource fallback ; user-chosen link presets

## DEFINITION OF DONE (every phase)
DONE_PHASE { gates active for the phase pass ; all listed ac:* hold or are deferred with a recorded reason ; contract text and code agree ; new risks/decisions recorded ; a demonstrable build exists }
