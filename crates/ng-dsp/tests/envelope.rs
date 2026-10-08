// <META - ROLE : T1.2 unit tests for Envelope. | L1-158>
// The workspace deny list is crate-wide, so it reaches this test target too. Tests are not audio
// code: asserting on a Result is what a test is for, and a panic here is the intended failure
// mode. `src/` keeps the deny list with no exemption.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use ng_dsp::envelope::{Envelope, Stage};

const SR: u32 = 44_100;

// ---------------------------------------------------------------- Envelope

#[test]
fn envelope_walks_dahdsr_in_order() {
    let mut e = Envelope::new(SR, (0.01, 0.05, 0.20), 0.6);
    assert_eq!(e.stage(), Stage::Idle);
    assert_eq!(e.tick(), 0.0);

    e.gate_on();
    assert_eq!(e.stage(), Stage::Attack);
    // Attack rises monotonically to exactly 1.0.
    let mut prev = 0.0;
    for _ in 0..441 {
        let v = e.tick();
        assert!(v > prev, "attack must rise: {prev} -> {v}");
        prev = v;
    }
    assert_eq!(prev, 1.0, "attack lands on 1.0 exactly");
    assert_eq!(e.stage(), Stage::Decay);

    // Decay falls to the sustain level and stays there.
    let mut last = e.tick();
    for _ in 0..2205 {
        last = e.tick();
    }
    assert_eq!(e.stage(), Stage::Sustain);
    assert!(
        (last - 0.6).abs() < 1e-5,
        "decay must land on sustain, got {last}"
    );

    // Sustain holds while gated.
    for _ in 0..1000 {
        assert!((e.tick() - 0.6).abs() < 1e-6);
    }
    assert_eq!(e.stage(), Stage::Sustain);

    e.gate_off();
    assert_eq!(e.stage(), Stage::Release);
    let _ = last;
}

#[test]
fn envelope_release_decays_to_idle_and_silence() {
    let mut e = Envelope::new(SR, (0.001, 0.001, 0.05), 0.5);
    e.gate_on();
    for _ in 0..500 {
        e.tick();
    }
    e.gate_off();
    let mut last = 0.0;
    // A 50 ms release is 2205 samples; allow a wide margin so the assertion is about "it ends",
    // not about the exact sample.
    for _ in 0..44_100 {
        last = e.tick();
        if e.is_idle() {
            break;
        }
    }
    assert!(e.is_idle(), "a 50 ms release must have finished");
    assert_eq!(last, 0.0, "and snapped to exact silence, not a denormal");
}

#[test]
fn envelope_release_is_monotonic_downward() {
    let mut e = Envelope::new(SR, (0.005, 0.005, 0.3), 0.7);
    e.gate_on();
    for _ in 0..500 {
        e.tick();
    }
    e.gate_off();
    let mut prev = e.level();
    for _ in 0..13_230 {
        let v = e.tick();
        assert!(v <= prev + 1e-7, "release must not rise: {prev} -> {v}");
        prev = v;
    }
}

#[test]
fn envelope_silence_frees_the_voice_immediately() {
    // C2.4: seek -> silence_all. No tail, no release ramp.
    let mut e = Envelope::new(SR, (0.001, 0.001, 5.0), 0.8);
    e.gate_on();
    for _ in 0..200 {
        e.tick();
    }
    e.silence();
    assert!(e.is_idle());
    assert_eq!(e.level(), 0.0);
    assert_eq!(e.tick(), 0.0);
}

#[test]
fn envelope_zero_length_stages_are_instant_but_do_not_divide_by_zero() {
    let mut e = Envelope::new(SR, (0.0, 0.0, 0.0), 0.5);
    e.gate_on();
    let v = e.tick();
    assert!(
        v.is_finite(),
        "a zero-length attack must not produce NaN or inf"
    );
    e.gate_off();
    for _ in 0..1000 {
        let v = e.tick();
        assert!(v.is_finite());
    }
    assert!(e.is_idle());
}

#[test]
fn envelope_negative_and_nan_stages_do_not_produce_nan() {
    for bad in [-1.0f32, f32::NAN, f32::INFINITY] {
        let mut e = Envelope::new(SR, (bad, bad, bad), 0.5);
        e.gate_on();
        for _ in 0..2000 {
            assert!(
                e.tick().is_finite(),
                "stage {bad} produced a non-finite level"
            );
        }
    }
}

#[test]
fn envelope_never_exceeds_unity() {
    let mut e = Envelope::new(SR, (0.02, 0.10, 1.0), 0.9);
    e.gate_on();
    for _ in 0..50_000 {
        let v = e.tick();
        assert!((0.0..=1.0).contains(&v), "level {v} escaped 0..=1");
    }
}

#[test]
fn envelope_never_reaches_the_denormal_range() {
    // The point of the denormal floor: the release must snap, not decay to 1e-38.
    let mut e = Envelope::new(SR, (0.001, 0.001, 0.010), 0.5);
    e.gate_on();
    for _ in 0..500 {
        e.tick();
    }
    e.gate_off();
    let mut min_positive = 1.0f32;
    for _ in 0..4410 {
        let v = e.tick();
        if v > 0.0 && v < min_positive {
            min_positive = v;
        }
    }
    assert!(
        min_positive > ng_dsp::envelope::DENORMAL_FLOOR,
        "decayed to {min_positive}, at or below the denormal floor"
    );
    assert_eq!(e.level(), 0.0);
}
