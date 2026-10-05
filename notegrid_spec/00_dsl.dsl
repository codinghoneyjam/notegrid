# 00_dsl.dsl — grammar, global sets, global FSM, global DENY, gates
# All other .dsl files in this package are written in this grammar.
# Audience: the AI/human implementer. Behaviour not defined here or in a CONTRACT is undefined -> BLOCKED.

## GRAMMAR
# line      := KEYWORD args | indented continuation
# block     := KEYWORD id <newline> indented-body        (2-space indent)
# set       := { a, b, c }      ops: UNION, MINUS, INTERSECT
# path set  := globs relative to ROOT unless prefixed with REPO:
# tags      := [VERIFY]  external fact/behaviour must be confirmed before relying on it
#              [MEASURE] numeric target; becomes a hard budget after measurement in the stated phase
# refs      := C<n>.<m> contract rule | I-<nn> project invariant | R-RT-<n> realtime rule
#              <MODULE>.R<nn> requirement | <MODULE>.AC<n> acceptance | gate:<id> | spike:<id> | task:<id>
# keywords  := GOAL NON_GOAL TERM DECIDE SET EDGE CRATE PACKAGE MODULE API INVARIANT RULE REQUIRE ACCEPT
#              TEST DESIGN FSM STATE GATE GLOBAL_DENY TASK SCOPE READS ALLOW DENY FLOW DONE WHEN
#              PHASE SPIKE RISK MILESTONE EXIT NOT_DO DEFAULT

## GLOBAL SETS
SET ROOT        = tools/notegrid
SET CRATES      = { ng-core, ng-score, ng-timeline, ng-dsp, ng-bank, ng-synth, ng-render, ng-export, ng-wasm, ng-cli }
SET PACKAGES    = { abi, document, editing, ui-grid, playback, bank-source, export-service, persistence, app }
SET HOTPATH     = { ng-dsp, ng-synth, ng-render }
SET ASSEMBLY    = { ng-wasm, ng-cli, app }
SET OWN_IO      = { ROOT/music_source, ROOT/music_output }
SET GAME_PATHS  = { res://, core/, entity/, ui/, world/, project.godot }
SET ENGINES     = { chromium, firefox, webkit }
SET FORMATS_OUT = { wav, ogg, midi }

## TASK FSM  (every TASK runs this machine unless it overrides FLOW)
FSM TASK_FLOW
  STATE ORIENT, TEST_FIRST, IMPLEMENT, VERIFY, DONE, BLOCKED
  START ORIENT
  ORIENT     --contract_and_reads_loaded-->            TEST_FIRST
  TEST_FIRST --tests_written_and_failing-->            IMPLEMENT
  IMPLEMENT  --code_complete-->                        VERIFY
  VERIFY     --all_gates_pass AND done_when_met-->     DONE
  VERIFY     --any_gate_fails-->                       IMPLEMENT
  ANY        --needs_change_outside_SCOPE-->           BLOCKED      # stop and report; never widen SCOPE yourself
  ANY        --contract_undefined_or_contradictory-->  BLOCKED
  BLOCKED    --scope_or_contract_amended_by_owner-->   ORIENT
END

## TASK DEFAULTS (inherited by every TASK; a TASK may only ADD to ALLOW and READS, never remove GLOBAL_DENY)
DEFAULT TASK.ALLOW { create|edit files inside SCOPE ; read files inside READS ; run gates ; add tests inside SCOPE }
DEFAULT TASK.FLOW  TASK_FLOW
DEFAULT TASK.DONE_WHEN { gates:ACTIVE(phase) pass ; every listed ac:* holds ; no GLOBAL_DENY violated }

## GLOBAL_DENY
GLOBAL_DENY
  edit outside TASK.SCOPE
  add a dependency not listed in MODULE.DEPENDS_ON (crate, package, or third-party)
  `extern "C"` or `unsafe` outside ng-wasm
  `unwrap` | `expect` | `panic!` | slice indexing that can panic, inside HOTPATH
  heap allocation reachable from render_block / ng_engine_render after init
  float transcendental call not routed through the `libm` crate (HOTPATH)
  HashMap/HashSet iteration order dependence; wall-clock time; unseeded randomness (anywhere in render or export)
  write outside OWN_IO ; delete or move any file ; write via a path not produced by C12 normalisation
  any string from GAME_PATHS or any `../` escape inside ROOT sources
  reference ROOT from game sources (*.gd, *.tscn, project.godot)
  edit golden fixtures without an entry in docs/golden_changes.md
  weaken, skip, or delete a contract test or gate to make a gate pass
  invent behaviour absent from the CONTRACT (-> BLOCKED instead)
  `UserAgent` sniffing (feature detection only)
  SharedArrayBuffer or cross-origin-isolation dependence
END

## GATES
GATE g01_fmt_clippy    ACTIVE_FROM P0  RUN cargo fmt --check ; cargo clippy -D warnings ; hotpath lints deny { unwrap_used, expect_used, indexing_slicing, panic }
GATE g02_rust_tests    ACTIVE_FROM P0  RUN cargo test (unit, proptest, contract tests, fuzz smoke)
GATE g03_crate_deps    ACTIVE_FROM P0  RUN script: cargo metadata graph == union of MODULE.DEPENDS_ON; `extern "C"` only in ng-wasm
GATE g04_wasm_size     ACTIVE_FROM P1  RUN cargo xtask build-wasm ; size(ng-wasm.wasm excluding bank) <= BUDGET.wasm_size ; on fail print top growth symbols
GATE g05_native_vs_wasm ACTIVE_FROM P0 RUN same inputs through ng-cli and through wasm in Node ; outputs byte-identical
GATE g06_ts_checks     ACTIVE_FROM P0  RUN tsc --noEmit ; eslint ; vitest (incl. fast-check properties)
GATE g07_ts_deps       ACTIVE_FROM P0  RUN dependency-cruiser: package graph == union of MODULE.DEPENDS_ON ; no cycles
GATE g08_abi_exports   ACTIVE_FROM P0  RUN set(wasm exports) == set(@ng/abi expected) ; ng_abi_version matches
GATE g09_schema_vectors ACTIVE_FROM P0 RUN shared vectors fixture/validation/*.json : Rust validate == JSON Schema verdict, same code+path
GATE g10_e2e_smoke     ACTIVE_FROM P1  RUN playwright on ENGINES: load -> place note -> play (worklet block non-silent) -> export (bytes verified) ; offline mode variant
GATE g11_license       ACTIVE_FROM P0  RUN cargo-deny ; license-checker (allow-list)
GATE g12_golden_reason ACTIVE_FROM P1  RUN any changed file under fixture/golden/** requires a new dated entry in docs/golden_changes.md
GATE g13_decoupling    ACTIVE_FROM P0  RUN scripts/check_decoupling: DR-1, DR-2, DR-6, DR-8 ; malicious-name/path vectors rejected by every sink
GATE g14_engine_identical ACTIVE_FROM P1 RUN same project rendered offline on each of ENGINES ; WAV bytes identical (C11.3)
GATE g15_rt_rules      ACTIVE_FROM P1  RUN counting-allocator test: 0 allocations in render_block after warm-up ; denormal test (R-RT-3)

## BUDGET  [MEASURE] — measured in P1, then hard
BUDGET wasm_size        <= 1 MB (wasm-opt -O3, bank excluded)
BUDGET worklet_cost     <= 30% of block time at 64 voices, 128 frames @ 44.1 kHz (2.9 ms block)
BUDGET offline_speed    >= 20x realtime (3 min, 16 lanes, stereo)
BUDGET edit_latency     undo/redo <= 50 ms @ 5000 notes ; grid >= 30 fps @ 5000 visible notes ; single add <= 5 ms
BUDGET bank_load_cached <= 1 s @ 64 MB image
BUDGET ci_time          full CI <= 15 min ; local test-all <= 5 min

## GOLDEN CHANGE LOG FORMAT (docs/golden_changes.md)
ENTRY := date | files | reason | impact
