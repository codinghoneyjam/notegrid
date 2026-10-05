// <META - FILE SUMMARY - SP-2 spike: sine via libm, 44100 frames f32; same input native vs wasm.>

pub fn render_sine(out: &mut [f32], sample_rate: u32) {
    let two_pi = 2.0f32 * core::f32::consts::PI;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / sample_rate as f32;
        *s = libm::sinf(two_pi * 440.0 * t);
    }
}

#[no_mangle]
pub extern "C" fn spike_render(out_ptr: u32, len: u32, sample_rate: u32) {
    let out = unsafe { std::slice::from_raw_parts_mut(out_ptr as *mut f32, len as usize) };
    render_sine(out, sample_rate);
}
