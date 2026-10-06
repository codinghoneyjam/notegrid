// <META - ROLE : R04.R01 synth_contract, generic over S: Synth; run against OscSynth (AC1, AC2). | L1-170>
// Tests are not HOTPATH: panicking here is the intended failure mode.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use ng_core::{Buses, LaneKind, NoteSink, Slot, Synth};
use ng_dsp::noise::Lcg;
use ng_synth::osc::OscSynth;

const SR: u32 = 44_100;

struct Buf {
    dl: Vec<f32>,
    dr: Vec<f32>,
    sl: Vec<f32>,
    sr: Vec<f32>,
}

impl Buf {
    fn new(n: usize) -> Self {
        Self {
            dl: vec![0.0; n],
            dr: vec![0.0; n],
            sl: vec![0.0; n],
            sr: vec![0.0; n],
        }
    }
}

fn render_n<S: Synth>(s: &mut S, n: usize) -> Buf {
    let mut b = Buf::new(n);
    {
        let mut buses = Buses {
            dry_l: &mut b.dl,
            dry_r: &mut b.dr,
            send_l: &mut b.sl,
            send_r: &mut b.sr,
        };
        s.render(&mut buses);
    }
    b
}

fn peak(b: &Buf) -> f32 {
    b.dl.iter()
        .chain(&b.dr)
        .cloned()
        .fold(0.0f32, |m, x| m.max(x.abs()))
}

fn all_finite(b: &Buf) -> bool {
    b.dl.iter()
        .chain(&b.dr)
        .chain(&b.sl)
        .chain(&b.sr)
        .all(|x| x.is_finite())
}

// ---------------------------------------------------------------- R04.R01 items

fn item_idle_output_exactly_zero<S: Synth>(make: &dyn Fn() -> S) {
    let mut s = make();
    let b = render_n(&mut s, 2048);
    assert!(b.dl.iter().all(|&x| x == 0.0));
    assert!(b.dr.iter().all(|&x| x == 0.0));
    assert!(b.sl.iter().all(|&x| x == 0.0));
    assert!(b.sr.iter().all(|&x| x == 0.0));
}

fn item_note_on_produces_energy<S: Synth>(make: &dyn Fn() -> S) {
    let mut s = make();
    s.set_program(Slot(0), LaneKind::Melodic, 0);
    s.note_on(Slot(0), 69, 100);
    let b = render_n(&mut s, 2048);
    assert!(peak(&b) > 1e-3, "note_on produced no energy");
}

fn item_note_off_below_60db_within_release<S: Synth>(make: &dyn Fn() -> S, release_s: f32) {
    let mut s = make();
    s.set_program(Slot(0), LaneKind::Melodic, 0);
    s.note_on(Slot(0), 69, 100);
    let _ = render_n(&mut s, 1024);
    s.note_off(Slot(0), 69);
    let tail = (release_s * SR as f32) as usize + 2048;
    let b = render_n(&mut s, tail);
    let last = &b.dl[tail - 512..];
    let m = last.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(
        m < 1e-3,
        "release tail at {} is {}",
        m,
        tail as f32 / SR as f32
    );
}

fn item_gain_zero_is_silence<S: Synth>(make: &dyn Fn() -> S) {
    let mut s = make();
    s.set_program(Slot(0), LaneKind::Melodic, 0);
    s.set_gain(Slot(0), 0.0);
    s.note_on(Slot(0), 69, 100);
    let b = render_n(&mut s, 2048);
    assert!(b.dl.iter().all(|&x| x == 0.0));
}

fn item_pan_centre_and_hard_left<S: Synth>(make: &dyn Fn() -> S) {
    let mut s = make();
    s.set_program(Slot(0), LaneKind::Melodic, 0);
    s.set_pan(Slot(0), 64);
    s.note_on(Slot(0), 69, 100);
    let b = render_n(&mut s, 1024);
    for (l, r) in b.dl.iter().zip(&b.dr) {
        assert_eq!(l, r, "centre pan must give L == R bit-for-bit");
    }
    assert!(peak(&b) > 1e-3);

    let mut s = make();
    s.set_program(Slot(0), LaneKind::Melodic, 0);
    s.set_pan(Slot(0), 0);
    s.note_on(Slot(0), 69, 100);
    let b = render_n(&mut s, 1024);
    assert!(b.dr.iter().all(|&x| x == 0.0), "hard left: R must be 0");
    assert!(peak(&b) > 1e-3);
}

fn item_two_instances_bit_identical<S: Synth>(make: &dyn Fn() -> S) {
    let script = |s: &mut S| {
        s.set_program(Slot(0), LaneKind::Melodic, 0);
        s.set_program(Slot(1), LaneKind::Drum, 0);
        s.note_on(Slot(0), 60, 100);
        s.note_on(Slot(1), 36, 110);
        let a = render_n(s, 256);
        s.note_off(Slot(0), 60);
        s.note_on(Slot(0), 64, 90);
        let b = render_n(s, 256);
        (a, b)
    };
    let mut a = make();
    let mut b = make();
    let (a1, a2) = script(&mut a);
    let (b1, b2) = script(&mut b);
    assert_eq!(a1.dl, b1.dl);
    assert_eq!(a1.dr, b1.dr);
    assert_eq!(a2.dl, b2.dl);
    assert_eq!(a2.dr, b2.dr);
}

fn item_random_events_no_nan<S: Synth>(make: &dyn Fn() -> S) {
    let mut s = make();
    let mut g = Lcg::new(0x5EED);
    let slots = [Slot(0), Slot(1), Slot(2), Slot(3)];
    for slot in slots {
        s.set_program(slot, LaneKind::Melodic, (g.next_u64() % 128) as u8);
    }
    let total = SR as usize * 10;
    let mut rendered = 0usize;
    while rendered < total {
        let n = 512usize.min(total - rendered);
        let event = g.next_u64() % 4;
        let slot = slots[(g.next_u64() % 4) as usize];
        match event {
            0 => s.note_on(slot, (g.next_u64() % 88) as u8, (g.next_u64() % 127) as u8),
            1 => s.note_off(slot, (g.next_u64() % 88) as u8),
            2 => s.set_gain(slot, (g.next_u64() % 100) as f32 / 100.0),
            _ => s.set_program(slot, LaneKind::Melodic, (g.next_u64() % 128) as u8),
        }
        let b = render_n(&mut s, n);
        assert!(
            all_finite(&b),
            "NaN/Inf in render after {} samples",
            rendered
        );
        rendered += n;
    }
}

fn item_voice_cap_respected<S: Synth>(make: &dyn Fn() -> S, cap: usize) {
    let mut s = make();
    for i in 0..(cap * 2) {
        s.note_on(Slot(0), 48 + (i % 12) as u8, 100);
    }
    assert!(s.active_voices() <= cap, "cap {} exceeded", cap);
}

fn item_silence_all<S: Synth>(make: &dyn Fn() -> S) {
    let mut s = make();
    s.note_on(Slot(0), 60, 100);
    s.note_on(Slot(1), 64, 100);
    let _ = render_n(&mut s, 512);
    s.silence_all();
    let b = render_n(&mut s, 1024);
    assert!(b.dl.iter().all(|&x| x == 0.0));
    assert_eq!(s.active_voices(), 0);
}

fn synth_contract<S: Synth>(make: &dyn Fn() -> S) {
    item_idle_output_exactly_zero(make);
    item_note_on_produces_energy(make);
    item_note_off_below_60db_within_release(make, 0.25);
    item_gain_zero_is_silence(make);
    item_pan_centre_and_hard_left(make);
    item_two_instances_bit_identical(make);
    item_random_events_no_nan(make);
    item_silence_all(make);
}

// ---------------------------------------------------------------- drivers

#[test]
fn osc_synth_passes_synth_contract() {
    synth_contract::<OscSynth>(&|| OscSynth::new(SR, 64));
}

#[test]
fn osc_synth_respects_the_voice_cap() {
    item_voice_cap_respected::<OscSynth>(&|| OscSynth::new(SR, 4), 4);
}

// ---------------------------------------------------------------- AC2

#[test]
fn ac2_all_128_programs_by_3_keys_are_non_silent() {
    let mut s = OscSynth::new(SR, 64);
    for program in 0..128u8 {
        for &key in &[48u8, 60, 72] {
            s.silence_all();
            s.set_program(Slot(0), LaneKind::Melodic, program);
            s.note_on(Slot(0), key, 100);
            let b = render_n(&mut s, SR as usize / 4);
            assert!(all_finite(&b), "program {program} key {key}: NaN/Inf");
            assert!(peak(&b) > 1e-4, "program {program} key {key}: silent");
        }
    }
}

#[test]
fn drum_lane_sounds_for_common_kit_keys() {
    let mut s = OscSynth::new(SR, 16);
    s.set_program(Slot(0), LaneKind::Drum, 0);
    for key in [35u8, 38, 42, 46, 49, 47] {
        s.silence_all();
        s.note_on(Slot(0), key, 110);
        let b = render_n(&mut s, SR as usize / 4);
        assert!(all_finite(&b), "drum key {key}: NaN/Inf");
        assert!(peak(&b) > 1e-4, "drum key {key}: silent");
    }
}

#[test]
fn open_and_closed_hat_choke_each_other() {
    let mut s = OscSynth::new(SR, 16);
    s.set_program(Slot(0), LaneKind::Drum, 0);
    s.note_on(Slot(0), 46, 110); // open
    let _ = render_n(&mut s, 512);
    s.note_on(Slot(0), 42, 110); // closed chokes open
    let b = render_n(&mut s, 2048);
    assert!(all_finite(&b));
    // The choked open hat must die immediately: after the closed hat's short tail the bus
    // is silent, whereas an un-choked open hat would still ring.
    let settled = render_n(&mut s, SR as usize / 4);
    let m = settled.dl.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(m < 1e-3, "open hat survived the choke: {m}");
}
