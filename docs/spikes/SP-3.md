# SP-3 spike report -- SoundFontSynth: build vs adopt

Date: 2026-10-12 | Owner: P3/T3.0

## Question
Build SoundFontSynth or adopt an existing crate behind a Synth adapter (R04 criteria)?

## Adopt criteria (ALL must hold)
1. Zero allocation in render path
2. Bit-exact determinism with libm pinned
3. wasm size within budget (<= 1 MB)
4. exclusiveClass + loop modes + >=129 channel mix control
5. Licence compatible

## Candidate evaluation

### rustysynth (v1.3.6, MIT, zero deps, ~77K downloads)

| Criterion | Verdict | Evidence |
|-----------|---------|----------|
| Zero allocation in render path | PASS | `Voice::process()` writes to pre-allocated `self.block: Vec<f32>`; `Synthesizer::render()` uses pre-allocated `block_left`/`block_right`. No heap allocation in the hot loop. |
| Bit-exact determinism with libm pinned | FAIL | Uses own `SoundFontMath` module (soundfont_math.rs) and `std::f32::consts`. Does NOT depend on libm. `angle.cos()`/`angle.sin()` use Rust std float methods, not libm. Cannot guarantee bit-identical output with the project's libm-pinned approach. |
| wasm size within budget | PASS | 195 KB source, ~36 KB compiled. Well within 1 MB budget. |
| exclusiveClass + loop modes + >=129 channel mix control | FAIL | exclusiveClass: supported (Voice field). Loop modes: supported (LoopMode enum). Channel mix: only 16 channels (CHANNEL_COUNT = 16). Project needs >=129 (128 GM lanes + 1 drum lane). |
| Licence compatible | PASS | MIT |

**Overall: FAIL** (2 of 5 criteria failed)

### rustysynth-ext (fork with SF3 via lewton)

Same architecture as rustysynth. Inherits the same failures:
- No libm (uses SoundFontMath)
- Only 16 channels

**Overall: FAIL**

### soundfont (v0.1.0, by OxiSynth, MIT)

| Criterion | Verdict | Evidence |
|-----------|---------|----------|
| Zero allocation in render path | N/A | Parser only, no synth. Would need own SoundFontSynth built on top. |
| Bit-exact determinism with libm pinned | N/A | Parser only. |
| wasm size within budget | PASS | 72 KB source. |
| exclusiveClass + loop modes + >=129 channel mix control | N/A | Parser only. |
| Licence compatible | PASS | MIT |

**Overall: Not applicable** (parser, not a synth). Using this crate means building our own synth.

### xsynth-soundfonts (v0.4.0, MIT)

| Criterion | Verdict | Evidence |
|-----------|---------|----------|
| Zero allocation in render path | N/A | Parser module for XSynth. Depends on `soundfont` crate. No synth. |
| Bit-exact determinism with libm pinned | N/A | Parser only. |
| wasm size within budget | PASS | Small. |
| exclusiveClass + loop modes + >=129 channel mix control | N/A | Parser only. |
| Licence compatible | PASS | MIT |

**Overall: Not applicable** (parser, not a synth).

### oxideav_midi (MIT, last commit 2026-07-06)

| Criterion | Verdict | Evidence |
|-----------|---------|----------|
| Zero allocation in render path | Unknown | New project, limited documentation. Voice generator exists but allocation behavior not verified. |
| Bit-exact determinism with libm pinned | FAIL | No libm dependency. Uses own math. |
| wasm size within budget | Unknown | New project, untested. |
| exclusiveClass + loop modes + >=129 channel mix control | Partial | Supports exclusiveClass, modulation envelope, filter. Channel count unclear. |
| Licence compatible | PASS | MIT |

**Overall: FAIL** (no libm, untested, too new).

## Decision
**FALLBACK (build)**.

No existing crate meets ALL five adopt criteria. The closest candidate, rustysynth, fails two:
1. **No libm**: Uses own `SoundFontMath` instead of libm. The project's determinism gate (g05) requires libm-pinned transcendentals for native == wasm bit-exact output. rustysynth's `SoundFontMath` cannot guarantee this.
2. **Only 16 channels**: The project's document model has up to 129 lanes (128 GM + 1 drum), each requiring independent mix control. rustysynth's 16-channel architecture is insufficient.

The `soundfont` and `xsynth-soundfonts` crates are parsers only -- adopting them still requires building a SoundFontSynth. `oxideav_midi` is too new and also lacks libm.

## Build plan
Build `SoundFontSynth` in `crates/ng-synth/src/soundfont/`:
- Use `soundfont` crate (v0.1.0) or write own SF2/SF3 parser (R03 scope)
- Use libm for all transcendentals (sin, exp, pow, log)
- Support >=129 channels (one per document lane)
- Pre-allocated voice pool (zero allocation in render path)
- Support exclusiveClass, loop modes, volume envelope, attenuation/pan/tuning
- Pass synth_contract tests (R04.R01)

## Risks
- Build effort is non-trivial (R04 SoundFontSynth is the largest P3 task)
- Fidelity vs schedule: filter/LFO deferred to v2 (R04 RISK)
- sf2_builder test fixtures needed for T3.1
