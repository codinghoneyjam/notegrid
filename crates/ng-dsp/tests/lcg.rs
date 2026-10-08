// <META - ROLE : T1.2 unit tests for Lcg noise generator. | L1-75>
// The workspace deny list is crate-wide, so it reaches this test target too. Tests are not audio
// code: asserting on a Result is what a test is for, and a panic here is the intended failure
// mode. `src/` keeps the deny list with no exemption.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use ng_dsp::noise::Lcg;

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
