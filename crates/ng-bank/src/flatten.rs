// <META - FILE SUMMARY - Preset/instrument generator merge into resolved zones (R03.DESIGN pre-flatten).>

use crate::error::{BankError, FormatError};
use crate::image::{LoopMode, PresetEntry, SampleEntry, SoundbankImage, ZoneEntry};
use crate::limits::MAX_NAME_BYTES;
use crate::record::{gen, Bag, Gen, InstrumentHeader, SampleHeader};
use crate::sf2::Sf2File;
use crate::timecents::timecents_to_seconds;

/// What a zone's generators say, accumulated per level. `Option` is meaningful: `None` means the
/// generator did not appear in that zone, which is different from "present and equal to the
/// default" because a preset-level value is added on top of whatever the instrument level gave.
#[derive(Clone, Copy, Debug, Default)]
struct Level {
    coarse_tune: Option<i32>,
    fine_tune: Option<i32>,
    scale_tuning: Option<i32>,
    atten: Option<i32>,
    pan: Option<i32>,
    reverb: Option<i32>,
    delay: Option<i32>,
    attack: Option<i32>,
    hold: Option<i32>,
    decay: Option<i32>,
    sustain: Option<i32>,
    release: Option<i32>,
    exclusive_class: Option<i32>,
    root_key: Option<i32>,
    loop_mode: Option<i32>,
    key_range: Option<i32>,
    vel_range: Option<i32>,
    start_coarse: Option<i32>,
    start_fine: Option<i32>,
    end_coarse: Option<i32>,
    end_fine: Option<i32>,
    loop_start_coarse: Option<i32>,
    loop_start_fine: Option<i32>,
    loop_end_coarse: Option<i32>,
    loop_end_fine: Option<i32>,
}

impl Level {
    fn absorb(&mut self, op: u16, amount: i16) {
        let v = i32::from(amount);
        match op {
            gen::COARSE_TUNE => self.coarse_tune = Some(v),
            gen::FINE_TUNE => self.fine_tune = Some(v),
            gen::SCALE_TUNING => self.scale_tuning = Some(v),
            gen::INITIAL_ATTENUATION => self.atten = Some(v),
            gen::PAN => self.pan = Some(v),
            gen::REVERB_EFFECTS_SEND => self.reverb = Some(v),
            gen::DELAY_VOL_ENV => self.delay = Some(v),
            gen::ATTACK_VOL_ENV => self.attack = Some(v),
            gen::HOLD_VOL_ENV => self.hold = Some(v),
            gen::DECAY_VOL_ENV => self.decay = Some(v),
            gen::SUSTAIN_VOL_ENV => self.sustain = Some(v),
            gen::RELEASE_VOL_ENV => self.release = Some(v),
            gen::EXCLUSIVE_CLASS => self.exclusive_class = Some(v),
            gen::OVERRIDING_ROOT_KEY => self.root_key = Some(v),
            gen::SAMPLE_MODES => self.loop_mode = Some(v),
            gen::KEY_RANGE => self.key_range = Some(v),
            gen::VEL_RANGE => self.vel_range = Some(v),
            gen::START_ADDRS_COARSE_OFFSET => self.start_coarse = Some(v),
            gen::START_ADDRS_OFFSET => self.start_fine = Some(v),
            gen::END_ADDRS_COARSE_OFFSET => self.end_coarse = Some(v),
            gen::END_ADDRS_OFFSET => self.end_fine = Some(v),
            gen::STARTLOOP_ADDRS_COARSE_OFFSET => self.loop_start_coarse = Some(v),
            gen::STARTLOOP_ADDRS_OFFSET => self.loop_start_fine = Some(v),
            gen::ENDLOOP_ADDRS_COARSE_OFFSET => self.loop_end_coarse = Some(v),
            gen::ENDLOOP_ADDRS_OFFSET => self.loop_end_fine = Some(v),
            _ => {}
        }
    }
}

/// Fold a zone's generator list into an accumulator. At the preset level, the generators SF2
/// 2.01 sec 8.5 forbids there (absolute addresses and identifiers) are dropped, not applied.
fn absorb(gens: &[Gen], span: (usize, usize), level: &mut Level, preset_level: bool) {
    for g in gens.get(span.0..span.1).unwrap_or(&[]) {
        if preset_level && gen::not_allowed_at_preset_level(g.op) {
            continue;
        }
        level.absorb(g.op, g.amount);
    }
}

/// Merge the two levels for one generator. The instrument level is absolute and overrides; the
/// preset level is relative and adds to the instrument value or to the SF2 default when the
/// instrument is silent (SF2 2.01 sec 8.5, R03.DESIGN "preset level additive, instrument level
/// overriding").
fn merge(instrument: Option<i32>, preset: Option<i32>, default: i32) -> i32 {
    instrument.unwrap_or(default).saturating_add(preset.unwrap_or(0))
}

fn bounded(instrument: Option<i32>, preset: Option<i32>, default: i32, op: u16) -> i32 {
    let v = merge(instrument, preset, default);
    match gen::absolute_range(op) {
        Some((lo, hi)) => v.clamp(lo, hi),
        None => v,
    }
}

/// A generator whose amount packs two bytes: low byte first (SF2 2.01 sec 4.4 `rangesType`).
fn range(value: Option<i32>) -> Option<(u8, u8)> {
    value.map(|v| ((v & 0xff) as u8, ((v >> 8) & 0xff) as u8))
}

fn intersect(a: (u8, u8), b: Option<(u8, u8)>) -> (u8, u8) {
    match b {
        None => a,
        Some((lo, hi)) => (a.0.max(lo), a.1.min(hi)),
    }
}

/// Absolute sample offset in frames; coarse offsets count 32768-sample blocks (SF2 2.01
/// sec 8.1.2: "in 32768 sample units").
fn frames(coarse: Option<i32>, fine: Option<i32>) -> i64 {
    i64::from(coarse.unwrap_or(0)) * 32_768 + i64::from(fine.unwrap_or(0))
}

fn zone_span(bags: &[Bag], gens: &[Gen], first: usize) -> (usize, usize) {
    let a = bags.get(first).map(|b| b.gen_ndx as usize).unwrap_or(0);
    let b = bags.get(first + 1).map(|n| n.gen_ndx as usize).unwrap_or(gens.len());
    (a, b.max(a))
}

/// Split a header's bag range into the global zones and the layers that follow. SF2
/// 2.01 sec 7.3: a global zone is recognised by the last generator not being the terminator
/// generator (31 `instrument` for presets, 43 `sampleID` for instruments). The global zone may
/// sit anywhere in the range -- the convention is first, but a file that appends it is still
/// read -- and its generators become the base the layers build on.
fn split_layers(
    bags: &[Bag],
    gens: &[Gen],
    first: usize,
    last: usize,
    terminator: u16,
) -> (Vec<(usize, usize)>, Vec<(usize, usize)>) {
    let mut globals = Vec::new();
    let mut layers = Vec::new();
    if last <= first || first >= bags.len() {
        return (globals, layers);
    }
    for i in first..last.min(bags.len()) {
        let span = zone_span(bags, gens, i);
        let is_global = gens.get(span.1.saturating_sub(1)).map(|g| g.op) != Some(terminator);
        if is_global {
            globals.push(span);
        } else {
            layers.push(span);
        }
    }
    (globals, layers)
}

fn terminal_value(gens: &[Gen], span: (usize, usize), op: u16) -> Option<usize> {
    gens.get(span.0..span.1)
        .unwrap_or(&[])
        .last()
        .filter(|g| g.op == op)
        .and_then(|g| usize::try_from(i32::from(g.amount)).ok())
}

/// Turn a parsed SoundFont into the flat image (R03.DESIGN "pre-flatten").
pub fn flatten(f: &Sf2File) -> Result<SoundbankImage, BankError> {
    let (samples, pcm) = carve_samples(f)?;
    let mut presets = Vec::new();
    let mut zones = Vec::new();
    for (pi, header) in f.presets.iter().enumerate() {
        let last = f.presets.get(pi + 1).map(|n| n.bag_ndx as usize).unwrap_or(f.preset_bags.len() - 1);
        let first = header.bag_ndx as usize;
        if last > f.preset_bags.len() || first > last {
            return Err(FormatError::NotMonotonic { id: "phdr", index: pi }.into());
        }
        let (globals, layers) = split_layers(&f.preset_bags, &f.preset_gens, first, last, gen::INSTRUMENT);
        if layers.is_empty() {
            continue; // SF2 2.01 sec 7.2: a preset with no layers is ignored.
        }
        let zone_first = zones.len() as u32;
        let mut pg = Level::default();
        for g in &globals {
            absorb(&f.preset_gens, *g, &mut pg, true);
        }
        for layer in layers {
            let mut pl = pg;
            absorb(&f.preset_gens, layer, &mut pl, true);
            let Some(ndx) = terminal_value(&f.preset_gens, layer, gen::INSTRUMENT) else { continue };
            let inst = f.instruments.get(ndx).ok_or(FormatError::IndexOutOfRange {
                id: "pgen instrument",
                index: u32::try_from(ndx).unwrap_or(u32::MAX),
                count: f.instruments.len(),
            })?;
            zones.extend(instrument_zones(f, inst, &pl, ndx, &samples)?);
        }
        presets.push(PresetEntry {
            bank: header.bank,
            program: header.program,
            drum: header.bank == 128,
            has_global_zone: !globals.is_empty(),
            name: clip(&header.name),
            zone_first,
            zone_count: (zones.len() as u32).saturating_sub(zone_first),
        });
    }
    Ok(SoundbankImage { name: clip(&f.name), presets, zones, samples, pcm })
}

fn clip(name: &str) -> String {
    name.chars().take(MAX_NAME_BYTES).collect()
}

fn instrument_zones(
    f: &Sf2File,
    inst: &InstrumentHeader,
    preset: &Level,
    inst_ndx: usize,
    samples: &[SampleEntry],
) -> Result<Vec<ZoneEntry>, BankError> {
    // The bag range closes at the next instrument's wInstBagNdx (SF2 2.01 sec 7.6).
    let last = f
        .instruments
        .get(inst_ndx + 1)
        .map(|n| n.bag_ndx as usize)
        .unwrap_or(f.inst_bags.len() - 1);
    let first = inst.bag_ndx as usize;
    if last > f.inst_bags.len() || first > last {
        return Err(FormatError::NotMonotonic { id: "inst", index: inst_ndx }.into());
    }
    let (globals, splits) = split_layers(&f.inst_bags, &f.inst_gens, first, last, gen::SAMPLE_ID);
    let mut ig = Level::default();
    for g in &globals {
        absorb(&f.inst_gens, *g, &mut ig, false);
    }
    let mut out = Vec::new();
    for split in splits {
        let mut sl = ig;
        absorb(&f.inst_gens, split, &mut sl, false);
        let Some(ndx) = terminal_value(&f.inst_gens, split, gen::SAMPLE_ID) else { continue };
        let shdr = f.samples.get(ndx).ok_or(FormatError::IndexOutOfRange {
            id: "igen sample",
            index: u32::try_from(ndx).unwrap_or(u32::MAX),
            count: f.samples.len(),
        })?;
        out.push(zone(shdr, ndx, &sl, preset, samples)?);
    }
    Ok(out)
}

/// Merge the four levels into one resolved zone and rebase its offsets onto the image blob.
fn zone(
    shdr: &SampleHeader,
    sample_ndx: usize,
    inst: &Level,
    preset: &Level,
    samples: &[SampleEntry],
) -> Result<ZoneEntry, BankError> {
    let sample = samples.get(sample_ndx).ok_or(FormatError::IndexOutOfRange {
        id: "sample",
        index: u32::try_from(sample_ndx).unwrap_or(u32::MAX),
        count: samples.len(),
    })?;
    let start_local = shdr.start as i64 + frames(inst.start_coarse, inst.start_fine);
    let end_local = shdr.end as i64 + frames(inst.end_coarse, inst.end_fine);
    let loop_start_local = shdr.loop_start as i64 + frames(inst.loop_start_coarse, inst.loop_start_fine);
    let loop_end_local = shdr.loop_end as i64 + frames(inst.loop_end_coarse, inst.loop_end_fine);
    let in_sample = start_local >= 0
        && start_local <= loop_start_local
        && loop_start_local <= loop_end_local
        && loop_end_local <= end_local
        && end_local <= i64::from(u32::MAX);
    if !in_sample {
        return Err(FormatError::BadSampleRange {
            index: sample_ndx,
            start: start_local.clamp(0, i64::from(u32::MAX)) as u32,
            end: end_local.clamp(0, i64::from(u32::MAX)) as u32,
            frames: shdr.end as usize,
        }
        .into());
    }
    let shift = i64::from(sample.start) - i64::from(shdr.start);
    let (key_lo, key_hi) = intersect((0, 127), range(preset.key_range));
    let (key_lo, key_hi) = intersect((key_lo, key_hi), range(inst.key_range));
    let (vel_lo, vel_hi) = intersect((0, 127), range(preset.vel_range));
    let (vel_lo, vel_hi) = intersect((vel_lo, vel_hi), range(inst.vel_range));
    let coarse = bounded(inst.coarse_tune, preset.coarse_tune, 0, gen::COARSE_TUNE);
    let fine = bounded(inst.fine_tune, preset.fine_tune, 0, gen::FINE_TUNE);
    Ok(ZoneEntry {
        key_lo,
        key_hi,
        vel_lo,
        vel_hi,
        root_key: bounded(inst.root_key, None, i32::from(shdr.original_key), gen::OVERRIDING_ROOT_KEY) as u8,
        loop_mode: inst
            .loop_mode
            .map(|v| LoopMode::from_generator(v as i16))
            .unwrap_or(LoopMode::None),
        exclusive_class: bounded(inst.exclusive_class, None, 0, gen::EXCLUSIVE_CLASS) as u8,
        sample_idx: u32::try_from(sample_ndx).unwrap_or(0),
        tune_cents: (coarse * 100 + fine).clamp(-12999, 12999) as i16,
        scale_tuning_cents: bounded(inst.scale_tuning, preset.scale_tuning, 100, gen::SCALE_TUNING) as u16,
        atten_cb: bounded(inst.atten, preset.atten, 0, gen::INITIAL_ATTENUATION) as u16,
        pan: bounded(inst.pan, preset.pan, 0, gen::PAN) as i16,
        reverb_send: bounded(inst.reverb, preset.reverb, 0, gen::REVERB_EFFECTS_SEND) as u16,
        env_sustain_cb: bounded(inst.sustain, preset.sustain, 0, gen::SUSTAIN_VOL_ENV) as u16,
        start: rebase(start_local, shift),
        end: rebase(end_local, shift),
        loop_start: rebase(loop_start_local, shift),
        loop_end: rebase(loop_end_local, shift),
        env_delay_s: timecents_to_seconds(merge(inst.delay, preset.delay, -12000)),
        env_attack_s: timecents_to_seconds(merge(inst.attack, preset.attack, -12000)),
        env_hold_s: timecents_to_seconds(merge(inst.hold, preset.hold, -12000)),
        env_decay_s: timecents_to_seconds(merge(inst.decay, preset.decay, -12000)),
        env_release_s: timecents_to_seconds(merge(inst.release, preset.release, -12000)),
    })
}

fn rebase(local: i64, shift: i64) -> u32 {
    u32::try_from(local.saturating_add(shift)).unwrap_or(0)
}

/// Copy each `shdr` span out of `smpl` into one contiguous blob, recording where it landed.
fn carve_samples(f: &Sf2File) -> Result<(Vec<SampleEntry>, Vec<i16>), BankError> {
    let mut out = Vec::with_capacity(f.samples.len());
    let mut pcm: Vec<i16> = Vec::with_capacity(f.pcm.len());
    for s in &f.samples {
        let base = u32::try_from(pcm.len()).unwrap_or(u32::MAX);
        let slice = f.pcm.get(s.start as usize..s.end as usize).ok_or(FormatError::BadSampleRange {
            index: 0,
            start: s.start,
            end: s.end,
            frames: f.pcm.len(),
        })?;
        pcm.extend_from_slice(slice);
        out.push(SampleEntry {
            start: base,
            end: base.saturating_add(s.end.saturating_sub(s.start)),
            rate: s.rate,
            name: clip(&s.name),
            original_key: s.original_key,
            pitch_correction_cents: s.pitch_correction,
            link: s.link,
            sample_type: s.sample_type,
        });
    }
    Ok((out, pcm))
}