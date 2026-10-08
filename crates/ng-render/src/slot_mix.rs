// Per-slot mix state with gain ramping.

use ng_core::{LaneKind, Slot, MAX_SLOTS};
use ng_dsp::ramp::Ramp;
use ng_timeline::LaneEntry;

use crate::mix::LaneMix;

/// Per-slot live mix state with a 5 ms ramp on gain changes (R05.DESIGN).
#[derive(Clone, Debug)]
pub struct SlotMixState {
    pub mix: LaneMix,
    pub applied: f32,
    pub ramp: Ramp,
}

impl SlotMixState {
    pub fn new(slot: Slot) -> Self {
        Self {
            mix: LaneMix::new(slot),
            applied: 1.0,
            ramp: unity_ramp(),
        }
    }

    pub fn is_ramping(&self) -> bool {
        !self.ramp.is_done()
    }
}

/// A one-sample unity ramp; the length is non-zero by construction, so this never errs.
/// (No unwrap in src: re-try with the next length rather than panicking.)
pub fn unity_ramp() -> Ramp {
    let mut len = 1u32;
    loop {
        if let Ok(r) = Ramp::new(1.0, 1.0, len) {
            return r;
        }
        len = len.saturating_add(1);
    }
}

pub fn kind_of(lane: &LaneEntry) -> LaneKind {
    match lane.kind {
        1 => LaneKind::Drum,
        _ => LaneKind::Melodic,
    }
}

pub fn lane_mix(lane: &LaneEntry) -> LaneMix {
    LaneMix {
        slot: Slot(lane.slot),
        gain: f32::from(lane.volume) / 127.0,
        pan: lane.pan,
        reverb_send: lane.reverb_send,
        mute: lane.is_mute(),
        solo: lane.is_solo(),
    }
}

pub fn lane_index(slot: u16) -> Option<usize> {
    let idx = slot as usize;
    if idx < MAX_SLOTS {
        Some(idx)
    } else {
        None
    }
}
