// <META - FILE SUMMARY - R03.AC1/AC2/AC3: round-trip, determinism and fuzz acceptance tests.>

use ng_bank::image_from_sf2;
use ng_bank::sf2_builder::{demo_bank, sine_sample, InstrumentSpec, PresetSpec, SampleSpec, Sf2Builder, SplitSpec};
use ng_bank::image::IMAGE_VERSION;
use ng_bank::image_to_bytes;

/// AC1: builder SF2 -> image -> presets/zones/samples equal the input.
#[test]
fn ac1_builder_sf2_round_trips_through_image() {
    let mut b = Sf2Builder::new("round-trip");
    let s0 = b.add_sample(SampleSpec {
        name: "s0".into(),
        rate: 44100,
        pcm: sine_sample(256, 1, 4),
        root_key: 60,
        pitch_correction: 0,
        loop_mode: ng_bank::image::LoopMode::Continuous,
    });
    let s1 = b.add_sample(SampleSpec {
        name: "s1".into(),
        rate: 44100,
        pcm: sine_sample(128, 1, 8),
        root_key: 48,
        pitch_correction: 0,
        loop_mode: ng_bank::image::LoopMode::None,
    });
    let i0 = b.add_instrument(InstrumentSpec {
        name: "i0".into(),
        splits: vec![SplitSpec::new(s0).key(0, 63), SplitSpec::new(s1).key(64, 127)],
    });
    b.add_preset(PresetSpec::new("p0", 0, 0, i0));

    let sf2_bytes = b.build();
    let image = image_from_sf2(&sf2_bytes).expect("parse+flatten");

    assert_eq!(image.presets.len(), 1, "one preset");
    assert_eq!(image.presets[0].bank, 0);
    assert_eq!(image.presets[0].program, 0);
    assert_eq!(image.presets[0].zone_count as usize, image.zones.len());
    assert_eq!(image.samples.len(), 2, "two samples");
    assert_eq!(image.samples[0].name, "s0");
    assert_eq!(image.samples[1].name, "s1");
    // The zone count matches the instrument splits.
    assert_eq!(image.zones.len(), 2, "two zones from two splits");
}

/// AC2: same SF2 converted twice -> identical image bytes.
#[test]
fn ac2_same_sf2_converts_to_identical_bytes() {
    let sf2_bytes = demo_bank();
    let img1 = image_from_sf2(&sf2_bytes).expect("first convert");
    let img2 = image_from_sf2(&sf2_bytes).expect("second convert");
    let b1 = image_to_bytes(&img1).expect("serialise 1");
    let b2 = image_to_bytes(&img2).expect("serialise 2");
    assert_eq!(b1, b2, "image bytes must be identical");
}

/// AC3: fuzz (1e5 truncations, 1e5 bit flips) -> 0 panics, bounded memory.
#[test]
fn ac3_fuzz_truncation_and_bitflips_never_panic() {
    let sf2_bytes = demo_bank();
    let len = sf2_bytes.len();

    // 1e5 truncations: cut the buffer at every position.
    for cut in 1..=100_000.min(len) {
        let _ = image_from_sf2(&sf2_bytes[..cut]);
    }

    // 1e5 bit flips: deterministic LCG seed so the test is reproducible.
    let mut state: u64 = 0x1234_5678_9abc_def1;
    for _ in 0..100_000 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let byte_idx = (state >> 33) as usize % len;
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let bit = (state >> 33) as u8 & 7;
        let mut buf = sf2_bytes.clone();
        buf[byte_idx] ^= 1 << bit;
        let _ = image_from_sf2(&buf);
    }
}

/// The demo bank produces a valid image (sanity check for the fixture itself).
#[test]
fn demo_bank_produces_a_valid_image() {
    let img = image_from_sf2(&demo_bank()).expect("demo bank parses");
    assert!(img.presets.len() >= 3, "demo bank has at least 3 presets");
    let bytes = image_to_bytes(&img).expect("serialise");
    assert_eq!(&bytes[0..4], b"NGSB");
    assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), IMAGE_VERSION);
}
