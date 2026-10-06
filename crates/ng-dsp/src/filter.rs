// <META - FILE SUMMARY - One-pole and biquad filters with libm-routed coefficients. | L1-380>
// HOTPATH: fixed-size coefficient arrays held inline, no allocation after construction, no
// indexing, no panic. Transcendentals go through `libm` (R04.DESIGN, R-RT-4).

// The f32 variants, not the f64 ones: a round trip through f64 would change the result between
// native and wasm targets (R-RT-4 wants bit-identical output).
use libm::{cosf, expf, log10f, powf, sinf, sqrtf, tanf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterError {
    /// A one-pole coefficient must be strictly below 1 or the pole never settles.
    OnePoleCoefficient,
    /// A biquad needs a 0 < Q; Q = 0 means an undamped resonator that blows up.
    InvalidQ,
    /// Frequencies outside (0, nyquist] cannot be realised at this rate.
    FrequencyOutOfRange,
    ZeroSampleRate,
}

// PartialEq on the filter types lets a test assert `Err(FilterError::..)` directly; it is only
// derived to satisfy Result comparison and costs nothing at runtime.
impl PartialEq for OnePole {
    fn eq(&self, other: &Self) -> bool {
        self.a == other.a && self.y == other.y
    }
}
impl PartialEq for BiquadFilter {
    fn eq(&self, other: &Self) -> bool {
        self.coeffs == other.coeffs && self.state == other.state
    }
}
impl PartialEq for Biquad {
    fn eq(&self, other: &Self) -> bool {
        self.b0 == other.b0
            && self.b1 == other.b1
            && self.b2 == other.b2
            && self.a1 == other.a1
            && self.a2 == other.a2
    }
}
impl PartialEq for BiquadState {
    fn eq(&self, other: &Self) -> bool {
        self.x1 == other.x1 && self.x2 == other.x2 && self.y1 == other.y1 && self.y2 == other.y2
    }
}

/// One-pole lowpass: `y[n] = y[n-1] + a * (x[n] - y[n-1])`.
///
/// `a` is in 0..1. `a` near 1 is a long tail and, past a point, denormal slow: see
/// `Filter::denormal_floor` for the snap that keeps that from happening.
#[derive(Clone, Debug)]
pub struct OnePole {
    a: f32,
    y: f32,
    denormal_floor: f32,
}

impl OnePole {
    pub fn new(a: f32) -> Result<Self, FilterError> {
        // Both bounds, not a negated `a > 0.0 && a < 1.0` chain: a NaN coefficient would fail
        // either form, and this states the NaN rejection explicitly rather than relying on it.
        if a <= 0.0 || a >= 1.0 || a.is_nan() {
            return Err(FilterError::OnePoleCoefficient);
        }
        Ok(Self {
            a,
            y: 0.0,
            denormal_floor: 1e-19,
        })
    }

    /// Coefficient from a cutoff in Hz. One-pole's -3 dB point sits at a = 1 - exp(-2*pi*fc/sr),
    /// computed through libm so wasm and native agree.
    pub fn from_cutoff(cutoff_hz: f32, sample_rate: u32) -> Result<Self, FilterError> {
        if sample_rate == 0 {
            return Err(FilterError::ZeroSampleRate);
        }
        let nyquist = sample_rate as f32 * 0.5;
        if cutoff_hz <= 0.0 || cutoff_hz.is_nan() || cutoff_hz > nyquist {
            return Err(FilterError::FrequencyOutOfRange);
        }
        let sr = sample_rate as f32;
        let a = 1.0 - expf(-2.0 * core::f32::consts::PI * cutoff_hz / sr);
        Self::new(a)
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let next = self.y + self.a * (x - self.y);
        // Denormal guard: the state decays geometrically and would otherwise sit in the subnormal
        // range for thousands of samples, where arithmetic is 10-100x slower (R-RT-3).
        self.y = if next.abs() < self.denormal_floor || next == 0.0 {
            0.0
        } else {
            next
        };
        self.y
    }

    pub fn reset(&mut self) {
        self.y = 0.0;
    }
}

/// Biquad coefficients, laid out for the direct-form-I difference equation.
#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

/// State of a biquad section: two samples of input and two of output.
#[derive(Clone, Copy, Debug, Default)]
struct BiquadState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

/// One biquad section. The reverb and the filters both need exactly this shape.
#[derive(Clone, Debug)]
pub struct BiquadFilter {
    coeffs: Biquad,
    state: BiquadState,
    denormal_floor: f32,
}

impl BiquadFilter {
    pub fn from_coeffs(coeffs: Biquad) -> Self {
        Self {
            coeffs,
            state: BiquadState::default(),
            denormal_floor: 1e-19,
        }
    }

    /// Peaking EQ, gain in dB, `q` strictly positive.
    pub fn peaking(
        freq_hz: f32,
        q: f32,
        gain_db: f32,
        sample_rate: u32,
    ) -> Result<Self, FilterError> {
        let (a0, coeffs) = peaking_coeffs(freq_hz, q, gain_db, sample_rate)?;
        Ok(Self::from_coeffs(normalize(coeffs, a0)))
    }

    /// Lowpass, RBJ cookbook form.
    pub fn lowpass(freq_hz: f32, q: f32, sample_rate: u32) -> Result<Self, FilterError> {
        let (a0, coeffs) = lowpass_coeffs(freq_hz, q, sample_rate)?;
        Ok(Self::from_coeffs(normalize(coeffs, a0)))
    }

    /// Highpass, RBJ cookbook form.
    pub fn highpass(freq_hz: f32, q: f32, sample_rate: u32) -> Result<Self, FilterError> {
        let (a0, coeffs) = highpass_coeffs(freq_hz, q, sample_rate)?;
        Ok(Self::from_coeffs(normalize(coeffs, a0)))
    }

    /// Bandpass with constant 0 dB peak gain (RBJ cookbook).
    pub fn bandpass(freq_hz: f32, q: f32, sample_rate: u32) -> Result<Self, FilterError> {
        let (a0, coeffs) = bandpass_coeffs(freq_hz, q, sample_rate)?;
        Ok(Self::from_coeffs(normalize(coeffs, a0)))
    }

    /// One sample in, one sample out. Direct form I.
    pub fn process(&mut self, x: f32) -> f32 {
        let y =
            self.coeffs.b0 * x + self.coeffs.b1 * self.state.x1 + self.coeffs.b2 * self.state.x2
                - self.coeffs.a1 * self.state.y1
                - self.coeffs.a2 * self.state.y2;
        self.state.x2 = self.state.x1;
        self.state.x1 = x;
        self.state.y2 = self.state.y1;
        self.state.y1 = y;
        // Feedback state decays into denormals when the input goes silent; snap instead (R-RT-3).
        if y.abs() < self.denormal_floor || y == 0.0 {
            self.state = BiquadState::default();
            return 0.0;
        }
        y
    }

    pub fn reset(&mut self) {
        self.state = BiquadState::default();
    }
}

/// Validate the (frequency, q) pair shared by every coefficient function.
fn validate(freq_hz: f32, q: f32, sample_rate: u32) -> Result<(), FilterError> {
    if sample_rate == 0 {
        return Err(FilterError::ZeroSampleRate);
    }
    if q <= 0.0 || q.is_nan() {
        return Err(FilterError::InvalidQ);
    }
    let nyquist = sample_rate as f32 * 0.5;
    if freq_hz <= 0.0 || freq_hz.is_nan() || freq_hz > nyquist {
        return Err(FilterError::FrequencyOutOfRange);
    }
    Ok(())
}

fn lowpass_coeffs(freq_hz: f32, q: f32, sample_rate: u32) -> Result<(f32, Biquad), FilterError> {
    validate(freq_hz, q, sample_rate)?;
    let w0 = 2.0 * core::f32::consts::PI * freq_hz / sample_rate as f32;
    let (sin_w0, cos_w0) = (sinf(w0), cosf(w0));
    let alpha = sin_w0 / (2.0 * q);
    let a0 = 1.0 + alpha;
    Ok((
        a0,
        Biquad {
            b0: (1.0 - cos_w0) * 0.5,
            b1: 1.0 - cos_w0,
            b2: (1.0 - cos_w0) * 0.5,
            a1: -2.0 * cos_w0,
            a2: 1.0 - alpha,
        },
    ))
}

fn highpass_coeffs(freq_hz: f32, q: f32, sample_rate: u32) -> Result<(f32, Biquad), FilterError> {
    validate(freq_hz, q, sample_rate)?;
    let w0 = 2.0 * core::f32::consts::PI * freq_hz / sample_rate as f32;
    let (sin_w0, cos_w0) = (sinf(w0), cosf(w0));
    let alpha = sin_w0 / (2.0 * q);
    let a0 = 1.0 + alpha;
    Ok((
        a0,
        Biquad {
            b0: (1.0 + cos_w0) * 0.5,
            b1: -(1.0 + cos_w0),
            b2: (1.0 + cos_w0) * 0.5,
            a1: -2.0 * cos_w0,
            a2: 1.0 - alpha,
        },
    ))
}

fn bandpass_coeffs(freq_hz: f32, q: f32, sample_rate: u32) -> Result<(f32, Biquad), FilterError> {
    validate(freq_hz, q, sample_rate)?;
    let w0 = 2.0 * core::f32::consts::PI * freq_hz / sample_rate as f32;
    let (sin_w0, cos_w0) = (sinf(w0), cosf(w0));
    let alpha = sin_w0 / (2.0 * q);
    let a0 = 1.0 + alpha;
    Ok((
        a0,
        Biquad {
            b0: alpha,
            b1: 0.0,
            b2: -alpha,
            a1: -2.0 * cos_w0,
            a2: 1.0 - alpha,
        },
    ))
}

fn peaking_coeffs(
    freq_hz: f32,
    q: f32,
    gain_db: f32,
    sample_rate: u32,
) -> Result<(f32, Biquad), FilterError> {
    validate(freq_hz, q, sample_rate)?;
    let a = powf(10.0, gain_db / 40.0);
    let w0 = 2.0 * core::f32::consts::PI * freq_hz / sample_rate as f32;
    let (sin_w0, cos_w0) = (sinf(w0), cosf(w0));
    let alpha = sin_w0 / (2.0 * q);
    let a0 = 1.0 + alpha / a;
    Ok((
        a0,
        Biquad {
            b0: 1.0 + alpha * a,
            b1: -2.0 * cos_w0,
            b2: 1.0 - alpha * a,
            a1: -2.0 * cos_w0,
            a2: 1.0 - alpha / a,
        },
    ))
}

fn normalize(coeffs: Biquad, a0: f32) -> Biquad {
    Biquad {
        b0: coeffs.b0 / a0,
        b1: coeffs.b1 / a0,
        b2: coeffs.b2 / a0,
        a1: coeffs.a1 / a0,
        a2: coeffs.a2 / a0,
    }
}

/// Two cascaded one-poles: a 12 dB/octave rolloff. Cheaper than one biquad and enough for the
/// reverb's high-frequency damping, where the exact shape does not matter.
#[derive(Clone, Debug)]
pub struct OnePoleCascade {
    stages: [OnePole; 2],
}

impl OnePoleCascade {
    pub fn new(cutoff_hz: f32, sample_rate: u32) -> Result<Self, FilterError> {
        Ok(Self {
            stages: [
                OnePole::from_cutoff(cutoff_hz, sample_rate)?,
                OnePole::from_cutoff(cutoff_hz, sample_rate)?,
            ],
        })
    }

    pub fn process(&mut self, x: f32) -> f32 {
        // Chained by hand rather than by iterator: two stages, and HOTPATH forbids the
        // indexing that a Vec would need.
        let first = self.stages[0].process(x);
        self.stages[1].process(first)
    }

    pub fn reset(&mut self) {
        self.stages[0].reset();
        self.stages[1].reset();
    }
}

/// Gain in dB to linear. Routes through libm::powf so wasm and native agree (R-RT-4).
pub fn db_to_linear(db: f32) -> f32 {
    powf(10.0, db / 20.0)
}

/// Linear to dB, floored at -200 so a silent buffer does not produce -inf.
pub fn linear_to_db(linear: f32) -> f32 {
    let floor = 1e-10f32;
    let m = if linear.abs() < floor {
        floor
    } else {
        linear.abs()
    };
    20.0 * log10f(m)
}

/// Coefficient for a Butterworth-ish one-pole given a time constant, used by the limiter's
/// lookahead smoothing.
pub fn smoothing_coefficient(time_ms: f32, sample_rate: u32) -> Result<f32, FilterError> {
    if sample_rate == 0 {
        return Err(FilterError::ZeroSampleRate);
    }
    if time_ms <= 0.0 || time_ms.is_nan() {
        return Err(FilterError::OnePoleCoefficient);
    }
    let a = 1.0 - expf(-1000.0 / (time_ms * sample_rate as f32));
    if a >= 1.0 {
        return Err(FilterError::OnePoleCoefficient);
    }
    Ok(a)
}

/// Two-pole resonant coefficient set used by the limiter's detector (a state-variable filter).
pub fn svf_coeffs(freq_hz: f32, q: f32, sample_rate: u32) -> Result<(f32, f32), FilterError> {
    validate(freq_hz, q, sample_rate)?;
    let f = 2.0 * sinf(core::f32::consts::PI * freq_hz / sample_rate as f32);
    let damp = 1.0 / q;
    Ok((f, damp))
}

/// Convenience for callers that only want a magnitude response, e.g. tests.
pub fn magnitude_at(freq_hz: f32, coeffs: &Biquad, sample_rate: u32) -> f32 {
    let w = 2.0 * core::f32::consts::PI * freq_hz / sample_rate as f32;
    let (sin_w, cos_w) = (sinf(w), cosf(w));
    let c2 = cos_w * cos_w;
    let num_r = coeffs.b0 + coeffs.b1 * cos_w + coeffs.b2 * c2;
    let num_i = -(coeffs.b1 + 2.0 * coeffs.b2 * cos_w) * sin_w;
    let den_r = 1.0 + coeffs.a1 * cos_w + coeffs.a2 * c2;
    let den_i = (coeffs.a1 + 2.0 * coeffs.a2 * cos_w) * sin_w;
    let num = sqrtf(num_r * num_r + num_i * num_i);
    let den = sqrtf(den_r * den_r + den_i * den_i);
    if den == 0.0 {
        0.0
    } else {
        num / den
    }
}

/// Bandwidth in radians for a given Q, used to size the limiter's detector band.
pub fn bandwidth(freq_hz: f32, q: f32, sample_rate: u32) -> Result<f32, FilterError> {
    validate(freq_hz, q, sample_rate)?;
    let w0 = 2.0 * core::f32::consts::PI * freq_hz / sample_rate as f32;
    let bw = w0 / q;
    Ok(2.0 * tanf(bw * 0.5))
}
