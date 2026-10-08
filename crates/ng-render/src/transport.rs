// Transport state management for Renderer.

use ng_core::Tick;

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
