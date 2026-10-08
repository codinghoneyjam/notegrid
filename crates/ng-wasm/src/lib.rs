// <META - FILE SUMMARY - C ABI skeleton (C4): version, alloc/free, validate/migrate/canonical, gm_tables; all unsafe isolated here. | L1-150>

pub mod buffers;
pub mod engine;
pub mod offline;
pub mod project;

use buffers::{error_ptr_len, read_bytes, set_error, set_result};
use ng_score::{gm_tables, validate_bytes};

#[no_mangle]
pub extern "C" fn ng_abi_version() -> u32 {
    1
}

#[no_mangle]
pub extern "C" fn ng_alloc(len: u32) -> u32 {
    let mut buf = vec![0u8; len as usize];
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr as u32
}

#[no_mangle]
pub unsafe extern "C" fn ng_free(ptr: u32, len: u32) {
    if ptr == 0 {
        return;
    }
    let _ = Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize);
}

#[no_mangle]
pub extern "C" fn ng_result_ptr() -> u32 {
    buffers::RESULT.lock().map(|r| r.as_ptr() as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn ng_result_len() -> u32 {
    buffers::RESULT.lock().map(|r| r.len() as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn ng_aux_ptr() -> u32 {
    buffers::AUX.lock().map(|r| r.as_ptr() as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn ng_aux_len() -> u32 {
    buffers::AUX.lock().map(|r| r.len() as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn ng_error_ptr() -> u32 {
    error_ptr_len().0
}

#[no_mangle]
pub extern "C" fn ng_error_len() -> u32 {
    error_ptr_len().1 as u32
}

#[no_mangle]
pub extern "C" fn ng_project_validate(ptr: u32, len: u32) -> i32 {
    let Some(bytes) = read_bytes(ptr, len) else {
        set_error("E_FORMAT", "invalid input pointer");
        return -2;
    };
    let report = validate_bytes(bytes);
    let json = format!(
        "{{\"errors\":[{}],\"warnings\":[{}]}}",
        report
            .errors
            .iter()
            .map(issue_json)
            .collect::<Vec<_>>()
            .join(","),
        report
            .warnings
            .iter()
            .map(issue_json)
            .collect::<Vec<_>>()
            .join(",")
    );
    set_result(json.into_bytes())
}

fn issue_json(i: &ng_score::Issue) -> String {
    format!(
        "{{\"code\":\"{}\",\"message\":{},\"path\":{}}}",
        i.code,
        quote(&i.message),
        quote(&i.path)
    )
}

fn quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[no_mangle]
pub extern "C" fn ng_project_migrate(ptr: u32, len: u32) -> i32 {
    let Some(bytes) = read_bytes(ptr, len) else {
        set_error("E_FORMAT", "invalid input pointer");
        return -2;
    };
    match ng_score::migrate(bytes) {
        Ok(project) => {
            let report = ng_score::validate(&project);
            for w in &report.warnings {
                if let Ok(mut a) = buffers::AUX.lock() {
                    a.extend_from_slice(format!("{}\n", w.message).as_bytes());
                }
            }
            set_result(ng_score::to_canonical_json(&project).into_bytes())
        }
        Err(e) => {
            set_error(e.code, &e.message);
            -3
        }
    }
}

#[no_mangle]
pub extern "C" fn ng_project_canonical(ptr: u32, len: u32) -> i32 {
    let Some(bytes) = read_bytes(ptr, len) else {
        set_error("E_FORMAT", "invalid input pointer");
        return -2;
    };
    match ng_score::migrate(bytes) {
        Ok(project) => set_result(ng_score::to_canonical_json(&project).into_bytes()),
        Err(e) => {
            set_error(e.code, &e.message);
            -3
        }
    }
}

#[no_mangle]
pub extern "C" fn ng_compile(ptr: u32, len: u32, sr: u32) -> i32 {
    let Some(bytes) = read_bytes(ptr, len) else {
        set_error("E_FORMAT", "invalid input pointer");
        return -2;
    };
    let project = ng_score::migrate(bytes).map_err(|e| (e.code, e.message));
    let project = match project {
        Ok(p) => p,
        Err((code, msg)) => {
            set_error(code, &msg);
            return -3;
        }
    };
    let report = ng_score::validate(&project);
    if !report.errors.is_empty() {
        set_error(
            "E_FORMAT",
            &report
                .errors
                .iter()
                .map(|e| format!("{} at {}", e.code, e.path))
                .collect::<Vec<_>>()
                .join(", "),
        );
        return -4;
    }
    let schedule = ng_timeline::compile(&project, sr).map_err(|e| ("E_COMPILE", format!("{e}")));
    match schedule {
        Ok(s) => set_result(s.to_bytes()),
        Err((code, msg)) => {
            set_error(code, &msg);
            -5
        }
    }
}

#[no_mangle]
pub extern "C" fn ng_gm_tables() -> i32 {
    let tables = gm_tables();
    set_result(tables.to_string().into_bytes())
}

#[no_mangle]
pub extern "C" fn ng_render_wav(ptr: u32, len: u32, sr: u32, depth: u32) -> i32 {
    let Some(bytes) = read_bytes(ptr, len) else {
        set_error("E_FORMAT", "invalid input pointer");
        return -2;
    };
    let depth = match depth {
        0 => ng_export::wav::WavDepth::Int16,
        1 => ng_export::wav::WavDepth::Int24,
        _ => ng_export::wav::WavDepth::Float32,
    };
    match project::render_project(bytes, sr, depth) {
        Ok(wav) => set_result(wav),
        Err((code, msg)) => {
            set_error(code, &msg);
            -4
        }
    }
}
