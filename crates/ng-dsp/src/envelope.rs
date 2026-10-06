// <META - FILE SUMMARY - DAHDSR envelope with a denormal-safe exponential release. | L1-160>
// HOTPATH: no allocation, no panic, no indexing, no wall clock.
//
// Attack and decay are linear in amplitude, which is what a DAHDSR means. Release is
// exponential because a linear release sounds wrong on long tails; it needs a power function,
// so it routes through `libm` as R04.DESIGN and R-RT-4 require (native == wasm bit-for-bit).

/// DAHDSR stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// Release target: the level the exponential release aims for. Chosen so a release of `n`
/// samples multiplies by `release_coef` until the level reaches the denormal floor, which is
/// where it snaps. Below the floor would be subnormal territory we deliberately never enter.
pub const RELEASE_ENDPOINT: f32 = 1e-5;
/// Below this the release snaps to zero instead of decaying into the denormal range (R04.R04).
/// Roughly -380 dBFS: inaudible, and far above the f32 subnormal range.
pub const DENORMAL_FLOOR: f32 = 1e-19;
/// A stage shorter than this is treated as instantaneous.
const MIN_STAGE_SAMPLES: u32 = 1;
/// Ceiling on any single stage, 60 s, so a typo cannot produce an envelope that never ends.
const MAX_STAGE_SAMPLES: u32 = 44_100 * 60;

/// Stage durations in samples, precomputed once at construction.
#[derive(Clone, Copy, Debug)]
struct StageSamples {
    attack: u32,
    decay: u32,
    #[allow(dead_code)]
    release: u32,
}

/// DAHDSR amplitude envelope.
///
/// `note_on` starts attack, decay follows into sustain, `note_off` starts the exponential
/// release. `next()` advances one sample; there is no block API yet because the renderer's
/// sub-block split (C2.3) means the caller drives it sample by sample at event boundaries.
#[derive(Clone, Debug)]
pub struct Envelope {
    stages: StageSamples,
    stage: Stage,
    /// Remaining samples in the active linear stage.
    remaining: u32,
    /// Per-sample increment of the active linear stage (negative while decaying).
    step: f32,
    level: f32,
    /// target^(1/release_samples): one multiply per sample walks the curve down.
    release_coef: f32,
    sustain_level: f32,
    denormal_floor: f32,
    gate_open: bool,
}

impl Envelope {
    /// `stage_seconds` is (attack, decay, release); `sustain` is a level in 0..=1.
    pub fn new(sample_rate: u32, stage_seconds: (f32, f32, f32), sustain: f32) -> Self {
        let to_samples = |sec: f32| -> u32 {
            if sec <= 0.0 || sec.is_nan() {
                // A zero or negative stage is instantaneous; NaN likewise.
                return MIN_STAGE_SAMPLES;
            }
            let n = (sec * sample_rate as f32).round() as u32;
            n.clamp(MIN_STAGE_SAMPLES, MAX_STAGE_SAMPLES)
        };
        let stages = StageSamples {
            attack: to_samples(stage_seconds.0),
            decay: to_samples(stage_seconds.1),
            release: to_samples(stage_seconds.2),
        };
        // coef^release_samples == RELEASE_ENDPOINT, so a full-scale release lands on the endpoint
        // exactly when the stage's sample budget runs out, and shorter levels snap sooner.
        let release_coef = RELEASE_ENDPOINT.powf(1.0 / stages.release as f32);
        Self {
            stages,
            stage: Stage::Idle,
            remaining: 0,
            step: 0.0,
            level: 0.0,
            release_coef,
            sustain_level: sustain.clamp(0.0, 1.0),
            denormal_floor: DENORMAL_FLOOR,
            gate_open: false,
        }
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn level(&self) -> f32 {
        self.level
    }

    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }

    /// note_on: start the attack stage.
    pub fn gate_on(&mut self) {
        self.level = 0.0;
        self.gate_open = true;
        self.stage = Stage::Attack;
        self.remaining = self.stages.attack;
        self.step = 1.0 / self.remaining as f32;
    }

    /// note_off: start the release stage, or go idle if already silent.
    pub fn gate_off(&mut self) {
        self.gate_open = false;
        if self.level <= 0.0 {
            self.stage = Stage::Idle;
            return;
        }
        self.stage = Stage::Release;
    }

    /// Advance one sample and return the new level. Named `tick` rather than `next` so it does
    /// not read as an Iterator method at the call site.
    pub fn tick(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => 0.0,
            Stage::Attack => {
                self.level += self.step;
                self.remaining -= 1;
                if self.remaining == 0 {
                    self.level = 1.0;
                    self.enter_decay();
                }
                self.level
            }
            Stage::Decay => {
                self.level -= self.step;
                self.remaining -= 1;
                if self.remaining == 0 {
                    self.level = self.sustain_level;
                    self.stage = Stage::Sustain;
                }
                self.level
            }
            Stage::Sustain => {
                if self.gate_open {
                    self.level = self.sustain_level;
                } else {
                    self.gate_off();
                    self.level = 0.0;
                }
                self.level
            }
            Stage::Release => {
                // Exponential decay. Rounding to zero above the denormal floor, not below it:
                // the loop tests `!= 0.0`, so a subnormal value would keep the voice alive and
                // cost denormal arithmetic every sample (R-RT-3).
                let next = self.level * self.release_coef;
                self.level = if next < self.denormal_floor || next == 0.0 {
                    0.0
                } else {
                    next
                };
                if self.level == 0.0 {
                    self.stage = Stage::Idle;
                }
                self.level
            }
        }
    }

    fn enter_decay(&mut self) {
        let drop = 1.0 - self.sustain_level;
        if drop <= 0.0 {
            self.level = self.sustain_level;
            self.stage = Stage::Sustain;
            return;
        }
        self.remaining = self.stages.decay;
        self.step = drop / self.remaining as f32;
        self.stage = Stage::Decay;
    }

    /// Silence immediately and free the slot (C2.4: seek -> silence_all).
    pub fn silence(&mut self) {
        self.level = 0.0;
        self.remaining = 0;
        self.step = 0.0;
        self.stage = Stage::Idle;
        self.gate_open = false;
    }
}
