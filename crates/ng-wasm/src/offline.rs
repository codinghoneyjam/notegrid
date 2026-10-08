// Offline render ABI exports
use crate::buffers::{read_bytes, set_error, set_result};
use ng_export::wav::WavDepth;
use ng_timeline::Schedule;
use ng_synth::osc::OscSynth;
use std::sync::Mutex;

pub struct OfflineState {
    pub schedule: Option<Schedule>,
    pub synth: Option<OscSynth>,
    pub pcm: Option<Vec<u8>>,
    pub position: usize,
    pub total_frames: usize,
    pub sample_rate: u32,
    pub cancelled: bool,
}

pub static OFFLINE: Mutex<OfflineState> = Mutex::new(OfflineState {
    schedule: None,
    synth: None,
    pcm: None,
    position: 0,
    total_frames: 0,
    sample_rate: 44100,
    cancelled: false,
});

#[no_mangle]
pub extern "C" fn ng_offline_begin(proj_ptr: u32, proj_len: u32, opts_ptr: u32, opts_len: u32) -> i32 {
    let Some(proj_bytes) = read_bytes(proj_ptr, proj_len) else {
        set_error("E_FORMAT", "invalid project pointer");
        return -2;
    };
    
    let sample_rate = if opts_len > 0 {
        let Some(opts_bytes) = read_bytes(opts_ptr, opts_len) else {
            set_error("E_FORMAT", "invalid options pointer");
            return -5;
        };
        let opts_str = std::str::from_utf8(opts_bytes).unwrap_or("{}");
        let opts: serde_json::Value = serde_json::from_str(opts_str).unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
        opts["sample_rate"].as_u64().unwrap_or(44100) as u32
    } else {
        44100
    };
    
    match OFFLINE.lock() {
        Ok(mut o) => {
            o.schedule = None;
            o.synth = None;
            o.pcm = Some(proj_bytes.to_vec());
            o.position = 0;
            o.total_frames = 0;
            o.sample_rate = sample_rate;
            o.cancelled = false;
            0
        }
        Err(_) => -7,
    }
}

#[no_mangle]
pub extern "C" fn ng_offline_step(_max_frames: u32) -> i32 {
    let (proj_bytes, sample_rate) = match OFFLINE.lock() {
        Ok(mut o) => {
            if o.cancelled || o.pcm.is_none() {
                return 0;
            }
            let bytes = o.pcm.take().unwrap();
            let sr = o.sample_rate;
            (bytes, sr)
        }
        Err(_) => return -4,
    };
    
    let depth = WavDepth::Int16;
    match crate::project::render_project(&proj_bytes, sample_rate, depth) {
        Ok(wav_bytes) => {
            match OFFLINE.lock() {
                Ok(mut o) => {
                    o.pcm = Some(wav_bytes);
                    o.position = 1;
                    o.total_frames = 1;
                    1
                }
                Err(_) => -4,
            }
        }
        Err((code, msg)) => {
            set_error(code, &msg);
            -3
        }
    }
}

#[no_mangle]
pub extern "C" fn ng_offline_progress() -> f64 {
    match OFFLINE.lock() {
        Ok(o) => {
            if o.total_frames == 0 {
                0.0
            } else {
                o.position as f64 / o.total_frames as f64
            }
        }
        Err(_) => 0.0,
    }
}

#[no_mangle]
pub extern "C" fn ng_offline_cancel() {
    if let Ok(mut o) = OFFLINE.lock() {
        o.cancelled = true;
    }
}

#[no_mangle]
pub extern "C" fn ng_offline_finish() -> i32 {
    match OFFLINE.lock() {
        Ok(mut o) => {
            if let Some(wav_bytes) = o.pcm.take() {
                set_result(wav_bytes);
                0
            } else {
                set_error("E_RENDER", "no data available");
                -2
            }
        }
        Err(_) => -3,
    }
}
