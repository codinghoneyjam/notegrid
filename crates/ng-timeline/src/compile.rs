// <META - ROLE : compile(project, sr) -> Schedule per R02.DESIGN; validate, convert, sort, build tables. | L1-160>

use ng_score::{LaneKindSerde, Project};

use crate::schedule::{LaneEntry, NoteEvent, Schedule, TempoEntry};
use crate::tempo_map::{TempoMap, TempoMapError};

const FLAG_MUTE: u8 = 0b0000_0001;
const FLAG_SOLO: u8 = 0b0000_0010;

/// Default mix values when a lane omits them (C0.4: volume/pan/reverb_send are 0-127, pan 64 = centre).
const DEFAULT_VOLUME: u8 = 100;
const DEFAULT_PAN: u8 = 64;
const DEFAULT_REVERB_SEND: u8 = 0;
/// C0.6: a melodic lane without a program is program 0.
const DEFAULT_PROGRAM: u8 = 0;
/// C0.6: only kit 0 (Standard) is guaranteed.
const DEFAULT_KIT: u8 = 0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileError {
    /// Validation rejected the project (C2: compile rejects projects with validation errors).
    pub codes: Vec<&'static str>,
    pub messages: Vec<String>,
    pub tempo: TempoMapError,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "compile failed: ")?;
        let parts: Vec<String> = self
            .codes
            .iter()
            .zip(&self.messages)
            .map(|(c, m)| format!("{c} ({m})"))
            .collect();
        write!(f, "{}", parts.join(", "))
    }
}

impl std::error::Error for CompileError {}

/// Compile a validated project into a Schedule at `sample_rate`.
///
/// Pure and deterministic (R02.R01): no clock, no randomness, no HashMap iteration. Note start
/// and end are each converted from ticks independently, so durations never drift (R02.DESIGN
/// step 3); a note whose end lands at or before its start is given a one-sample tail.
pub fn compile(project: &Project, sample_rate: u32) -> Result<Schedule, CompileError> {
    // 1 validate: errors -> CompileError, warnings do not block (R02.DESIGN step 1).
    let report = ng_score::validate(project);
    if !report.errors.is_empty() {
        return Err(CompileError {
            codes: report.errors.iter().map(|e| e.code).collect(),
            messages: report.errors.iter().map(|e| e.message.clone()).collect(),
            tempo: TempoMapError::EmptyTempo,
        });
    }

    // Build the map through ng-score's bpm -> mpq conversion; C0.2 has one definition.
    let map = TempoMap::from_project(project, sample_rate).map_err(|tempo| CompileError {
        codes: vec!["E_TEMPO"],
        messages: vec![format!("tempo map: {tempo:?}")],
        tempo,
    })?;

    // 2 slot = lane array index (C2.8).
    let lanes: Vec<LaneEntry> = project
        .lanes
        .iter()
        .enumerate()
        .map(|(index, lane)| {
            let (kind, program) = match lane.kind {
                LaneKindSerde::Melodic => (
                    0u8,
                    lane.program.unwrap_or(u32::from(DEFAULT_PROGRAM)) as u8,
                ),
                LaneKindSerde::Drum => (1u8, lane.kit.unwrap_or(u32::from(DEFAULT_KIT)) as u8),
            };
            let mut flags = 0u8;
            if lane.mute.unwrap_or(false) {
                flags |= FLAG_MUTE;
            }
            if lane.solo.unwrap_or(false) {
                flags |= FLAG_SOLO;
            }
            LaneEntry {
                slot: index as u16,
                program,
                kind,
                volume: lane.volume.unwrap_or(u32::from(DEFAULT_VOLUME)) as u8,
                pan: lane.pan.unwrap_or(u32::from(DEFAULT_PAN)) as u8,
                reverb_send: lane.reverb_send.unwrap_or(u32::from(DEFAULT_REVERB_SEND)) as u8,
                flags,
            }
        })
        .collect();

    // LaneId -> slot. BTreeMap-free: I-01 guarantees unique lane ids, so a plain Vec lookup is
    // enough and keeps the pass order-independent.
    let slot_of: Vec<(u32, u16)> = project
        .lanes
        .iter()
        .enumerate()
        .map(|(index, lane)| (lane.id, index as u16))
        .collect();

    // 3 convert note start and end EACH via tick_to_sample (never add a duration).
    let mut notes: Vec<NoteEvent> = Vec::with_capacity(project.notes.len());
    for note in &project.notes {
        let slot = match slot_of.iter().find(|(id, _)| *id == note.lane()) {
            Some((_, slot)) => *slot,
            // I-02 already rejected this, so it is unreachable after validation.
            None => return Err(missing_lane(note.lane())),
        };
        let start_tick = note.start();
        let end_tick = note.start().saturating_add(note.duration());
        let start = map.tick_to_sample(start_tick);
        let mut end = map.tick_to_sample(end_tick);
        // 3b if end <= start then end = start + 1
        if end <= start {
            end = start.saturating_add(1);
        }
        // C1 NOTE_ENCODING: [lane, start, duration, pitch, velocity] -- pitch is index 3.
        notes.push(NoteEvent {
            start,
            end,
            slot,
            pitch: note.3,
            vel: note.4,
        });
    }

    // 4 sort (start, slot, pitch) -- C3 ORDER.
    notes.sort_by_key(|n| (n.start, n.slot, n.pitch));

    // 5 tempo table: the change point and its sample position (C3.1).
    let tempos: Vec<TempoEntry> = map
        .segments()
        .iter()
        .map(|seg| TempoEntry {
            tick: seg.tick,
            mpq: seg.mpq,
            sample: map.tick_to_sample(seg.tick),
        })
        .collect();

    Schedule::new(sample_rate, lanes, tempos, notes).map_err(|e| CompileError {
        codes: vec!["E_COMPILE"],
        messages: vec![format!("schedule: {e}")],
        tempo: TempoMapError::EmptyTempo,
    })
}

fn missing_lane(lane: u32) -> CompileError {
    CompileError {
        codes: vec!["E_LANE_MISSING"],
        messages: vec![format!("lane {lane} does not exist")],
        tempo: TempoMapError::EmptyTempo,
    }
}
