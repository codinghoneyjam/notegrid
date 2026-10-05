MODULE R01 core_score
  LANG rust ; PHASE P0 ; CRATES { ng-core, ng-score }
  DEPENDS_ON { ng-core: {} ; ng-score: { ng-core } }
  DEPENDED_BY { every crate }
  PURPOSE document data structures + invariant validation + migration + canonical serialisation + GM tables ; base types and contract traits
  OWNS    { Project model, validate(I-xx), migrate chain, canonical JSON, GM tables(128 programs, 16 families, drum key names), ScoreExporter/ScoreImporter traits, Synth/AudioEncoder traits }
  NOT_OWNS { time conversion(R02), sound(R04), file-format export(R06) }
  DESIGN
    model Project{title, tempo:Vec<{tick,mpq}>, meter:Vec<{tick,num,den}>, loop_region, length_ticks, lanes:Vec<Lane>, notes:Vec<Note{lane,start,dur,pitch,vel}>, ext:BTreeMap<String,Value> /*x_ fields, ordered*/}
    serde + hand-written canonical writer ; input parsed to serde_json::Value, version-dispatched, then converted
    schema authority = schema/project.schema.json (maintained by hand) ; Rust implements structure parsing + I-xx itself ;
      shared vectors fixture/validation/*.json {input, expect:[{code,path}]} run through BOTH Rust and a JSON-Schema validator (gate:g09)
    TS types generated from the schema (json-schema-to-typescript)
    migrate: fn(Value)->Value chain v1->v2->..., currently v1 only (empty chain + test scaffold) ; higher version -> E_VERSION_TOO_NEW
    GM tables: static arrays exposed by ng_gm_tables as JSON ; TS consumes generated output only (D11) ; drum key names GM1 35-81 plus GM2 27-34/82-87 [VERIFY name table]
  REQUIRE
    R01.R01 validate collects every error/warning (never stops early), path = JSON Pointer
    R01.R02 never panics on any input (malformed JSON, huge values) ; failures are Result
    R01.R03 canonical serialisation deterministic ; load->save->load stable (C0.2)
    R01.R04 ng-core has zero third-party dependencies
    R01.R05 lane order = array order ; LaneId is a stable identity independent of order
    R01.R06 limits C1.LIMIT enforced
  ACCEPT
    R01.AC1 fixture/example_project.json passes schema and I-xx ; canonical output equals input bytes (or the documented normalisation golden)
    R01.AC2 >= 40 negative vectors (I-01..I-10, W-1..W-3, schema violations): Rust and schema agree on verdict
    R01.AC3 fuzzing (1e6 random/truncated/nested inputs) -> 0 panics
    R01.AC4 ng_gm_tables returns 128 programs and 16 families
  TEST proptest(random project -> canonical round trip), shared vectors, fuzz
  RISK serde_json size in wasm [MEASURE] ; over budget -> hand-written parser behind the same API
END
