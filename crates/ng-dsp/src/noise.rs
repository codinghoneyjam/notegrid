// <META - FILE SUMMARY - Seeded SplitMix64 for reproducible noise (R04.DESIGN: no unseeded randomness). | L1-70>
// HOTPATH: no allocation, no panic, no indexing. Used by the reverb's diffusion burst and, from
// T1.3, by OscSynth drums -- so native and wasm must agree bit-for-bit (R-RT-4).

/// Deterministic 64-bit generator: the SplitMix64 finalizer over an incrementing counter.
///
/// SplitMix64 rather than a bare xorshift because audio needs the *low* bits to be as well
/// distributed as the high ones, and xorshift's low bits are its weakest. Every operation is
/// wrapping integer arithmetic, so the sequence is identical on every target.
#[derive(Clone, Debug)]
pub struct Lcg {
    state: u64,
}

const GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
const MIX_A: u64 = 0xBF58_476D_1CE4_E5B9;
const MIX_B: u64 = 0x94D0_49BB_1331_11EB;
/// Substituted for a zero seed, which would otherwise be a fixed point of the mixer.
const ZERO_SEED_REPLACEMENT: u64 = 0xD1B5_4A32_D192_ED03;
/// 2^24: the number of mantissa bits an f32 holds exactly, so the scale below is a pure shift.
const F32_MANTISSA_STEPS: f32 = 16_777_216.0;

impl Lcg {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                ZERO_SEED_REPLACEMENT
            } else {
                seed
            },
        }
    }

    /// Next 64 bits of the sequence.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GAMMA);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(MIX_A);
        z = (z ^ (z >> 27)).wrapping_mul(MIX_B);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1). Takes the top 24 bits, which are the ones f32 represents exactly.
    pub fn next_unit(&mut self) -> f32 {
        let top24 = self.next_u64() >> 40;
        top24 as f32 / F32_MANTISSA_STEPS
    }

    /// Uniform in [-1, 1).
    pub fn next_bipolar(&mut self) -> f32 {
        self.next_unit() * 2.0 - 1.0
    }

    /// Return to the state this generator started from, so a render repeats exactly.
    pub fn reset(&mut self, seed: u64) {
        *self = Self::new(seed);
    }
}
