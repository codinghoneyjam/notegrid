// <META - FILE SUMMARY - 8-comb + 4-allpass stereo reverb skeleton with denormal-protected feedback (R04). | L1-230>
// HOTPATH: no allocation after construction, no indexing, no panic, no transcendentals, no clock.
// Feedback lines are the denormal hazard here; see FeedbackLine::process.

use crate::filter::OnePoleCascade;
use crate::noise::Lcg;

/// Comb filters per channel (R04.OWNS: 8 combs + 4 allpass, stereo offsets).
pub const COMBS: usize = 8;
/// Allpass filters per channel.
pub const ALLPASS: usize = 4;

/// Longest delay line, samples, at 48 kHz. The base lengths below are 44.1 kHz values scaled by
/// the actual rate; 1617 * 48000/44100 + headroom fits.
const MAX_DELAY: usize = 2048;

/// Mutually prime-ish comb delays in samples at 44100. Coprime-ish lengths avoid the ringing
/// pitch that shared or even delays produce.
const COMB_BASE: [u32; COMBS] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_BASE: [u32; ALLPASS] = [556, 441, 341, 225];
/// Right channel comb offset: decorrelates the two impulse responses without changing their
/// density (R04.OWNS: "stereo offsets").
const STEREO_OFFSET: u32 = 23;

/// Output trim. Eight parallel combs at feedback 0.8 would otherwise sum to a DC gain near 10,
/// which would need the caller to pull the send down by 20 dB.
/// Allpass feedback: standard for a diffusion stage. |g| < 1 keeps it stable.
const ALLPASS_FEEDBACK: f32 = 0.5;

/// Below this a sample is snapped to zero instead of being stored or fed back (R04.R04).
///
/// 1e-19 is about -380 dBFS, so the snap is inaudible, and it sits far above the f32 subnormal
/// range (which starts near 1.2e-38). Values between the floor and the subnormal range would
/// otherwise sit in the feedback path costing 10-100x per operation.
pub const DENORMAL_FLOOR: f32 = 1e-19;

/// A Schroeder allpass: `y[n] = -g * x[n] + d[n] + g * y[n - D]`.
///
/// This is NOT a comb. A comb has gain `1/(1 - g)` at DC, which is why a bank of them sums to a
/// large multiple of the input; an allpass has unity gain at every frequency and only changes
/// phase, which is what a diffusion stage is for. Confusing the two is what makes a reverb either
/// inaudible or unusable.
#[derive(Clone, Debug)]
struct Allpass {
    buf: Box<[f32]>,
    idx: usize,
    gain: f32,
}

impl Allpass {
    /// `delay_samples` is the 44100 value; scaled to `sample_rate` like the comb lines.
    fn new(delay_samples: u32, gain: f32, sample_rate: u32) -> Option<Self> {
        if sample_rate == 0 {
            return None;
        }
        let scale = sample_rate as f32 / 44_100.0;
        let len = ((delay_samples as f32) * scale).round();
        let len = if len < 1.0 { 1 } else { len as usize };
        if len >= MAX_DELAY {
            return None;
        }
        Some(Self {
            buf: vec![0.0f32; len].into_boxed_slice(),
            idx: 0,
            gain,
        })
    }

    /// `y[n] = -g * x[n] + d[n]`, then `d[n+1] = y[n] + g * x[n]`.
    ///
    /// The line stores `d`, not the output. Storing the output instead is the easy mistake and it
    /// is unstable: it feeds the un-attenuated signal back, so the stage accumulates gain every
    /// pass and the reverb diverges.
    fn process(&mut self, x: f32) -> f32 {
        let raw = self.buf.get(self.idx).copied().unwrap_or(0.0);
        // Floor the read as well as the write: `d` is part of the output path.
        let d = if raw.abs() < DENORMAL_FLOOR || raw == 0.0 {
            0.0
        } else {
            raw
        };
        let y = d - self.gain * x;
        let next_d = y + self.gain * x;
        let stored = if next_d.abs() < DENORMAL_FLOOR || next_d == 0.0 {
            0.0
        } else {
            next_d
        };
        if let Some(cell) = self.buf.get_mut(self.idx) {
            *cell = stored;
        }
        self.idx = (self.idx + 1) % self.buf.len().max(1);
        y
    }

    fn flush(&mut self) {
        for cell in self.buf.iter_mut() {
            *cell = 0.0;
        }
        self.idx = 0;
    }

    fn peak(&self) -> f32 {
        self.buf.iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }
}

/// One feedback delay line with a denormal floor.
///
/// Heap-allocated at construction and never resized: HOTPATH forbids allocation reachable from
/// render_block after init, and a `Box` here keeps 12 delay lines per channel off the stack.
#[derive(Clone, Debug)]
struct FeedbackLine {
    buf: Box<[f32]>,
    read: usize,
    write: usize,
    /// Feedback gain, always < 1 so the line decays.
    feedback: f32,
    /// Damping one-pole in the feedback path: what makes a comb sound like a room rather than a
    /// metallic ring.
    damping: OnePoleCascade,
}

impl FeedbackLine {
    /// `delay_samples` is the 44100 value; it is scaled to `sample_rate` here.
    fn new(delay_samples: u32, feedback: f32, damping_hz: f32, sample_rate: u32) -> Option<Self> {
        if sample_rate == 0 {
            return None;
        }
        let scale = sample_rate as f32 / 44_100.0;
        let len = ((delay_samples as f32) * scale).round();
        let len = if len < 1.0 { 1 } else { len as usize };
        if len >= MAX_DELAY {
            return None;
        }
        Some(Self {
            buf: vec![0.0f32; len].into_boxed_slice(),
            read: 0,
            write: 0,
            feedback,
            damping: OnePoleCascade::new(damping_hz, sample_rate).ok()?,
        })
    }

    fn len(&self) -> usize {
        self.buf.len()
    }

    /// One sample in, one sample out.
    fn process(&mut self, x: f32) -> f32 {
        let len = self.len();
        let raw = self.buf.get(self.read).copied().unwrap_or(0.0);
        // The read value is the stage's output, so it needs the floor too, not just what is
        // written: a subnormal left in the line would be handed to the caller every pass.
        let delayed = if raw.abs() < DENORMAL_FLOOR || raw == 0.0 {
            0.0
        } else {
            raw
        };
        let damped = self.damping.process(delayed);
        let fed = damped * self.feedback;
        let stored = x + fed;
        // Also guard what is *stored*, not just what is fed back: a subnormal sitting in the line
        // is read, damped and multiplied every sample regardless of what comes in.
        if let Some(cell) = self.buf.get_mut(self.write) {
            *cell = if stored.abs() < DENORMAL_FLOOR || stored == 0.0 {
                0.0
            } else {
                stored
            };
        }
        self.read = (self.read + 1) % len;
        self.write = (self.write + 1) % len;
        delayed
    }

    /// Flush to silence, releasing the FPU from denormal mode (R04.R04).
    fn flush(&mut self) {
        self.damping.reset();
        self.read = 0;
        self.write = 0;
        for cell in self.buf.iter_mut() {
            *cell = 0.0;
        }
    }

    fn peak(&self) -> f32 {
        self.buf.iter().fold(0.0f32, |m, v| {
            let a = v.abs();
            if a > m {
                a
            } else {
                m
            }
        })
    }
}

/// One channel of the reverb: combs in parallel, then allpasses in series.
#[derive(Clone, Debug)]
struct Channel {
    combs: Box<[FeedbackLine]>,
    allpass: Box<[Allpass]>,
    /// Attenuation applied to the parallel comb sum before the diffusion stage. The bank sums
    /// eight combs, each with DC gain 1/(1 - feedback), so this keeps the summed level sane.
    comb_gain: f32,
}

impl Channel {
    fn new(sample_rate: u32, feedback: f32, damping_hz: f32, offset: u32) -> Option<Self> {
        let mut combs: Vec<FeedbackLine> = Vec::with_capacity(COMBS);
        for (i, &base) in COMB_BASE.iter().enumerate() {
            // Offset every comb, alternating the full offset and half of it, so the two channels
            // share no identical delay line. Leaving seven of them identical is what makes a
            // stereo reverb collapse back to a mono point source.
            let delay = base + if i % 2 == 0 { offset } else { offset / 2 };
            combs.push(FeedbackLine::new(delay, feedback, damping_hz, sample_rate)?);
        }
        let mut allpass: Vec<Allpass> = Vec::with_capacity(ALLPASS);
        for (i, &base) in ALLPASS_BASE.iter().enumerate() {
            let delay = base + if i % 2 == 0 { offset } else { offset / 2 };
            allpass.push(Allpass::new(delay, ALLPASS_FEEDBACK, sample_rate)?);
        }
        // The comb bank's DC gain is COMBS / (1 - feedback); divide by that so the parallel sum
        // is near unity for DC and the caller can feed the reverb straight from a send bus.
        let comb_gain = (1.0 - feedback) / COMBS as f32;
        // Sanity: with feedback 0.8 this is 0.025, and 8 combs at 5x DC gain sum to ~1.0.
        debug_assert!(comb_gain > 0.0 && comb_gain < 1.0);
        Some(Self {
            combs: combs.into_boxed_slice(),
            allpass: allpass.into_boxed_slice(),
            comb_gain,
        })
    }

    fn process(&mut self, x: f32) -> f32 {
        let mut sum = 0.0f32;
        for comb in self.combs.iter_mut() {
            sum += comb.process(x);
        }
        sum *= self.comb_gain;
        for ap in self.allpass.iter_mut() {
            sum = ap.process(sum);
        }
        if sum.abs() < DENORMAL_FLOOR || sum == 0.0 {
            0.0
        } else {
            sum
        }
    }

    fn flush(&mut self) {
        for line in self.combs.iter_mut() {
            line.flush();
        }
        for line in self.allpass.iter_mut() {
            line.flush();
        }
    }

    fn peak(&self) -> f32 {
        let combs = self.combs.iter().fold(0.0f32, |m, l| m.max(l.peak()));
        self.allpass.iter().fold(combs, |m, l| m.max(l.peak()))
    }
}

/// Stereo reverb skeleton: 8 combs + 4 allpass per channel, seeded noise for early diffusion.
///
/// This is the "skeleton" R04 specifies: the topology and the denormal behaviour are real, the
/// early-reflection model is a noise burst rather than a measured impulse. R04.R04 tests the
/// flush, which is what costs real money when it is wrong -- a reverb that rings forever pins the
/// FPU in denormal mode for the rest of the render.
#[derive(Clone, Debug)]
pub struct Reverb {
    left: Box<Channel>,
    right: Box<Channel>,
    /// Early-reflection diffusion burst, seeded so it is bit-identical across renders.
    diffusion: Lcg,
    diffusion_remaining: usize,
    diffusion_gain: f32,
    sample_rate: u32,
    seed: u64,
}

impl Reverb {
    /// `feedback` in (0, 1) controls decay time. At 1 the reverb never dies.
    pub fn new(sample_rate: u32, feedback: f32, damping_hz: f32, seed: u64) -> Option<Self> {
        if sample_rate == 0 || feedback <= 0.0 || feedback.is_nan() || feedback >= 1.0 {
            return None;
        }
        Some(Self {
            left: Box::new(Channel::new(sample_rate, feedback, damping_hz, 0)?),
            right: Box::new(Channel::new(
                sample_rate,
                feedback,
                damping_hz,
                STEREO_OFFSET,
            )?),
            diffusion: Lcg::new(seed),
            diffusion_remaining: 0,
            diffusion_gain: 0.0,
            sample_rate,
            seed,
        })
    }

    /// One stereo frame in, one out. A non-finite input is replaced by silence rather than
    /// propagating: a single NaN in a feedback network never leaves it.
    pub fn process(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let mut l = if input_l.is_finite() { input_l } else { 0.0 };
        let mut r = if input_r.is_finite() { input_r } else { 0.0 };
        if self.diffusion_remaining > 0 {
            self.diffusion_remaining -= 1;
            let g = self.diffusion_gain;
            l += self.diffusion.next_bipolar() * g;
            r += self.diffusion.next_bipolar() * g;
        }
        let out_l = self.left.process(l);
        let out_r = self.right.process(r);
        (out_l, out_r)
    }

    /// Trigger a seeded noise burst for the first `samples` frames after a note starts.
    pub fn early_reflection(&mut self, samples: u32, gain: f32) {
        self.diffusion_remaining = if gain.is_finite() {
            samples as usize
        } else {
            0
        };
        self.diffusion_gain = if gain.is_finite() { gain } else { 0.0 };
    }

    /// Peak absolute level across every delay line. R04.R04 asserts this after a flush.
    pub fn tail_peak(&self) -> f32 {
        self.left.peak().max(self.right.peak())
    }

    /// Silence every delay line and reset the state (R04.R04: flushes below -120 dBFS).
    pub fn flush(&mut self) {
        self.left.flush();
        self.right.flush();
        self.diffusion_remaining = 0;
        self.diffusion_gain = 0.0;
        self.diffusion.reset(self.seed);
    }

    /// Push silence through for `decay_samples` and report how quiet the tail is, in dBFS.
    /// Convenience for the R04.R04 assertion; not used in the audio path.
    pub fn measure_flush_floor_dbfs(&mut self, decay_samples: u32) -> f32 {
        self.flush();
        let mut i = 0;
        while i < decay_samples {
            let (l, r) = self.process(0.0, 0.0);
            if l == 0.0 && r == 0.0 {
                break;
            }
            i += 1;
        }
        crate::filter::linear_to_db(self.tail_peak())
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}
