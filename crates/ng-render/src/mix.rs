// <META - FILE SUMMARY - Lane mixing model: mute/solo resolution and per-slot ramped gain. | L1-70>
// HOTPATH: no allocation, no panic, no indexing.

/// Per-lane mix state as the UI knows it: gain is linear 0..=1, pan 0..=127, send 0..=127.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaneMix {
    pub slot: ng_core::Slot,
    pub gain: f32,
    pub pan: u8,
    pub reverb_send: u8,
    pub mute: bool,
    pub solo: bool,
}

impl LaneMix {
    pub fn new(slot: ng_core::Slot) -> Self {
        Self {
            slot,
            gain: 1.0,
            pan: 64,
            reverb_send: 0,
            mute: false,
            solo: false,
        }
    }
}

/// R05.DESIGN: effective_gain = any_solo ? (solo ? gain : 0) : (mute ? 0 : gain).
/// Pure function so both the renderer and the tests share one definition.
pub fn effective_gain(lane: &LaneMix, any_solo: bool) -> f32 {
    if any_solo {
        if lane.solo {
            lane.gain
        } else {
            0.0
        }
    } else if lane.mute {
        0.0
    } else {
        lane.gain
    }
}
