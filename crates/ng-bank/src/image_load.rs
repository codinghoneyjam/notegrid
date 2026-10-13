// <META - FILE SUMMARY - NGSB reader: version and range validation over untrusted bytes, then a linear copy (C10.1 minimal copy, R03.R02).>

use crate::error::{BankError, FormatError};
use crate::image::{
    validate, validate_zone, LoopMode, PresetEntry, SampleEntry, SoundbankImage, ZoneEntry, HEADER_LEN,
    IMAGE_MAGIC, IMAGE_VERSION, PRESET_LEN, SAMPLE_LEN, ZONE_LEN,
};
use crate::limits::{MAX_IMAGE_BYTES, MAX_NAME_BYTES, MAX_PRESETS, MAX_SAMPLES};
use crate::riff::{read_exact, u16_at, u32_at};

/// Header offsets, named so the reader below reads like the C10.1 layout.
mod h {
    pub const MAGIC: usize = 0;
    pub const VERSION: usize = 4;
    pub const FLAGS: usize = 6;
    pub const NAME_LEN: usize = 8;
    pub const PRESET_COUNT: usize = 12;
    pub const ZONE_COUNT: usize = 16;
    pub const SAMPLE_COUNT: usize = 20;
    pub const PRESET_OFF: usize = 24;
    pub const ZONE_OFF: usize = 28;
    pub const SAMPLE_OFF: usize = 32;
    pub const STRING_OFF: usize = 36;
    pub const BLOB_OFF: usize = 40;
    pub const TOTAL_LEN: usize = 44;
}

/// Load an image. C10.1 asks for a minimal copy rather than a parse, so this validates the
/// header and every range first (R03.R02) and then walks the fixed-size tables once. It is not a
/// SoundFont parse and never touches the RIFF layer.
pub fn from_bytes(bytes: &[u8]) -> Result<SoundbankImage, BankError> {
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err(BankError::limit("image bytes", MAX_IMAGE_BYTES, bytes.len() as u64));
    }
    let hdr = read_header(bytes)?;
    let (preset_count, zone_count, sample_count) = counts(&hdr)?;
    let (preset_off, zone_off, sample_off, string_off, blob_off, total) = offsets(&hdr)?;
    check_total(bytes, total)?;
    let pcm = read_blob(bytes, blob_off, total)?;
    let strings = strings_at(bytes, string_off, blob_off)?;
    let presets = read_presets(bytes, preset_off, preset_count, &strings)?;
    let samples = read_samples(bytes, sample_off, sample_count, &strings)?;
    let zones = read_zones(bytes, zone_off, zone_count, &samples)?;
    let img = SoundbankImage { name: strings.bank.clone(), presets, zones, samples, pcm };
    validate(&img)?;
    Ok(img)
}

struct Header {
    name_len: u32,
    preset_count: u32,
    zone_count: u32,
    sample_count: u32,
    preset_off: u32,
    zone_off: u32,
    sample_off: u32,
    string_off: u32,
    blob_off: u32,
    total_len: u32,
}

fn read_header(bytes: &[u8]) -> Result<Header, BankError> {
    if bytes.len() < HEADER_LEN {
        return Err(FormatError::TooShort { need: HEADER_LEN, got: bytes.len() }.into());
    }
    let magic = read_exact(bytes, h::MAGIC, 4)?;
    if magic != IMAGE_MAGIC {
        return Err(FormatError::BadMagic {
            expected: "NGSB",
            got: [magic[0], magic[1], magic[2], magic[3]],
        }
        .into());
    }
    let version = u16_at(bytes, h::VERSION)?;
    if version != IMAGE_VERSION {
        return Err(FormatError::Limit { what: "image version", limit: IMAGE_VERSION as u64, got: version as u64 }.into());
    }
    let flags = u16_at(bytes, h::FLAGS)?;
    if flags != 0 {
        return Err(FormatError::Limit { what: "image flags", limit: 0, got: flags as u64 }.into());
    }
    Ok(Header {
        name_len: u32_at(bytes, h::NAME_LEN)?,
        preset_count: u32_at(bytes, h::PRESET_COUNT)?,
        zone_count: u32_at(bytes, h::ZONE_COUNT)?,
        sample_count: u32_at(bytes, h::SAMPLE_COUNT)?,
        preset_off: u32_at(bytes, h::PRESET_OFF)?,
        zone_off: u32_at(bytes, h::ZONE_OFF)?,
        sample_off: u32_at(bytes, h::SAMPLE_OFF)?,
        string_off: u32_at(bytes, h::STRING_OFF)?,
        blob_off: u32_at(bytes, h::BLOB_OFF)?,
        total_len: u32_at(bytes, h::TOTAL_LEN)?,
    })
}

type HeaderParts = (u32, u32, u32);

fn counts(h: &Header) -> Result<HeaderParts, BankError> {
    cap(h.preset_count, MAX_PRESETS, "presets")?;
    cap(h.zone_count, MAX_PRESETS * 64, "zones")?;
    cap(h.sample_count, MAX_SAMPLES, "samples")?;
    cap(h.name_len, MAX_NAME_BYTES, "bank name")?;
    Ok((h.preset_count, h.zone_count, h.sample_count))
}

type Offsets = (u32, u32, u32, u32, u32, u32);

fn offsets(h: &Header) -> Result<Offsets, BankError> {
    let (preset_off, zone_off, sample_off, string_off, blob_off, total) =
        (h.preset_off, h.zone_off, h.sample_off, h.string_off, h.blob_off, h.total_len);
    if preset_off != HEADER_LEN as u32 {
        return Err(FormatError::NotMonotonic { id: "preset_off", index: 0 }.into());
    }
    let expected_zone = crate::image::align_up(preset_off as usize + h.preset_count as usize * PRESET_LEN) as u32;
    let expected_sample = crate::image::align_up(expected_zone as usize + h.zone_count as usize * ZONE_LEN) as u32;
    if zone_off != expected_zone || sample_off != expected_sample {
        return Err(FormatError::NotMonotonic { id: "section offsets", index: 0 }.into());
    }
    if string_off < sample_off + h.sample_count * SAMPLE_LEN as u32 || blob_off < string_off {
        return Err(FormatError::NotMonotonic { id: "string/blob offsets", index: 0 }.into());
    }
    Ok((preset_off, zone_off, sample_off, string_off, blob_off, total))
}

fn check_total(bytes: &[u8], total: u32) -> Result<(), BankError> {
    if total as usize != bytes.len() {
        return Err(FormatError::ChunkSize { id: "NGSB", declared: total, available: bytes.len() }.into());
    }
    Ok(())
}

fn cap(value: u32, max: usize, what: &'static str) -> Result<(), BankError> {
    if value as usize > max {
        return Err(BankError::limit(what, max as u64, value as u64));
    }
    Ok(())
}

fn read_blob(bytes: &[u8], blob_off: u32, total: u32) -> Result<Vec<i16>, BankError> {
    let start = blob_off as usize;
    let len = total as usize - start.min(total as usize);
    let body = read_exact(bytes, start, len)?;
    if len % 2 != 0 {
        return Err(FormatError::ChunkSize { id: "NGSB blob", declared: u32::try_from(len).unwrap_or(u32::MAX), available: len.saturating_sub(1) }.into());
    }
    let mut pcm = Vec::with_capacity(len / 2);
    for pair in body.chunks_exact(2) {
        let mut it = pair.iter();
        let lo = it.next().copied().unwrap_or(0);
        let hi = it.next().copied().unwrap_or(0);
        pcm.push(i16::from_le_bytes([lo, hi]));
    }
    Ok(pcm)
}

/// The string table is the byte range between the end of the sample table and the start of the
/// PCM blob. Every name is a NUL-terminated run inside it.
struct Strings {
    raw: Vec<u8>,
    bank: String,
}

impl Strings {
    fn at(&self, off: u32, len: u16) -> String {
        let start = off as usize;
        if start > self.raw.len() {
            return String::new();
        }
        let tail = self.raw.get(start..).unwrap_or(&[]);
        let stop = tail.iter().position(|b| *b == 0).unwrap_or(tail.len()).min(len as usize);
        tail.get(..stop).unwrap_or(&[]).iter().map(|b| char::from(*b)).collect()
    }
}

fn strings_at(bytes: &[u8], string_off: u32, blob_off: u32) -> Result<Strings, BankError> {
    let start = string_off as usize;
    let end = (blob_off as usize).min(bytes.len());
    if start > end {
        return Err(FormatError::NotMonotonic { id: "string table", index: 0 }.into());
    }
    let raw = read_exact(bytes, start, end - start)?.to_vec();
    let bank = raw
        .iter()
        .position(|b| *b == 0)
        .and_then(|p| raw.get(..p))
        .unwrap_or(&[])
        .iter()
        .map(|b| char::from(*b))
        .collect();
    Ok(Strings { raw, bank })
}

fn read_presets(
    bytes: &[u8],
    off: u32,
    count: u32,
    strings: &Strings,
) -> Result<Vec<PresetEntry>, BankError> {
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        let o = off as usize + i * PRESET_LEN;
        let reserved = u32_at(bytes, o + 20)?;
        if reserved != 0 {
            return Err(FormatError::NotMonotonic { id: "preset reserved", index: i }.into());
        }
        let name_off = u32_at(bytes, o + 16)?;
        let name_len = u16_at(bytes, o + 6)?;
        out.push(PresetEntry {
            bank: u16_at(bytes, o)?,
            program: u16_at(bytes, o + 2)?,
            drum: byte(bytes, o + 4)? != 0,
            has_global_zone: byte(bytes, o + 5)? != 0,
            name: strings.at(name_off, name_len),
            zone_first: u32_at(bytes, o + 8)?,
            zone_count: u32_at(bytes, o + 12)?,
        });
    }
    Ok(out)
}

fn read_zones(
    bytes: &[u8],
    off: u32,
    count: u32,
    samples: &[SampleEntry],
) -> Result<Vec<ZoneEntry>, BankError> {
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        let o = off as usize + i * ZONE_LEN;
        let z = ZoneEntry {
            key_lo: byte(bytes, o)?,
            key_hi: byte(bytes, o + 1)?,
            vel_lo: byte(bytes, o + 2)?,
            vel_hi: byte(bytes, o + 3)?,
            root_key: byte(bytes, o + 4)?,
            loop_mode: LoopMode::from_u8(byte(bytes, o + 5)?)?,
            exclusive_class: byte(bytes, o + 6)?,
            sample_idx: u32_at(bytes, o + 8)?,
            tune_cents: i16::from_le_bytes([byte(bytes, o + 12)?, byte(bytes, o + 13)?]),
            scale_tuning_cents: u16_at(bytes, o + 14)?,
            atten_cb: u16_at(bytes, o + 16)?,
            pan: i16::from_le_bytes([byte(bytes, o + 18)?, byte(bytes, o + 19)?]),
            reverb_send: u16_at(bytes, o + 20)?,
            env_sustain_cb: u16_at(bytes, o + 24)?,
            start: u32_at(bytes, o + 28)?,
            end: u32_at(bytes, o + 32)?,
            loop_start: u32_at(bytes, o + 36)?,
            loop_end: u32_at(bytes, o + 40)?,
            env_delay_s: f32::from_le_bytes(quad(bytes, o + 44)?),
            env_attack_s: f32::from_le_bytes(quad(bytes, o + 48)?),
            env_hold_s: f32::from_le_bytes(quad(bytes, o + 52)?),
            env_decay_s: f32::from_le_bytes(quad(bytes, o + 56)?),
            env_release_s: f32::from_le_bytes(quad(bytes, o + 60)?),
        };
        validate_zone(&z, i, samples)?;
        out.push(z);
    }
    Ok(out)
}

fn read_samples(
    bytes: &[u8],
    off: u32,
    count: u32,
    strings: &Strings,
) -> Result<Vec<SampleEntry>, BankError> {
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        let o = off as usize + i * SAMPLE_LEN;
        out.push(SampleEntry {
            start: u32_at(bytes, o)?,
            end: u32_at(bytes, o + 4)?,
            rate: u32_at(bytes, o + 8)?,
            name: strings.at(u32_at(bytes, o + 12)?, MAX_NAME_BYTES as u16),
            original_key: byte(bytes, o + 16)?,
            pitch_correction_cents: byte(bytes, o + 17)? as i8,
            link: u16_at(bytes, o + 20)?,
            sample_type: u16_at(bytes, o + 22)?,
        });
    }
    Ok(out)
}

fn byte(bytes: &[u8], off: usize) -> Result<u8, FormatError> {
    read_exact(bytes, off, 1)?.first().copied().ok_or(FormatError::TooShort { need: off + 1, got: bytes.len() })
}

fn quad(bytes: &[u8], off: usize) -> Result<[u8; 4], FormatError> {
    let b = read_exact(bytes, off, 4)?;
    Ok([b[0], b[1], b[2], b[3]])
}