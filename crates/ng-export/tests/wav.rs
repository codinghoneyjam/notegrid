// <META - ROLE : R06.AC1 — WAV 16/24/float read-back within ±1 LSB; smpl and INFO parse. | L1-160>
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use ng_core::{AudioEncoder, EncodeMeta, PcmBuffer};
use ng_export::wav::{WavDepth, WavEncoder, WavOptions};

fn sample_pcm() -> PcmBuffer {
    let mut data = Vec::new();
    for i in 0..2000usize {
        let t = i as f32 / 44_100.0;
        let v = (t * 440.0 * core::f32::consts::TAU).sin() * 0.8;
        data.push(v);
        data.push(v * 0.5);
    }
    PcmBuffer {
        sample_rate: 44_100,
        channels: 2,
        frames: 2000,
        data,
    }
}

fn meta() -> EncodeMeta {
    EncodeMeta {
        title: Some("example track".to_string()),
        loop_points: Some((100, 1800)),
    }
}

// A tiny WAV reader: only what the test needs — fmt, smpl, and a LIST chunk walk,
// plus the data frames pulled back out at the right depth.
struct ReadWav {
    audio_format: u16,
    channels: u16,
    bits: u16,
    frame_count: usize,
    data: Vec<u8>,
    smpl_loop_start: u32,
    smpl_loop_end: u32,
    info_title: Option<String>,
}

fn read_u32(buf: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
}
fn read_u16(buf: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([buf[at], buf[at + 1]])
}

fn parse_wav(bytes: &[u8]) -> ReadWav {
    assert!(&bytes[0..4] == b"RIFF", "missing RIFF");
    assert!(&bytes[8..12] == b"WAVE", "missing WAVE");
    let mut out = ReadWav {
        audio_format: 0,
        channels: 0,
        bits: 0,
        frame_count: 0,
        data: Vec::new(),
        smpl_loop_start: 0,
        smpl_loop_end: 0,
        info_title: None,
    };
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let tag = &bytes[at..at + 4];
        let size = read_u32(bytes, at + 4) as usize;
        let body = &bytes[at + 8..at + 8 + size];
        match tag {
            b"fmt " => {
                out.audio_format = read_u16(body, 0);
                out.channels = read_u16(body, 2);
                out.bits = read_u16(body, 14);
            }
            b"data" => {
                out.data = body.to_vec();
                let bytes_per = out.bits as usize / 8;
                out.frame_count = out.data.len() / (bytes_per * out.channels as usize);
            }
            b"smpl" => {
                // smpl body = 36-byte header, then per loop: cue(4) type(4) start(4) end(4)...
                out.smpl_loop_start = read_u32(body, 36 + 8);
                out.smpl_loop_end = read_u32(body, 36 + 12);
            }
            b"LIST" if &body[0..4] == b"INFO" => {
                let mut i = 4;
                while i + 8 <= body.len() {
                    let stag = &body[i..i + 4];
                    let slen = read_u32(body, i + 4) as usize;
                    let sbody = &body[i + 8..i + 8 + slen];
                    if stag == b"INAM" {
                        let end = sbody.iter().position(|&b| b == 0).unwrap_or(sbody.len());
                        out.info_title = Some(String::from_utf8_lossy(&sbody[..end]).to_string());
                    }
                    i += 8 + slen + (slen % 2);
                }
            }
            _ => {}
        }
        at += 8 + size + (size % 2);
    }
    out
}

fn frames_out(rw: &ReadWav) -> Vec<f32> {
    let mut v = Vec::new();
    match rw.bits {
        16 => {
            for chunk in rw.data.chunks(2) {
                let s = i16::from_le_bytes([chunk[0], chunk[1]]);
                v.push(s as f32 / 32767.0);
            }
        }
        24 => {
            for chunk in rw.data.chunks(3) {
                let mut bytes = [0u8; 4];
                bytes[0] = chunk[0];
                bytes[1] = chunk[1];
                bytes[2] = chunk[2];
                bytes[3] = if chunk[2] & 0x80 != 0 { 0xFF } else { 0 };
                let s = i32::from_le_bytes(bytes);
                v.push(s as f32 / 8388607.0);
            }
        }
        32 => {
            assert_eq!(rw.audio_format, 3);
            for chunk in rw.data.chunks(4) {
                v.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
            }
        }
        _ => panic!("unsupported depth"),
    }
    v
}

fn assert_close(actual: &[f32], expected: &[f32], lsb: f32) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected.iter()) {
        assert!((a - e).abs() <= lsb, "{} vs {} (>{})", a, e, lsb);
    }
}

#[test]
fn wav_int16_round_trip_within_one_lsb() {
    let enc = WavEncoder::new();
    let pcm = sample_pcm();
    let bytes = enc
        .encode(
            &pcm,
            &meta(),
            &WavOptions {
                depth: WavDepth::Int16,
                ..Default::default()
            },
        )
        .unwrap();
    let rw = parse_wav(&bytes);
    assert_eq!(rw.audio_format, 1);
    assert_eq!(rw.bits, 16);
    assert_eq!(rw.channels, 2);
    assert_eq!(rw.frame_count, pcm.frames);
    assert_eq!(rw.smpl_loop_start, 100);
    assert_eq!(rw.smpl_loop_end, 1800);
    assert_eq!(rw.info_title.as_deref(), Some("example track"));
    let got = frames_out(&rw);
    assert_close(&got, &pcm.data, 2.0 / 32767.0);
}

#[test]
fn wav_int24_round_trip_within_one_lsb() {
    let enc = WavEncoder::new();
    let pcm = sample_pcm();
    let bytes = enc
        .encode(
            &pcm,
            &meta(),
            &WavOptions {
                depth: WavDepth::Int24,
                ..Default::default()
            },
        )
        .unwrap();
    let rw = parse_wav(&bytes);
    assert_eq!(rw.audio_format, 1);
    assert_eq!(rw.bits, 24);
    let got = frames_out(&rw);
    assert_close(&got, &pcm.data, 2.0 / 8_388_607.0);
}

#[test]
fn wav_float32_round_trip_is_bit_close() {
    let enc = WavEncoder::new();
    let pcm = sample_pcm();
    let bytes = enc
        .encode(
            &pcm,
            &meta(),
            &WavOptions {
                depth: WavDepth::Float32,
                ..Default::default()
            },
        )
        .unwrap();
    let rw = parse_wav(&bytes);
    assert_eq!(rw.audio_format, 3);
    assert_eq!(rw.bits, 32);
    let got = frames_out(&rw);
    assert_eq!(got.len(), pcm.data.len());
    for (a, e) in got.iter().zip(pcm.data.iter()) {
        assert!((a - e).abs() < 1e-7 || a.to_bits() == e.to_bits());
    }
}

#[test]
fn wav_no_loop_meta_has_no_smpl_chunk() {
    let enc = WavEncoder::new();
    let bytes = enc
        .encode(
            &sample_pcm(),
            &EncodeMeta {
                title: None,
                loop_points: None,
            },
            &WavOptions::default(),
        )
        .unwrap();
    let rw = parse_wav(&bytes);
    assert_eq!(rw.smpl_loop_start, 0);
    assert_eq!(rw.smpl_loop_end, 0);
    assert_eq!(rw.info_title, None);
}

#[test]
fn wav_clip_count_is_reported() {
    let enc = WavEncoder::new();
    let mut pcm = sample_pcm();
    pcm.data[0] = 1.5;
    pcm.data[1] = -1.5;
    let _ = enc.encode(
        &pcm,
        &meta(),
        &WavOptions {
            depth: WavDepth::Int16,
            ..Default::default()
        },
    );
    assert_eq!(enc.last_clip_count(), 2);
}

#[test]
fn wav_deterministic_same_input_same_bytes() {
    let enc = WavEncoder::new();
    let a = enc
        .encode(&sample_pcm(), &meta(), &WavOptions::default())
        .unwrap();
    let b = enc
        .encode(&sample_pcm(), &meta(), &WavOptions::default())
        .unwrap();
    assert_eq!(a, b);
}
