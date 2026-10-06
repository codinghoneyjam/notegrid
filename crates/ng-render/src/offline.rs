// <META - FILE SUMMARY - render_offline: headless PCM render with progress/cancel, loop tail_fold. | L1-120>
// HOTPATH: reuses the Renderer, allocation-free in steady state.

use ng_core::{PcmBuffer, Slot};
use ng_timeline::Schedule;

use crate::master::MasterBus;
use crate::mix::LaneMix;
use crate::renderer::{RenderConfig, Renderer};

/// Loop handling for offline export (C2.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LoopMode {
    /// Render once through, then the release tail, truncated at tail_max_sec.
    #[default]
    None,
    /// Render past the loop end until decayed, then fold samples >= E onto S..E (C2.7);
    /// the output file length is exactly the loop end.
    TailFold,
}

#[derive(Clone, Debug)]
pub struct OfflineOptions {
    /// Hard cap on the tail after the scheduled end (C2.7).
    pub tail_max_sec: f32,
    pub loop_mode: LoopMode,
    /// Render only this slot (stems = repeat per lane), muting every other lane.
    pub solo_lane: Option<u16>,
    /// Treat every lane as unmuted/unsoloed for the mix (C4 offline_options).
    pub ignore_mute_solo: bool,
    /// Progress cadence in frames.
    pub progress_interval: usize,
}

impl Default for OfflineOptions {
    fn default() -> Self {
        Self {
            tail_max_sec: 10.0,
            loop_mode: LoopMode::None,
            solo_lane: None,
            ignore_mute_solo: false,
            progress_interval: 4096,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
// Variant names mirror the C9 error-code strings.
#[allow(non_camel_case_types)]
pub enum RenderError {
    /// Progress callback returned false (C2.7 tail: no partial output is returned).
    E_RENDER_CANCELED,
    /// Renderer could not be constructed (master bus error).
    E_RENDER_INIT,
}

/// Offline render: drive the same Renderer&synth used live (R05.R02/R05.R03) so the bytes
/// are transport-identical to a realtime pass through the same code path.
pub fn render_offline<S: ng_core::Synth>(
    schedule: Schedule,
    synth: S,
    opts: &OfflineOptions,
    mut progress: impl FnMut(f32) -> bool,
) -> Result<PcmBuffer, RenderError> {
    let sample_rate = schedule.sample_rate;
    let total = schedule.total_samples;
    let master = MasterBus::new_default(sample_rate).map_err(|_| RenderError::E_RENDER_INIT)?;
    let cfg = RenderConfig::new(sample_rate);
    let mut renderer = Renderer::new(synth, master, cfg);
    renderer.set_schedule(schedule);
    if opts.solo_lane.is_some() || opts.ignore_mute_solo {
        let lanes: Vec<LaneMix> = {
            // Rebuild the mix from the schedule's lane entries, applying solo/mute override.
            let mut lanes = Vec::new();
            if let Some(schedule) = renderer.schedule() {
                for lane in &schedule.lanes {
                    let mut m = LaneMix::new(Slot(lane.slot));
                    m.gain = f32::from(lane.volume) / 127.0;
                    m.pan = lane.pan;
                    m.reverb_send = lane.reverb_send;
                    if opts.ignore_mute_solo {
                        m.mute = false;
                        m.solo = false;
                    } else {
                        m.mute = lane.is_mute();
                        m.solo = lane.is_solo();
                    }
                    lanes.push(m);
                }
            }
            if let Some(slot) = opts.solo_lane {
                for m in &mut lanes {
                    m.mute = m.slot.0 != slot;
                    m.solo = m.slot.0 == slot;
                }
            }
            lanes
        };
        renderer.set_mix(&lanes);
    }
    renderer.play();

    let tail_samples = (opts.tail_max_sec * sample_rate as f32) as u64;
    let mut out_l: Vec<f32> = Vec::new();
    let mut out_r: Vec<f32> = Vec::new();
    let mut l = [0.0f32; 128];
    let mut r = [0.0f32; 128];
    let hard_cap = total.saturating_add(tail_samples);
    let mut peak = 0.0f32;
    let mut silent_above: usize = 0;
    let mut frames = 0u64;
    let mut last_progress = 0u64;
    while frames < hard_cap {
        let status = renderer.render_block(&mut l, &mut r);
        for (lv, rv) in l.iter().zip(r.iter()) {
            peak = peak.max(lv.abs().max(rv.abs()));
            out_l.push(*lv);
            out_r.push(*rv);
        }
        frames += l.len() as u64;
        if peak < 1e-4 {
            silent_above += 1;
        } else {
            silent_above = 0;
        }
        // C2.7 stop condition: no active voices AND master below -80 dBFS.
        if frames >= total && renderer.active_voices() == 0 && silent_above > 8 {
            break;
        }
        let _ = status;
        if frames - last_progress >= opts.progress_interval as u64 {
            last_progress = frames;
            let frac = (frames as f32) / (hard_cap as f32);
            if !progress(frac.min(1.0)) {
                return Err(RenderError::E_RENDER_CANCELED);
            }
        }
    }
    if !progress(1.0) {
        return Err(RenderError::E_RENDER_CANCELED);
    }

    // Truncate or fold to the deliverable length (C2.7).
    let (start, end) = (0u64, total);
    let mut final_l: Vec<f32>;
    let mut final_r: Vec<f32>;
    match opts.loop_mode {
        LoopMode::None => {
            final_l = out_l;
            final_r = out_r;
        }
        LoopMode::TailFold => {
            let span = end.saturating_sub(start);
            final_l = vec![0.0; span as usize];
            final_r = vec![0.0; span as usize];
            for p in 0..out_l.len() as u64 {
                let dst = if p < end {
                    p
                } else {
                    start + ((p - start) % span.max(1))
                };
                if let Some(f) = final_l.get_mut(dst as usize) {
                    *f += out_l.get(p as usize).copied().unwrap_or(0.0);
                }
                if let Some(f) = final_r.get_mut(dst as usize) {
                    *f += out_r.get(p as usize).copied().unwrap_or(0.0);
                }
            }
        }
    }

    let frames = final_l.len();
    let interleaved: Vec<f32> = final_l
        .iter()
        .zip(final_r.iter())
        .flat_map(|(l, r)| [*l, *r])
        .collect();
    Ok(PcmBuffer {
        sample_rate,
        channels: 2,
        frames,
        data: interleaved,
    })
}
