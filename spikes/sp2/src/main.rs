fn main() {
    let mut buf = vec![0f32; 44100];
    ng_spike_sp2::render_sine(&mut buf, 44100);
    let bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, buf.len() * 4) };
    std::fs::write("spikes/sp2/native_sine.f32le", bytes).unwrap();
    println!("wrote {} bytes", bytes.len());
}
