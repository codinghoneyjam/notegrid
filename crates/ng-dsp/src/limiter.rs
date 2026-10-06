// <META - FILE SUMMARY - Master-bus soft limiter: peak detector with lookahead, soft knee, denormal-safe release. | L1-200>
// HOTPATH: fixed-capacity lookahead rings held inline, no allocation after construction, no
// indexing, no panic, no wall clock.

use crate::filter::smoothing_coefficient;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimiterError {
    /// A ceiling above 1.0 would be gain, not limiting.
    CeilingOutOfRange,
    /// Release or lookahead outside a usable range.
    TimingOutOfRange,
    ZeroSampleRate,
}

/// Soft limiter for the master bus (C2.5: master chain ends in SoftLimiter).
///
/// Peak detection with a lookahead delay, so the gain reduction is already in place by the time
/// the peak arrives; a hard clipper would need zero lookahead and alias. The knee is smoothstep
/// between threshold and ceiling, which keeps the derivative continuous at both ends.
///
/// Stereo-linked: one gain for both channels, driven by the louder of the pair. A limiter that
/// treats channels independently pumps the stereo image whenever they differ.
#[derive(Clone, Debug)]
pub struct SoftLimiter {
    sample_rate: u32,
    ceiling: f32,
    threshold: f32,
    /// Per-sample gain recovery coefficient.
    release_coef: f32,
    /// Per-sample envelope release coefficient.
    detector_coef: f32,
    /// Current gain, 0..=1.
    gain: f32,
    /// Smoothed envelope follower of |input|.
    detector: f32,
    /// Delay line for the left channel.
    left: DelayRing,
    /// Delay line for the right channel.
    right: DelayRing,
    denormal_floor: f32,
}

/// PartialEq exists so a test can assert `Err(LimiterError::..)` on a Result; never used to compare
/// two limiters for audio equality.
impl PartialEq for SoftLimiter {
    fn eq(&self, other: &Self) -> bool {
        self.sample_rate == other.sample_rate
            && self.ceiling == other.ceiling
            && self.threshold == other.threshold
            && self.gain == other.gain
            && self.detector == other.detector
    }
}

/// Fixed-capacity delay line. `buf` is allocated once at construction and never resized, so
/// render_block performs no allocation (GLOBAL_DENY).
#[derive(Clone, Debug)]
struct DelayRing {
    buf: Box<[f32]>,
    write: usize,
    /// True once the ring has been filled, i.e. the output is a real delayed sample rather than
    /// the initial silence.
    primed: bool,
}

impl DelayRing {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0f32; len.max(1)].into_boxed_slice(),
            write: 0,
            primed: false,
        }
    }

    fn len(&self) -> usize {
        self.buf.len()
    }

    /// Push `x`, return the sample leaving the ring.
    fn push(&mut self, x: f32) -> f32 {
        let len = self.len();
        if len == 1 {
            // Zero lookahead: pass through.
            return x;
        }
        let out = self.buf.get(self.write).copied().unwrap_or(0.0);
        if let Some(cell) = self.buf.get_mut(self.write) {
            *cell = x;
        }
        self.write = (self.write + 1) % len;
        if !self.primed && self.write == 0 {
            self.primed = true;
        }
        out
    }

    fn flush(&mut self) {
        for cell in self.buf.iter_mut() {
            *cell = 0.0;
        }
        self.write = 0;
        self.primed = false;
    }
}

impl SoftLimiter {
    /// Longest lookahead supported inline: 10 ms at 48 kHz.
    pub const MAX_LOOKAHEAD: usize = 480;

    pub fn new(
        sample_rate: u32,
        ceiling: f32,
        threshold: f32,
        release_ms: f32,
        lookahead_ms: f32,
    ) -> Result<Self, LimiterError> {
        if sample_rate == 0 {
            return Err(LimiterError::ZeroSampleRate);
        }
        if ceiling <= 0.0 || ceiling.is_nan() || ceiling > 1.0 {
            return Err(LimiterError::CeilingOutOfRange);
        }
        if release_ms <= 0.0 || release_ms.is_nan() {
            return Err(LimiterError::TimingOutOfRange);
        }
        let lookahead = ((lookahead_ms * 0.001) * sample_rate as f32).round();
        if lookahead.is_nan() || lookahead < 0.0 || lookahead > Self::MAX_LOOKAHEAD as f32 {
            return Err(LimiterError::TimingOutOfRange);
        }
        let release_coef = smoothing_coefficient(release_ms, sample_rate)
            .map_err(|_| LimiterError::TimingOutOfRange)?;
        let detector_coef = smoothing_coefficient(release_ms * 0.5, sample_rate)
            .map_err(|_| LimiterError::TimingOutOfRange)?;
        let len = lookahead as usize;
        Ok(Self {
            sample_rate,
            ceiling,
            threshold: threshold.min(ceiling),
            release_coef,
            detector_coef,
            gain: 1.0,
            detector: 0.0,
            left: DelayRing::new(len),
            right: DelayRing::new(len),
            denormal_floor: 1e-19,
        })
    }

    /// The default master-bus setting: -1 dBFS ceiling, soft knee from -6 dBFS, 5 ms lookahead.
    pub fn new_default(sample_rate: u32) -> Result<Self, LimiterError> {
        Self::new(
            sample_rate,
            crate::filter::db_to_linear(-1.0),
            crate::filter::db_to_linear(-6.0),
            50.0,
            5.0,
        )
    }

    /// One sample in, one out.
    pub fn process(&mut self, x: f32) -> f32 {
        let (out, _) = self.process_stereo(x, x);
        out
    }

    /// Stereo-linked: one gain for both channels, driven by the louder of the pair.
    ///
    /// Detection runs on the *input*, before the delay, so the gain computed now applies to the
    /// sample that arrives `lookahead` frames from now -- which is the peak we just saw.
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let l = if left.is_finite() { left } else { 0.0 };
        let r = if right.is_finite() { right } else { 0.0 };

        self.detect(l.abs().max(r.abs()));

        let mut out_l = self.left.push(l) * self.gain;
        let mut out_r = self.right.push(r) * self.gain;

        if out_l.abs() < self.denormal_floor {
            // Both the gain and the delayed signal decay toward denormals once the input stops;
            // snap so the FPU leaves denormal mode.
            out_l = 0.0;
            self.detector = 0.0;
            if self.gain < self.denormal_floor {
                self.gain = 1.0;
            }
        }
        if out_r.abs() < self.denormal_floor {
            out_r = 0.0;
        }
        (out_l, out_r)
    }

    /// Follow the envelope and update the gain.
    fn detect(&mut self, peak: f32) {
        if peak > self.detector {
            // Instant attack.
            self.detector = peak;
        } else {
            self.detector += self.detector_coef * (peak - self.detector);
        }

        let target = if self.detector > self.threshold {
            soft_ratio(self.detector, self.threshold, self.ceiling)
        } else {
            1.0
        };

        if target < self.gain {
            // Instant reduction: the peak is already arriving.
            self.gain = target;
        } else {
            // Smooth recovery, or the gain would breathe audibly.
            self.gain += self.release_coef * (target - self.gain);
        }
    }

    /// Current gain, for a meter. 1.0 = no reduction.
    pub fn gain(&self) -> f32 {
        self.gain
    }

    pub fn detector(&self) -> f32 {
        self.detector
    }

    pub fn ceiling(&self) -> f32 {
        self.ceiling
    }

    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Lookahead delay in frames.
    pub fn lookahead_frames(&self) -> usize {
        self.left.len()
    }

    pub fn reset(&mut self) {
        self.gain = 1.0;
        self.detector = 0.0;
        self.left.flush();
        self.right.flush();
    }
}

/// Smoothstep soft knee: unity below the threshold, falling to `ceiling / detector` at or above
/// the ceiling, with a continuous derivative at both ends.
fn soft_ratio(detector: f32, threshold: f32, ceiling: f32) -> f32 {
    if detector <= 0.0 {
        return 1.0;
    }
    let span = ceiling - threshold;
    if span <= 0.0 {
        return ceiling / detector;
    }
    let over = detector - threshold;
    if over >= span {
        ceiling / detector
    } else {
        let t = over / span;
        // 3t^2 - 2t^3: derivative is zero at t=0 and t=1.
        let shaped = t * t * (3.0 - 2.0 * t);
        let compressed = threshold + shaped * span;
        if compressed > 0.0 {
            compressed / detector
        } else {
            1.0
        }
    }
}
