# 01_system.dsl — product definition, decisions, architecture, repository layout

## PRODUCT
GOAL G1 offline browser app, no server; installable PWA; optional single-file build
GOAL G2 note-placement composer: x-axis = time, y-axis = instrument lanes; lane expands into a pitch roll
GOAL G3 MIDI GM 128 programs (0-127) plus drum kits (drum lane kind)
GOAL G4 export WAV, OGG(Vorbis), MIDI ; import MIDI
GOAL G5 audio export uses the same engine as preview (no second synth)
GOAL G6 desktop browsers: Chrome, Edge, Firefox, Safari — latest 2 versions each
GOAL G7 lives inside the Godot game repo as a fully decoupled module (ROOT = tools/notegrid)
NON_GOAL server, cloud sync, collaboration, VST/AU, audio-clip recording tracks, live mic input,
         generative-AI composition, mobile/tablet/touch UI, notation (staff) view,
         pitch-bend/modulation curve editing (data model must stay extensible),
         OAuth-based cloud-drive bank sources (extension point: SoundbankSource)

## TERMS
TERM tick       integer time unit, PPQ = 480 (quarter note = 480 ticks); the only source of truth for time
TERM lane       instrument assignment unit AND one vertical row; kind = melodic(program 0-127) | drum(kit)
TERM note       (lane, start_tick, duration_ticks, pitch 0-127, velocity 1-127)
TERM schedule   binary compile of a project into sample-positioned events (contract C3); shared by preview and export
TERM image      SoundbankImage: parsed soundbank in a load-cheap binary form
TERM engine     one ng-wasm instance (AudioWorklet or Worker) driving Renderer + Synth
TERM sink       OutputSink: where exported bytes go (download | picked directory | CLI filesystem inside ROOT)

## DECISIONS (final)
DECIDE D01 preview and export call the same Rust render core (worklet = realtime blocks, worker = offline full render)
DECIDE D02 time = integer ticks; tempo = integer microseconds per quarter (mpq); tick->sample accumulates in u128 and rounds ONCE at the end
DECIDE D03 notes belong to lanes; the instrument belongs to the lane; an "all programs" view = a view feature that auto-creates one lane per program
DECIDE D04 the live document lives in TS memory; schema, validation, migration, canonical serialisation live in Rust
DECIDE D05 WASM exposed through a C ABI without wasm-bindgen, thin hand-written TS wrapper [VERIFY: AudioWorkletGlobalScope lacks TextDecoder, so bindgen glue is not usable as-is]
DECIDE D06 two encoder abstractions: AudioEncoder (PCM -> bytes: WAV, OGG) and ScoreExporter (project -> bytes: MIDI)
DECIDE D07 OGG encoding = libvorbis compiled to its own WASM module behind a TS AudioEncoder adapter, lazy-loaded
DECIDE D08 soundbank parsed once in a worker into an image; image sent to engines; image cached in IndexedDB
DECIDE D09 Synth is a trait: OscSynth (instant sound, test baseline) + SoundFontSynth; SoundFontSynth = build-vs-adopt gate (spike:SP-3)
DECIDE D10 same-lane same-pitch overlap is an invariant violation (editor auto-trims); the compiler never resolves overlaps
DECIDE D11 music-theory data (GM names, families, drum key names) originate in ng-score and are generated for TS
DECIDE D12 sample positions cross the ABI as f64 (exact below 2^53)
DECIDE D13 no bundled soundbank; acquired through SoundbankSource: UrlSource (external HTTPS link, CORS required) primary, LocalFileSource fallback; cached after first acquisition
DECIDE D14 supported: desktop Chrome/Edge/Firefox/Safari, latest 2; feature detection with fallbacks; CI on 3 engines; WASM output byte-identical across engines
DECIDE D15 placement: ROOT inside the game repo; sources and outputs inside ROOT; two-way reference count = 0; contact surface = copying files
DECIDE D16 game integration: OGG default; Godot import-dock Loop (+ Loop Offset when an intro exists); sidecar JSON auxiliary; WAV smpl optional; target Godot 4.8-dev6
DECIDE D17 MIDI import in v1, built in P4 next to the exporter
DECIDE D18 write scope = ROOT/music_source and ROOT/music_output only; reflecting outputs into the game folder is a manual copy (extension point: a game-side script OUTSIDE ROOT)

## DEFAULTS (not open questions)
DEFAULT game_sync       manual copy  ROOT/music_output -> core/assets/audio/music
DEFAULT bank_url        entered by the user at runtime; may be saved as a named link preset (name, url, optional sha256, note)
DEFAULT bank_fallback   if the link is not CORS-enabled -> user downloads the file and uses LocalFileSource

## ARCHITECTURE
CRATE ng-core      DEPENDS_ON {}                                         OWNS primitives(Tick,LaneId,Slot), base errors, traits(Synth=NoteSink+ChannelMix+AudioSource, AudioEncoder), PcmBuffer
CRATE ng-score     DEPENDS_ON { ng-core }                                OWNS Project model, validate(I-xx), migrate, canonical JSON, GM tables, traits(ScoreExporter, ScoreImporter)
CRATE ng-timeline  DEPENDS_ON { ng-core, ng-score }                      OWNS TempoMap, compile(project,sr)->Schedule, schedule (de)serialisation
CRATE ng-dsp       DEPENDS_ON { ng-core }                                OWNS Envelope, Ramp, interpolation, filters, Reverb, SoftLimiter, seeded Lcg
CRATE ng-bank      DEPENDS_ON { ng-core }                                OWNS SF2/SF3 parser, SoundbankImage build/load, preset lookup
CRATE ng-synth     DEPENDS_ON { ng-core, ng-dsp, ng-bank }               OWNS OscSynth, SoundFontSynth
CRATE ng-render    DEPENDS_ON { ng-core, ng-timeline, ng-dsp }           OWNS Renderer<S: Synth>, render_offline, master bus (knows the Synth TRAIT only, never an implementation)
CRATE ng-export    DEPENDS_ON { ng-core, ng-score }                      OWNS WavEncoder, MidiExporter, MidiImporter
CRATE ng-wasm      DEPENDS_ON { ALL other crates }                       OWNS C ABI facade + composition (only crate allowed `extern "C"` / `unsafe`)
CRATE ng-cli       DEPENDS_ON { ALL other crates }                       OWNS native headless tool, FsSink limited to OWN_IO

PACKAGE abi            DEPENDS_ON {}                                      OWNS wasm loader, typed ABI wrapper, generated schema types, GM JSON
PACKAGE document       DEPENDS_ON { abi(types) }                          OWNS Document model, immutable updates, queries
PACKAGE editing        DEPENDS_ON { document }                            OWNS Command, History, selection, clipboard, snap, quantize, Intent handling
PACKAGE ui-grid        DEPENDS_ON { document(readonly) }                  OWNS canvas rendering, viewport, hit-test, input -> Intent   FORBIDDEN { editing, playback }
PACKAGE playback       DEPENDS_ON { abi }                                 OWNS AudioEngine (worklet), schedule sync policy            FORBIDDEN { ui-grid }
PACKAGE bank-source    DEPENDS_ON { abi(types) }                          OWNS SoundbankSource implementations                        FORBIDDEN { playback, persistence }
PACKAGE export-service DEPENDS_ON { abi }                                 OWNS offline render orchestration, AudioEncoder adapters, OutputSink implementations
PACKAGE persistence    DEPENDS_ON { abi }                                 OWNS ProjectStore implementations, autosave, bank image cache
PACKAGE app            DEPENDS_ON { ALL other packages }                  OWNS composition root, layout, PWA, settings               ONLY place that imports implementations of interfaces

EDGE direction: consumers own abstractions; ng-render -> trait Synth <- ng-synth ; playback/persistence/export-service -> interfaces <- app wiring

## RUNTIME THREADS
THREAD main        { UI, editing, persistence, compile_schedule, validate, MIDI export }  NOT { audio rendering }
THREAD worklet     { ng_engine_render (128-frame blocks), message handlers }              NOT { allocation in process(), locks, parsing, panics }
THREAD bank_worker { bank parse -> image }
THREAD offline     { full render with progress/cancel, WAV encode }
THREAD ogg_worker  { libvorbis encode, PCM streamed in 8192-frame chunks }
COMM postMessage with transferables ; no SharedArrayBuffer ; worklet reports position ~30 Hz as (sample, tick, playing)

## SOLID MAP
RULE S  one reason to change per crate/package (parse != synth != drive != serialise != UI)
RULE O  new format = new AudioEncoder/ScoreExporter impl ; new voice engine = new Synth impl ; new edit = new Command ; new bank source = new SoundbankSource ; new output = new OutputSink
RULE L  every Synth impl passes the same synth_contract tests before it may be swapped in
RULE I  Synth = NoteSink + ChannelMix + AudioSource ; AudioEngine knows transport only ; AudioEncoder and ScoreExporter are separate
RULE D  Renderer depends on trait Synth ; TS depends on AudioEngine / ProjectStore / AudioEncoder / SoundbankSource / OutputSink ; implementations are injected only in app and ng-wasm/ng-cli

## REPOSITORY LAYOUT
LAYOUT
  <game-repo>/project.godot, core/, entity/, ui/, world/ ...          # game, untouched
  <game-repo>/tools/.gdignore                                          # Godot never scans tools/
  <game-repo>/tools/notegrid/                                          # ROOT
    notegrid.root                                                      # root marker for CLI write-scope checks
    music_source/                                                      # project files (*.ngproj.json), committed
    music_output/                                                      # exports (wav/ogg/mid/loop.json), gitignored
    crates/ packages/ xtask/ vendor/vorbis/ schema/ fixture/ docs/ scripts/
    README.md LICENSE CHANGELOG.md                                     # tool-owned
  <game-repo>/core/assets/audio/music/                                 # game import location, filled by manual copy
END
RULE DR-1 game sources never mention ROOT
RULE DR-2 ROOT sources never mention GAME_PATHS ; no game-specific config files exist in ROOT
RULE DR-3 contact surface = files copied from music_output to the game folder
RULE DR-4 independent build/CI with path filter ROOT/** ; node_modules, target, dist, music_output are gitignored
RULE DR-5 independent versioning: tag notegrid-vX.Y.Z
RULE DR-6 extractable: no relative path leaves ROOT (git subtree split --prefix=tools/notegrid works)
RULE DR-7 repo-wide analysis tools (file-tree / line-count) must exclude ROOT (node_modules/target would distort statistics)
RULE DR-8 writes only inside OWN_IO; CLI rejects --out outside ROOT; no escape flag (see C12)
RULE godot_import copying over the same file name normally keeps the existing .import settings (Loop etc.) [VERIFY on 4.8-dev6]
