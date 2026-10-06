// <META - ROLE : R02.AC3 property: sample_to_tick(tick_to_sample(t)) == t and strict monotonicity over bpm/sample-rate ranges. | L1-120>
use ng_timeline::TempoMap;
use proptest::prelude::*;

/// Build a map from bpm values at given ticks, converting bpm through the C0.2 rule.
fn map_from_bpm(sample_rate: u32, points: &[(u32, f64)]) -> TempoMap {
    let mpq_points: Vec<(u32, u32)> = points
        .iter()
        .map(|(tick, bpm)| (*tick, (60_000_000.0 / bpm).round() as u32))
        .collect();
    TempoMap::from_mpq_segments(sample_rate, &mpq_points).expect("map builds")
}

proptest! {
    #[test]
    fn ac3_roundtrip_single_tempo(
        bpm in 20.0f64..=400.0,
        sr_idx in 0usize..2,
        ticks in prop::collection::vec(0u32..200_000, 1..64),
    ) {
        let sr = if sr_idx == 0 { 44100 } else { 48000 };
        let map = map_from_bpm(sr, &[(0, bpm)]);
        for t in ticks {
            let s = map.tick_to_sample(t);
            let back = map.sample_to_tick(s);
            prop_assert!(back == t, "bpm={} sr={} tick={} sample={} back={}", bpm, sr, t, s, back);
        }
    }

    #[test]
    fn ac3_roundtrip_with_tempo_changes(
        sr_idx in 0usize..2,
        a in 20.0f64..=400.0,
        b in 20.0f64..=400.0,
        change in 1u32..100_000,
        probe in prop::collection::vec(0u32..200_000, 1..32),
    ) {
        let sr = if sr_idx == 0 { 44100 } else { 48000 };
        let map = map_from_bpm(sr, &[(0, a), (change, b)]);
        for t in probe {
            let s = map.tick_to_sample(t);
            let back = map.sample_to_tick(s);
            prop_assert!(
                back == t,
                "sr={} a={} b={} change={} tick={} sample={} back={}",
                sr, a, b, change, t, s, back
            );
        }
    }

    #[test]
    fn ac3_strictly_monotonic(
        sr_idx in 0usize..2,
        a in 20.0f64..=400.0,
        b in 20.0f64..=400.0,
        change in 1u32..100_000,
        span in 1u32..50_000,
    ) {
        let sr = if sr_idx == 0 { 44100 } else { 48000 };
        let map = map_from_bpm(sr, &[(0, a), (change, b)]);
        let mut prev = map.tick_to_sample(0);
        for t in 1..=span {
            let cur = map.tick_to_sample(t);
            prop_assert!(cur > prev, "not monotonic at tick={}: {} <= {}", t, cur, prev);
            prev = cur;
        }
    }

    #[test]
    fn ac3_both_rates_covered(
        bpm in 20.0f64..=400.0,
        tick in 0u32..500_000,
    ) {
        // Guards the property test's rate index actually reaching both rates.
        let m44 = map_from_bpm(44100, &[(0, bpm)]);
        let m48 = map_from_bpm(48000, &[(0, bpm)]);
        let back44 = m44.sample_to_tick(m44.tick_to_sample(tick));
        let back48 = m48.sample_to_tick(m48.tick_to_sample(tick));
        prop_assert!(back44 == tick, "44100 tick={} back={}", tick, back44);
        prop_assert!(back48 == tick, "48000 tick={} back={}", tick, back48);
    }
}

#[test]
fn ac3_known_values_44100_and_48000() {
    let m44 = map_from_bpm(44100, &[(0, 124.0)]);
    let m48 = map_from_bpm(48000, &[(0, 124.0)]);
    assert_eq!(m44.tick_to_sample(7680), 341_419);
    assert_eq!(m48.tick_to_sample(7680), 371_613);
    // 48000 is strictly faster in samples per second for the same musical length.
    assert!(m48.tick_to_sample(7680) > m44.tick_to_sample(7680));
}
