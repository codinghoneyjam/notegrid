// Buffer management utilities for C ABI

use std::sync::Mutex;

pub static RESULT: Mutex<Vec<u8>> = Mutex::new(Vec::new());
pub static AUX: Mutex<Vec<u8>> = Mutex::new(Vec::new());
pub static ERROR_BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());

pub fn set_error(code: &str, message: &str) {
    let json = serde_json_error(code, message);
    if let Ok(mut e) = ERROR_BUF.lock() {
        *e = json;
    }
}

fn serde_json_error(code: &str, message: &str) -> Vec<u8> {
    format!("{{\"code\":\"{code}\",\"message\":\"{message}\"}}").into_bytes()
}

pub fn error_ptr_len() -> (u32, usize) {
    match ERROR_BUF.lock() {
        Ok(e) => (e.as_ptr() as u32, e.len()),
        Err(_) => (0, 0),
    }
}

pub fn set_result(bytes: Vec<u8>) -> i32 {
    match RESULT.lock() {
        Ok(mut r) => {
            *r = bytes;
            0
        }
        Err(_) => -1,
    }
}

pub fn read_bytes<'a>(ptr: u32, len: u32) -> Option<&'a [u8]> {
    if ptr == 0 || len == 0 {
        return None;
    }
    Some(unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) })
}
