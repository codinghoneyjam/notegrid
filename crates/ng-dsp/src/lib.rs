// <META - FILE SUMMARY - R04 DSP parts: Envelope, Ramp, interpolators, filters, SoftLimiter, seeded Lcg. | L1-40>
// HOTPATH crate (SET HOTPATH in spec/dsl/00_dsl.dsl): the workspace deny list applies here.
// No unwrap, no expect, no panic, no slice indexing -- a fallible path returns Result or an
// Option and the caller decides. Tests may use them freely; see tests/common/mod.rs.
pub mod envelope;
pub mod filter;
pub mod limiter;
pub mod noise;
pub mod ramp;
pub mod reverb;

/// Sample rate default per C0.5.
pub const DEFAULT_SAMPLE_RATE: u32 = 44_100;
