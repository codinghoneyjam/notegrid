// <META - ROLE : T1.2 unit tests for Biquad v2 forms. | L1-68>
// The workspace deny list is crate-wide, so it reaches this test target too. Tests are not audio
// code: asserting on a Result is what a test is for, and a panic here is the intended failure
// mode. `src/` keeps the deny list with no exemption.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use ng_dsp::filter::{BiquadFilter, FilterError};

const SR: u32 = 44_100;

// ---------------------------------------------------------------- Biquad v2 forms

#[test]
fn biquad_highpass_rejects_invalid_parameters() {
    assert_eq!(
        BiquadFilter::highpass(1000.0, 0.0, SR),
        Err(FilterError::InvalidQ)
    );
    assert!(BiquadFilter::highpass(1000.0, 0.707, SR).is_ok());
}

#[test]
fn biquad_highpass_passes_high_and_blocks_dc() {
    let mut f = BiquadFilter::highpass(500.0, 0.707, SR).unwrap();
    // DC settles to zero through a highpass.
    for _ in 0..20_000 {
        let y = f.process(1.0);
        assert!(y.is_finite());
    }
    let settled = f.process(1.0);
    assert!(settled.abs() < 1e-3, "DC through a highpass: {settled}");
    // A 5 kHz square wave survives with most of its amplitude.
    let mut peak = 0.0f32;
    for i in 0..2_000 {
        let t = i as f32 / SR as f32;
        let x = if (t * 5_000.0 * 2.0) as i32 % 2 == 0 {
            1.0
        } else {
            -1.0
        };
        peak = peak.max(f.process(x).abs());
    }
    assert!(peak > 0.5, "5 kHz through a 500 Hz highpass: {peak}");
}

#[test]
fn biquad_bandpass_rejects_invalid_parameters() {
    assert_eq!(
        BiquadFilter::bandpass(1000.0, 0.0, SR),
        Err(FilterError::InvalidQ)
    );
    assert_eq!(
        BiquadFilter::bandpass(60_000.0, 1.0, SR),
        Err(FilterError::FrequencyOutOfRange)
    );
    assert!(BiquadFilter::bandpass(1000.0, 1.0, SR).is_ok());
}

#[test]
fn biquad_bandpass_passes_the_center_and_blocks_the_bands() {
    // Centre a bandpass at 1 kHz; DC and 10 kHz should be strongly attenuated, 1 kHz passes.
    let mut f = BiquadFilter::bandpass(1_000.0, 4.0, SR).unwrap();
    for _ in 0..20_000 {
        let _ = f.process(1.0);
    }
    assert!(f.process(1.0).abs() < 1e-3, "DC must be blocked");

    let mut passes_1k = 0.0f32;
    for i in 0..4_410 {
        let x = ng_dsp_test_sin(1_000.0, i);
        passes_1k = passes_1k.max(f.process(x).abs());
    }
    let mut f2 = BiquadFilter::bandpass(1_000.0, 4.0, SR).unwrap();
    for i in 0..4_410 {
        let x = ng_dsp_test_sin(8_000.0, i);
        f2.process(x);
    }
    let mut passes_8k = 0.0f32;
    for i in 4_410..8_820 {
        let x = ng_dsp_test_sin(8_000.0, i);
        passes_8k = passes_8k.max(f2.process(x).abs());
    }
    assert!(
        passes_1k > 0.5 && passes_8k < 0.1 * passes_1k,
        "1k {passes_1k} vs 8k {passes_8k}"
    );
}

fn ng_dsp_test_sin(freq: f32, i: i32) -> f32 {
    libm::sinf(2.0 * core::f32::consts::PI * freq * i as f32 / SR as f32)
}
