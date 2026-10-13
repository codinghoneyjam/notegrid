// <META - FILE SUMMARY - SF2 timecents to seconds. The table lives in timecents_table.rs so the result is platform-independent.>

include!("timecents_table.rs");

/// Clamp applied before the lookup: SF2 2.01 sec 8.1.2 gives the absolute range of the volume
/// envelope time generators as -12000..5000 (delay, hold) and -12000..8000 (attack, decay, release).
pub const TIMECENT_MIN: i32 = -12000;
pub const TIMECENT_MAX: i32 = 8000;

/// Seconds for one timecent value.
///
/// `2^(tc/1200)` is evaluated as an exact power of two (`f32::powi` only shifts the exponent, so
/// it is exact) times a generated table lookup for the fractional part. The answer is therefore
/// the same f32 bit pattern on every target, which is what lets R03.R02 / R03.AC2 hold without
/// libm -- and libm would need a DECISIONS.md entry, which GLOBAL_DENY puts outside this crate.
pub fn timecents_to_seconds(timecents: i32) -> f32 {
    let tc = timecents.clamp(TIMECENT_MIN, TIMECENT_MAX);
    // `%` truncates towards zero, so `rem_euclid` is what keeps the remainder inside 0..1200
    // for the negative half of the range.
    let exp = tc.div_euclid(1200);
    let frac = tc.rem_euclid(1200).clamp(0, 1199) as usize;
    let frac_value = f32::from_bits(TIMECENT_Q.get(frac).copied().unwrap_or(0x3F80_0000));
    frac_value * f32::powi(2.0, exp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_match_the_spec_examples() {
        // SF2 2.01 sec 8.5 works two examples: 1200 timecents -> 2 s and 3600 -> 8 s.
        assert_eq!(timecents_to_seconds(1200), 2.0);
        assert_eq!(timecents_to_seconds(3600), 8.0);
        assert_eq!(timecents_to_seconds(0), 1.0);
        // Sec 8.1.2: the default -12000 is about 1 ms, and the maximum 8000 is about 100 s.
        assert_eq!(timecents_to_seconds(-12000).to_bits(), 2f32.powi(-10).to_bits());
        assert!((timecents_to_seconds(8000) - 101.593_736_9).abs() < 0.001);
    }

    #[test]
    fn out_of_range_is_clamped_not_wrapped() {
        assert_eq!(timecents_to_seconds(-32768), timecents_to_seconds(TIMECENT_MIN));
        assert_eq!(timecents_to_seconds(32767), timecents_to_seconds(TIMECENT_MAX));
    }

    #[test]
    fn conversion_is_monotonic_positive_and_finite() {
        let mut last = 0.0f32;
        for tc in TIMECENT_MIN..=TIMECENT_MAX {
            let s = timecents_to_seconds(tc);
            assert!(s.is_finite() && s > 0.0, "tc={tc} -> {s}");
            assert!(s >= last, "not monotonic at tc={tc}: {s} < {last}");
            last = s;
        }
    }

    #[test]
    fn negative_exponent_path_is_exact() {
        // -12000 = -10 * 1200, so the fractional part is zero and the answer is 2^-10 exactly.
        assert_eq!(timecents_to_seconds(-12000), 2f32.powi(-10));
        assert_eq!(timecents_to_seconds(6000).to_bits(), 32f32.to_bits());
    }
}