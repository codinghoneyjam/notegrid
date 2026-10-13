// <META - FILE SUMMARY - RIFF sfbk parser: INFO, sdta, pdta (SF2 2.01 sec 4). Every read is bounds-checked (R03.R01).>

use crate::error::{BankError, FormatError};
use crate::limits::{MAX_PRESETS, MAX_SAMPLES, MAX_NAME_BYTES, MAX_SOURCE_BYTES};
use crate::record::{
    record_count, record_off, Bag, Gen, InstrumentHeader, PresetHeader, SampleHeader, IBAG_LEN,
    IGEN_LEN, IMOD_LEN, INST_LEN, PBAG_LEN, PGEN_LEN, PHDR_LEN, PMOD_LEN, SHDR_LEN,
};
use crate::riff::{find, find_list, fixed_name, u16_at, u32_at};

/// Sample-type bit marking an SF3 Ogg-Vorbis sample (SF3 convention: `shdr` offsets become byte
/// offsets into the stream). Not decodable here; see `parse_sf2`.
pub const SF3_SAMPLE_BIT: u16 = 0x0010;
/// SF2 2.01 sec 4.5 ROM sample types (the 0x8000 family): the audio lives in a ROM, not in `smpl`.
pub const ROM_SAMPLE_BIT: u16 = 0x8000;

/// The hydra, decoded but not yet flattened (R03.DESIGN keeps this an internal step).
pub struct Sf2File {
    pub name: String,
    pub version_major: u16,
    pub version_minor: u16,
    pub presets: Vec<PresetHeader>,
    pub preset_bags: Vec<Bag>,
    pub preset_gens: Vec<Gen>,
    pub instruments: Vec<InstrumentHeader>,
    pub inst_bags: Vec<Bag>,
    pub inst_gens: Vec<Gen>,
    pub samples: Vec<SampleHeader>,
    /// The whole `smpl` chunk as signed 16-bit little-endian frames (SF2 2.01 sec 6.1).
    pub pcm: Vec<i16>,
}

/// Parse a SoundFont into the hydra. Rejects truncation, bad offsets and cycles (R03.R01).
///
/// SF3 (Ogg-Vorbis samples) and ROM samples are reported as `E_BANK_UNSUPPORTED`: v1 of this
/// crate decodes nothing but the 16-bit `smpl` chunk, and R03.RISK/spike:SP-4 owns the SF3
/// route (it needs a pure-Rust Vorbis decoder, which would be a new third-party dependency).
pub fn parse_sf2(bytes: &[u8]) -> Result<Sf2File, BankError> {
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(BankError::limit("source bytes", MAX_SOURCE_BYTES, bytes.len() as u64));
    }
    check_riff(bytes)?;
    // Skip the 12-byte RIFF header; chunks start at offset 12.
    let body = &bytes[12..];
    let info = find_list(body, b"INFO").ok_or(FormatError::MissingList("INFO"))?;
    let (version_major, version_minor) = read_ifil(info)?;
    reject_unsupported_version(version_major, version_minor)?;
    let sdta = find_list(body, b"sdta").ok_or(FormatError::MissingList("sdta"))?;
    let pdta = find_list(body, b"pdta").ok_or(FormatError::MissingList("pdta"))?;
    let pcm = read_smpl(sdta)?;
    let mut f = read_pdta(pdta, pcm)?;
    f.name = read_inam(info);
    f.version_major = version_major;
    f.version_minor = version_minor;
    Ok(f)
}

fn check_riff(bytes: &[u8]) -> Result<(), FormatError> {
    if bytes.len() < 12 {
        return Err(FormatError::TooShort { need: 12, got: bytes.len() });
    }
    let id = [bytes[0], bytes[1], bytes[2], bytes[3]];
    if &id != b"RIFF" {
        return Err(FormatError::BadMagic { expected: "RIFF", got: id });
    }
    let form = [bytes[8], bytes[9], bytes[10], bytes[11]];
    if &form != b"sfbk" {
        return Err(FormatError::BadMagic { expected: "sfbk", got: form });
    }
    // Redundant length check of SF2 2.01 sec 3.3. A truncated file fails here before any
    // sub-chunk is trusted, which is what makes every proper prefix of a valid bank an error.
    let declared = u32_at(bytes, 4)? as usize;
    let total = declared.checked_add(8).ok_or(FormatError::IntegerOverflow("RIFF size"))?;
    if bytes.len() < total {
        return Err(FormatError::ChunkSize {
            id: "RIFF",
            declared: u32::try_from(declared).unwrap_or(u32::MAX),
            available: bytes.len().saturating_sub(8),
        });
    }
    Ok(())
}

fn read_ifil(info: &[u8]) -> Result<(u16, u16), FormatError> {
    let body = find(info, b"ifil").ok_or(FormatError::MissingChunk("ifil"))?;
    if body.len() != 4 {
        return Err(FormatError::ChunkSize {
            id: "ifil",
            declared: u32::try_from(body.len()).unwrap_or(u32::MAX),
            available: body.len(),
        });
    }
    Ok((u16_at(body, 0)?, u16_at(body, 2)?))
}

fn reject_unsupported_version(major: u16, minor: u16) -> Result<(), BankError> {
    if major < 2 {
        return Err(BankError::Unsupported(format!("ifil version {major}.{minor} predates SF2")));
    }
    if major > 2 {
        return Err(BankError::Unsupported(format!(
            "ifil version {major}.{minor} is SF3 or newer; Ogg-Vorbis sample decode is not implemented (SP-4)"
        )));
    }
    Ok(())
}

fn read_smpl(sdta: &[u8]) -> Result<Vec<i16>, FormatError> {
    let Some(body) = find(sdta, b"smpl") else { return Ok(Vec::new()) };
    // SF2 2.01 sec 6.1: the chunk holds an even number of bytes. A dangling byte is structural
    // damage, not something to round away.
    if body.len() % 2 != 0 {
        return Err(FormatError::ChunkSize {
            id: "smpl",
            declared: u32::try_from(body.len()).unwrap_or(u32::MAX),
            available: body.len().saturating_sub(1),
        });
    }
    let mut pcm = Vec::with_capacity(body.len() / 2);
    for pair in body.chunks_exact(2) {
        let mut it = pair.iter();
        let lo = it.next().copied().unwrap_or(0);
        let hi = it.next().copied().unwrap_or(0);
        pcm.push(i16::from_le_bytes([lo, hi]));
    }
    Ok(pcm)
}

fn read_pdta(pdta: &[u8], pcm: Vec<i16>) -> Result<Sf2File, BankError> {
    // pmod/imod are validated for shape only. R03.DESIGN lists generators, not modulators, for
    // v1, so their content is not flattened; the terminal record still has to be present.
    record_count(need(pdta, "pmod")?, "pmod", PMOD_LEN)?;
    record_count(need(pdta, "imod")?, "imod", IMOD_LEN)?;

    let mut presets = read_headers(need(pdta, "phdr")?, "phdr", PHDR_LEN, PresetHeader::parse, MAX_PRESETS)?;
    let mut instruments = read_headers(need(pdta, "inst")?, "inst", INST_LEN, InstrumentHeader::parse, MAX_PRESETS)?;
    let samples = read_samples(need(pdta, "shdr")?, pcm.len() as u32)?;
    let preset_bags = read_bags(need(pdta, "pbag")?, "pbag", PBAG_LEN)?;
    let preset_gens = read_gens(need(pdta, "pgen")?, "pgen", PGEN_LEN)?;
    let inst_bags = read_bags(need(pdta, "ibag")?, "ibag", IBAG_LEN)?;
    let inst_gens = read_gens(need(pdta, "igen")?, "igen", IGEN_LEN)?;

    check_index_chain(&presets.iter().map(|p| p.bag_ndx).collect::<Vec<_>>(), &preset_bags, preset_gens.len(), "phdr", "pbag")?;
    check_index_chain(&instruments.iter().map(|i| i.bag_ndx).collect::<Vec<_>>(), &inst_bags, inst_gens.len(), "inst", "ibag")?;
    // The terminal records only carried the closing indices; they are not presets/instruments.
    presets.pop();
    instruments.pop();

    Ok(Sf2File {
        name: String::new(),
        version_major: 0,
        version_minor: 0,
        presets,
        preset_bags,
        preset_gens,
        instruments,
        inst_bags,
        inst_gens,
        samples,
        pcm,
    })
}

fn need<'a>(pdta: &'a [u8], id: &'static str) -> Result<&'a [u8], FormatError> {
    find(pdta, id.as_bytes()).ok_or(FormatError::MissingChunk(id))
}

/// Read a header table including its terminal sentinel, which is checked and then dropped.
fn read_headers<T>(
    data: &[u8],
    id: &'static str,
    record: usize,
    parse: fn(&[u8], usize) -> Result<T, FormatError>,
    cap: usize,
) -> Result<Vec<T>, FormatError> {
    let count = record_count(data, id, record)?;
    if count < 2 {
        return Err(FormatError::MissingTerminal(id));
    }
    if count - 1 > cap {
        return Err(FormatError::Limit { what: id, limit: cap as u64, got: (count - 1) as u64 });
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        out.push(parse(data, record_off(data, id, record, i)?)?);
    }
    Ok(out)
}

/// The structural checks of SF2 2.01 sec 7.2/7.3 applied to a header -> bag -> generator chain:
/// indices never decrease (a decrease is a cycle, R03.R01), every header bag index stays inside
/// the bag table, and the terminal bag closes the generator table.
fn check_index_chain(
    header_bag_ndx: &[u16],
    bags: &[Bag],
    gen_count: usize,
    header_id: &'static str,
    bag_id: &'static str,
) -> Result<(), FormatError> {
    if bags.is_empty() {
        return Err(FormatError::MissingTerminal(bag_id));
    }
    let last_bag = bags.len() - 1;
    let terminal_gen = bags[last_bag].gen_ndx as usize;
    let mut prev = 0u16;
    for n in header_bag_ndx {
        if *n < prev || *n as usize > last_bag {
            return Err(FormatError::NotMonotonic { id: header_id, index: 0 });
        }
        prev = *n;
    }
    // SF2 2.01 sec 7.2/7.6: the terminal header record's bagNdx points at the terminal bag.
    if header_bag_ndx.last().copied().unwrap_or(0) as usize != last_bag {
        return Err(FormatError::NotMonotonic { id: header_id, index: header_bag_ndx.len() });
    }
    let mut prev = 0u16;
    for b in bags {
        if b.gen_ndx < prev || b.gen_ndx as usize > terminal_gen {
            return Err(FormatError::NotMonotonic { id: bag_id, index: 0 });
        }
        prev = b.gen_ndx;
    }
    if terminal_gen != gen_count.checked_sub(1).unwrap_or(usize::MAX) {
        return Err(FormatError::NotMonotonic { id: bag_id, index: bags.len() });
    }
    Ok(())
}

fn read_bags(data: &[u8], id: &'static str, record: usize) -> Result<Vec<Bag>, FormatError> {
    let count = record_count(data, id, record)?;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        out.push(Bag::parse(data, record_off(data, id, record, i)?)?);
    }
    Ok(out)
}

fn read_gens(data: &[u8], id: &'static str, record: usize) -> Result<Vec<Gen>, FormatError> {
    let count = record_count(data, id, record)?;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        out.push(Gen::parse(data, record_off(data, id, record, i)?)?);
    }
    Ok(out)
}

fn read_inam(info: &[u8]) -> String {
    let Some(body) = find(info, b"INAM") else { return String::new() };
    let end = body.iter().position(|c| *c == 0).unwrap_or(body.len());
    fixed_name(body, 0, end).unwrap_or_default().chars().take(MAX_NAME_BYTES).collect()
}

fn read_samples(data: &[u8], frames: u32) -> Result<Vec<SampleHeader>, BankError> {
    let count = record_count(data, "shdr", SHDR_LEN)?;
    if count < 2 {
        return Err(BankError::Format(FormatError::MissingTerminal("shdr")));
    }
    if count - 1 > MAX_SAMPLES {
        return Err(BankError::limit("shdr", MAX_SAMPLES as u64, (count - 1) as u64));
    }
    let mut out = Vec::with_capacity(count - 1);
    for i in 0..count - 1 {
        let s = SampleHeader::parse(data, record_off(data, "shdr", SHDR_LEN, i)?)?;
        validate_sample(&s, i, frames)?;
        out.push(s);
    }
    Ok(out)
}

fn validate_sample(s: &SampleHeader, index: usize, frames: u32) -> Result<(), BankError> {
    if s.sample_type & ROM_SAMPLE_BIT != 0 {
        return Err(BankError::Unsupported(format!(
            "sample {index} is a ROM sample; only smpl-resident audio is implemented"
        )));
    }
    if s.sample_type & SF3_SAMPLE_BIT != 0 {
        return Err(BankError::Unsupported(format!(
            "sample {index} is SF3 Ogg-Vorbis; SP-4 owns the Vorbis decoder"
        )));
    }
    if s.rate == 0 {
        return Err(FormatError::ZeroSampleRate { index }.into());
    }
    let ordered = s.start <= s.end
        && s.end <= frames
        && s.start <= s.loop_start
        && s.loop_start <= s.loop_end
        && s.loop_end <= s.end;
    if !ordered {
        return Err(FormatError::BadSampleRange {
            index,
            start: s.start,
            end: s.end,
            frames: frames as usize,
        }
        .into());
    }
    Ok(())
}