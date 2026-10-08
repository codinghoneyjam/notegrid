// <META - ROLE : T1.2 unit tests for Ramp. | L1-78>
// The workspace deny list is crate-wide, so it reaches this test target too. Tests are not audio
// code: asserting on a Result is what a test is for, and a panic here is the intended failure
// mode. `src/` keeps the deny list with no exemption.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use ng_dsp::ramp::{Ramp, RampError};

const SR: u32 = 44_100;

// ---------------------------------------------------------------- Ramp

#[test]
fn ramp_reaches_target_exactly() {
    let mut r = Ramp::new(0.0, 1.0, 100).unwrap();
    for _ in 0..99 {
        r.tick();
        assert!(!r.is_done());
    }
    assert_eq!(r.tick(), 1.0, "the last sample lands exactly on target");
    assert!(r.is_done());
    // Past the end it holds, with no accumulated drift.
    assert_eq!(r.tick(), 1.0);
    assert_eq!(r.tick(), 1.0);
}

#[test]
fn ramp_default_is_five_milliseconds() {
    let r = Ramp::new_default(0.0, 1.0, SR).unwrap();
    // 5 ms at 44100 = 220.5 samples; the ramp must round to a whole sample count.
    let stepped = Ramp::new(0.0, 1.0, 221).unwrap();
    assert_eq!(Ramp::DEFAULT_MS, 5.0);
    assert!((r.target() - stepped.target()).abs() < f32::EPSILON);
    let mut a = Ramp::new_default(0.0, 1.0, SR).unwrap();
    let mut b = stepped;
    let mut diff = 0.0f32;
    for _ in 0..400 {
        diff = diff.max((a.tick() - b.tick()).abs());
    }
    assert!(
        diff < 1e-6,
        "5 ms default must match a 221-sample ramp, max diff {diff}"
    );
}

#[test]
fn ramp_48000_differs_from_44100_as_expected() {
    // 5 ms is 220.5 samples at 44100 and 240 at 48000; the counts must differ, the ratio must not.
    let mut a = Ramp::new_default(0.0, 1.0, 44_100).unwrap();
    let mut b = Ramp::new_default(0.0, 1.0, 48_000).unwrap();
    let mut a_n = 0;
    let mut b_n = 0;
    while !a.is_done() {
        a.tick();
        a_n += 1;
    }
    while !b.is_done() {
        b.tick();
        b_n += 1;
    }
    assert_eq!(a_n, 221, "44100");
    assert_eq!(b_n, 240, "48000");
}

#[test]
fn ramp_rejects_zero_length_and_snaps() {
    assert_eq!(Ramp::new(0.0, 1.0, 0), Err(RampError::ZeroLength));
    let mut r = Ramp::new(0.0, 1.0, 100).unwrap();
    for _ in 0..10 {
        r.tick();
    }
    assert!(r.current() > 0.0);
    assert_eq!(r.snap(), 1.0);
    assert!(r.is_done());
    assert_eq!(r.current(), 1.0);
}

#[test]
fn ramp_advance_matches_repeated_ticks() {
    let mut a = Ramp::new(0.2, 0.9, 50).unwrap();
    let mut b = Ramp::new(0.2, 0.9, 50).unwrap();
    b.advance(10);
    for _ in 0..10 {
        a.tick();
    }
    assert_eq!(a.current(), b.current());
}
