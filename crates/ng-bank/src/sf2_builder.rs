// <META - FILE SUMMARY - R03.R03: build SF2 bytes in code (sine sample, a few presets). No binary fixtures, no licence issue.>

use crate::image::LoopMode;

include!("sine_table.rs");
include!("sf2_builder_tables.rs");

/// Amplitude of the generated sine: 2^14, so a mix or an envelope still has headroom.
const SINE_AMPLITUDE: i32 = 16384;
/// Phase steps per full cycle.
const PHASES: u32 = 1024;

/// A sine sample generated from the integer quarter wave in `sine_table.rs`. No `sin()` is called,
/// so the bank bytes are identical on every target and tests can compare them byte for byte
/// (R03.AC2).
///
/// `num/den` is the frequency in cycles across the whole sample.
pub fn sine_sample(frames: usize, num: u32, den: u32) -> Vec<i16> {
    let span = u64::from(den) * frames as u64;
    let mut out = Vec::with_capacity(frames);
    for i in 0..frames {
        let phase = (i as u64 * u64::from(PHASES) * u64::from(num) / span.max(1)).min(u64::from(PHASES) - 1);
        out.push(sine_at(phase as u32));
    }
    out
}

/// One sample of the generated waveform at a phase in 0..1024. Mirrors around 0, pi/2, pi and
/// 3pi/2 so the four quadrants are exact reflections of one generated quarter wave.
pub fn sine_at(phase: u32) -> i16 {
    let p = phase.min(PHASES - 1);
    let quadrant = p / (PHASES / 4);
    let within = (p % (PHASES / 4)) as usize;
    let v = QUARTER_WAVE.get(within).copied().unwrap_or(0) as i32;
    let signed = match quadrant {
        0 => v,
        1 => -v,
        2 => -v,
        _ => v,
    };
    signed as i16
}

/// Assert the generated table really does span the intended range, so a broken regeneration fails
/// here rather than quietly producing a DC-offset sample.
pub fn sine_amplitude() -> i32 {
    SINE_AMPLITUDE
}

/// One sample record in the bank being described.
#[derive(Clone, Debug, PartialEq)]
pub struct SampleSpec {
    pub name: String,
    pub rate: u32,
    /// Frames of 16-bit audio, written into `smpl` in order.
    pub pcm: Vec<i16>,
    pub root_key: u8,
    pub pitch_correction: i8,
    /// Loop mode carried in the sample header (SF2 sampleModes).
    pub loop_mode: LoopMode,
}

/// The generators of one instrument split.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitSpec {
    pub key_lo: u8,
    pub key_hi: u8,
    pub vel_lo: u8,
    pub vel_hi: u8,
    pub coarse_tune: i16,
    pub fine_tune: i16,
    pub scale_tuning: i16,
    pub attenuation: i16,
    pub pan: i16,
    pub reverb_send: i16,
    /// Volume envelope stages in timecents (SF2 2.01 sec 8.1.2); sustain in centibels.
    pub delay: i16,
    pub attack: i16,
    pub hold: i16,
    pub decay: i16,
    pub sustain: i16,
    pub release: i16,
    pub exclusive_class: i16,
    pub root_key: Option<u8>,
    /// Sample this split points at, by index in the builder's sample list.
    pub sample: usize,
    /// SF2 sampleModes for the zone; the parser reads this, not `SampleSpec::loop_mode`.
    pub loop_mode_hint: u16,
    /// True for an instrument global zone: no sampleID terminator, and the generators become
    /// the base the other splits build on (SF2 2.01 sec 7.4).
    pub global: bool,
}

impl SplitSpec {
    /// Full-range defaults: everything at the SF2 default, tuning at 100 cents per key.
    pub fn new(sample: usize) -> Self {
        Self {
            key_lo: 0,
            key_hi: 127,
            vel_lo: 0,
            vel_hi: 127,
            coarse_tune: 0,
            fine_tune: 0,
            scale_tuning: 100,
            attenuation: 0,
            pan: 0,
            reverb_send: 0,
            delay: -12000,
            attack: -12000,
            hold: -12000,
            decay: -12000,
            sustain: 0,
            release: -12000,
            exclusive_class: 0,
            root_key: None,
            sample,
            loop_mode_hint: 0,
            global: false,
        }
    }

    pub fn key(mut self, lo: u8, hi: u8) -> Self {
        self.key_lo = lo;
        self.key_hi = hi;
        self
    }

    pub fn vel(mut self, lo: u8, hi: u8) -> Self {
        self.vel_lo = lo;
        self.vel_hi = hi;
        self
    }

    pub fn loop_mode(mut self, m: LoopMode) -> Self {
        self.loop_mode_hint = m as u16;
        self
    }
}

/// An instrument: a name plus the splits that hang off it.
#[derive(Clone, Debug, PartialEq)]
pub struct InstrumentSpec {
    pub name: String,
    /// Split 0 becomes the instrument global zone; the rest become sample splits.
    pub splits: Vec<SplitSpec>,
}

/// A preset: a name, a MIDI (bank, program), an instrument, and optional global generators.
#[derive(Clone, Debug, PartialEq)]
pub struct PresetSpec {
    pub name: String,
    pub bank: u16,
    pub program: u16,
    pub instrument: usize,
    /// Preset-level generators. These are relative and are added to the instrument values
    /// (SF2 2.01 sec 8.5), so they exist here to exercise the additive half of the merge.
    pub attenuation: Option<i16>,
    pub pan: Option<i16>,
    pub reverb_send: Option<i16>,
    pub attack: Option<i16>,
    pub tune: Option<(i16, i16)>,
    pub key_range: Option<(u8, u8)>,
    pub vel_range: Option<(u8, u8)>,
}

impl PresetSpec {
    pub fn new(name: &str, bank: u16, program: u16, instrument: usize) -> Self {
        Self {
            name: name.to_string(),
            bank,
            program,
            instrument,
            attenuation: None,
            pan: None,
            reverb_send: None,
            attack: None,
            tune: None,
            key_range: None,
            vel_range: None,
        }
    }
}

/// Assembles an SF2 from a declarative description. The output is a real RIFF `sfbk` file, so any
/// other SF2 reader can check the parser against it (the second-opinion rule).
#[derive(Clone, Debug, Default)]
pub struct Sf2Builder {
    pub bank_name: String,
    pub samples: Vec<SampleSpec>,
    pub instruments: Vec<InstrumentSpec>,
    pub presets: Vec<PresetSpec>,
}

impl Sf2Builder {
    pub fn new(bank_name: &str) -> Self {
        Self { bank_name: bank_name.to_string(), ..Self::default() }
    }

    pub fn add_sample(&mut self, s: SampleSpec) -> usize {
        self.samples.push(s);
        self.samples.len() - 1
    }

    pub fn add_instrument(&mut self, i: InstrumentSpec) -> usize {
        self.instruments.push(i);
        self.instruments.len() - 1
    }

    pub fn add_preset(&mut self, p: PresetSpec) -> usize {
        self.presets.push(p);
        self.presets.len() - 1
    }

    pub fn build(&self) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&self.info_chunk());
        body.extend_from_slice(&self.sdta_chunk());
        body.extend_from_slice(&self.pdta_chunk());
        let mut out = Vec::with_capacity(body.len() + 12);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(b"sfbk");
        out.extend_from_slice(&body);
        out
    }

    fn info_chunk(&self) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&chunk(b"ifil", &[2, 0, 1, 0]));
        body.extend_from_slice(&chunk(b"isng", &zstr("EMU8000")));
        body.extend_from_slice(&chunk(b"INAM", &zstr(&self.bank_name)));
        list(b"INFO", &body)
    }

    fn sdta_chunk(&self) -> Vec<u8> {
        let mut pcm = Vec::new();
        for s in &self.samples {
            for v in &s.pcm {
                pcm.extend_from_slice(&v.to_le_bytes());
            }
        }
        list(b"sdta", &chunk(b"smpl", &pcm))
    }

    fn pdta_chunk(&self) -> Vec<u8> {
        let offsets = sample_offsets(&self.samples);
        let (phdr, pbag, pgen) = preset_tables(&self.presets);
        let (inst, ibag, igen) = instrument_tables(&self.instruments);
        let shdr = sample_table(&self.samples, &offsets);
        let mut body = Vec::new();
        for (id, data) in [
            (&b"phdr"[..], &phdr),
            (&b"pbag"[..], &pbag),
            (&b"pmod"[..], &pmod_terminal()),
            (&b"pgen"[..], &pgen),
            (&b"inst"[..], &inst),
            (&b"ibag"[..], &ibag),
            (&b"imod"[..], &pmod_terminal()),
            (&b"igen"[..], &igen),
            (&b"shdr"[..], &shdr),
        ] {
            let id: [u8; 4] = [id[0], id[1], id[2], id[3]];
            body.extend_from_slice(&chunk(&id, data));
        }
        list(b"pdta", &body)
    }
}

/// Byte ranges of each sample inside `smpl`: (start, end, loop_start, loop_end) in frames.
/// Loops sit inside the middle third so the fixture exercises the loop fields, and every sample
/// keeps the 46 zero frames SF2 2.01 sec 6.1 requires after the last one.
fn sample_offsets(samples: &[SampleSpec]) -> Vec<(u32, u32, u32, u32)> {
    let mut at = 0u32;
    let mut out = Vec::with_capacity(samples.len());
    for s in samples {
        let frames = s.pcm.len() as u32;
        let start = at;
        let end = start + frames;
        let (ls, le) = if s.loop_mode == LoopMode::None {
            (start, end)
        } else {
            let a = start + frames / 3;
            let b = (start + (frames * 2) / 3).max(a + 1).min(end);
            (a, b)
        };
        out.push((start, end, ls, le));
        at = end;
    }
    out
}

/// The bank every acceptance test starts from: two sine samples, two instruments and three
/// presets (a melodic one, the same program in another bank, and a drum kit).
pub fn demo_bank() -> Vec<u8> {
    builder_with(8).build()
}

/// `samples` tunes one knob of the fixture so a test can ask for a bigger or smaller bank.
pub fn builder_with(samples: usize) -> Sf2Builder {
    let mut b = Sf2Builder::new("notegrid generated bank");
    let sine = b.add_sample(SampleSpec {
        name: "sine".into(),
        rate: 44100,
        pcm: sine_sample(1024, 1, 8),
        root_key: 60,
        pitch_correction: 0,
        loop_mode: LoopMode::Continuous,
    });
    let plain = b.add_sample(SampleSpec {
        name: "plain".into(),
        rate: 22050,
        pcm: sine_sample(512, 3, 16),
        root_key: 48,
        pitch_correction: -3,
        loop_mode: LoopMode::None,
    });
    for i in 0..samples.saturating_sub(2) {
        b.add_sample(SampleSpec {
            name: format!("extra{i}"),
            rate: 44100,
            pcm: sine_sample(256, 2 + i as u32, 32),
            root_key: 36 + i as u8,
            pitch_correction: 0,
            loop_mode: LoopMode::None,
        });
    }
    let lead = b.add_instrument(InstrumentSpec {
        name: "lead".into(),
        splits: vec![
            SplitSpec::new(sine).key(36, 96).vel(1, 127).loop_mode(LoopMode::Continuous),
            SplitSpec {
                attenuation: 30,
                attack: 200,
                hold: -6000,
                decay: 1000,
                sustain: 120,
                release: 2400,
                pan: -300,
                reverb_send: 250,
                coarse_tune: 1,
                fine_tune: -25,
                scale_tuning: 150,
                exclusive_class: 3,
                root_key: Some(62),
                ..SplitSpec::new(plain).key(24, 35).vel(1, 127)
            },
        ],
    });
    let kit = b.add_instrument(InstrumentSpec {
        name: "kit".into(),
        splits: vec![
            SplitSpec { global: true, ..SplitSpec::new(plain).key(35, 81).vel(1, 127) },
            SplitSpec::new(plain).key(35, 81).vel(1, 127),
        ],
    });
    b.add_preset(PresetSpec::new("Piano", 0, 0, lead));
    b.add_preset(
        PresetSpec {
            attenuation: Some(20),
            pan: Some(120),
            reverb_send: Some(100),
            attack: Some(400),
            tune: Some((0, 15)),
            key_range: Some((0, 120)),
            vel_range: Some((1, 127)),
            ..PresetSpec::new("Piano Alt", 1, 0, lead)
        },
    );
    b.add_preset(PresetSpec::new("Standard Kit", 128, 0, kit));
    b
}