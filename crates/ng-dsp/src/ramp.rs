// <META - FILE SUMMARY - Parameter ramp: 5 ms linear ramp used to avoid clicks on any parameter change. | L1-70>
// HOTPATH: no allocation, no panic, no indexing.

/// PartialEq exists so a test can assert `Err(RampError::..)` on a Result; never used to compare
/// two ramps for audio equality.
impl PartialEq for Ramp {
    fn eq(&self, other: &Self) -> bool {
        self.remaining == other.remaining
            && self.step == other.step
            && self.current == other.current
            && self.target == other.target
    }
}

/// One linear ramp from `from` to `to` over a fixed number of samples.
///
/// R04.OWNS specifies a 5 ms ramp. At 44100 that is 220.5 samples, so the length is computed
/// in samples and the ramp may end one sample early or late depending on the rate; the exact
/// sample count is derived once and then stepped incrementally, so there is no drift.
#[derive(Clone, Debug)]
pub struct Ramp {
    remaining: u32,
    step: f32,
    current: f32,
    target: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RampError {
    /// A zero-length ramp cannot interpolate toward a different value; it would be a step.
    ZeroLength,
}

impl Ramp {
    pub const DEFAULT_MS: f32 = 5.0;

    /// Ramp from `from` to `to` over `length_samples`.
    pub fn new(from: f32, to: f32, length_samples: u32) -> Result<Self, RampError> {
        if length_samples == 0 {
            return Err(RampError::ZeroLength);
        }
        let step = (to - from) / length_samples as f32;
        Ok(Self {
            remaining: length_samples,
            step,
            current: from,
            target: to,
        })
    }

    /// Ramp over a duration in milliseconds at `sample_rate`. C0.5: 44100 default, 48000 valid.
    pub fn new_ms(from: f32, to: f32, ms: f32, sample_rate: u32) -> Result<Self, RampError> {
        let samples = ((ms * 0.001) * sample_rate as f32).round();
        // Round to at least one sample: a ramp must always make progress.
        let length = if samples < 1.0 { 1 } else { samples as u32 };
        Self::new(from, to, length)
    }

    /// The 5 ms ramp R04 specifies.
    pub fn new_default(from: f32, to: f32, sample_rate: u32) -> Result<Self, RampError> {
        Self::new_ms(from, to, Self::DEFAULT_MS, sample_rate)
    }

    /// Advance by one sample and return the value for it. Holds `target` once finished.
    /// Named `tick` rather than `next` so it does not read as an Iterator method.
    pub fn tick(&mut self) -> f32 {
        if self.remaining == 0 {
            return self.target;
        }
        self.current += self.step;
        self.remaining -= 1;
        if self.remaining == 0 {
            self.current = self.target;
        }
        self.current
    }

    /// Advance by `n` samples, returning the last value produced.
    pub fn advance(&mut self, n: u32) -> f32 {
        for _ in 0..n {
            let v = self.tick();
            if self.remaining == 0 {
                return v;
            }
        }
        self.current
    }

    /// Jump to the target without ramping (a parameter change while muted, say).
    pub fn snap(&mut self) -> f32 {
        self.remaining = 0;
        self.current = self.target;
        self.current
    }

    pub fn is_done(&self) -> bool {
        self.remaining == 0
    }

    pub fn current(&self) -> f32 {
        self.current
    }

    pub fn target(&self) -> f32 {
        self.target
    }
}
