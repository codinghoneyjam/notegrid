// <META - FILE SUMMARY - Renderer<S: Synth>: transport FSM, sample-accurate event queue (C2.1..2.6), loop wrap, mix ramps. | L1-380>
// HOTPATH: zero heap allocation in render_block after warm-up (R-RT-1, R05.R01), no panic, no indexing.

use ng_core::{Buses, LaneKind, Slot, Tick, AUDITION_SLOT, MAX_SLOTS};
use ng_dsp::ramp::Ramp;
use ng_timeline::{LaneEntry, Schedule, TempoMap};

use crate::master::MasterBus;
use crate::mix::{effective_gain, LaneMix};

/// Fixed note-off queue capacity (R05.DESIGN).
pub const OFF_QUEUE_CAP: usize = 512;
/// Ramp length for mix gain changes comes from Ramp::new_default (5 ms).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportState {
    Stopped,
    Playing,
    Paused,
    Ended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderConfig {
    pub sample_rate: u32,
}

impl RenderConfig {
    pub fn new(sample_rate: u32) -> Self {
        Self { sample_rate }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockStatus {
    pub position_sample: u64,
    pub position_tick: Tick,
    pub playing: bool,
    pub ended: bool,
}

#[derive(Clone, Copy, Debug)]
struct OffEvent {
    /// Output-clock position (monotonic, not transport position).
    at: u64,
    slot: Slot,
    key: u8,
}

/// Fixed-capacity min queue keyed by output clock (R05.DESIGN). Full -> force the earliest
/// off early, and count it; "dropped_offs" never panics and never waits (AC4).
#[derive(Clone, Debug, Default)]
struct OffQueue {
    events: Vec<OffEvent>,
    dropped_offs: u64,
}

impl OffQueue {
    fn new(capacity: usize) -> Self {
        Self {
            events: Vec::with_capacity(capacity),
            dropped_offs: 0,
        }
    }

    /// Returns the forced-early off when the queue was full (R05.DESIGN).
    fn push(&mut self, ev: OffEvent) -> Option<OffEvent> {
        let pos = self
            .events
            .iter()
            .position(|e| e.at >= ev.at)
            .unwrap_or(self.events.len());
        if self.events.len() >= self.events.capacity() {
            let forced = self.events.remove(0);
            self.dropped_offs = self.dropped_offs.saturating_add(1);
            self.events.insert(pos.min(self.events.len()), ev);
            return Some(forced);
        }
        self.events.insert(pos, ev);
        None
    }

    fn front_at(&self) -> Option<u64> {
        self.events.first().map(|e| e.at)
    }

    fn clear(&mut self) {
        self.events.clear();
    }
}

/// Per-slot live mix state with a 5 ms ramp on gain changes (R05.DESIGN).
#[derive(Clone, Debug)]
struct SlotMixState {
    mix: LaneMix,
    applied: f32,
    ramp: Ramp,
}

impl SlotMixState {
    fn new(slot: Slot) -> Self {
        Self {
            mix: LaneMix::new(slot),
            applied: 1.0,
            ramp: unity_ramp(),
        }
    }

    fn is_ramping(&self) -> bool {
        !self.ramp.is_done()
    }
}

/// A one-sample unity ramp; the length is non-zero by construction, so this never errs.
/// (No unwrap in src: re-try with the next length rather than panicking.)
fn unity_ramp() -> Ramp {
    let mut len = 1u32;
    loop {
        if let Ok(r) = Ramp::new(1.0, 1.0, len) {
            return r;
        }
        len = len.saturating_add(1);
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EventCounters {
    pub note_ons: u64,
    pub note_offs: u64,
}

/// Renderer<S: Synth> — the single realtime/offline implementation (R05).
///
/// Transport position `pos_sample` lives in the schedule domain and wraps at loop end; the
/// note-off queue lives in the monotonic output-clock domain, so a loop wrap never alters
/// the end of an already-sounding note (C2.1, R05.DESIGN).
pub struct Renderer<S: ng_core::Synth> {
    synth: S,
    master: MasterBus,
    cfg: RenderConfig,
    state: TransportState,
    schedule: Option<Schedule>,
    tempo: Option<TempoMap>,
    pos_sample: u64,
    out_clock: u64,
    queue: OffQueue,
    next_note: usize,
    loop_range: Option<(u64, u64)>,
    slots: Vec<SlotMixState>,
    any_solo: bool,
    counters: EventCounters,
    /// Send bus buffers, allocated once and reused (R-RT-1).
    send_l: Vec<f32>,
    send_r: Vec<f32>,
}

impl<S: ng_core::Synth> Renderer<S> {
    pub fn new(synth: S, master: MasterBus, cfg: RenderConfig) -> Self {
        let mut slots = Vec::with_capacity(MAX_SLOTS);
        for i in 0..MAX_SLOTS {
            slots.push(SlotMixState::new(Slot(i as u16)));
        }
        Self {
            synth,
            master,
            cfg,
            state: TransportState::Stopped,
            schedule: None,
            tempo: None,
            pos_sample: 0,
            out_clock: 0,
            queue: OffQueue::new(OFF_QUEUE_CAP),
            next_note: 0,
            loop_range: None,
            any_solo: false,
            slots,
            counters: EventCounters::default(),
            send_l: Vec::with_capacity(8192),
            send_r: Vec::with_capacity(8192),
        }
    }

    pub fn state(&self) -> TransportState {
        self.state
    }

    pub fn counters(&self) -> EventCounters {
        self.counters
    }

    pub fn dropped_offs(&self) -> u64 {
        self.queue.dropped_offs
    }

    pub fn position(&self) -> (u64, Tick) {
        let tick = self
            .tempo
            .as_ref()
            .map(|t| t.sample_to_tick(self.pos_sample))
            .unwrap_or(0);
        (self.pos_sample, tick)
    }

    pub fn set_schedule(&mut self, schedule: Schedule) {
        // C2.6: preserve the current TICK position through the swap.
        let pairs: Vec<(u32, u32)> = schedule.tempos.iter().map(|t| (t.tick, t.mpq)).collect();
        let new_tempo = TempoMap::from_mpq_segments(schedule.sample_rate, &pairs).ok();
        match (self.tempo.as_ref(), new_tempo) {
            (Some(old), Some(new)) => {
                let tick = old.sample_to_tick(self.pos_sample);
                self.pos_sample = new.tick_to_sample(tick);
                self.tempo = Some(new);
            }
            (_, Some(new)) => {
                self.tempo = Some(new);
            }
            _ => {}
        }
        self.next_note = self.lower_bound_note(self.pos_sample);
        self.any_solo = schedule.lanes.iter().any(|l| l.is_solo());
        for lane in &schedule.lanes {
            if let Some(idx) = lane_index(lane.slot) {
                if let Some(sm) = self.slots.get_mut(idx) {
                    sm.mix = lane_mix(lane);
                    self.apply_target(idx);
                }
            }
            self.synth
                .set_program(Slot(lane.slot), kind_of(lane), lane.program);
        }
        self.schedule = Some(schedule);
    }

    pub fn set_mix(&mut self, lanes: &[LaneMix]) {
        for lane in lanes {
            if let Some(idx) = lane_index(lane.slot.0) {
                if let Some(sm) = self.slots.get_mut(idx) {
                    sm.mix = *lane;
                    self.apply_target(idx);
                }
            }
        }
        self.any_solo = lanes.iter().any(|l| l.solo);
        for lane in lanes {
            self.synth.set_pan(lane.slot, lane.pan);
            self.synth.set_reverb_send(lane.slot, lane.reverb_send);
        }
    }

    /// Retarget one slot's ramp. R05.DESIGN: per-slot Ramp 5 ms on effective gain.
    fn apply_target(&mut self, idx: usize) {
        let any_solo = self.any_solo;
        let Some(sm) = self.slots.get(idx) else {
            return;
        };
        let target = effective_gain(&sm.mix, any_solo);
        if (target - sm.ramp.target()).abs() < f32::EPSILON {
            return;
        }
        let from = sm.applied;
        if let Ok(ramp) = Ramp::new_default(from, target, self.cfg.sample_rate) {
            if let Some(sm) = self.slots.get_mut(idx) {
                sm.ramp = ramp;
            }
        }
    }

    pub fn schedule(&self) -> Option<&Schedule> {
        self.schedule.as_ref()
    }

    pub fn active_voices(&self) -> usize {
        self.synth.active_voices()
    }

    pub fn set_loop(&mut self, range: Option<(u64, u64)>) {
        self.loop_range = range.filter(|&(a, b)| a < b);
    }

    pub fn play(&mut self) {
        match self.state {
            TransportState::Ended => {
                self.queue.clear();
                self.pos_sample = 0;
                self.next_note = self.lower_bound_note(0);
                self.state = TransportState::Playing;
            }
            TransportState::Paused | TransportState::Stopped => {
                self.state = TransportState::Playing;
            }
            TransportState::Playing => {}
        }
    }

    pub fn pause(&mut self) {
        if self.state == TransportState::Playing {
            self.synth.release_all();
            self.queue.clear();
            self.state = TransportState::Paused;
        }
    }

    pub fn stop(&mut self) {
        self.synth.release_all();
        self.queue.clear();
        self.pos_sample = 0;
        self.next_note = self.lower_bound_note(0);
        self.state = TransportState::Stopped;
    }

    pub fn seek(&mut self, sample: u64) {
        // L-01: notes straddling the seek point are not revived — the queue is cleared and
        // every voice is silenced before the transport moves.
        self.synth.silence_all();
        self.queue.clear();
        self.master.flush_reverb();
        self.pos_sample = sample;
        self.next_note = self.lower_bound_note(sample);
        if self.state == TransportState::Ended {
            self.state = TransportState::Paused;
        }
    }

    pub fn audition_on(&mut self, kind: LaneKind, program: u8, key: u8, vel: u8) {
        self.synth.set_program(AUDITION_SLOT, kind, program);
        self.synth.note_on(AUDITION_SLOT, key, vel);
        self.counters.note_ons = self.counters.note_ons.saturating_add(1);
    }

    pub fn audition_off(&mut self, key: u8) {
        self.synth.note_off(AUDITION_SLOT, key);
        self.counters.note_offs = self.counters.note_offs.saturating_add(1);
    }

    fn lower_bound_note(&self, sample: u64) -> usize {
        match &self.schedule {
            Some(s) => s
                .notes
                .iter()
                .position(|n| n.start >= sample)
                .unwrap_or(s.notes.len()),
            None => 0,
        }
    }

    /// Fire due note-offs at exactly this output-clock time.
    fn fire_offs(&mut self, out: u64) {
        loop {
            let due = match self.queue.events.first() {
                Some(e) if e.at == out => *e,
                _ => break,
            };
            self.queue.events.remove(0);
            self.synth.note_off(due.slot, due.key);
            self.counters.note_offs = self.counters.note_offs.saturating_add(1);
        }
    }

    /// Fire note-ons at the current transport position. Offs are clock-based; ons are not.
    fn fire_note_ons(&mut self) {
        let Some(sched) = self.schedule.as_ref() else {
            return;
        };
        while let Some(note) = sched.notes.get(self.next_note) {
            if note.start > self.pos_sample {
                break;
            }
            let slot = Slot(note.slot);
            self.synth.note_on(slot, note.pitch, note.vel);
            self.counters.note_ons = self.counters.note_ons.saturating_add(1);
            let dur = note.end.saturating_sub(note.start);
            let off_at = self.out_clock.saturating_add(dur);
            if let Some(forced) = self.queue.push(OffEvent {
                at: off_at,
                slot,
                key: note.pitch,
            }) {
                // Queue was full: force the earliest off now (R05.DESIGN), count it.
                self.synth.note_off(forced.slot, forced.key);
                self.counters.note_offs = self.counters.note_offs.saturating_add(1);
            }
            self.next_note += 1;
        }
    }

    fn any_ramping(&self) -> bool {
        self.slots.iter().any(|s| s.is_ramping())
    }

    /// Tick every active slot's gain ramp once per sample, pushing the new value into the
    /// synth so adjacent samples are never separated by a gain step (AC5 click-free).
    fn tick_gain_ramps(&mut self) {
        for idx in 0..self.slots.len() {
            let Some(sm) = self.slots.get_mut(idx) else {
                continue;
            };
            if sm.is_ramping() {
                let v = sm.ramp.tick();
                sm.applied = v;
                let slot = sm.mix.slot;
                self.synth.set_gain(slot, v);
            }
        }
    }

    pub fn render_block(&mut self, l: &mut [f32], r: &mut [f32]) -> BlockStatus {
        let n = l.len().min(r.len());
        let mut i = 0usize;
        while i < n {
            // Events at exactly this sample fire before any audio produced at it.
            self.fire_offs(self.out_clock);
            if self.state == TransportState::Playing {
                self.fire_note_ons();
            }

            let mut sub = n - i;
            if self.any_ramping() {
                // Gain changes must be sample-accurate: one sample per step.
                sub = sub.min(1);
            } else {
                if let Some(front) = self.queue.front_at() {
                    if front > self.out_clock {
                        sub = sub.min((front - self.out_clock) as usize);
                    } else {
                        sub = 0;
                    }
                }
                if self.state == TransportState::Playing {
                    if let Some(note_start) = self
                        .schedule
                        .as_ref()
                        .and_then(|s| s.notes.get(self.next_note))
                        .map(|note| note.start)
                    {
                        if note_start > self.pos_sample {
                            sub = sub.min((note_start - self.pos_sample) as usize);
                        } else {
                            sub = 0;
                        }
                    }
                    match self.loop_range {
                        Some((_, b)) => {
                            if b > self.pos_sample {
                                sub = sub.min((b - self.pos_sample) as usize);
                            } else {
                                sub = 0;
                            }
                        }
                        None => {
                            if let Some(sched) = &self.schedule {
                                if sched.total_samples > self.pos_sample {
                                    sub = sub.min((sched.total_samples - self.pos_sample) as usize);
                                } else {
                                    sub = 0;
                                }
                            }
                        }
                    }
                }
            }

            if sub == 0 {
                // A zero-length segment means an event sits exactly at the boundary; the
                // fire/wrap checks below consume it. No audio is produced for this step.
                self.boundary();
                continue;
            }

            if self.any_ramping() {
                self.tick_gain_ramps();
            }

            let (_head_l, tail_l) = l.split_at_mut(i);
            let (_head_r, tail_r) = r.split_at_mut(i);
            let (seg_l, _) = tail_l.split_at_mut(sub);
            let (seg_r, _) = tail_r.split_at_mut(sub);
            self.render_segment(seg_l, seg_r);

            i += sub;
            self.out_clock = self.out_clock.saturating_add(sub as u64);
            if self.state == TransportState::Playing {
                self.pos_sample = self.pos_sample.saturating_add(sub as u64);
            }
            self.boundary();
        }
        let (pos_sample, position_tick) = self.position();
        BlockStatus {
            position_sample: pos_sample,
            position_tick,
            playing: self.state == TransportState::Playing,
            ended: self.state == TransportState::Ended,
        }
    }

    /// Transport boundary handling after a segment: loop wrap or end-of-schedule.
    fn boundary(&mut self) {
        if self.state != TransportState::Playing {
            return;
        }
        match self.loop_range {
            Some((a, b)) => {
                if self.pos_sample >= b {
                    self.pos_sample = a;
                    self.next_note = self.lower_bound_note(a);
                }
            }
            None => {
                if let Some(sched) = &self.schedule {
                    if self.pos_sample >= sched.total_samples {
                        self.pos_sample = sched.total_samples;
                        self.next_note = sched.notes.len();
                        self.state = TransportState::Ended;
                    }
                }
            }
        }
    }

    fn render_segment(&mut self, seg_l: &mut [f32], seg_r: &mut [f32]) {
        let n = seg_l.len().min(seg_r.len());
        // Grow the reusable send buffers once, on the first large block; steady state
        // render is allocation-free (R-RT-1, R05.R01).
        if self.send_l.len() < n {
            self.send_l.resize(n, 0.0);
            self.send_r.resize(n, 0.0);
        }
        let (send_l, _) = self.send_l.split_at_mut(n);
        let (send_r, _) = self.send_r.split_at_mut(n);
        send_l.fill(0.0);
        send_r.fill(0.0);
        {
            let mut buses = Buses {
                dry_l: seg_l,
                dry_r: seg_r,
                send_l,
                send_r,
            };
            self.synth.render(&mut buses);
        }
        let it = seg_l
            .iter_mut()
            .zip(seg_r.iter_mut())
            .zip(send_l.iter().zip(send_r.iter()));
        for ((dl, dr), (sl, sr)) in it {
            let (o_l, o_r) = self.master.process(*dl, *dr, *sl, *sr);
            *dl = o_l;
            *dr = o_r;
        }
    }
}

fn lane_index(slot: u16) -> Option<usize> {
    let idx = slot as usize;
    if idx < MAX_SLOTS {
        Some(idx)
    } else {
        None
    }
}

fn kind_of(lane: &LaneEntry) -> LaneKind {
    match lane.kind {
        1 => LaneKind::Drum,
        _ => LaneKind::Melodic,
    }
}

fn lane_mix(lane: &LaneEntry) -> LaneMix {
    LaneMix {
        slot: Slot(lane.slot),
        gain: f32::from(lane.volume) / 127.0,
        pan: lane.pan,
        reverb_send: lane.reverb_send,
        mute: lane.is_mute(),
        solo: lane.is_solo(),
    }
}
