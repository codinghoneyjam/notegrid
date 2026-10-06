// <META - ROLE : T1.4 contract tests: R05.AC1..AC6, R05.R04 loop counters, boundary independence. | L1-230>
// A TestSynth with a simple release tail stands in for real voices so ng-render's tests do
// not need a dependency on ng-synth (g03 exactness, R05.R05).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use ng_core::{AudioSource, Buses, ChannelMix, LaneKind, NoteSink, Slot};
use ng_render::{LoopMode, MasterBus, OfflineOptions, RenderConfig, RenderError, Renderer};
use ng_timeline::{LaneEntry, NoteEvent, Schedule, TempoEntry};

const SR: u32 = 44_100;

// A deterministic Synth for tests: one sine per active voice, with a geometric release tail
// of roughly 50 ms after note_off, matching the "tail rings out" behaviour the renderer is
// supposed to give voices (C2 FSM design).
struct TestVoice {
    active: bool,
    releasing: bool,
    amp: f32,
    phase: f32,
    slot: Slot,
    key: u8,
}

struct TestSynth {
    voices: Vec<TestVoice>,
    gains: Vec<f32>,
    serial: u64,
}

impl TestSynth {
    fn new() -> Self {
        let voices = (0..64)
            .map(|_| TestVoice {
                active: false,
                releasing: false,
                amp: 0.0,
                phase: 0.0,
                slot: Slot(0),
                key: 0,
            })
            .collect::<Vec<_>>();
        let gains = vec![1.0; 129];
        Self {
            voices,
            gains,
            serial: 0,
        }
    }
}

impl NoteSink for TestSynth {
    fn set_program(&mut self, _slot: Slot, _kind: LaneKind, _program: u8) {}

    fn note_on(&mut self, slot: Slot, key: u8, vel: u8) {
        let free = self
            .voices
            .iter()
            .position(|v| !v.active)
            .unwrap_or((self.serial as usize) % 64);
        self.serial += 1;
        if let Some(v) = self.voices.get_mut(free) {
            v.active = true;
            v.releasing = false;
            v.amp = f32::from(vel) / 127.0 * 0.5;
            v.phase = 0.0;
            v.slot = slot;
            v.key = key;
        }
    }

    fn note_off(&mut self, slot: Slot, key: u8) {
        for v in self.voices.iter_mut() {
            if v.active && v.slot == slot && v.key == key {
                v.releasing = true;
            }
        }
    }

    fn release_all(&mut self) {
        for v in self.voices.iter_mut() {
            if v.active {
                v.releasing = true;
            }
        }
    }

    fn silence_all(&mut self) {
        for v in self.voices.iter_mut() {
            v.active = false;
            v.releasing = false;
            v.amp = 0.0;
        }
    }
}

impl ChannelMix for TestSynth {
    fn set_gain(&mut self, slot: Slot, gain: f32) {
        if let Some(g) = self.gains.get_mut(slot.0 as usize) {
            *g = gain;
        }
    }

    fn set_pan(&mut self, _slot: Slot, _pan: u8) {}

    fn set_reverb_send(&mut self, _slot: Slot, _send: u8) {}
}

impl AudioSource for TestSynth {
    fn sample_rate(&self) -> u32 {
        SR
    }

    fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    fn render(&mut self, out: &mut Buses) {
        let n = out.dry_l.len().min(out.dry_r.len());
        for i in 0..n {
            let mut l = 0.0f32;
            let mut r = 0.0f32;
            for v in self.voices.iter_mut() {
                if !v.active {
                    continue;
                }
                if v.releasing {
                    v.amp *= 0.995;
                    if v.amp < 1e-4 {
                        v.active = false;
                        v.amp = 0.0;
                        continue;
                    }
                }
                let g = *self.gains.get(v.slot.0 as usize).unwrap_or(&1.0);
                let s = v.amp * libm::sinf(v.phase * core::f32::consts::TAU) * g;
                v.phase += 440.0 / SR as f32;
                if v.phase >= 1.0 {
                    v.phase -= 1.0;
                }
                l += s;
                r += s;
            }
            if let Some(dl) = out.dry_l.get_mut(i) {
                *dl = l;
            }
            if let Some(dr) = out.dry_r.get_mut(i) {
                *dr = r;
            }
            if let Some(sl) = out.send_l.get_mut(i) {
                *sl = 0.0;
            }
            if let Some(sr) = out.send_r.get_mut(i) {
                *sr = 0.0;
            }
        }
    }
}

fn synth() -> TestSynth {
    TestSynth::new()
}

fn lane(slot: u16, program: u8, kind: u8, flags: u8) -> LaneEntry {
    LaneEntry {
        slot,
        program,
        kind,
        volume: 127,
        pan: 64,
        reverb_send: 0,
        flags,
    }
}

/// Minimal single-lane project: one note, one 120 bpm segment.
fn one_note_schedule() -> Schedule {
    Schedule {
        sample_rate: SR,
        lanes: vec![lane(0, 0, 0, 0)],
        tempos: vec![TempoEntry {
            tick: 0,
            mpq: 500_000,
            sample: 0,
        }],
        notes: vec![NoteEvent {
            start: 0,
            end: 4410,
            slot: 0,
            pitch: 60,
            vel: 100,
        }],
        total_samples: 4410,
    }
}

fn render_with_block_size(schedule: Schedule, block: usize) -> (Vec<f32>, Vec<f32>) {
    let synth = synth();
    let master = MasterBus::new_default(SR).unwrap();
    let mut renderer = Renderer::new(synth, master, RenderConfig::new(SR));
    renderer.set_schedule(schedule);
    renderer.play();
    let total = 12_000usize;
    let mut out_l: Vec<f32> = Vec::with_capacity(total + block);
    let mut out_r: Vec<f32> = Vec::with_capacity(total + block);
    let mut l = vec![0.0; block];
    let mut r = vec![0.0; block];
    while out_l.len() < total {
        renderer.render_block(&mut l, &mut r);
        for x in l.iter() {
            out_l.push(*x);
        }
        for x in r.iter() {
            out_r.push(*x);
        }
    }
    out_l.truncate(total);
    out_r.truncate(total);
    (out_l, out_r)
}

// ---------------------------------------------------------------- AC1 + R05.R02

#[test]
fn ac1_block_sizes_produce_identical_bytes() {
    let a = render_with_block_size(one_note_schedule(), 1);
    let b = render_with_block_size(one_note_schedule(), 7);
    let c = render_with_block_size(one_note_schedule(), 64);
    let d = render_with_block_size(one_note_schedule(), 128);
    let e = render_with_block_size(one_note_schedule(), 4096);
    assert_eq!(a.0, b.0, "block 1 vs 7 differ");
    assert_eq!(a.0, c.0, "block 1 vs 64 differ");
    assert_eq!(a.0, d.0, "block 1 vs 128 differ");
    assert_eq!(a.0, e.0, "block 1 vs 4096 differ");
    assert_eq!(a.1, e.1, "right channels differ");
}

// ---------------------------------------------------------------- AC4

#[test]
fn ac4_queue_full_does_not_panic_or_wait() {
    let mut notes = Vec::new();
    for i in 0..700u64 {
        notes.push(NoteEvent {
            start: i * 4,
            end: 1_000_000, // all offs pile up far in the future -> the 512-cap fills
            slot: 0,
            pitch: 60,
            vel: 80,
        });
    }
    let schedule = Schedule {
        sample_rate: SR,
        lanes: vec![lane(0, 0, 0, 0)],
        tempos: vec![TempoEntry {
            tick: 0,
            mpq: 500_000,
            sample: 0,
        }],
        notes,
        total_samples: 2800,
    };
    let synth = synth();
    let master = MasterBus::new_default(SR).unwrap();
    let mut renderer = Renderer::new(synth, master, RenderConfig::new(SR));
    renderer.set_schedule(schedule);
    renderer.play();
    let mut l = vec![0.0; 128];
    let mut r = vec![0.0; 128];
    for _ in 0..32 {
        renderer.render_block(&mut l, &mut r);
    }
    // Queue capacity 512 < 700 pending offs: the excess must be forced, never panic.
    assert!(renderer.dropped_offs() > 0, "expected forced offs");
    assert_eq!(renderer.counters().note_ons, 700);
}

// ---------------------------------------------------------------- AC3

#[test]
fn ac3_set_schedule_preserves_tick_and_sounding_note() {
    let schedule = one_note_schedule();
    let synth = synth();
    let master = MasterBus::new_default(SR).unwrap();
    let mut renderer = Renderer::new(synth, master, RenderConfig::new(SR));
    renderer.set_schedule(schedule.clone());
    renderer.play();
    let mut l = vec![0.0; 2048];
    let mut r = vec![0.0; 2048];
    renderer.render_block(&mut l, &mut r);
    let (pos0, tick0) = renderer.position();
    let ons_before = renderer.counters().note_ons;

    // Swap in the same schedule plus a note far ahead.
    let mut swapped = schedule;
    swapped.notes.push(NoteEvent {
        start: 20_000,
        end: 21_000,
        slot: 0,
        pitch: 64,
        vel: 90,
    });
    swapped.total_samples = 21_000;
    renderer.set_schedule(swapped);
    let (pos1, tick1) = renderer.position();
    assert!(
        (tick1 as i64 - tick0 as i64).abs() <= 1,
        "tick moved: {tick0} -> {tick1}"
    );
    assert!(pos1.abs_diff(pos0) <= 64, "sample jump: {pos0} -> {pos1}");
    assert_eq!(
        renderer.counters().note_ons,
        ons_before,
        "a sounding note refired"
    );
    // The note added far ahead must eventually trigger.
    let mut got_second = false;
    for _ in 0..300 {
        renderer.render_block(&mut l, &mut r);
        if renderer.counters().note_ons > ons_before {
            got_second = true;
            break;
        }
    }
    assert!(got_second, "the note added ahead never triggered");
}

// ---------------------------------------------------------------- AC5

#[test]
fn ac5_mute_toggle_is_click_free() {
    let schedule = one_note_schedule();
    let synth = synth();
    let master = MasterBus::new_default(SR).unwrap();
    let mut renderer = Renderer::new(synth, master, RenderConfig::new(SR));
    renderer.set_schedule(schedule);
    renderer.play();
    let mut l = vec![0.0; 4096];
    let mut r = vec![0.0; 4096];
    let mut stream: Vec<f32> = Vec::new();
    renderer.render_block(&mut l, &mut r);
    stream.extend_from_slice(&l);
    // Toggle mute on.
    let mut m = ng_render::LaneMix::new(Slot(0));
    m.mute = true;
    renderer.set_mix(&[m]);
    renderer.render_block(&mut l, &mut r);
    stream.extend_from_slice(&l);
    let mut max_step = 0.0f32;
    for pair in stream.windows(2) {
        let step = (pair[1] - pair[0]).abs();
        if step > max_step {
            max_step = step;
        }
    }
    assert!(
        max_step < 0.05,
        "discontinuity at the mute edge: {max_step}"
    );
}

// ---------------------------------------------------------------- AC2 + R05.R04

#[test]
fn ac2_looped_note_release_folds_into_the_file() {
    // One note occupying the last 2000 samples; its release tail continues past the end.
    let schedule = Schedule {
        sample_rate: SR,
        lanes: vec![lane(0, 0, 0, 0)],
        tempos: vec![TempoEntry {
            tick: 0,
            mpq: 500_000,
            sample: 0,
        }],
        notes: vec![NoteEvent {
            start: 2410,
            end: 4410,
            slot: 0,
            pitch: 60,
            vel: 100,
        }],
        total_samples: 4410,
    };
    let plain = ng_render::render_offline(
        schedule.clone(),
        synth(),
        &OfflineOptions::default(),
        |_| true,
    )
    .unwrap();
    let folded = ng_render::render_offline(
        schedule,
        synth(),
        &OfflineOptions {
            loop_mode: LoopMode::TailFold,
            ..Default::default()
        },
        |_| true,
    )
    .unwrap();
    assert_eq!(
        folded.frames, 4410,
        "folded file length must equal total_samples (E)"
    );
    // The folded tail places energy in the first samples; the plain render is silent there.
    let mut folded_energy = 0.0f32;
    for i in 0..1000 {
        folded_energy += folded.data[2 * i].abs();
    }
    let mut plain_energy = 0.0f32;
    for i in 0..1000 {
        plain_energy += plain.data[2 * i].abs();
    }
    assert!(
        folded_energy > plain_energy * 10.0 + 1e-3,
        "fold should deposit release energy at the start: folded {folded_energy} vs plain {plain_energy}"
    );
}

#[test]
fn r04_loop_counters_match_exactly() {
    let loop_len = 2048u64;
    let schedule = Schedule {
        sample_rate: SR,
        lanes: vec![lane(0, 0, 0, 0)],
        tempos: vec![TempoEntry {
            tick: 0,
            mpq: 500_000,
            sample: 0,
        }],
        notes: vec![
            NoteEvent {
                start: 256,
                end: 1024,
                slot: 0,
                pitch: 60,
                vel: 100,
            },
            NoteEvent {
                start: 1024,
                end: 1536,
                slot: 0,
                pitch: 64,
                vel: 90,
            },
        ],
        total_samples: loop_len,
    };
    let synth = synth();
    let master = MasterBus::new_default(SR).unwrap();
    let mut renderer = Renderer::new(synth, master, RenderConfig::new(SR));
    renderer.set_schedule(schedule);
    renderer.set_loop(Some((0, loop_len)));
    renderer.play();
    let mut l = vec![0.0; 512];
    let mut r = vec![0.0; 512];
    let passes = 100usize;
    for _ in 0..(loop_len as usize * passes / 512) {
        renderer.render_block(&mut l, &mut r);
    }
    let counters = renderer.counters();
    assert_eq!(
        counters.note_ons,
        (2 * passes) as u64,
        "ons over 100 passes"
    );
    assert_eq!(
        counters.note_offs,
        (2 * passes) as u64,
        "offs over 100 passes"
    );
}

// ---------------------------------------------------------------- AC6

#[test]
fn ac6_offline_cancel_stops_at_the_progress_callback() {
    let result = ng_render::render_offline(
        one_note_schedule(),
        synth(),
        &OfflineOptions {
            progress_interval: 128,
            ..Default::default()
        },
        |_| false,
    );
    assert!(matches!(result, Err(RenderError::E_RENDER_CANCELED)));
}
