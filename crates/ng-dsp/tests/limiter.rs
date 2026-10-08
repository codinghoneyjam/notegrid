// <META - ROLE : T1.2 unit tests for SoftLimiter. | L1-193>
// The workspace deny list is crate-wide, so it reaches this test target too. Tests are not audio
// code: asserting on a Result is what a test is for, and a panic here is the intended failure
// mode. `src/` keeps the deny list with no exemption.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use ng_dsp::filter::db_to_linear;
use ng_dsp::limiter::{LimiterError, SoftLimiter};

const SR: u32 = 44_100;

// ---------------------------------------------------------------- SoftLimiter

#[test]
fn limiter_rejects_invalid_configuration() {
    assert_eq!(
        SoftLimiter::new(SR, 1.5, 0.5, 50.0, 5.0),
        Err(LimiterError::CeilingOutOfRange)
    );
    assert_eq!(
        SoftLimiter::new(SR, 0.0, 0.5, 50.0, 5.0),
        Err(LimiterError::CeilingOutOfRange)
    );
    assert_eq!(
        SoftLimiter::new(SR, 0.9, 0.5, -1.0, 5.0),
        Err(LimiterError::TimingOutOfRange)
    );
    assert_eq!(
        SoftLimiter::new(SR, 0.9, 0.5, 50.0, 1000.0),
        Err(LimiterError::TimingOutOfRange),
        "lookahead longer than the inline buffer"
    );
    assert_eq!(
        SoftLimiter::new(0, 0.9, 0.5, 50.0, 5.0),
        Err(LimiterError::ZeroSampleRate)
    );
    assert!(SoftLimiter::new_default(SR).is_ok());
}

#[test]
fn limiter_passes_quiet_signal_untouched() {
    let mut l = SoftLimiter::new_default(SR).unwrap();
    let quiet = 0.05;
    // The lookahead ring starts silent, so skip it before asserting on the passthrough value.
    for _ in 0..l.lookahead_frames() + 100 {
        let _ = l.process(quiet);
    }
    for _ in 0..10_000 {
        let y = l.process(quiet);
        assert!(
            (y - quiet).abs() < 1e-3,
            "quiet signal should pass through, got {y}"
        );
    }
    assert!(
        l.gain() > 0.999,
        "no gain reduction expected, got {}",
        l.gain()
    );
}

#[test]
fn limiter_delays_by_exactly_the_lookahead_window() {
    // C2.3 sample accuracy: the limiter must not change timing, only gain.
    let mut l = SoftLimiter::new_default(SR).unwrap();
    // 5 ms at 44100 is 220.5 frames; the ring rounds to 221.
    assert_eq!(l.lookahead_frames(), 221);
    for _ in 0..l.lookahead_frames() {
        assert_eq!(
            l.process(1.0),
            0.0,
            "the window starts with silence, not input"
        );
    }
    // The first real sample arrives delayed. It is already gain-reduced (1.0 is above the
    // ceiling), so assert the timing and the ceiling, not the literal value.
    let first_real = l.process(1.0);
    assert!(first_real > 0.0, "the input must appear after the window");
    assert!(
        first_real <= l.ceiling() + 1e-6,
        "first real sample {first_real} must respect the ceiling {}",
        l.ceiling()
    );
}

#[test]
fn limiter_never_exceeds_the_ceiling_in_steady_state() {
    let ceiling = db_to_linear(-1.0);
    let mut l = SoftLimiter::new_default(SR).unwrap();
    for _ in 0..2_000 {
        let _ = l.process(0.8);
    }
    let mut peak = 0.0f32;
    for i in 0..50_000 {
        let x = if (i / 25) % 2 == 0 { 0.9 } else { -0.9 };
        let y = l.process(x);
        assert!(y.is_finite());
        peak = peak.max(y.abs());
    }
    assert!(
        peak <= ceiling + 1e-3,
        "peak {peak} exceeded ceiling {ceiling}"
    );
    // 0.9 in, ceiling 0.891: the required reduction is only ~0.99, so assert the relationship
    // rather than an arbitrary threshold.
    assert!(
        (l.gain() - ceiling / 0.9).abs() < 0.01,
        "gain {} should be ceiling/input = {}",
        l.gain(),
        ceiling / 0.9
    );
}

#[test]
fn limiter_reduces_gain_further_for_louder_input() {
    let mut soft = SoftLimiter::new_default(SR).unwrap();
    let mut hard = SoftLimiter::new_default(SR).unwrap();
    for _ in 0..5_000 {
        let _ = soft.process(0.7);
    }
    for _ in 0..5_000 {
        let _ = hard.process(1.0);
    }
    assert!(
        hard.gain() < soft.gain(),
        "a louder signal must be attenuated more: {} vs {}",
        hard.gain(),
        soft.gain()
    );
}

#[test]
fn limiter_recovers_gain_after_silence() {
    let mut l = SoftLimiter::new_default(SR).unwrap();
    for _ in 0..5_000 {
        let _ = l.process(1.0);
    }
    let loud_gain = l.gain();
    for _ in 0..44_100 {
        let _ = l.process(0.0);
    }
    assert!(
        l.gain() > loud_gain,
        "gain must recover: {loud_gain} -> {}",
        l.gain()
    );
    assert!(
        l.gain() > 0.99,
        "gain should be back to unity, got {}",
        l.gain()
    );
}

#[test]
fn limiter_stereo_is_linked() {
    let mut l = SoftLimiter::new_default(SR).unwrap();
    for _ in 0..2_000 {
        let (left, right) = l.process_stereo(0.05, 0.05);
        assert!((left - right).abs() < 1e-6, "linked channels must match");
    }
    // Only the left channel is loud. A linked limiter must apply the same gain to both, so the
    // quiet right channel comes out attenuated too -- that is what "linked" means.
    let mut peak = 0.0f32;
    for _ in 0..20_000 {
        let (left, right) = l.process_stereo(0.9, 0.0);
        peak = peak.max(left.abs()).max(right.abs());
    }
    assert!(
        peak <= db_to_linear(-1.0) + 1e-3,
        "peak {peak} above ceiling"
    );
    // The quiet side is not left at unity: the shared gain was applied to it too.
    assert!(
        l.gain() < 0.999,
        "a loud left channel must reduce the shared gain, got {}",
        l.gain()
    );
}

#[test]
fn limiter_reset_restores_unity_gain() {
    let mut l = SoftLimiter::new_default(SR).unwrap();
    for _ in 0..5_000 {
        let _ = l.process(1.0);
    }
    l.reset();
    assert_eq!(l.gain(), 1.0);
    assert_eq!(l.detector(), 0.0);
}

#[test]
fn limiter_output_is_finite_for_nan_and_infinite_input() {
    let mut l = SoftLimiter::new_default(SR).unwrap();
    // A NaN reaching the master bus must not poison the gain state permanently.
    for _ in 0..1_000 {
        let _ = l.process(f32::NAN);
    }
    let mut y = 0.0;
    for _ in 0..10_000 {
        y = l.process(0.5);
    }
    assert!(y.is_finite(), "limiter did not recover from NaN input");
}
