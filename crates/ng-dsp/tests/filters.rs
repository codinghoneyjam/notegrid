// <META - ROLE : T1.2 unit tests for Filters. | L1-181>
// The workspace deny list is crate-wide, so it reaches this test target too. Tests are not audio
// code: asserting on a Result is what a test is for, and a panic here is the intended failure
// mode. `src/` keeps the deny list with no exemption.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use ng_dsp::filter::{
    db_to_linear, linear_to_db, BiquadFilter, FilterError, OnePole, OnePoleCascade,
};

const SR: u32 = 44_100;

// ---------------------------------------------------------------- Filters

#[test]
fn one_pole_rejects_invalid_coefficients() {
    assert_eq!(OnePole::new(0.0), Err(FilterError::OnePoleCoefficient));
    assert_eq!(OnePole::new(1.0), Err(FilterError::OnePoleCoefficient));
    assert_eq!(OnePole::new(-0.5), Err(FilterError::OnePoleCoefficient));
    assert_eq!(OnePole::new(f32::NAN), Err(FilterError::OnePoleCoefficient));
    assert!(OnePole::new(0.5).is_ok());
}

#[test]
fn one_pole_from_cutoff_rejects_impossible_frequencies() {
    assert_eq!(
        OnePole::from_cutoff(0.0, SR),
        Err(FilterError::FrequencyOutOfRange)
    );
    assert_eq!(
        OnePole::from_cutoff(-100.0, SR),
        Err(FilterError::FrequencyOutOfRange)
    );
    assert_eq!(
        OnePole::from_cutoff(22_051.0, SR),
        Err(FilterError::FrequencyOutOfRange),
        "above Nyquist"
    );
    assert_eq!(
        OnePole::from_cutoff(1000.0, 0),
        Err(FilterError::ZeroSampleRate)
    );
    assert!(OnePole::from_cutoff(1000.0, SR).is_ok());
}

#[test]
fn one_pole_passes_low_frequencies_and_attenuates_high() {
    let mut lp = OnePole::from_cutoff(200.0, SR).unwrap();

    // DC: the output must converge to the input.
    let mut dc = 0.0f32;
    for _ in 0..20_000 {
        dc = lp.process(1.0);
    }
    assert!(dc > 0.99, "DC must pass: {dc}");

    // A 5 kHz square: the fundamental is 25x above the 200 Hz corner, so a one-pole leaves
    // roughly 1/25 of the amplitude, with harmonics falling faster still.
    // 5 kHz at 44100 is a half-period of ~4 samples.
    lp.reset();
    let mut hi_peak = 0.0f32;
    for i in 0..20_000 {
        let x = if (i / 4) % 2 == 0 { 1.0 } else { -1.0 };
        hi_peak = hi_peak.max(lp.process(x).abs());
    }
    assert!(hi_peak < 0.15, "5 kHz square must be attenuated: {hi_peak}");
    assert!(
        hi_peak < dc / 4.0,
        "high frequencies must attenuate well below DC"
    );
}

#[test]
fn one_pole_decays_without_denormals() {
    let mut lp = OnePole::new(0.9999).unwrap();
    for _ in 0..10_000 {
        let _ = lp.process(1.0);
    }
    for _ in 0..100_000 {
        let v = lp.process(0.0);
        if v == 0.0 {
            return; // snapped, as required
        }
        assert!(v > 1e-19, "decayed to {v}, inside the denormal band");
    }
    panic!("one-pole never snapped to zero");
}

#[test]
fn biquad_rejects_invalid_parameters() {
    assert_eq!(
        BiquadFilter::lowpass(1000.0, 0.0, SR),
        Err(FilterError::InvalidQ)
    );
    assert_eq!(
        BiquadFilter::lowpass(-1.0, 0.7, SR),
        Err(FilterError::FrequencyOutOfRange)
    );
    assert_eq!(
        BiquadFilter::peaking(30_000.0, 0.7, 3.0, SR),
        Err(FilterError::FrequencyOutOfRange)
    );
    assert!(BiquadFilter::lowpass(1000.0, 0.707, SR).is_ok());
    assert!(BiquadFilter::peaking(1000.0, 1.0, -6.0, SR).is_ok());
}

#[test]
fn biquad_lowpass_is_stable() {
    let mut f = BiquadFilter::lowpass(1000.0, 0.707, SR).unwrap();
    for i in 0..20_000 {
        let x = if (i / 50) % 2 == 0 { 1.0 } else { -1.0 };
        let y = f.process(x);
        assert!(y.is_finite(), "biquad blew up at sample {i}: {y}");
        assert!(y.abs() <= 1.5, "biquad gain runaway at sample {i}: {y}");
    }
}

#[test]
fn biquad_decays_without_denormals() {
    let mut f = BiquadFilter::lowpass(500.0, 0.707, SR).unwrap();
    for _ in 0..5_000 {
        let _ = f.process(1.0);
    }
    for _ in 0..200_000 {
        let v = f.process(0.0);
        if v == 0.0 {
            return;
        }
        assert!(
            v.abs() > 1e-19,
            "biquad decayed to {v}, inside the denormal band"
        );
    }
    panic!("biquad never snapped to zero");
}

#[test]
fn db_helpers_round_trip() {
    for db in [-60.0f32, -12.0, -1.0, 0.0, 6.0] {
        let lin = db_to_linear(db);
        assert!(
            (linear_to_db(lin) - db).abs() < 0.01,
            "{db} dB round-tripped to {}",
            linear_to_db(lin)
        );
    }
    assert_eq!(db_to_linear(0.0), 1.0);
    assert!(
        linear_to_db(0.0) < -190.0,
        "silence must floor, not return -inf"
    );
    assert!(linear_to_db(0.0).is_finite());
}

#[test]
fn one_pole_cascade_attenuates_more_than_one_pole() {
    let cutoff = 1_000.0;
    let mut one = OnePole::from_cutoff(cutoff, SR).unwrap();
    let mut two = OnePoleCascade::new(cutoff, SR).unwrap();
    // Settle both on DC, then compare their response to the same square burst.
    for _ in 0..20_000 {
        one.process(1.0);
        two.process(1.0);
    }
    let mut one_peak = 0.0f32;
    let mut two_peak = 0.0f32;
    for i in 0..4_000 {
        let x = if (i / 20) % 2 == 0 { 1.0 } else { -1.0 };
        one_peak = one_peak.max(one.process(x).abs());
        two_peak = two_peak.max(two.process(x).abs());
    }
    assert!(
        two_peak < one_peak,
        "cascading two poles must roll off faster: {two_peak} vs {one_peak}"
    );
}

#[test]
fn one_pole_cascade_reset_clears_the_state() {
    let mut c = OnePoleCascade::new(1_000.0, SR).unwrap();
    for _ in 0..5_000 {
        let _ = c.process(1.0);
    }
    c.reset();
    // After reset the first sample starts from zero state, so the output is the coefficient
    // times the input -- non-zero, and far smaller than the settled value.
    let first = c.process(1.0);
    assert!(
        first < 0.5,
        "reset should start from a zero state, got {first}"
    );
}
