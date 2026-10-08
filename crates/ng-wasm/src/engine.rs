// Engine ABI exports
use crate::buffers::{read_bytes, set_error};
use ng_render::{MasterBus, Renderer};
use ng_synth::osc::OscSynth;
use ng_timeline::Schedule;
use std::sync::Mutex;

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
    let voices = if max_voices == 0 { 64 } else { max_voices as usize };
    
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
pub extern "C" fn ng_engine_set_mix(_ptr: u32, _len: u32) -> i32 {
    set_error("E_NOT_IMPLEMENTED", "ng_engine_set_mix not implemented in P1");
    -6
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
pub extern "C" fn ng_engine_audition(_kind: u32, _program: u32, _key: u32, _vel: u32, _on: u32) -> i32 {
    set_error("E_NOT_IMPLEMENTED", "ng_engine_audition not implemented in P1");
    -6
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
