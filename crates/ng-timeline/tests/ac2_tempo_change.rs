// <META - ROLE : R02.AC2 tempo-change projects: change-point sample positions equal an independent exact-rational computation. | L1-100>

/// The independent opinion. Deliberately a different formulation from tempo_map.rs: this
/// walks segment by segment summing exact integer microseconds and converts once, while
/// TempoMap accumulates a running prefix. Both must agree on every probe point.
///
/// The fixtures in spec/fixture/tempo/ come from scripts/gen_tempo_vectors.py, which does the
/// same thing in Python's Fraction with no floating point at all. This test is the in-crate
/// cross-check; the JSON is the out-of-crate one.
mod exact {
    const PPQ: u128 = 480;

    pub fn round_half_up(n: u128, d: u128) -> u128 {
        // (2n + d) / (2d): ties up, no float involved.
        (2 * n + d) / (2 * d)
    }

    pub fn mpq_of(bpm_milli: u128) -> u32 {
        // mpq = round_half_up(60_000_000 / bpm), bpm given in thousandths to stay exact.
        round_half_up(60_000_000_000, bpm_milli) as u32
    }

    /// Sample position of `tick` under the given tempo list (tick, bpm_in_thousandths).
    pub fn sample_of(tick: u32, tempo: &[(u32, u128)], sample_rate: u32) -> u64 {
        let mut micros: u128 = 0;
        let mut acc: u128 = 0; // ticks already consumed
        for (i, (start_tick, bpm_milli)) in tempo.iter().enumerate() {
            let segment_end = tempo.get(i + 1).map_or(u32::MAX, |(t, _)| *t);
            if u128::from(*start_tick) > u128::from(tick) {
                break;
            }
            let covered_end = u128::from(segment_end).min(u128::from(tick));
            if covered_end > u128::from(*start_tick) {
                acc += covered_end - u128::from(*start_tick);
                micros += (covered_end - u128::from(*start_tick)) * u128::from(mpq_of(*bpm_milli));
            }
            let _ = acc;
        }
        round_half_up(micros * u128::from(sample_rate), PPQ * 1_000_000) as u64
    }
}

use ng_timeline::TempoMap;

fn map_for(sample_rate: u32, tempo: &[(u32, f64)]) -> TempoMap {
    let points: Vec<(u32, u32)> = tempo
        .iter()
        .map(|(t, b)| (*t, (60_000_000.0 / b).round() as u32))
        .collect();
    TempoMap::from_mpq_segments(sample_rate, &points).expect("map builds")
}

fn exact_for(sample_rate: u32, tempo: &[(u32, f64)]) -> Vec<(u32, u64)> {
    // bpm as thousandths, exact, so the reference never touches a float either.
    let milli: Vec<(u32, u128)> = tempo
        .iter()
        .map(|(t, b)| (*t, (b * 1000.0).round() as u128))
        .collect();
    let mut probes: Vec<u32> = vec![0];
    for (t, _) in &milli {
        probes.extend([t.saturating_sub(1), *t, t.saturating_add(1)]);
    }
    probes.push(7680);
    probes.retain(|t| *t <= 7680);
    probes.sort_unstable();
    probes.dedup();
    probes
        .into_iter()
        .map(|t| (t, exact::sample_of(t, &milli, sample_rate)))
        .collect()
}

const CASES: &[(&str, &[(u32, f64)])] = &[
    ("single_124bpm", &[(0, 124.0)]),
    ("two_changes", &[(0, 120.0), (1920, 90.0), (4800, 150.0)]),
    ("change_on_bar_boundary", &[(0, 100.0), (3840, 175.0)]),
    (
        "slow_to_fast_extremes",
        &[(0, 20.0), (960, 400.0), (3360, 20.5)],
    ),
    ("late_single_change", &[(0, 132.0), (7200, 68.0)]),
];

#[test]
fn ac2_change_points_match_the_exact_rational_reference() {
    let mut checks = 0;
    for (name, tempo) in CASES {
        for sr in [44100u32, 48000] {
            let map = map_for(sr, tempo);
            for (tick, expected) in exact_for(sr, tempo) {
                assert_eq!(
                    map.tick_to_sample(tick),
                    expected,
                    "{name} @ {sr} tick {tick}: implementation and reference disagree"
                );
                checks += 1;
            }
        }
    }
    assert!(checks >= 50, "expected many probe points, got {checks}");
}

#[test]
fn ac2_matches_the_generated_fixtures() {
    // spec/fixture/tempo/*.json is generated out-of-crate from Python Fractions.
    let mut count = 0;
    for entry in std::fs::read_dir(tempo_dir()).expect("tempo fixtures exist") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("readable"))
                .expect("parses");
        let tempo: Vec<(u32, f64)> = v["tempo"]
            .as_array()
            .expect("tempo array")
            .iter()
            .map(|t| {
                (
                    t["tick"].as_u64().expect("tick") as u32,
                    t["bpm"].as_f64().expect("bpm"),
                )
            })
            .collect();
        for sr in ["44100", "48000"] {
            let sample_rate: u32 = sr.parse().expect("rate");
            let map = map_for(sample_rate, &tempo);
            for (tick, expected) in v["expected"][sr].as_object().expect("expected object") {
                let tick: u32 = tick.parse().expect("tick key");
                assert_eq!(
                    map.tick_to_sample(tick),
                    expected.as_u64().expect("value"),
                    "{} @ {sr} tick {tick}",
                    path.display()
                );
                count += 1;
            }
        }
    }
    assert!(
        count >= 30,
        "expected a meaningful number of checks, got {count}"
    );
}

fn tempo_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/fixture/tempo")
}
