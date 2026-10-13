// <META - FILE SUMMARY - SF2 hydra record layout (SF2 2.01 sec 7.2-7.10) and the generator operators R03.DESIGN flattens.>

use crate::error::FormatError;
use crate::riff::{fixed_name, i16_at, read_exact, u16_at, u32_at};

/// phdr / inst / shdr record sizes, fixed by SF2 2.01 sec 7.2, 7.6, 7.10.
pub const PHDR_LEN: usize = 38;
pub const PBAG_LEN: usize = 4;
pub const PMOD_LEN: usize = 10;
pub const PGEN_LEN: usize = 4;
pub const INST_LEN: usize = 22;
pub const IBAG_LEN: usize = 4;
pub const IMOD_LEN: usize = 10;
pub const IGEN_LEN: usize = 4;
pub const SHDR_LEN: usize = 46;

/// A preset header. The last record in phdr is the EOP terminal and is never read as a preset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresetHeader {
    pub name: String,
    pub program: u16,
    pub bank: u16,
    pub bag_ndx: u16,
}

impl PresetHeader {
    pub fn parse(data: &[u8], off: usize) -> Result<Self, FormatError> {
        Ok(Self {
            name: fixed_name(data, off, 20)?,
            program: u16_at(data, off + 20)?,
            bank: u16_at(data, off + 22)?,
            bag_ndx: u16_at(data, off + 24)?,
        })
    }
}

/// An instrument header. The last record in inst is the EOI terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstrumentHeader {
    pub name: String,
    pub bag_ndx: u16,
}

impl InstrumentHeader {
    pub fn parse(data: &[u8], off: usize) -> Result<Self, FormatError> {
        Ok(Self { name: fixed_name(data, off, 20)?, bag_ndx: u16_at(data, off + 20)? })
    }
}

/// A sample header. The last record in shdr is the EOS terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleHeader {
    pub name: String,
    pub start: u32,
    pub end: u32,
    pub loop_start: u32,
    pub loop_end: u32,
    pub rate: u32,
    pub original_key: u8,
    pub pitch_correction: i8,
    pub link: u16,
    pub sample_type: u16,
}

impl SampleHeader {
    pub fn parse(data: &[u8], off: usize) -> Result<Self, FormatError> {
        Ok(Self {
            name: fixed_name(data, off, 20)?,
            start: u32_at(data, off + 20)?,
            end: u32_at(data, off + 24)?,
            loop_start: u32_at(data, off + 28)?,
            loop_end: u32_at(data, off + 32)?,
            rate: u32_at(data, off + 36)?,
            original_key: *read_exact(data, off + 40, 1)?
                .first()
                .ok_or(FormatError::TooShort { need: off + 41, got: data.len() })?,
            pitch_correction: *read_exact(data, off + 41, 1)?
                .first()
                .ok_or(FormatError::TooShort { need: off + 42, got: data.len() })? as i8,
            link: u16_at(data, off + 42)?,
            sample_type: u16_at(data, off + 44)?,
        })
    }
}

/// A pbag / ibag record: where this zone's generator (and modulator) list starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bag {
    pub gen_ndx: u16,
    pub mod_ndx: u16,
}

impl Bag {
    pub fn parse(data: &[u8], off: usize) -> Result<Self, FormatError> {
        Ok(Self { gen_ndx: u16_at(data, off)?, mod_ndx: u16_at(data, off + 2)? })
    }
}

/// A pgen / igen record: operator plus a signed 16-bit amount.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gen {
    pub op: u16,
    pub amount: i16,
}

impl Gen {
    pub fn parse(data: &[u8], off: usize) -> Result<Self, FormatError> {
        Ok(Self { op: u16_at(data, off)?, amount: i16_at(data, off + 2)? })
    }
}

/// Validate the length of a fixed-record chunk and return how many records it holds. The count
/// comes from the chunk length, never from a declared field, so a truncated chunk cannot ask for
/// more records than it carries (R03.R01). No allocation happens here at all.
pub fn record_count(data: &[u8], id: &'static str, record: usize) -> Result<usize, FormatError> {
    if record == 0 || data.len() % record != 0 {
        return Err(FormatError::RecordSize { id, len: data.len(), record });
    }
    let count = data.len() / record;
    if count > crate::limits::MAX_RECORDS {
        return Err(FormatError::Limit {
            what: id,
            limit: crate::limits::MAX_RECORDS as u64,
            got: count as u64,
        });
    }
    Ok(count)
}

/// Byte offset of record `i` in a fixed-record chunk, or a format error if it does not exist.
pub fn record_off(data: &[u8], id: &'static str, record: usize, i: usize) -> Result<usize, FormatError> {
    if i >= data.len() / record.max(1) {
        return Err(FormatError::IndexOutOfRange {
            id,
            index: u32::try_from(i).unwrap_or(u32::MAX),
            count: data.len() / record.max(1),
        });
    }
    i.checked_mul(record).ok_or(FormatError::IntegerOverflow("record offset"))
}

/// Generator operators used by R03.DESIGN v1. The numbers are the SF2 2.01 sec 8.1.2 table.
pub mod gen {
    pub const START_ADDRS_OFFSET: u16 = 0;
    pub const END_ADDRS_OFFSET: u16 = 1;
    pub const STARTLOOP_ADDRS_OFFSET: u16 = 2;
    pub const ENDLOOP_ADDRS_OFFSET: u16 = 3;
    pub const START_ADDRS_COARSE_OFFSET: u16 = 4;
    pub const END_ADDRS_COARSE_OFFSET: u16 = 12;
    pub const REVERB_EFFECTS_SEND: u16 = 16;
    pub const PAN: u16 = 17;
    pub const DELAY_VOL_ENV: u16 = 22;
    pub const ATTACK_VOL_ENV: u16 = 23;
    pub const HOLD_VOL_ENV: u16 = 24;
    pub const DECAY_VOL_ENV: u16 = 25;
    pub const SUSTAIN_VOL_ENV: u16 = 26;
    pub const RELEASE_VOL_ENV: u16 = 27;
    pub const INSTRUMENT: u16 = 31;
    pub const KEY_RANGE: u16 = 33;
    pub const VEL_RANGE: u16 = 34;
    pub const STARTLOOP_ADDRS_COARSE_OFFSET: u16 = 35;
    pub const KEYNUM: u16 = 36;
    pub const INITIAL_ATTENUATION: u16 = 38;
    pub const ENDLOOP_ADDRS_COARSE_OFFSET: u16 = 40;
    pub const COARSE_TUNE: u16 = 41;
    pub const FINE_TUNE: u16 = 42;
    pub const SAMPLE_ID: u16 = 43;
    pub const SAMPLE_MODES: u16 = 44;
    pub const SCALE_TUNING: u16 = 46;
    pub const EXCLUSIVE_CLASS: u16 = 47;
    pub const OVERRIDING_ROOT_KEY: u16 = 48;

    /// SF2 2.01 sec 8.5: these must be ignored at the preset level because they are absolute
    /// addresses or identifiers rather than relative offsets.
    pub fn not_allowed_at_preset_level(op: u16) -> bool {
        matches!(
            op,
            START_ADDRS_OFFSET
                | END_ADDRS_OFFSET
                | STARTLOOP_ADDRS_OFFSET
                | ENDLOOP_ADDRS_OFFSET
                | START_ADDRS_COARSE_OFFSET
                | END_ADDRS_COARSE_OFFSET
                | STARTLOOP_ADDRS_COARSE_OFFSET
                | KEYNUM
                | ENDLOOP_ADDRS_COARSE_OFFSET
                | SAMPLE_MODES
                | EXCLUSIVE_CLASS
                | OVERRIDING_ROOT_KEY
        )
    }

    /// SF2 2.01 sec 8.1.2 absolute ranges, as (min, max).
    pub fn absolute_range(op: u16) -> Option<(i32, i32)> {
        Some(match op {
            INITIAL_ATTENUATION => (0, 1440),
            REVERB_EFFECTS_SEND => (0, 1000),
            PAN => (-500, 500),
            DELAY_VOL_ENV | HOLD_VOL_ENV => (-12000, 5000),
            ATTACK_VOL_ENV | DECAY_VOL_ENV | RELEASE_VOL_ENV => (-12000, 8000),
            SUSTAIN_VOL_ENV => (0, 1440),
            KEY_RANGE | VEL_RANGE => (0, 255),
            COARSE_TUNE => (-120, 120),
            FINE_TUNE => (-99, 99),
            SCALE_TUNING => (0, 1200),
            EXCLUSIVE_CLASS => (0, 127),
            OVERRIDING_ROOT_KEY => (0, 127),
            _ => return None,
        })
    }

    /// SF2 2.01 sec 8.1.2 defaults.
    pub fn default_amount(op: u16) -> Option<i32> {
        Some(match op {
            INITIAL_ATTENUATION | SUSTAIN_VOL_ENV | REVERB_EFFECTS_SEND => 0,
            PAN => 0, // centre (SF2 2.01 sec 8.1.2: pan -500 left, +500 right, default 0)
            DELAY_VOL_ENV | ATTACK_VOL_ENV | HOLD_VOL_ENV | DECAY_VOL_ENV | RELEASE_VOL_ENV => -12000,
            KEY_RANGE | VEL_RANGE | COARSE_TUNE | FINE_TUNE => 0,
            SCALE_TUNING => 100, // cents per key
            EXCLUSIVE_CLASS | OVERRIDING_ROOT_KEY => 0,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phdr_bytes() -> Vec<u8> {
        let mut v = vec![0u8; PHDR_LEN];
        v[..5].copy_from_slice(b"Piano");
        v[20..22].copy_from_slice(&0u16.to_le_bytes());
        v[22..24].copy_from_slice(&0u16.to_le_bytes());
        v[24..26].copy_from_slice(&1u16.to_le_bytes());
        v
    }

    #[test]
    fn phdr_fields_land_on_the_spec_offsets() {
        let b = phdr_bytes();
        let p = PresetHeader::parse(&b, 0).expect("valid phdr record");
        assert_eq!(p.name, "Piano");
        assert_eq!(p.bag_ndx, 1);
    }

    #[test]
    fn shdr_fields_land_on_the_spec_offsets() {
        let mut v = vec![0u8; SHDR_LEN];
        v[..5].copy_from_slice(b"sine1");
        v[20..24].copy_from_slice(&10u32.to_le_bytes());
        v[24..28].copy_from_slice(&1010u32.to_le_bytes());
        v[36..40].copy_from_slice(&44100u32.to_le_bytes());
        v[40] = 60;
        v[41] = 0xff; // -1 cents
        v[44..46].copy_from_slice(&1u16.to_le_bytes());
        let s = SampleHeader::parse(&v, 0).expect("valid shdr record");
        assert_eq!((s.start, s.end, s.rate, s.original_key, s.sample_type), (10, 1010, 44100, 60, 1));
        assert_eq!(s.pitch_correction, -1);
    }

    #[test]
    fn split_records_rejects_a_ragged_chunk() {
        let e = record_count(&[0u8; 7], "shdr", SHDR_LEN).err();
        assert!(matches!(e, Some(FormatError::RecordSize { id: "shdr", .. })));
    }

    #[test]
    fn split_records_counts_from_the_chunk_not_a_field() {
        assert_eq!(record_count(&[0u8; SHDR_LEN * 3], "shdr", SHDR_LEN), Ok(3));
        assert!(matches!(
            record_off(&[0u8; SHDR_LEN * 2], "shdr", SHDR_LEN, 2),
            Err(FormatError::IndexOutOfRange { id: "shdr", .. })
        ));
    }
}