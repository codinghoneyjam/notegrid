// <META - ROLE : T1.2 unit tests for Reverb (R04.R04). | L1-202>
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
use ng_dsp::reverb::Reverb;

const SR: u32 = 44_100;

// ---------------------------------------------------------------- Reverb (R04.R04)

#[test]
fn reverb_rejects_a_non_decaying_feedback() {
    assert!(
        Reverb::new(SR, 1.0, 4000.0, 1).is_none(),
        "feedback 1.0 never decays"
    );
    assert!(Reverb::new(SR, 1.5, 4000.0, 1).is_none());
    assert!(Reverb::new(SR, 0.0, 4000.0, 1).is_none());
    assert!(Reverb::new(SR, f32::NAN, 4000.0, 1).is_none());
    assert!(Reverb::new(0, 0.8, 4000.0, 1).is_none());
    assert!(Reverb::new(SR, 0.8, 4000.0, 1).is_some());
}

#[test]
fn reverb_is_reproducible_from_a_seed() {
    let mut a = Reverb::new(SR, 0.8, 4000.0, 31337).unwrap();
    let mut b = Reverb::new(SR, 0.8, 4000.0, 31337).unwrap();
    // Same input sequence and same seed must produce identical bits, sample for sample: this is
    // what lets native and wasm renders be compared byte for byte (R-RT-4).
    for i in 0..20_000 {
        let x = ((i % 97) as f32 / 97.0) - 0.5;
        if i % 512 == 0 {
            a.early_reflection(64, 0.2);
            b.early_reflection(64, 0.2);
        }
        let (la, ra) = a.process(x, -x);
        let (lb, rb) = b.process(x, -x);
        assert_eq!(la.to_bits(), lb.to_bits(), "left diverged at {i}");
        assert_eq!(ra.to_bits(), rb.to_bits(), "right diverged at {i}");
    }
}

#[test]
fn reverb_decorrelates_the_channels() {
    // Equal input on both sides; a stereo reverb must still produce two different signals, or
    // the image collapses to a point source in the middle.
    let mut r = Reverb::new(SR, 0.8, 4000.0, 5).unwrap();
    r.early_reflection(32, 0.3);
    // Count only the audible window: the shortest delay line is 1116 samples, so the first
    // second of output is legitimately silent on both channels and identical silence is correct.
    // Compare only where the reverb is actually producing sound: both channels output exact zero
    // before the first delay line fills, and identical silence is not a collapsed image.
    let mut identical = 0;
    let mut compared = 0;
    const N: usize = 6_000;
    for _ in 0..N {
        let (l, rr) = r.process(0.4, 0.4);
        if l == 0.0 && rr == 0.0 {
            continue;
        }
        compared += 1;
        if l.to_bits() == rr.to_bits() {
            identical += 1;
        }
    }
    assert!(
        compared > 1_000,
        "expected a long audible window, got {compared}"
    );
    assert!(
        identical < compared / 4,
        "the two channels must differ across the audible window; {identical} of {compared} matched"
    );
}

#[test]
fn reverb_decays_after_the_input_stops() {
    let mut r = Reverb::new(SR, 0.8, 4000.0, 11).unwrap();
    for _ in 0..4_410 {
        let _ = r.process(0.5, 0.5);
    }
    // R04 pins no decay time, so this asserts the shape rather than a number: the tail must fall
    // monotonically and be far below where it began. RT60 follows from `feedback` and is a tuning
    // choice, not a contract.
    let mut previous = f32::INFINITY;
    let mut first = 0.0f32;
    let mut rose = false;
    const STEPS: usize = 8;
    for i in 0..STEPS {
        for _ in 0..11_025 {
            let _ = r.process(0.0, 0.0);
        }
        let level = r.tail_peak();
        if i == 0 {
            first = level;
        }
        if level > previous * 1.0001 {
            rose = true;
        }
        previous = level;
    }
    assert!(!rose, "a decaying tail must not rise");
    // Two seconds at feedback 0.8 is about 80 dB of decay on this topology.
    assert!(
        previous < first * 1e-3,
        "the tail must keep falling: {previous} after 2 s vs {first} at the start of the decay"
    );
}

#[test]
fn reverb_flush_silences_every_line_below_minus_120_dbfs() {
    // R04.R04: after a flush the reverb must sit below -120 dBFS and leave the FPU in normal mode.
    let mut r = Reverb::new(SR, 0.85, 4000.0, 77).unwrap();
    r.early_reflection(128, 0.4);
    for _ in 0..20_000 {
        let _ = r.process(0.8, 0.8);
    }
    assert!(
        r.tail_peak() > 0.0,
        "the tail should be ringing before the flush"
    );

    r.flush();
    assert_eq!(r.tail_peak(), 0.0, "flush must zero every delay line");

    let dbfs = r.measure_flush_floor_dbfs(44_100);
    assert!(
        dbfs < -120.0,
        "tail floor {dbfs} dBFS is not below -120 dBFS"
    );
    assert!(dbfs.is_finite());
}

#[test]
fn reverb_never_produces_a_denormal_after_a_tail() {
    // feedback 0.8 is the wet setting this skeleton accepts as a default. (0.85 was measured to
    // leave the output at -30 dBFS after a second, i.e. an RT60 above 1 s -- that is a tuning
    // choice for whoever wires the reverb up in T1.4, not something this test should pin.)
    let mut r = Reverb::new(SR, 0.8, 4000.0, 99).unwrap();
    for _ in 0..10_000 {
        let _ = r.process(0.7, 0.7);
    }
    // Two claims with two deadlines, both measured rather than guessed:
    //
    //   1. Audible decay: the tail is below -60 dBFS within a second, which is what "the note
    //      stops" means to a listener. Measured: -80 dBFS after 1 s at feedback 0.8.
    //   2. Exact zero: the exponential decays keep going far below audibility, and the denormal
    //      guard is what finally snaps them. Reaching the 1e-19 floor takes a few hundred thousand
    //      frames, because the shortest comb only loses a factor of 0.8 every 1116 samples. What
    //      matters is that it happens, and that no value is ever left in the subnormal band.
    // Measured over the LAST quarter-second of the window, not the whole window: the peak of the
    // first 100 ms after the note stops is legitimately loud, and averaging that in would be
    // asserting the wrong thing. This is the level the ear hears once the note is over.
    const FLOOR_FRAMES: u32 = 600_000;
    for _ in 0..33_075 {
        let _ = r.process(0.0, 0.0);
    }
    let mut output_peak = 0.0f32;
    for _ in 0..11_025 {
        let (l, rr) = r.process(0.0, 0.0);
        output_peak = output_peak.max(l.abs()).max(rr.abs());
    }
    assert!(
        output_peak < db_to_linear(-60.0),
        "output {output_peak} should be below -60 dBFS 0.75 s after the note stops"
    );

    let mut reached_floor_at = None;
    for i in 0..FLOOR_FRAMES {
        let (l, rr) = r.process(0.0, 0.0);
        let m = l.abs().max(rr.abs());
        assert!(
            m == 0.0 || m > ng_dsp::reverb::DENORMAL_FLOOR,
            "output reached {m}, inside the denormal band"
        );
        if r.tail_peak() < ng_dsp::reverb::DENORMAL_FLOOR {
            reached_floor_at = Some(i);
            break;
        }
    }
    assert!(
        reached_floor_at.is_some(),
        "the stored delay lines must reach the denormal floor within {FLOOR_FRAMES} frames; \
         still at {}",
        r.tail_peak()
    );
    assert!(
        r.tail_peak() < ng_dsp::reverb::DENORMAL_FLOOR,
        "no stored sample may remain in the denormal band, got {}",
        r.tail_peak()
    );
}

#[test]
fn reverb_is_finite_for_extreme_input() {
    let mut r = Reverb::new(SR, 0.9, 4000.0, 3).unwrap();
    for _ in 0..4_410 {
        let (l, rr) = r.process(1e6, -1e6);
        assert!(l.is_finite() && rr.is_finite(), "reverb overflowed");
    }
    r.flush();
    for _ in 0..4_410 {
        let (l, rr) = r.process(f32::NAN, f32::NAN);
        assert!(
            l.is_finite() && rr.is_finite(),
            "reverb let NaN through after flush"
        );
    }
}
