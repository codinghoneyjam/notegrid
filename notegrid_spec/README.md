# NoteGrid spec (DSL edition)

Working name. Offline browser composer for game music: time on the x-axis, instrument lanes on the y-axis,
MIDI GM 128 programs plus drum kits, export to WAV / OGG / MIDI, import MIDI.
Rust (WASM core) + TypeScript (UI) + C (libvorbis, OGG only). Lives inside the Godot game repo as a fully decoupled module (`tools/notegrid`).

## Files
| file | content |
|---|---|
| `00_dsl.dsl` | grammar, global sets, TASK state machine, GLOBAL_DENY, gates, budgets |
| `01_system.dsl` | goals, terms, decisions, architecture (crates/packages/threads), SOLID map, repository layout, decoupling rules |
| `02_contracts.dsl` | interface contracts C0-C12 (units, project format, traits, schedule binary, C ABI, worklet protocol, TS interfaces, realtime rules, export formats, errors, browser and output-safety contracts) |
| `03_roadmap.dsl` | phases P0-P6 as a state machine, TASK blocks (SCOPE / READS / DONE WHEN), spikes, risks |
| `module/*.dsl` | 15 modules: `R01-R07` Rust, `T01-T07` TypeScript, `X01` cross-cutting |
| `schema/project.schema.json` | project file schema (structure) |
| `fixture/example_project.json` | example passing schema and invariants |

## Reading order
`00_dsl` -> `01_system` -> `02_contracts` -> module you implement -> your TASK in `03_roadmap`.

## Rules for the implementer
- A TASK may touch only its SCOPE. Needing more -> state BLOCKED and report; never widen SCOPE yourself.
- Anything not defined in a CONTRACT or MODULE is undefined -> BLOCKED, not guessed.
- `[VERIFY]` = confirm the external fact before relying on it. `[MEASURE]` = target that becomes a hard budget after measurement.
- There are no open questions. Remaining uncertainty is isolated in SPIKE blocks with PASS / FALLBACK rules.
