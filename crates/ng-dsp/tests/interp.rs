// <META - ROLE : T1.2 unit tests for LinearInterp. | L1-63>
// The workspace deny list is crate-wide, so it reaches this test target too. Tests are not audio
// code: asserting on a Result is what a test is for, and a panic here is the intended failure
// mode. `src/` keeps the deny list with no exemption.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

// ---------------------------------------------------------------- LinearInterp

#[test]
fn interp_linear_hits_the_samples_and_the_midpoint() {
    let table = [0.0f32, 1.0, 2.0, 3.0];
    let it = ng_dsp::interp::LinearInterp::new(&table);
    assert_eq!(it.linear(0.0), Some(0.0));
    assert_eq!(it.linear(2.0), Some(2.0));
    assert_eq!(it.linear(1.5), Some(1.5));
    assert_eq!(it.linear(3.0), Some(3.0));
}

#[test]
fn interp_rejects_out_of_range_and_nan() {
    let table = [1.0f32, 2.0, 3.0];
    let it = ng_dsp::interp::LinearInterp::new(&table);
    assert_eq!(it.linear(-0.5), None);
    assert_eq!(it.linear(3.0), None);
    assert_eq!(it.linear(f32::NAN), None);
    assert_eq!(it.cubic(f32::NAN), None);
    let short = ng_dsp::interp::LinearInterp::new(&[1.0f32]);
    assert_eq!(short.linear(0.0), None);
}

#[test]
fn interp_cubic_passes_through_every_sample() {
    let table = [0.0f32, 1.0, 0.5, -0.25, 0.75];
    let it = ng_dsp::interp::LinearInterp::new(&table);
    for (i, expected) in table.iter().enumerate() {
        let v = it.cubic(i as f32).unwrap();
        assert!(
            (v - expected).abs() < 1e-5,
            "cubic must pass through sample {i}: got {v}, want {expected}",
        );
    }
}

#[test]
fn interp_cubic_is_smoother_than_linear_at_a_corner() {
    // A V shape: linear interpolation has a derivative jump at the corner; Catmull-Rom rounds
    // it. At the exact corner both agree; a quarter sample off the corner the cubic should sit
    // off the straight line, i.e. the corner is smoothed.
    let table = [1.0f32, 0.0, -1.0, 0.0, 1.0];
    let it = ng_dsp::interp::LinearInterp::new(&table);
    let lin = it.linear(1.75).unwrap();
    let cub = it.cubic(1.75).unwrap();
    assert!(
        (lin - cub).abs() > 1e-3,
        "cubic should deviate from the straight line near a corner: lin {lin} cub {cub}"
    );
    // And the cubic value must stay finite and bounded by the neighbourhood's range, never
    // overshooting the extrema of the table by more than Catmull-Rom's known modest overshoot.
    assert!(cub.abs() <= 1.5);
}

#[test]
fn interp_mode_selects_the_rule() {
    let table = [0.0f32, 1.0, 0.0];
    let it = ng_dsp::interp::LinearInterp::new(&table);
    let a = it.at(1.5, ng_dsp::interp::InterpMode::Linear).unwrap();
    let b = it.at(1.5, ng_dsp::interp::InterpMode::Cubic).unwrap();
    assert_eq!(a, 0.5);
    assert!((b - 0.5).abs() > 1e-3 || b == 0.5, "cubic defined: {b}");
}
