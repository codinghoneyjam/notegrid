// <META - FILE SUMMARY - SoundbankImage types and the NGSB writer: versioned little-endian tables over an aligned sample blob (C10.1).>

use crate::error::{BankError, FormatError};
use crate::limits::{IMAGE_ALIGN, MAX_IMAGE_BYTES, MAX_NAME_BYTES, MAX_PRESETS, MAX_SAMPLES};

pub const IMAGE_MAGIC: [u8; 4] = *b"NGSB";
pub const IMAGE_VERSION: u16 = 1;

pub const HEADER_LEN: usize = 48;
pub const PRESET_LEN: usize = 24;
pub const ZONE_LEN: usize = 72;
pub const SAMPLE_LEN: usize = 32;

/// SF2 sampleModes (generator 44). v1 maps the three values the format defines; 2 is reserved
/// by SF2 2.01 sec 8.1.2 and reads as no loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LoopMode {
    #[default]
    None = 0,
    Continuous = 1,
    UntilRelease = 3,
}

impl LoopMode {
    pub fn from_generator(amount: i16) -> Self {
        match amount {
            1 => Self::Continuous,
            3 => Self::UntilRelease,
            _ => Self::None,
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(v: u8) -> Result<Self, FormatError> {
        match v {
            0 => Ok(Self::None),
            1 => Ok(Self::Continuous),
            3 => Ok(Self::UntilRelease),
            _ => Err(FormatError::Limit { what: "loop_mode", limit: 3, got: v as u64 }),
        }
    }
}

/// One preset: the MIDI (bank, program) it answers to and its span in the zone table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresetEntry {
    pub bank: u16,
    pub program: u16,
    /// A drum preset is the GM percussion bank (C0.6: bank 128).
    pub drum: bool,
    /// The preset carried a global (first) zone (SF2 2.01 sec 7.3).
    pub has_global_zone: bool,
    pub name: String,
    pub zone_first: u32,
    pub zone_count: u32,
}

/// One resolved zone: preset and instrument generators already merged (R03.DESIGN).
///
/// `start` / `end` / `loop_start` / `loop_end` are absolute frame indices into
/// `SoundbankImage::pcm`, not offsets inside the source sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoneEntry {
    pub key_lo: u8,
    pub key_hi: u8,
    pub vel_lo: u8,
    pub vel_hi: u8,
    pub root_key: u8,
    pub loop_mode: LoopMode,
    pub exclusive_class: u8,
    pub sample_idx: u32,
    /// coarseTune * 100 + fineTune (SF2 2.01 sec 8.1.2 ranges -120..120 and -99..99).
    pub tune_cents: i16,
    /// scaleTuning: cents of pitch shift per key, default 100.
    pub scale_tuning_cents: u16,
    pub atten_cb: u16,
    /// SF2 pan generator: -500 far left, 0 centre, +500 far right (SF2 2.01 sec 8.1.2). This is
    /// not the C0.4 lane pan, which is 0..127 with 64 at the centre.
    pub pan: i16,
    pub reverb_send: u16,
    /// SF2 sustainVolEnv generator 37 in centibels (0 = attack peak, 1440 = silent).
    pub env_sustain_cb: u16,
    pub start: u32,
    pub end: u32,
    pub loop_start: u32,
    pub loop_end: u32,
    pub env_delay_s: f32,
    pub env_attack_s: f32,
    pub env_hold_s: f32,
    pub env_decay_s: f32,
    pub env_release_s: f32,
}

/// One sample, carved out of `smpl` and relocated into the image blob.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleEntry {
    pub start: u32,
    pub end: u32,
    pub rate: u32,
    pub name: String,
    pub original_key: u8,
    pub pitch_correction_cents: i8,
    pub link: u16,
    pub sample_type: u16,
}

/// The load-cheap form of a soundbank: fixed-size tables plus one contiguous PCM blob. Built
/// once off the audio thread (R-RT-5) and copied into each engine (C10.2).
#[derive(Clone, Debug, PartialEq)]
pub struct SoundbankImage {
    pub name: String,
    pub presets: Vec<PresetEntry>,
    pub zones: Vec<ZoneEntry>,
    pub samples: Vec<SampleEntry>,
    pub pcm: Vec<i16>,
}

pub(crate) fn align_up(v: usize) -> usize {
    v.div_ceil(IMAGE_ALIGN) * IMAGE_ALIGN
}

/// Checks that do not depend on how the image was produced. `from_bytes` runs them on untrusted
/// input before any structure is handed out (R03.R02).
pub fn validate(img: &SoundbankImage) -> Result<(), BankError> {
    if img.presets.len() > MAX_PRESETS {
        return Err(BankError::limit("presets", MAX_PRESETS as u64, img.presets.len() as u64));
    }
    if img.samples.len() > MAX_SAMPLES {
        return Err(BankError::limit("samples", MAX_SAMPLES as u64, img.samples.len() as u64));
    }
    if img.name.len() > MAX_NAME_BYTES {
        return Err(BankError::limit("bank name", MAX_NAME_BYTES as u64, img.name.len() as u64));
    }
    let frames = img.pcm.len() as u64;
    for s in &img.samples {
        if s.rate == 0 || s.end < s.start || s.end as u64 > frames {
            return Err(FormatError::BadSampleRange {
                index: 0,
                start: s.start,
                end: s.end,
                frames: frames as usize,
            }
            .into());
        }
    }
    for (i, z) in img.zones.iter().enumerate() {
        validate_zone(z, i, &img.samples)?;
    }
    for p in &img.presets {
        let span = p.zone_first as u64 + p.zone_count as u64;
        if span > img.zones.len() as u64 {
            return Err(FormatError::IndexOutOfRange {
                id: "preset zones",
                index: p.zone_first,
                count: img.zones.len(),
            }
            .into());
        }
    }
    Ok(())
}

pub(crate) fn validate_zone(z: &ZoneEntry, index: usize, samples: &[SampleEntry]) -> Result<(), BankError> {
    if z.key_lo > z.key_hi {
        return Err(FormatError::Limit { what: "zone key_lo", limit: z.key_hi as u64, got: z.key_lo as u64 }.into());
    }
    if z.vel_lo > z.vel_hi {
        return Err(FormatError::Limit { what: "zone vel_lo", limit: z.vel_hi as u64, got: z.vel_lo as u64 }.into());
    }
    let sample = samples.get(z.sample_idx as usize).ok_or(FormatError::IndexOutOfRange {
        id: "sample_idx",
        index: z.sample_idx,
        count: samples.len(),
    })?;
    if z.start < sample.start || z.end > sample.end {
        return Err(FormatError::BadSampleRange {
            index,
            start: z.start,
            end: z.end,
            frames: sample.end as usize,
        }
        .into());
    }
    let ordered = z.start <= z.end && z.start <= z.loop_start && z.loop_start <= z.loop_end && z.loop_end <= z.end;
    if !ordered {
        return Err(FormatError::BadSampleRange {
            index,
            start: z.start,
            end: z.end,
            frames: z.end as usize,
        }
        .into());
    }
    Ok(())
}

/// Serialise an image. The same `SoundbankImage` always produces the same bytes (R03.R02): every
/// field is written at a computed offset, every name is sanitised to printable ASCII, and no
/// iteration order is involved.
pub fn to_bytes(img: &SoundbankImage) -> Result<Vec<u8>, BankError> {
    validate(img)?;
    let (strings, preset_names, sample_names) = pack_strings(img);
    let preset_off = HEADER_LEN;
    let zone_off = align_up(preset_off + img.presets.len() * PRESET_LEN);
    let sample_off = align_up(zone_off + img.zones.len() * ZONE_LEN);
    let string_off = align_up(sample_off + img.samples.len() * SAMPLE_LEN);
    let blob_off = align_up(string_off + strings.len());
    let pcm_bytes = img.pcm.len().checked_mul(2).ok_or(FormatError::IntegerOverflow("pcm bytes"))?;
    let total = blob_off.checked_add(pcm_bytes).ok_or(FormatError::IntegerOverflow("image length"))?;
    if total as u64 > MAX_IMAGE_BYTES {
        return Err(BankError::limit("image bytes", MAX_IMAGE_BYTES, total as u64));
    }
    let mut out = vec![0u8; total];
    put(&mut out, 0, &IMAGE_MAGIC);
    put(&mut out, 4, &IMAGE_VERSION.to_le_bytes());
    put(&mut out, 6, &0u16.to_le_bytes());
    put(&mut out, 8, &(img.name.len() as u32).to_le_bytes());
    put(&mut out, 12, &(img.presets.len() as u32).to_le_bytes());
    put(&mut out, 16, &(img.zones.len() as u32).to_le_bytes());
    put(&mut out, 20, &(img.samples.len() as u32).to_le_bytes());
    put(&mut out, 24, &(preset_off as u32).to_le_bytes());
    put(&mut out, 28, &(zone_off as u32).to_le_bytes());
    put(&mut out, 32, &(sample_off as u32).to_le_bytes());
    put(&mut out, 36, &(string_off as u32).to_le_bytes());
    put(&mut out, 40, &(blob_off as u32).to_le_bytes());
    put(&mut out, 44, &(total as u32).to_le_bytes());
    for (i, p) in img.presets.iter().enumerate() {
        let o = preset_off + i * PRESET_LEN;
        put(&mut out, o, &p.bank.to_le_bytes());
        put(&mut out, o + 2, &p.program.to_le_bytes());
        put(&mut out, o + 4, &[u8::from(p.drum), u8::from(p.has_global_zone)]);
        put(&mut out, o + 6, &(p.name.len() as u16).to_le_bytes());
        put(&mut out, o + 8, &p.zone_first.to_le_bytes());
        put(&mut out, o + 12, &p.zone_count.to_le_bytes());
        put(&mut out, o + 16, &preset_names.get(i).copied().unwrap_or(0).to_le_bytes());
        put(&mut out, o + 20, &0u32.to_le_bytes());
    }
    for (i, z) in img.zones.iter().enumerate() {
        let o = zone_off + i * ZONE_LEN;
        put(&mut out, o, &[z.key_lo, z.key_hi, z.vel_lo, z.vel_hi, z.root_key, z.loop_mode.to_u8(), z.exclusive_class, 0]);
        put(&mut out, o + 8, &z.sample_idx.to_le_bytes());
        put(&mut out, o + 12, &z.tune_cents.to_le_bytes());
        put(&mut out, o + 14, &z.scale_tuning_cents.to_le_bytes());
        put(&mut out, o + 16, &z.atten_cb.to_le_bytes());
        put(&mut out, o + 18, &z.pan.to_le_bytes());
        put(&mut out, o + 20, &z.reverb_send.to_le_bytes());
        put(&mut out, o + 24, &z.env_sustain_cb.to_le_bytes());
        put(&mut out, o + 28, &z.start.to_le_bytes());
        put(&mut out, o + 32, &z.end.to_le_bytes());
        put(&mut out, o + 36, &z.loop_start.to_le_bytes());
        put(&mut out, o + 40, &z.loop_end.to_le_bytes());
        put(&mut out, o + 44, &z.env_delay_s.to_le_bytes());
        put(&mut out, o + 48, &z.env_attack_s.to_le_bytes());
        put(&mut out, o + 52, &z.env_hold_s.to_le_bytes());
        put(&mut out, o + 56, &z.env_decay_s.to_le_bytes());
        put(&mut out, o + 60, &z.env_release_s.to_le_bytes());
    }
    for (i, s) in img.samples.iter().enumerate() {
        let o = sample_off + i * SAMPLE_LEN;
        put(&mut out, o, &s.start.to_le_bytes());
        put(&mut out, o + 4, &s.end.to_le_bytes());
        put(&mut out, o + 8, &s.rate.to_le_bytes());
        put(&mut out, o + 12, &sample_names.get(i).copied().unwrap_or(0).to_le_bytes());
        put(&mut out, o + 16, &[s.original_key, s.pitch_correction_cents as u8]);
        put(&mut out, o + 18, &0u16.to_le_bytes());
        put(&mut out, o + 20, &s.link.to_le_bytes());
        put(&mut out, o + 22, &s.sample_type.to_le_bytes());
    }
    put(&mut out, string_off, &strings);
    for (i, s) in img.pcm.iter().enumerate() {
        put(&mut out, blob_off + i * 2, &s.to_le_bytes());
    }
    Ok(out)
}

/// Concatenate every name into one NUL-terminated table and return the per-entry offsets. Names
/// are sanitised to printable ASCII so the image can never carry an artifact that breaks the
/// 100% ASCII rule, whatever the source bank contained.
fn pack_strings(img: &SoundbankImage) -> (Vec<u8>, Vec<u32>, Vec<u32>) {
    let mut strings: Vec<u8> = Vec::new();
    let mut offsets = Vec::new();
    let mut push = |s: &str| -> u32 {
        let at = strings.len() as u32;
        offsets.push(at);
        for b in s.bytes().chain([0u8]) {
            strings.push(if (0x20..0x7f).contains(&b) { b } else { b'_' });
        }
        at
    };
    let _bank_name_at = push(&img.name);
    let preset_names: Vec<u32> = img.presets.iter().map(|p| push(&p.name)).collect();
    let sample_names: Vec<u32> = img.samples.iter().map(|s| push(&s.name)).collect();
    (strings, preset_names, sample_names)
}

pub(crate) fn put(out: &mut [u8], off: usize, src: &[u8]) {
    if let Some(dst) = out.get_mut(off..off.saturating_add(src.len())) {
        dst.copy_from_slice(src);
    }
}