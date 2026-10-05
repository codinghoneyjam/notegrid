// <META - FILE SUMMARY - Core audio types and contract traits (C2 ng-core portion). Zero third-party deps.>

/// Tick = u32 ; PPQ = 480 fixed (C0.1).
pub type Tick = u32;

/// Stable lane identity independent of array order (R01.R05).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LaneId(pub u32);

/// Slot indexes the lanes array (C2.8). Reordering lanes requires recompiling the schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slot(pub u16);

pub const MAX_SLOTS: usize = 129;
pub const AUDITION_SLOT: Slot = Slot(128);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneKind {
    Melodic,
    Drum,
}

/// Note events are issue targets: note_on at start, note_off scheduled by the renderer (C2.1).
pub trait NoteSink {
    fn set_program(&mut self, slot: Slot, kind: LaneKind, program: u8);
    fn note_on(&mut self, slot: Slot, key: u8, vel: u8);
    fn note_off(&mut self, slot: Slot, key: u8);
    fn release_all(&mut self);
    fn silence_all(&mut self);
}

/// Per-lane mix controls. gain is linear 0..1 after mute/solo resolution.
pub trait ChannelMix {
    fn set_gain(&mut self, slot: Slot, gain: f32);
    fn set_pan(&mut self, slot: Slot, pan: u8);
    fn set_reverb_send(&mut self, slot: Slot, send: u8);
}

/// Equal-length bus slices passed to AudioSource::render (overwrites).
pub struct Buses<'a> {
    pub dry_l: &'a mut [f32],
    pub dry_r: &'a mut [f32],
    pub send_l: &'a mut [f32],
    pub send_r: &'a mut [f32],
}

pub trait AudioSource {
    fn sample_rate(&self) -> u32;
    fn render(&mut self, out: &mut Buses);
    fn active_voices(&self) -> usize;
}

pub trait Synth: NoteSink + ChannelMix + AudioSource + Send {}
impl<T: NoteSink + ChannelMix + AudioSource + Send> Synth for T {}

/// Interleaved f32 PCM (C0.5).
pub struct PcmBuffer {
    pub sample_rate: u32,
    pub channels: u8,
    pub frames: usize,
    pub data: Vec<f32>,
}

pub struct EncodeMeta {
    pub title: Option<String>,
    /// Loop points in samples, [start, end).
    pub loop_points: Option<(u32, u32)>,
}

#[derive(Debug, Clone)]
pub struct EncodeError {
    pub code: &'static str,
    pub message: String,
}

pub trait AudioEncoder {
    type Options: Default;
    fn id(&self) -> &'static str;
    fn extension(&self) -> &'static str;
    fn mime(&self) -> &'static str;
    fn encode(
        &self,
        pcm: &PcmBuffer,
        meta: &EncodeMeta,
        opts: &Self::Options,
    ) -> Result<Vec<u8>, EncodeError>;
}
