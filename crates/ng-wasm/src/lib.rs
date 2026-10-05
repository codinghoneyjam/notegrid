// <META - FILE SUMMARY - C ABI skeleton (C4): version, alloc/free, validate/migrate/canonical, gm_tables; all unsafe isolated here. | L1-150>

use ng_score::Project;
use std::sync::Mutex;

static RESULT: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static AUX: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static ERROR_BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());

fn set_error(code: &str, message: &str) {
    let json = serde_json_error(code, message);
    if let Ok(mut e) = ERROR_BUF.lock() {
        *e = json;
    }
}

fn serde_json_error(code: &str, message: &str) -> Vec<u8> {
    format!("{{\"code\":\"{code}\",\"message\":\"{message}\"}}").into_bytes()
}

/// Returns a copy of the error buffer, pointed via static; caller must not free.
fn error_ptr_len() -> (u32, usize) {
    match ERROR_BUF.lock() {
        Ok(e) => (e.as_ptr() as u32, e.len()),
        Err(_) => (0, 0),
    }
}

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

/// # Safety
/// `ptr` must have been produced by `ng_alloc` with the same `len`, and must
/// not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn ng_free(ptr: u32, len: u32) {
    if ptr == 0 {
        return;
    }
    let _ = Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize);
}

fn read_bytes<'a>(ptr: u32, len: u32) -> Option<&'a [u8]> {
    if ptr == 0 || len == 0 {
        return None;
    }
    Some(unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) })
}

fn set_result(bytes: Vec<u8>) -> i32 {
    match RESULT.lock() {
        Ok(mut r) => {
            *r = bytes;
            0
        }
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn ng_result_ptr() -> u32 {
    RESULT.lock().map(|r| r.as_ptr() as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn ng_result_len() -> u32 {
    RESULT.lock().map(|r| r.len() as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn ng_aux_ptr() -> u32 {
    AUX.lock().map(|r| r.as_ptr() as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn ng_aux_len() -> u32 {
    AUX.lock().map(|r| r.len() as u32).unwrap_or(0)
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
    let report = ng_score::validate_bytes(bytes);
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
                // warnings are preserved in AUX for callers
                if let Ok(mut a) = AUX.lock() {
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
pub extern "C" fn ng_gm_tables() -> i32 {
    let tables = ng_score::gm_tables();
    set_result(tables.to_string().into_bytes())
}

// suppression: Project used via type imports in future engine code
#[allow(unused)]
fn _type_marker(_: Option<&Project>) {}
