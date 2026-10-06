// <META - FILE SUMMARY - Master bus: synth dry + Reverb(send) -> SoftLimiter (C2.5). | L1-45>
// HOTPATH: no allocation, no panic, no indexing.

use ng_dsp::limiter::{LimiterError, SoftLimiter};
use ng_dsp::reverb::Reverb;

/// Master chain: dry + Reverb(send bus) -> SoftLimiter. Effects are pure ng-dsp parts (C2.5).
pub struct MasterBus {
    reverb: Reverb,
    limiter: SoftLimiter,
}

impl MasterBus {
    pub fn new(sample_rate: u32, reverb: Reverb, limiter: SoftLimiter) -> Self {
        let _ = sample_rate; // recorded by the caller's RenderConfig; parts carry their own
        Self { reverb, limiter }
    }

    pub fn new_default(sample_rate: u32) -> Result<Self, MasterBusError> {
        // Feedback is deliberately < 1 so the tail decays; an excess here is a config bug,
        // surfaced as None rather than a runtime blow-up.
        let reverb = Reverb::new(sample_rate, 0.72, 4_000.0, 7).ok_or(MasterBusError::Reverb)?;
        let limiter = SoftLimiter::new_default(sample_rate).map_err(MasterBusError::Limiter)?;
        Ok(Self { reverb, limiter })
    }

    /// One frame through the chain. Sends are mixed into the reverb input; the reverb output
    /// is added to the dry signal before limiting.
    pub fn process(&mut self, dry_l: f32, dry_r: f32, send_l: f32, send_r: f32) -> (f32, f32) {
        let (rl, rr) = self.reverb.process(send_l, send_r);
        self.limiter.process_stereo(dry_l + rl, dry_r + rr)
    }

    pub fn flush_reverb(&mut self) {
        self.reverb.flush();
    }
}

#[derive(Debug)]
pub enum MasterBusError {
    Reverb,
    Limiter(LimiterError),
}
