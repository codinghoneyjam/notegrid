// <META - FILE SUMMARY - WAV (RIFF/WAVE) encoder: 16/24-bit int and 32-bit float, smpl + INFO, clip count. | L1-200>
// Not HOTPATH: encoding is offline IO work. Still no panic paths on bad input — return errors.

use ng_core::{AudioEncoder, EncodeError, EncodeMeta, PcmBuffer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WavDepth {
    #[default]
    Int16,
    Int24,
    Float32,
}

#[derive(Clone, Debug, Default)]
pub struct WavOptions {
    pub depth: WavDepth,
    /// Seeded TPDF dither, OFF by default (C8). Kept for completeness; deterministic so
    /// R06.R02 holds when enabled.
    pub dither: bool,
    pub dither_seed: u64,
}

pub struct WavEncoder {
    /// Samples at/over full scale seen by the last encode (C8 "clip count reported").
    last_clip_count: std::cell::Cell<u64>,
}

impl WavEncoder {
    pub fn new() -> Self {
        Self {
            last_clip_count: std::cell::Cell::new(0),
        }
    }

    pub fn last_clip_count(&self) -> u64 {
        self.last_clip_count.get()
    }
}

impl Default for WavEncoder {
    fn default() -> Self {
        Self::new()
    }
}

fn err(msg: &str) -> EncodeError {
    EncodeError {
        code: "E_ENCODE",
        message: msg.to_string(),
    }
}

fn push_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn push_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn push_tag(out: &mut Vec<u8>, tag: &[u8; 4]) {
    out.extend_from_slice(tag);
}

fn put_chunk(out: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    push_tag(out, tag);
    push_u32(out, payload.len() as u32);
    out.extend_from_slice(payload);
    // Chunks pad to even length (C8).
    if payload.len() % 2 == 1 {
        out.push(0);
    }
}

fn info_chunk(title: &str) -> Vec<u8> {
    let mut body = Vec::new();
    push_tag(&mut body, b"INFO");
    let mut inam = title.as_bytes().to_vec();
    inam.push(0);
    if inam.len() % 2 == 1 {
        inam.push(0);
    }
    let mut sub = Vec::new();
    push_tag(&mut sub, b"INAM");
    push_u32(&mut sub, inam.len() as u32);
    sub.extend_from_slice(&inam);
    body.extend_from_slice(&sub);
    body
}

fn smpl_chunk(loop_points: (u32, u32)) -> Vec<u8> {
    let mut body = Vec::new();
    // 36 bytes of fixed header, one loop record (24 bytes).
    push_u32(&mut body, 0); // manufacturer
    push_u32(&mut body, 0); // product
    push_u32(&mut body, 0); // sample period
    push_u32(&mut body, 60); // MIDI unity note
    push_u32(&mut body, 0); // pitch fraction
    push_u32(&mut body, 0); // SMPTE format
    push_u32(&mut body, 0); // SMPTE offset
    push_u32(&mut body, 1); // num sample loops
    push_u32(&mut body, 0); // sampler data
                            // loop record
    push_u32(&mut body, 0); // cue id
    push_u32(&mut body, 0); // type: forward loop
    push_u32(&mut body, loop_points.0); // start
    push_u32(&mut body, loop_points.1); // end
    push_u32(&mut body, 0); // fraction
    push_u32(&mut body, u32::MAX); // infinite play count
    body
}

impl AudioEncoder for WavEncoder {
    type Options = WavOptions;

    fn id(&self) -> &'static str {
        "wav"
    }

    fn extension(&self) -> &'static str {
        "wav"
    }

    fn mime(&self) -> &'static str {
        "audio/wav"
    }

    fn encode(
        &self,
        pcm: &PcmBuffer,
        meta: &EncodeMeta,
        opts: &Self::Options,
    ) -> Result<Vec<u8>, EncodeError> {
        if pcm.channels == 0 || pcm.channels > 2 {
            return Err(err("only mono/stereo supported"));
        }
        if pcm.frames * pcm.channels as usize != pcm.data.len() {
            return Err(err("frame/data length mismatch"));
        }
        let bytes_per_sample: usize = match opts.depth {
            WavDepth::Int16 => 2,
            WavDepth::Int24 => 3,
            WavDepth::Float32 => 4,
        };
        let block_align = (pcm.channels as usize) * bytes_per_sample;
        let byte_rate = pcm.sample_rate as usize * block_align;
        let audio_format: u16 = match opts.depth {
            WavDepth::Int16 | WavDepth::Int24 => 1,
            WavDepth::Float32 => 3,
        };
        let bits: u16 = match opts.depth {
            WavDepth::Int16 => 16,
            WavDepth::Int24 => 24,
            WavDepth::Float32 => 32,
        };

        let mut data = Vec::with_capacity(pcm.data.len() * bytes_per_sample);
        let mut clips = 0u64;
        let mut dither_state = opts.dither_seed.wrapping_mul(0x9E37_79B9).wrapping_add(1);
        for &x in pcm.data.iter() {
            match convert_sample(x, opts.depth, opts.dither, &mut dither_state) {
                (SampleCode::Int(v), clipped) => {
                    if clipped {
                        clips += 1;
                    }
                    match opts.depth {
                        WavDepth::Int16 => {
                            data.extend_from_slice(&(v as i16).to_le_bytes());
                        }
                        WavDepth::Int24 => {
                            data.extend_from_slice(&[
                                (v & 0xFF) as u8,
                                ((v >> 8) & 0xFF) as u8,
                                ((v >> 16) & 0xFF) as u8,
                            ]);
                        }
                        WavDepth::Float32 => {}
                    }
                }
                (SampleCode::Float(v), clipped) => {
                    if clipped {
                        clips += 1;
                    }
                    data.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
        self.last_clip_count.set(clips);

        let mut fmt = Vec::new();
        push_u16(&mut fmt, audio_format);
        push_u16(&mut fmt, pcm.channels as u16);
        push_u32(&mut fmt, pcm.sample_rate);
        push_u32(&mut fmt, byte_rate as u32);
        push_u16(&mut fmt, block_align as u16);
        push_u16(&mut fmt, bits);

        let mut chunks = Vec::new();
        put_chunk(&mut chunks, b"fmt ", &fmt);
        if audio_format == 3 {
            // Sample count is required for non-PCM formats; keep the file self-describing.
            let mut fact = Vec::new();
            push_u32(&mut fact, pcm.frames as u32);
            put_chunk(&mut chunks, b"fact", &fact);
        }
        if let Some(loop_points) = meta.loop_points {
            let smpl = smpl_chunk(loop_points);
            put_chunk(&mut chunks, b"smpl", &smpl);
        }
        if let Some(title) = &meta.title {
            let mut list = Vec::new();
            list.extend_from_slice(&info_chunk(title));
            put_chunk(&mut chunks, b"LIST", &list);
        }
        put_chunk(&mut chunks, b"data", &data);

        let riff_size = 4 + chunks.len();
        if riff_size > u32::MAX as usize {
            return Err(err("WAV exceeds 4 GiB"));
        }
        let mut out = Vec::with_capacity(8 + riff_size);
        push_tag(&mut out, b"RIFF");
        push_u32(&mut out, riff_size as u32);
        push_tag(&mut out, b"WAVE");
        out.extend_from_slice(&chunks);
        Ok(out)
    }
}

/// The converted value for one sample: an integer code for PCM depths, the float itself otherwise.
enum SampleCode {
    Int(i32),
    Float(f32),
}

/// Convert one f32 sample to its integer/float code. Returns the code and whether it clipped.
fn convert_sample(x: f32, depth: WavDepth, dither: bool, state: &mut u64) -> (SampleCode, bool) {
    let mut t = x;
    let mut clipped = false;
    if t >= 1.0 {
        t = 1.0;
        clipped = true;
    } else if t <= -1.0 {
        t = -1.0;
        clipped = true;
    }
    match depth {
        WavDepth::Float32 => (SampleCode::Float(t), clipped),
        WavDepth::Int16 => {
            let scale = (1u32 << 15) as f32 - 1.0;
            let mut v_in = t * scale;
            if dither {
                v_in += tpdf(state) * 0.5;
            }
            (
                SampleCode::Int(v_in.round().clamp(-32768.0, 32767.0) as i32),
                clipped,
            )
        }
        WavDepth::Int24 => {
            let scale = (1u32 << 23) as f32 - 1.0;
            let mut v_in = t * scale;
            if dither {
                v_in += tpdf(state) * 0.5;
            }
            (
                SampleCode::Int(v_in.round().clamp(-8_388_608.0, 8_388_607.0) as i32),
                clipped,
            )
        }
    }
}

/// Triangular dithering noise from a SplitMix64 state, in [-1, 1).
fn tpdf(state: &mut u64) -> f32 {
    let next = |s: &mut u64| {
        *s = s.wrapping_add(0x9E37_79B9);
        let mut z = *s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB);
        z ^ (z >> 31)
    };
    let a = (next(state) >> 40) as f32 / (1u32 << 24) as f32;
    let b = (next(state) >> 40) as f32 / (1u32 << 24) as f32;
    a + b - 1.0
}
