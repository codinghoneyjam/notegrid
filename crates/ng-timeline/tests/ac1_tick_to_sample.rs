// <META - ROLE : R02.AC1 tick->sample at 124bpm must equal the contract example exactly. | L1-60>
#[macro_use]
mod common;

use ng_timeline::TempoMap;

fn mpq124() -> Vec<(u32, u32)> {
    // C0.2: mpq = round(60_000_000 / 124) = 483871, computed here from the rule rather than
    // copied, so a change to the rule is caught by this test.
    let raw = 60_000_000f64 / 124.0;
    let mpq = raw.round() as u32;
    assert_eq!(mpq, 483_871, "mpq for 124 bpm per C0.2");
    vec![(0, mpq)]
}

#[test]
fn ac1_124bpm_7680_ticks_matches_the_contract_example() {
    let map = TempoMap::from_mpq_segments(44100, &mpq124()).expect("map builds");
    assert_eq!(map.tick_to_sample(7680), 341_419, "C0.3 EXAMPLE @44100");

    let map48 = TempoMap::from_mpq_segments(48000, &mpq124()).expect("map builds");
    assert_eq!(map48.tick_to_sample(7680), 371_613, "C0.3 EXAMPLE @48000");
}

#[test]
fn ac1_tick_zero_is_sample_zero_at_any_rate() {
    for sr in [44100u32, 48000] {
        let map = TempoMap::from_mpq_segments(sr, &mpq124()).expect("map builds");
        assert_eq!(map.tick_to_sample(0), 0, "sr {sr}");
    }
}

#[test]
fn ac1_matches_the_generated_tempo_vectors() {
    // spec/fixture/tempo/*.json is produced by scripts/gen_tempo_vectors.py from exact rational
    // arithmetic (R02.AC2's second opinion). Run it with --check in CI to prove freshness.
    let mut count = 0;
    for entry in std::fs::read_dir(common::tempo_vectors_dir()).expect("tempo vectors exist") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("vector readable");
        let v: serde_json::Value = serde_json::from_str(&text).expect("vector parses");
        let tempo = v["tempo"].as_array().expect("tempo array");
        for sr in ["44100", "48000"] {
            let sample_rate: u32 = sr.parse().expect("sample rate");
            let points: Vec<(u32, u32)> = tempo
                .iter()
                .map(|t| {
                    let tick = t["tick"].as_u64().expect("tick") as u32;
                    let bpm = t["bpm"].as_f64().expect("bpm");
                    let mpq = (60_000_000.0 / bpm).round() as u32;
                    (tick, mpq)
                })
                .collect();
            let map = TempoMap::from_mpq_segments(sample_rate, &points)
                .unwrap_or_else(|e| panic!("{} @ {sr}: {e:?}", path.display()));
            for (tick, expected) in v["expected"][sr].as_object().expect("expected object") {
                let tick: u32 = tick.parse().expect("tick key");
                let expected: u64 = expected.as_u64().expect("expected value");
                assert_eq!(
                    map.tick_to_sample(tick),
                    expected,
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
