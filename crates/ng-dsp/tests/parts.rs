// <META - ROLE : T1.2 unit tests for the DSP parts: Envelope, Ramp, Lcg, OnePole/Biquad, SoftLimiter, and the R04.R04 denormal assertion. | L1-260>
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
use ng_dsp::filter::{
    db_to_linear, linear_to_db, BiquadFilter, FilterError, OnePole, OnePoleCascade,
};
use ng_dsp::limiter::{LimiterError, SoftLimiter};
use ng_dsp::noise::Lcg;
use ng_dsp::ramp::{Ramp, RampError};
use ng_dsp::reverb::Reverb;

const SR: u32 = 44_100;

// ---------------------------------------------------------------- Lcg

#[test]
fn lcg_is_reproducible_from_a_seed() {
    let mut a = Lcg::new(12345);
    let mut b = Lcg::new(12345);
    for _ in 0..1000 {
        assert_eq!(a.next_u64(), b.next_u64());
    }
}

#[test]
fn lcg_resets_to_the_same_state() {
    let mut g = Lcg::new(99);
    for _ in 0..100 {
        let _ = g.next_u64();
    }
    g.reset(99);
    let mut fresh = Lcg::new(99);
    assert_eq!(g.next_u64(), fresh.next_u64());
}

#[test]
fn lcg_seed_zero_does_not_stick() {
    // A zero state would be a fixed point of the mixer; the constructor substitutes a constant.
    let mut g = Lcg::new(0);
    let first = g.next_u64();
    let mut repeats = 0;
    for _ in 0..1000 {
        if g.next_u64() == first {
            repeats += 1;
        }
    }
    assert_eq!(repeats, 0, "seed 0 must not produce a fixed point");
}

#[test]
fn lcg_unit_range_and_distribution() {
    let mut g = Lcg::new(7);
    let mut sum = 0.0f64;
    let mut buckets = [0u32; 8];
    const N: u32 = 80_000;
    for _ in 0..N {
        let u = g.next_unit();
        assert!((0.0..1.0).contains(&u), "next_unit out of range: {u}");
        sum += f64::from(u);
        let b = (u * 8.0) as usize;
        if b < 8 {
            buckets[b] += 1;
        }
    }
    let mean = sum / f64::from(N);
    assert!((mean - 0.5).abs() < 0.01, "mean {mean} should be ~0.5");
    for (i, count) in buckets.iter().enumerate() {
        assert!(
            (*count as i64 - N as i64 / 8).abs() < N as i64 / 40,
            "bucket {i} has {count}, expected ~{}",
            N / 8
        );
    }
}

#[test]
fn lcg_bipolar_is_centered() {
    let mut g = Lcg::new(4242);
    let mut sum = 0.0f64;
    const N: u32 = 40_000;
    for _ in 0..N {
        let b = g.next_bipolar();
        assert!((-1.0..1.0).contains(&b));
        sum += f64::from(b);
    }
    assert!((sum / f64::from(N)).abs() < 0.01);
}

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
