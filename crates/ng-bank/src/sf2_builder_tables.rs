// <META - FILE SUMMARY - sf2_builder table writers: phdr/pbag/pgen, inst/ibag/igen, shdr and the RIFF chunk helpers.>

// Included into `sf2_builder`, so the spec types are already in scope here.

/// A NUL-terminated string in a fixed field, even-length overall (SF2 2.01 sec 5.3).
pub(super) fn zstr(s: &str) -> Vec<u8> {
    let mut v: Vec<u8> = s.bytes().take(255).map(|b| if (0x20..0x7f).contains(&b) { b } else { b'_' }).collect();
    v.push(0);
    if v.len() % 2 == 1 {
        v.push(0);
    }
    v
}

/// A fixed-width name field: exactly `len` bytes, zero padded (SF2 2.01 sec 7.2).
fn name20(s: &str) -> [u8; 20] {
    let mut out = [0u8; 20];
    for (i, b) in s.bytes().take(19).enumerate() {
        if (0x20..0x7f).contains(&b) {
            out[i] = b;
        }
    }
    out
}

pub(super) fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 + body.len() + 1);
    v.extend_from_slice(id);
    v.extend_from_slice(&(body.len() as u32).to_le_bytes());
    v.extend_from_slice(body);
    if body.len() % 2 == 1 {
        v.push(0);
    }
    v
}

pub(super) fn list(form: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut inner = Vec::with_capacity(4 + body.len());
    inner.extend_from_slice(form);
    inner.extend_from_slice(body);
    chunk(b"LIST", &inner)
}

/// The EOM terminal of a pmod/imod chunk: an all-zero record that closes the table.
fn pmod_terminal() -> Vec<u8> {
    vec![0u8; 10]
}

fn gen_record(op: u16, amount: i16) -> Vec<u8> {
    let mut v = Vec::with_capacity(4);
    v.extend_from_slice(&op.to_le_bytes());
    v.extend_from_slice(&amount.to_le_bytes());
    v
}

fn bag_record(gen_ndx: u16, mod_ndx: u16) -> Vec<u8> {
    let mut v = Vec::with_capacity(4);
    v.extend_from_slice(&gen_ndx.to_le_bytes());
    v.extend_from_slice(&mod_ndx.to_le_bytes());
    v
}

/// Preset global-zone generators, in ascending operator order so the fixture is readable.
fn preset_global_gens(p: &PresetSpec) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    if let Some((lo, hi)) = p.key_range {
        out.push(gen_record(33, i16::from_le_bytes([lo, hi])));
    }
    if let Some((lo, hi)) = p.vel_range {
        out.push(gen_record(34, i16::from_le_bytes([lo, hi])));
    }
    if let Some(a) = p.attenuation {
        out.push(gen_record(38, a));
    }
    if let Some((coarse, fine)) = p.tune {
        out.push(gen_record(41, coarse));
        out.push(gen_record(42, fine));
    }
    if let Some(a) = p.attack {
        out.push(gen_record(23, a));
    }
    if let Some(a) = p.pan {
        out.push(gen_record(17, a));
    }
    if let Some(a) = p.reverb_send {
        out.push(gen_record(16, a));
    }
    out
}

fn preset_layer_gens(p: &PresetSpec) -> Vec<Vec<u8>> {
    // SF2 2.01 sec 7.5: generator 31 (instrument) is always last in a preset layer.
    let mut out = preset_global_gens(p);
    out.push(gen_record(31, p.instrument as i16));
    out
}

fn instrument_gens(s: &SplitSpec) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    out.push(gen_record(33, i16::from_le_bytes([s.key_lo, s.key_hi])));
    out.push(gen_record(34, i16::from_le_bytes([s.vel_lo, s.vel_hi])));
    out.push(gen_record(38, s.attenuation));
    out.push(gen_record(23, s.attack));
    out.push(gen_record(22, s.delay));
    out.push(gen_record(24, s.hold));
    out.push(gen_record(25, s.decay));
    out.push(gen_record(26, s.sustain));
    out.push(gen_record(27, s.release));
    out.push(gen_record(16, s.reverb_send));
    out.push(gen_record(17, s.pan));
    out.push(gen_record(41, s.coarse_tune));
    out.push(gen_record(42, s.fine_tune));
    out.push(gen_record(46, s.scale_tuning));
    out.push(gen_record(47, s.exclusive_class));
    if let Some(k) = s.root_key {
        out.push(gen_record(48, i16::from(k)));
    }
    out.push(gen_record(44, s.loop_mode_code() as i16));
    out
}

impl SplitSpec {
    /// SF2 sampleModes for this split's sample (SF2 2.01 sec 8.1.2).
    pub fn loop_mode_code(&self) -> u16 {
        self.loop_mode_hint
    }
}

/// Build phdr, pbag and pgen. Every preset gets one bag: either a single layer, or a global bag
/// followed by the layer bag, depending on whether preset-level generators were requested.
pub(super) fn preset_tables(presets: &[PresetSpec]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut phdr = Vec::new();
    let mut pbag = Vec::new();
    let mut pgen = Vec::new();
    for p in presets {
        phdr.extend_from_slice(&name20(&p.name));
        phdr.extend_from_slice(&p.program.to_le_bytes());
        phdr.extend_from_slice(&p.bank.to_le_bytes());
        phdr.extend_from_slice(&((pbag.len() / 4) as u16).to_le_bytes());
        phdr.extend_from_slice(&[0u8; 12]);
        let global = preset_global_gens(p);
        let mut bag = (pgen.len() / 4) as u16;
        if !global.is_empty() {
            pbag.extend_from_slice(&bag_record(bag, 0));
            for g in &global {
                pgen.extend_from_slice(g);
            }
            bag += 1;
        }
        pbag.extend_from_slice(&bag_record(bag, 0));
        for g in preset_layer_gens(p) {
            pgen.extend_from_slice(&g);
        }
    }
    // The terminal record's bagNdx points at the terminal bag (SF2 2.01 sec 7.2).
    let mut eop = name20("EOP").to_vec();
    eop.extend_from_slice(&0u16.to_le_bytes());
    eop.extend_from_slice(&0u16.to_le_bytes());
    eop.extend_from_slice(&((pbag.len() / 4) as u16).to_le_bytes());
    eop.extend_from_slice(&[0u8; 12]);
    phdr.extend_from_slice(&eop);
    pbag.extend_from_slice(&bag_record((pgen.len() / 4) as u16, 0));
    pgen.extend_from_slice(&[0u8; 4]);
    (phdr, pbag, pgen)
}

/// Build inst, ibag and igen. A split marked `global` becomes the instrument global zone; the
/// rest become sample splits closed by a sampleID generator.
pub(super) fn instrument_tables(instruments: &[InstrumentSpec]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut inst = Vec::new();
    let mut ibag = Vec::new();
    let mut igen = Vec::new();
    for i in instruments {
        inst.extend_from_slice(&name20(&i.name));
        inst.extend_from_slice(&((ibag.len() / 4) as u16).to_le_bytes());
        for split in &i.splits {
            ibag.extend_from_slice(&bag_record((igen.len() / 4) as u16, 0));
            let mut gens = instrument_gens(split);
            if !split.global {
                // Only a real split carries the sampleID terminator (SF2 2.01 sec 7.8).
                gens.push(gen_record(43, split.sample as i16));
            }
            for g in gens {
                igen.extend_from_slice(&g);
            }
        }
    }
    // The terminal record's bagNdx points at the terminal bag (SF2 2.01 sec 7.6).
    let mut eoi = name20("EOI").to_vec();
    eoi.extend_from_slice(&((ibag.len() / 4) as u16).to_le_bytes());
    inst.extend_from_slice(&eoi);
    ibag.extend_from_slice(&bag_record((igen.len() / 4) as u16, 0));
    igen.extend_from_slice(&[0u8; 4]);
    (inst, ibag, igen)
}

/// Build shdr. `offsets` carries (start, end, loop_start, loop_end) frames per sample.
pub(super) fn sample_table(samples: &[SampleSpec], offsets: &[(u32, u32, u32, u32)]) -> Vec<u8> {
    let mut shdr = Vec::with_capacity(samples.len() * 46 + 46);
    for (i, s) in samples.iter().enumerate() {
        let (start, end, ls, le) = offsets.get(i).copied().unwrap_or((0, 0, 0, 0));
        shdr.extend_from_slice(&name20(&s.name));
        shdr.extend_from_slice(&start.to_le_bytes());
        shdr.extend_from_slice(&end.to_le_bytes());
        shdr.extend_from_slice(&ls.to_le_bytes());
        shdr.extend_from_slice(&le.to_le_bytes());
        shdr.extend_from_slice(&s.rate.to_le_bytes());
        shdr.push(s.root_key);
        shdr.push(s.pitch_correction as u8);
        shdr.extend_from_slice(&0u16.to_le_bytes());
        shdr.extend_from_slice(&1u16.to_le_bytes());
    }
    shdr.extend_from_slice(&name20("EOS"));
    shdr.extend_from_slice(&[0u8; 26]);
    shdr
}
