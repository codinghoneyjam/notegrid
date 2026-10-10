// Engine ABI exports
use crate::buffers::{read_bytes, set_error};
use ng_core::{LaneKind, Slot};
use ng_render::{LaneMix, MasterBus, Renderer};
use ng_synth::osc::OscSynth;
use ng_timeline::{Schedule, TempoMap};
use std::sync::Mutex;

#[derive(Debug)]
pub enum MixError {
    Format(&'static str),
    Range(&'static str),
}

fn parse_mix_records(bytes: &[u8]) -> Result<Vec<LaneMix>, MixError> {
    if !bytes.len().is_multiple_of(6) {
        return Err(MixError::Format("payload length must be a multiple of 6"));
    }
    let mut lanes = Vec::with_capacity(bytes.len() / 6);
    for chunk in bytes.as_chunks::<6>().0 {
        let slot = u16::from_le_bytes([chunk[0], chunk[1]]);
        let volume = chunk[2];
        let pan = chunk[3];
        let reverb_send = chunk[4];
        let flags = chunk[5];
        if slot >= 128 {
            return Err(MixError::Range("slot must be <= 127"));
        }
        if volume > 127 {
            return Err(MixError::Range("volume must be <= 127"));
        }
        if pan > 127 {
            return Err(MixError::Range("pan must be <= 127"));
        }
        if reverb_send > 127 {
            return Err(MixError::Range("reverb_send must be <= 127"));
        }
        if flags & 0b1111_1110 != 0 {
            return Err(MixError::Format("reserved flag bits must be zero"));
        }
        lanes.push(LaneMix {
            slot: Slot(slot),
            gain: f32::from(volume) / 127.0,
            pan,
            reverb_send,
            mute: flags & 1 != 0,
            solo: false,
        });
    }
    Ok(lanes)
}

pub struct EngineState {
    pub renderer: Option<Renderer<OscSynth>>,
    pub sample_rate: u32,
    pub max_voices: usize,
}

pub static ENGINE: Mutex<EngineState> = Mutex::new(EngineState {
    renderer: None,
    sample_rate: 44100,
    max_voices: 64,
});

#[no_mangle]
pub extern "C" fn ng_engine_init(sr: u32, synth_kind: u32, max_voices: u32) -> i32 {
    let sample_rate = if sr == 0 { 44100 } else { sr };
    let voices = if max_voices == 0 {
        64
    } else {
        max_voices as usize
    };

    if synth_kind != 0 {
        set_error("E_UNSUPPORTED", "only OscSynth (kind 0) is supported in P1");
        return -6;
    }

    let synth = OscSynth::new(sample_rate, voices);
    let master = match MasterBus::new_default(sample_rate) {
        Ok(m) => m,
        Err(_) => {
            set_error("E_ENGINE", "master bus init failed");
            return -6;
        }
    };
    let renderer = Renderer::new(synth, master, ng_render::RenderConfig::new(sample_rate));

    match ENGINE.lock() {
        Ok(mut e) => {
            e.renderer = Some(renderer);
            e.sample_rate = sample_rate;
            e.max_voices = voices;
            0
        }
        Err(_) => -7,
    }
}

#[no_mangle]
pub extern "C" fn ng_engine_set_schedule(ptr: u32, len: u32) -> i32 {
    let Some(bytes) = read_bytes(ptr, len) else {
        set_error("E_FORMAT", "invalid input pointer");
        return -2;
    };
    let schedule = Schedule::from_bytes(bytes);
    let schedule = match schedule {
        Ok(s) => s,
        Err(e) => {
            set_error("E_FORMAT", &format!("{e}"));
            return -3;
        }
    };

    match ENGINE.lock() {
        Ok(mut e) => {
            if let Some(ref mut renderer) = e.renderer {
                renderer.set_schedule(schedule);
                0
            } else {
                set_error("E_ENGINE", "engine not initialized");
                -4
            }
        }
        Err(_) => -5,
    }
}

#[no_mangle]
pub extern "C" fn ng_engine_set_loop_ticks(start_tick: u32, end_tick: u32, enabled: u32) -> i32 {
    match ENGINE.lock() {
        Ok(mut e) => {
            if let Some(ref mut renderer) = e.renderer {
                if enabled == 0 {
                    renderer.set_loop(None);
                    return 0;
                }
                let schedule = match renderer.schedule() {
                    Some(s) => s,
                    None => {
                        set_error("E_ENGINE", "no schedule loaded");
                        return -3;
                    }
                };
                let sample_rate = schedule.sample_rate;
                let pairs: Vec<(u32, u32)> =
                    schedule.tempos.iter().map(|t| (t.tick, t.mpq)).collect();
                let tempo = match TempoMap::from_mpq_segments(sample_rate, &pairs) {
                    Ok(t) => t,
                    Err(_) => {
                        set_error("E_ENGINE", "tempo map build failed");
                        return -4;
                    }
                };
                let s0 = tempo.tick_to_sample(start_tick);
                let s1 = tempo.tick_to_sample(end_tick);
                renderer.set_loop(Some((s0, s1)));
                0
            } else {
                set_error("E_ENGINE", "engine not initialized");
                -2
            }
        }
        Err(_) => -5,
    }
}

#[no_mangle]
pub extern "C" fn ng_engine_set_mix(ptr: u32, len: u32) -> i32 {
    let Some(bytes) = read_bytes(ptr, len) else {
        set_error("E_FORMAT", "invalid mix payload");
        return -2;
    };
    let lanes = match parse_mix_records(bytes) {
        Ok(l) => l,
        Err(MixError::Format(m)) => {
            set_error("E_FORMAT", m);
            return -2;
        }
        Err(MixError::Range(m)) => {
            set_error("E_RANGE", m);
            return -3;
        }
    };
    match ENGINE.lock() {
        Ok(mut e) => {
            if let Some(ref mut renderer) = e.renderer {
                renderer.set_mix(&lanes);
                0
            } else {
                set_error("E_ENGINE", "engine not initialized");
                -4
            }
        }
        Err(_) => -5,
    }
}

#[no_mangle]
pub extern "C" fn ng_engine_transport(cmd: u32, arg: f64) -> i32 {
    match ENGINE.lock() {
        Ok(mut e) => {
            if let Some(ref mut renderer) = e.renderer {
                match cmd {
                    0 => renderer.play(),
                    1 => renderer.pause(),
                    2 => renderer.stop(),
                    3 => renderer.seek(arg as u64),
                    _ => {
                        set_error("E_INVALID_CMD", "invalid transport command");
                        return -2;
                    }
                }
                0
            } else {
                set_error("E_ENGINE", "engine not initialized");
                -3
            }
        }
        Err(_) => -4,
    }
}

#[no_mangle]
pub extern "C" fn ng_engine_set_loop(start: f64, end: f64, enabled: u32) -> i32 {
    match ENGINE.lock() {
        Ok(mut e) => {
            if let Some(ref mut renderer) = e.renderer {
                let range = if enabled != 0 {
                    Some((start as u64, end as u64))
                } else {
                    None
                };
                renderer.set_loop(range);
                0
            } else {
                set_error("E_ENGINE", "engine not initialized");
                -2
            }
        }
        Err(_) => -3,
    }
}

#[no_mangle]
pub extern "C" fn ng_engine_audition(kind: u32, program: u32, key: u32, vel: u32, on: u32) -> i32 {
    if on != 0 {
        if kind > 1 {
            set_error("E_RANGE", "kind must be 0 or 1");
            return -2;
        }
        if program > 127 {
            set_error("E_RANGE", "program must be <= 127");
            return -2;
        }
        if key > 127 {
            set_error("E_RANGE", "key must be <= 127");
            return -2;
        }
        if vel == 0 || vel > 127 {
            set_error("E_RANGE", "vel must be 1..=127 when on");
            return -2;
        }
    } else {
        if key > 127 {
            set_error("E_RANGE", "key must be <= 127");
            return -2;
        }
    }
    match ENGINE.lock() {
        Ok(mut e) => {
            if let Some(ref mut renderer) = e.renderer {
                if on != 0 {
                    let kind = if kind == 1 {
                        LaneKind::Drum
                    } else {
                        LaneKind::Melodic
                    };
                    renderer.audition_on(kind, program as u8, key as u8, vel as u8);
                } else {
                    renderer.audition_off(key as u8);
                }
                0
            } else {
                set_error("E_ENGINE", "engine not initialized");
                -3
            }
        }
        Err(_) => -4,
    }
}

#[no_mangle]
pub extern "C" fn ng_engine_render(l_ptr: u32, r_ptr: u32, frames: u32) -> i32 {
    let l_buf = unsafe { std::slice::from_raw_parts_mut(l_ptr as *mut f32, frames as usize) };
    let r_buf = unsafe { std::slice::from_raw_parts_mut(r_ptr as *mut f32, frames as usize) };

    match ENGINE.lock() {
        Ok(mut e) => {
            if let Some(ref mut renderer) = e.renderer {
                let status = renderer.render_block(l_buf, r_buf);
                let mut bits = 0u32;
                if status.playing {
                    bits |= 1;
                }
                if status.ended {
                    bits |= 2;
                }
                bits as i32
            } else {
                set_error("E_ENGINE", "engine not initialized");
                -4
            }
        }
        Err(_) => -5,
    }
}

#[no_mangle]
pub extern "C" fn ng_engine_position_sample() -> f64 {
    match ENGINE.lock() {
        Ok(e) => {
            if let Some(ref renderer) = e.renderer {
                renderer.position_sample() as f64
            } else {
                0.0
            }
        }
        Err(_) => 0.0,
    }
}

#[no_mangle]
pub extern "C" fn ng_engine_position_tick() -> u32 {
    match ENGINE.lock() {
        Ok(e) => {
            if let Some(ref renderer) = e.renderer {
                renderer.position_tick()
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mix_records_valid() {
        let bytes = [0u8, 0, 100, 64, 0, 0];
        let result = parse_mix_records(&bytes);
        assert!(result.is_ok());
        let lanes = result.unwrap();
        assert_eq!(lanes.len(), 1);
        assert_eq!(lanes[0].slot, Slot(0));
        assert!((lanes[0].gain - 100.0 / 127.0).abs() < 1e-6);
        assert_eq!(lanes[0].pan, 64);
        assert_eq!(lanes[0].reverb_send, 0);
        assert!(!lanes[0].mute);
        assert!(!lanes[0].solo);
    }

    #[test]
    fn parse_mix_records_mute_flag() {
        let bytes = [0u8, 0, 100, 64, 0, 1];
        let result = parse_mix_records(&bytes);
        assert!(result.is_ok());
        let lanes = result.unwrap();
        assert!(lanes[0].mute);
    }

    #[test]
    fn parse_mix_records_slot_127_ok() {
        let bytes = [127u8, 0, 100, 64, 0, 0];
        let result = parse_mix_records(&bytes);
        assert!(result.is_ok());
        assert_eq!(result.unwrap()[0].slot, Slot(127));
    }

    #[test]
    fn parse_mix_records_slot_128_range() {
        let bytes = [0x80u8, 0x00, 100, 64, 0, 0];
        let result = parse_mix_records(&bytes);
        assert!(matches!(result, Err(MixError::Range(_))));
    }

    #[test]
    fn parse_mix_records_volume_128_range() {
        let bytes = [0u8, 0, 128, 64, 0, 0];
        let result = parse_mix_records(&bytes);
        assert!(matches!(result, Err(MixError::Range(_))));
    }

    #[test]
    fn parse_mix_records_pan_128_range() {
        let bytes = [0u8, 0, 100, 128, 0, 0];
        let result = parse_mix_records(&bytes);
        assert!(matches!(result, Err(MixError::Range(_))));
    }

    #[test]
    fn parse_mix_records_send_128_range() {
        let bytes = [0u8, 0, 100, 64, 128, 0];
        let result = parse_mix_records(&bytes);
        assert!(matches!(result, Err(MixError::Range(_))));
    }

    #[test]
    fn parse_mix_records_len_not_multiple_of_6() {
        let bytes = [0u8, 0, 100, 64, 0];
        let result = parse_mix_records(&bytes);
        assert!(matches!(result, Err(MixError::Format(_))));
    }

    #[test]
    fn parse_mix_records_reserved_flag_bits() {
        let bytes = [0u8, 0, 100, 64, 0, 0b100];
        let result = parse_mix_records(&bytes);
        assert!(matches!(result, Err(MixError::Format(_))));
    }

    #[test]
    fn parse_mix_records_empty() {
        let bytes: [u8; 0] = [];
        let result = parse_mix_records(&bytes);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    /// Every entry point's error table, pinned against the ENGINE static.
    ///
    /// The static is process-global and the test harness runs tests on parallel threads, so
    /// the whole state machine lives in ONE test fn: the fresh-engine assertions run first,
    /// before anything calls `ng_engine_init`.
    #[test]
    fn abi_state_machine_error_tables() {
        // --- fresh engine: no renderer yet ---
        assert_eq!(
            ng_engine_set_loop_ticks(0, 7680, 1),
            -2,
            "set_loop_ticks without a renderer"
        );
        assert_eq!(
            ng_engine_set_mix(0, 0),
            -2,
            "set_mix rejects ptr 0 before any state"
        );
        assert_eq!(
            ng_engine_audition(0, 0, 60, 100, 1),
            -3,
            "audition without a renderer"
        );

        // --- initialized, still no schedule ---
        assert_eq!(ng_engine_init(44100, 0, 64), 0, "engine init");
        assert_eq!(
            ng_engine_set_mix(0, 0),
            -2,
            "set_mix rejects ptr 0 after init"
        );
        assert_eq!(
            ng_engine_set_loop_ticks(0, 7680, 1),
            -3,
            "a tick loop-set before any schedule must fail, never silently succeed"
        );
        assert_eq!(
            ng_engine_set_loop_ticks(0, 0, 0),
            0,
            "clearing a loop needs no schedule"
        );

        // --- audition argument validation runs before the lock ---
        assert_eq!(
            ng_engine_audition(2, 0, 60, 100, 1),
            -2,
            "kind must be 0 or 1"
        );
        assert_eq!(
            ng_engine_audition(0, 128, 60, 100, 1),
            -2,
            "program must be <= 127"
        );
        assert_eq!(
            ng_engine_audition(0, 0, 128, 100, 1),
            -2,
            "key must be <= 127"
        );
        assert_eq!(
            ng_engine_audition(0, 0, 60, 0, 1),
            -2,
            "vel 0 is no note on"
        );
        assert_eq!(
            ng_engine_audition(0, 0, 60, 128, 1),
            -2,
            "vel must be <= 127"
        );
        assert_eq!(
            ng_engine_audition(0, 0, 128, 0, 0),
            -2,
            "key is validated on release too"
        );
        assert_eq!(ng_engine_audition(0, 0, 60, 100, 1), 0, "valid note on");
        assert_eq!(
            ng_engine_audition(0, 0, 60, 0, 0),
            0,
            "valid note off; kind / program / vel are ignored on release"
        );
    }
}
