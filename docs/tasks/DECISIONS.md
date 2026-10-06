# Decisions taken outside the spec

Decisions made while implementing, that the spec does not (or did not) record. Each entry must
either be folded into `spec/dsl/` or stand with a stated reason.

A DECIDE entry in `01_system.dsl` is the canonical home; this file is the staging area for
decisions that have not been promoted there yet.

Format, newest first:

```
## <date> — <one line>
Affects:   <task id, crate, or gate>
Decision:  <what was chosen>
Because:   <why, and what the alternatives cost>
Status:    <promoted to spec / open>
```

---

## 2026-10-06 — g01 hotpath lint boundary is crate-wide

Affects: gate g01, `crates/ng-dsp`, `crates/ng-synth`, `crates/ng-render`

Decision: `SET HOTPATH = { ng-dsp, ng-synth, ng-render }` is exactly the set that opts into
`[lints] workspace = true`. No marker attribute, so a new hot-path function cannot opt itself out.

Because: the alternative — a marker attribute naming individual functions — has to be added before
each hot function is written, and forgetting it is silent. Crate-wide has no per-function bookkeeping.
`ng-wasm` needs no exemption: it is not in SET HOTPATH, and its `unsafe` / `extern "C"` isolation is
already enforced by g03.

Status: **open** — `00_dsl.dsl` declares the deny list but does not say how the boundary is drawn.
The implementation and its check are in `scripts/check_hotpath_lints.py`.

## 2026-10-06 — `ALL crates` is an upper bound, not an obligation

Affects: gate g03, `01_system.dsl` ARCHITECTURE

Decision: a crate declared `DEPENDS_ON { ALL crates }` may depend on any existing crate and on none
of them yet. An explicit dependency list is exact in both directions; `ALL` is a ceiling only.

Because: requiring `ng-cli` to depend on `ng-timeline` the day `ng-timeline` appears makes gate g03
unsatisfiable during the very task that creates it. Both directions still fail on *undeclared*
deps, which is the direction that matters for decoupling.

Status: **promoted** — implemented in `scripts/spec_deps.py`; the semantics live in its docstring.

## 2026-10-06 — R02.R02 is enforced by type, not by grep

Affects: `crates/ng-timeline`, `module/R02_timeline.dsl` R02.R02

Decision: `TempoMap` holds `mpq` values and never sees a float `bpm`; `from_project` routes the
bpm→mpq conversion through `ng_score::TempoItem::mpq()`. No other crate can assemble a
tick→sample conversion at all.

Because: the spec described R02.R02 as a "grep-deny" rule ("`* sample_rate` together with `bpm`"),
which a later rename of an intermediate variable defeats silently. Closing the path with types
means there is no path to defeat. C0.2 keeps a single definition of the bpm→mpq conversion as a
side effect.

Status: **open** — R02.R02 should be restated in the module file; the grep rule is superseded.

## 2026-10-06 — task state lives in the spec, not in a status file

Affects: `spec/dsl/03_roadmap.dsl`, `docs/tasks/`

Decision: `STATE` lines in the roadmap are the only record of task progress. `docs/tasks/board.md`
is generated, and `scripts/check_task_state.py --check` fails CI if it is stale or if a `DONE`
claim is not backed by the workspace.

Because: this repo has twice recorded progress that the code did not support (g01's unimplemented
lint clause; T1.1–T1.8 recorded `DONE` with no crates). See `docs/tasks/README.md`.

Status: **open** — the roadmap's own header documents the vocabulary; the gate is not yet listed
in `00_dsl.dsl` GATES.
