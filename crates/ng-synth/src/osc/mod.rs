// <META - FILE SUMMARY - OscSynth: fixed-pool oscillator synth implementing the Synth contract (R04). | L1-330>
// HOTPATH: no allocation after construction, no indexing, no panic, no clock, seeded noise only (R-RT-1, R-RT-4).
//
// GM's 16 families collapse to (waveform, envelope, detune) triples. That is enough for the
// test baseline R04 asks for — an instant sound every program can render — and no more: this
// is the contract's reference synth, not a fidelity synth.

use ng_core::{AudioSource, Buses, ChannelMix, LaneKind, NoteSink, Slot, Synth, MAX_SLOTS};
use ng_dsp::envelope::Envelope;
use ng_dsp::envelope::Stage;
use ng_dsp::filter::BiquadFilter;
use ng_dsp::noise::Lcg;

use libm::{cosf, expf, powf, sinf};

/// Fixed voice pool default from R04.DESIGN.
pub const DEFAULT_VOICES: usize = 64;

// ---------------------------------------------------------------- waveforms / families

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wave {
    Sine,
    Triangle,
    Square,
    Saw,
    Noise,
}

/// (waveform, attack, decay, release, sustain, detune cents) per GM family.
/// Program p belongs to family p / 8; family order is the GM table order.
const FAMILY: [(Wave, f32, f32, f32, f32, f32); 16] = [
    (Wave::Triangle, 0.005, 0.10, 0.20, 0.6, 0.0), // 0 Piano
    (Wave::Sine, 0.002, 0.15, 0.30, 0.0, 0.0),     // 1 Chromatic percussion
    (Wave::Square, 0.003, 0.05, 0.10, 0.8, 0.0),   // 2 Organ
    (Wave::Saw, 0.005, 0.10, 0.15, 0.5, 3.0),      // 3 Guitar
    (Wave::Sine, 0.002, 0.10, 0.10, 0.9, 0.0),     // 4 Bass
    (Wave::Saw, 0.080, 0.10, 0.50, 0.7, 6.0),      // 5 Strings
    (Wave::Saw, 0.020, 0.10, 0.30, 0.7, 6.0),      // 6 Ensemble
    (Wave::Square, 0.010, 0.08, 0.15, 0.7, 0.0),   // 7 Brass
    (Wave::Square, 0.008, 0.08, 0.15, 0.7, 4.0),   // 8 Reed
    (Wave::Sine, 0.010, 0.10, 0.20, 0.6, 0.0),     // 9 Pipe
    (Wave::Saw, 0.002, 0.05, 0.15, 0.6, 0.0),      // 10 SynthLead
    (Wave::Triangle, 0.050, 0.20, 0.80, 0.7, 8.0), // 11 SynthPad
    (Wave::Sine, 0.300, 0.30, 1.00, 0.5, 0.0),     // 12 SynthEffects
    (Wave::Triangle, 0.005, 0.05, 0.10, 0.5, 0.0), // 13 Ethnic
    (Wave::Noise, 0.001, 0.02, 0.10, 0.0, 0.0),    // 14 Percussive
    (Wave::Noise, 0.500, 0.50, 1.00, 0.5, 0.0),    // 15 SFX
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DrumKind {
    Kick,
    Snare,
    Clap,
    HatClosed,
    HatOpen,
    Tom,
    Cymbal,
}

fn drum_kind(key: u8) -> DrumKind {
    match key {
        35 | 36 => DrumKind::Kick,
        38 | 40 => DrumKind::Snare,
        39 | 37 => DrumKind::Clap,
        42 | 44 => DrumKind::HatClosed,
        46 => DrumKind::HatOpen,
        49 | 51 | 52 | 53 | 55 | 57 | 59 => DrumKind::Cymbal,
        _ => DrumKind::Tom,
    }
}

/// Hat open/closed choke each other (R04.DESIGN).
fn rival_hat(kind: DrumKind) -> DrumKind {
    match kind {
        DrumKind::HatClosed => DrumKind::HatOpen,
        DrumKind::HatOpen => DrumKind::HatClosed,
        other => other,
    }
}

// ---------------------------------------------------------------- voice and lane

#[derive(Clone, Copy, Debug)]
struct Lane {
    slot: Slot,
    kind: LaneKind,
    program: u8,
    /// Linear 0..=1 after mute/solo resolution (C2 ChannelMix).
    gain: f32,
    pan: u8,
    reverb_send: u8,
}

impl Lane {
    fn new(slot: Slot) -> Self {
        Self {
            slot,
            kind: LaneKind::Melodic,
            program: 0,
            gain: 1.0,
            pan: 64,
            reverb_send: 0,
        }
    }
}

struct Voice {
    active: bool,
    slot: Slot,
    key: u8,
    /// Monotonic trigger stamp for deterministic stealing.
    note_clock: u64,
    vel_gain: f32,
    phase: f32,
    detune_phase: f32,
    base_freq: f32,
    detune_freq: f32,
    wave: Wave,
    envelope: Envelope,
    is_drum: bool,
    drum: DrumKind,
    drum_t: f32,
    lcg: Lcg,
    drum_filter: Option<BiquadFilter>,
    inv_sr: f32,
}

impl Voice {
    fn idle(sample_rate: u32) -> Self {
        Self {
            active: false,
            slot: Slot(0),
            key: 0,
            note_clock: 0,
            vel_gain: 0.0,
            phase: 0.0,
            detune_phase: 0.0,
            base_freq: 0.0,
            detune_freq: 0.0,
            wave: Wave::Sine,
            envelope: Envelope::new(sample_rate, (0.001, 0.001, 0.001), 0.0),
            is_drum: false,
            drum: DrumKind::Kick,
            drum_t: 0.0,
            lcg: Lcg::new(0),
            drum_filter: None,
            inv_sr: 1.0 / sample_rate as f32,
        }
    }

    /// A voice is free when it never started, its envelope finished, or its percussive
    /// envelope decayed to the sustain-zero plateau.
    fn is_free(&self) -> bool {
        !self.active || self.envelope.is_idle() || self.envelope.level() == 0.0
    }

    fn lcg_next(&mut self) -> f32 {
        self.lcg.next_bipolar()
    }

    fn next_sample(&mut self) -> f32 {
        if self.is_drum {
            return self.drum_next();
        }
        let amp = self.envelope.tick();
        if amp == 0.0 {
            return 0.0;
        }
        let noise = if self.wave == Wave::Noise {
            self.lcg_next()
        } else {
            0.0
        };
        let mut s = shape(self.wave, self.phase, noise);
        self.phase = advance_phase(self.phase, self.base_freq * self.inv_sr);
        if self.detune_freq != self.base_freq {
            let dnoise = if self.wave == Wave::Noise {
                self.lcg_next()
            } else {
                0.0
            };
            let d = shape(self.wave, self.detune_phase, dnoise);
            s = 0.5 * (s + d);
            self.detune_phase = advance_phase(self.detune_phase, self.detune_freq * self.inv_sr);
        }
        s * amp * self.vel_gain
    }

    fn drum_next(&mut self) -> f32 {
        self.drum_t += self.inv_sr;
        let env = self.envelope.tick();
        if env == 0.0 {
            return 0.0;
        }
        let raw = match self.drum {
            DrumKind::Kick => {
                // Fast downward pitch sweep: the thump.
                let f = 45.0 + 110.0 * expf(-self.drum_t / 0.018);
                self.phase = advance_phase(self.phase, f * self.inv_sr);
                sinf(self.phase * core::f32::consts::TAU)
            }
            DrumKind::Tom => {
                let f = self.base_freq * (1.0 + 0.8 * expf(-self.drum_t / 0.025));
                self.phase = advance_phase(self.phase, f * self.inv_sr);
                sinf(self.phase * core::f32::consts::TAU)
            }
            DrumKind::Snare => {
                let n = self.lcg_next();
                let filtered = match self.drum_filter.as_mut() {
                    Some(f) => f.process(n),
                    None => n,
                };
                self.phase = advance_phase(self.phase, self.base_freq * self.inv_sr);
                0.7 * filtered + 0.3 * sinf(self.phase * core::f32::consts::TAU)
            }
            DrumKind::Clap | DrumKind::HatClosed | DrumKind::HatOpen | DrumKind::Cymbal => {
                let n = self.lcg_next();
                match self.drum_filter.as_mut() {
                    Some(f) => f.process(n),
                    None => n,
                }
            }
        };
        raw * env * self.vel_gain
    }
}

fn shape(wave: Wave, phase: f32, noise: f32) -> f32 {
    match wave {
        Wave::Sine => sinf(phase * core::f32::consts::TAU),
        Wave::Triangle => {
            let p = phase - phase.floor();
            1.0 - 4.0 * (p - 0.5).abs()
        }
        Wave::Square => {
            let p = phase - phase.floor();
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Wave::Saw => {
            let p = phase - phase.floor();
            2.0 * p - 1.0
        }
        Wave::Noise => noise,
    }
}

fn advance_phase(phase: f32, step: f32) -> f32 {
    let mut p = phase + step;
    if p >= 1.0 {
        p -= 1.0;
    }
    p
}

fn drum_freq(kind: DrumKind, key: u8) -> f32 {
    match kind {
        DrumKind::Kick => 50.0,
        DrumKind::Tom => key_to_freq(key.saturating_add(12)),
        DrumKind::Snare => 180.0,
        _ => 0.0,
    }
}

fn drum_shape(kind: DrumKind) -> (Wave, f32, f32, f32, f32) {
    match kind {
        DrumKind::Kick => (Wave::Sine, 0.001, 0.120, 0.080, 0.0),
        DrumKind::Tom => (Wave::Sine, 0.001, 0.150, 0.100, 0.0),
        DrumKind::Snare => (Wave::Noise, 0.001, 0.100, 0.050, 0.0),
        DrumKind::Clap => (Wave::Noise, 0.001, 0.080, 0.050, 0.0),
        DrumKind::HatClosed => (Wave::Noise, 0.001, 0.030, 0.020, 0.0),
        DrumKind::HatOpen => (Wave::Noise, 0.001, 0.300, 0.150, 0.0),
        DrumKind::Cymbal => (Wave::Noise, 0.002, 0.600, 0.800, 0.0),
    }
}

fn drum_filter(kind: DrumKind, sample_rate: u32) -> Option<BiquadFilter> {
    match kind {
        DrumKind::Snare => BiquadFilter::bandpass(1_800.0, 1.0, sample_rate).ok(),
        DrumKind::Clap => BiquadFilter::bandpass(1_200.0, 1.2, sample_rate).ok(),
        DrumKind::HatClosed | DrumKind::HatOpen => {
            BiquadFilter::highpass(7_000.0, 0.707, sample_rate).ok()
        }
        DrumKind::Cymbal => BiquadFilter::highpass(6_000.0, 0.707, sample_rate).ok(),
        DrumKind::Kick | DrumKind::Tom => None,
    }
}

fn family_spec(family: usize) -> &'static (Wave, f32, f32, f32, f32, f32) {
    match FAMILY.get(family) {
        Some(f) => f,
        // program/8 is always 0..=15 because program is u8 and validated upstream, but stay total.
        None => &FAMILY[0],
    }
}

fn key_to_freq(key: u8) -> f32 {
    440.0 * powf(2.0, (f32::from(key) - 69.0) / 12.0)
}

fn pan_gains(pan: u8) -> (f32, f32) {
    // Constant-power pan with exact anchors: centre 64 gives L == R bit-for-bit, 0 and 127
    // give hard left/right with the opposite side exactly 0 (R04.R01).
    if pan == 64 {
        return (
            core::f32::consts::FRAC_1_SQRT_2,
            core::f32::consts::FRAC_1_SQRT_2,
        );
    }
    if pan == 0 {
        return (1.0, 0.0);
    }
    if pan >= 127 {
        return (0.0, 1.0);
    }
    let t = (f32::from(pan) - 64.0) / 64.0;
    let theta = t * core::f32::consts::FRAC_PI_4;
    (
        cosf(core::f32::consts::FRAC_PI_4 - theta),
        sinf(core::f32::consts::FRAC_PI_4 + theta),
    )
}

// ---------------------------------------------------------------- OscSynth

/// OscSynth: a melodic/drum polyphonic oscillator synth.
///
/// Lane state is fixed at construction: `MAX_SLOTS` lanes, each with a program that applies
/// from the next `note_on` (same rule SoundFontSynth will use, R04). No allocation in
/// `note_on`/`note_off`/`render` after construction (R-RT-1, checked by g15).
pub struct OscSynth {
    sample_rate: u32,
    voices: Vec<Voice>,
    lanes: Vec<Lane>,
    clock: u64,
}

impl OscSynth {
    pub fn new(sample_rate: u32, voice_pool: usize) -> Self {
        let n_voices = if voice_pool == 0 {
            DEFAULT_VOICES
        } else {
            voice_pool
        };
        let mut voices = Vec::with_capacity(n_voices);
        for _ in 0..n_voices {
            voices.push(Voice::idle(sample_rate));
        }
        let mut lanes = Vec::with_capacity(MAX_SLOTS);
        for i in 0..MAX_SLOTS {
            lanes.push(Lane::new(Slot(i as u16)));
        }
        Self {
            sample_rate,
            voices,
            lanes,
            clock: 0,
        }
    }

    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }

    fn lane_mut(&mut self, slot: Slot) -> Option<&mut Lane> {
        self.lanes.iter_mut().find(|l| l.slot == slot)
    }

    /// Oldest voice in release first, then quietest, ties by slot number (R04.DESIGN).
    fn steal_index(&self) -> Option<usize> {
        let mut best_release: Option<(u64, Slot, usize)> = None;
        let mut best_quiet: Option<(f32, Slot, usize)> = None;
        for (i, v) in self.voices.iter().enumerate() {
            if !v.active {
                continue;
            }
            let level = v.envelope.level();
            match best_quiet {
                None => best_quiet = Some((level, v.slot, i)),
                Some((bl, bs, _)) => {
                    if level < bl || (level == bl && v.slot.0 < bs.0) {
                        best_quiet = Some((level, v.slot, i));
                    }
                }
            }
            if v.envelope.stage() == Stage::Release {
                match best_release {
                    None => best_release = Some((v.note_clock, v.slot, i)),
                    Some((bc, bs, _)) => {
                        if v.note_clock < bc || (v.note_clock == bc && v.slot.0 < bs.0) {
                            best_release = Some((v.note_clock, v.slot, i));
                        }
                    }
                }
            }
        }
        match best_release {
            Some((_, _, i)) => Some(i),
            None => best_quiet.map(|(_, _, i)| i),
        }
    }

    fn trigger(&mut self, slot: Slot, key: u8, vel: u8) {
        let Some(lane) = self.lane_mut(slot) else {
            return;
        };
        let kind = lane.kind;
        let program = lane.program;
        let is_drum = kind == LaneKind::Drum;
        let drum = if is_drum {
            drum_kind(key)
        } else {
            DrumKind::Kick
        };

        // Hat open/closed are exclusive in the same slot (R04.DESIGN).
        if is_drum && (drum == DrumKind::HatClosed || drum == DrumKind::HatOpen) {
            let rival = rival_hat(drum);
            for v in self.voices.iter_mut() {
                if v.active && v.is_drum && v.slot == slot && v.drum == rival {
                    v.envelope.silence();
                    v.active = false;
                }
            }
        }

        let free = self.voices.iter().position(|v| v.is_free());
        let idx = free.or_else(|| self.steal_index());
        let Some(i) = idx else { return };
        let Some(v) = self.voices.get_mut(i) else {
            return;
        };

        self.clock = self.clock.saturating_add(1);
        v.active = true;
        v.slot = slot;
        v.key = key;
        v.note_clock = self.clock;
        v.vel_gain = f32::from(vel) / 127.0;
        v.is_drum = is_drum;
        v.drum = drum;
        v.phase = 0.0;
        v.detune_phase = 0.0;
        v.drum_t = 0.0;
        v.drum_filter = None;
        v.lcg = Lcg::new(
            (u64::from(slot.0) << 48)
                ^ (u64::from(key) << 32)
                ^ self.clock.wrapping_mul(0x9E37_79B9),
        );

        if is_drum {
            let (wave, attack, decay, release, sustain) = drum_shape(drum);
            v.wave = wave;
            v.base_freq = drum_freq(drum, key);
            v.detune_freq = v.base_freq;
            v.envelope = Envelope::new(self.sample_rate, (attack, decay, release), sustain);
            v.drum_filter = drum_filter(drum, self.sample_rate);
        } else {
            let family = (program / 8) as usize;
            let &(wave, attack, decay, release, sustain, detune_cents) = family_spec(family);
            v.wave = wave;
            v.base_freq = key_to_freq(key);
            v.detune_freq = v.base_freq * powf(2.0, detune_cents / 1200.0);
            v.envelope = Envelope::new(self.sample_rate, (attack, decay, release), sustain);
        }
        v.envelope.gate_on();
    }
}

impl NoteSink for OscSynth {
    fn set_program(&mut self, slot: Slot, kind: LaneKind, program: u8) {
        if let Some(l) = self.lane_mut(slot) {
            l.kind = kind;
            l.program = program;
        }
    }

    fn note_on(&mut self, slot: Slot, key: u8, vel: u8) {
        self.trigger(slot, key, vel);
    }

    fn note_off(&mut self, slot: Slot, key: u8) {
        for v in self.voices.iter_mut() {
            if v.active && v.slot == slot && v.key == key {
                v.envelope.gate_off();
            }
        }
    }

    fn release_all(&mut self) {
        for v in self.voices.iter_mut() {
            if v.active {
                v.envelope.gate_off();
            }
        }
    }

    fn silence_all(&mut self) {
        for v in self.voices.iter_mut() {
            v.envelope.silence();
            v.active = false;
        }
    }
}

impl ChannelMix for OscSynth {
    fn set_gain(&mut self, slot: Slot, gain: f32) {
        if let Some(l) = self.lane_mut(slot) {
            l.gain = if gain.is_nan() {
                0.0
            } else {
                gain.clamp(0.0, 1.0)
            };
        }
    }

    fn set_pan(&mut self, slot: Slot, pan: u8) {
        if let Some(l) = self.lane_mut(slot) {
            l.pan = pan;
        }
    }

    fn set_reverb_send(&mut self, slot: Slot, send: u8) {
        if let Some(l) = self.lane_mut(slot) {
            l.reverb_send = send;
        }
    }
}

impl AudioSource for OscSynth {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn active_voices(&self) -> usize {
        self.voices
            .iter()
            .filter(|v| v.active && !v.envelope.is_idle())
            .count()
    }

    fn render(&mut self, out: &mut Buses) {
        // Overwrite, per C2 "equal-length bus slices passed as (overwrites)".
        let mut dl = out.dry_l.iter_mut();
        let mut dr = out.dry_r.iter_mut();
        let mut sl = out.send_l.iter_mut();
        let mut sr = out.send_r.iter_mut();
        while let (Some(a), Some(b), Some(c), Some(d)) =
            (dl.next(), dr.next(), sl.next(), sr.next())
        {
            let mut dry_l = 0.0f32;
            let mut dry_r = 0.0f32;
            let mut send_l = 0.0f32;
            let mut send_r = 0.0f32;
            for v in self.voices.iter_mut() {
                if !v.active || v.envelope.is_idle() {
                    continue;
                }
                let (gain, pan, send) = match self.lanes.get(v.slot.0 as usize) {
                    Some(l) => (l.gain, l.pan, l.reverb_send),
                    None => continue,
                };
                let s = v.next_sample();
                let (pl, pr) = pan_gains(pan);
                dry_l += s * gain * pl;
                dry_r += s * gain * pr;
                let send_gain = s * (f32::from(send) / 127.0);
                send_l += send_gain * pl;
                send_r += send_gain * pr;
            }
            *a = dry_l;
            *b = dry_r;
            *c = send_l;
            *d = send_r;
        }
    }
}

// As a trait-object-free sanity check that OscSynth satisfies Synth (the blanket impl in
// ng-core covers it), reference it at compile time.
fn _assert_synth<S: Synth>() {}
#[allow(dead_code)]
fn assert_osc_is_synth() {
    _assert_synth::<OscSynth>();
}
