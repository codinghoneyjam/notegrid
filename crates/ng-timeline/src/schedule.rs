// <META - ROLE : C3 schedule binary: header/lane/tempo/note tables with to_bytes/from_bytes. | L1-300>

use std::error::Error;
use std::fmt;

/// Magic and version of the schedule container (C3).
pub const SCHEDULE_MAGIC: [u8; 4] = *b"NGSC";
pub const SCHEDULE_VERSION: u16 = 1;

pub const HEADER_LEN: usize = 32;
pub const LANE_LEN: usize = 8;
pub const TEMPO_LEN: usize = 16;
pub const NOTE_LEN: usize = 24;

const FLAG_MUTE: u8 = 0b0000_0001;
const FLAG_SOLO: u8 = 0b0000_0010;

/// Lane table entry (C3 Lane, 8 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaneEntry {
    pub slot: u16,
    pub program: u8,
    pub kind: u8,
    pub volume: u8,
    pub pan: u8,
    pub reverb_send: u8,
    /// bit0 mute, bit1 solo (C3).
    pub flags: u8,
}

impl LaneEntry {
    pub fn is_mute(&self) -> bool {
        self.flags & FLAG_MUTE != 0
    }
    pub fn is_solo(&self) -> bool {
        self.flags & FLAG_SOLO != 0
    }
}

/// Tempo table entry (C3 Tempo, 16 bytes): the change point in ticks, its mpq, and the
/// sample position of that change point. C3.1: the worklet converts sample->tick from this
/// table, so the UI never needs time-conversion logic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TempoEntry {
    pub tick: u32,
    pub mpq: u32,
    pub sample: u64,
}

/// Note event (C3 Note, 24 bytes). C2.1: note_on at start, note_off at end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteEvent {
    pub start: u64,
    pub end: u64,
    pub slot: u16,
    pub pitch: u8,
    pub vel: u8,
}

/// Compiled schedule: sample-positioned events plus the tables needed to play them back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schedule {
    pub sample_rate: u32,
    pub lanes: Vec<LaneEntry>,
    pub tempos: Vec<TempoEntry>,
    pub notes: Vec<NoteEvent>,
    /// Max note end; tail excluded (C3 ORDER).
    pub total_samples: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScheduleError {
    TooShort {
        need: usize,
        got: usize,
    },
    BadMagic([u8; 4]),
    BadVersion(u16),
    BadFlags(u16),
    ZeroSampleRate,
    /// A count in the header does not match the byte length (R02.R04).
    LengthMismatch {
        field: &'static str,
        declared: u32,
        needed: usize,
    },
    TrailingBytes {
        extra: usize,
    },
    NoteNotSorted {
        index: usize,
    },
    NoteEndBeforeStart {
        index: usize,
    },
    LaneSlotOutOfRange {
        index: usize,
        slot: u16,
        lane_count: u16,
    },
    /// Reserved bytes must be zero (C3 Note reserved u32=0).
    NonZeroReserved {
        index: usize,
    },
    IntegerOverflow(&'static str),
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { need, got } => write!(f, "need {need} bytes, got {got}"),
            Self::BadMagic(m) => write!(f, "bad magic {m:02x?}"),
            Self::BadVersion(v) => write!(f, "unsupported schedule version {v}"),
            Self::BadFlags(v) => write!(f, "unsupported flags {v:#x}"),
            Self::ZeroSampleRate => write!(f, "sample_rate must be nonzero"),
            Self::LengthMismatch {
                field,
                declared,
                needed,
            } => {
                write!(
                    f,
                    "{field} declares {declared} but the buffer implies {needed}"
                )
            }
            Self::TrailingBytes { extra } => write!(f, "{extra} trailing bytes"),
            Self::NoteNotSorted { index } => {
                write!(f, "note {index} breaks the (start, slot, pitch) order")
            }
            Self::NoteEndBeforeStart { index } => write!(f, "note {index} ends before it starts"),
            Self::LaneSlotOutOfRange {
                index,
                slot,
                lane_count,
            } => {
                write!(
                    f,
                    "note {index} targets slot {slot} but there are {lane_count} lanes"
                )
            }
            Self::NonZeroReserved { index } => {
                write!(f, "note {index} has non-zero reserved bytes")
            }
            Self::IntegerOverflow(field) => write!(f, "{field} overflows"),
        }
    }
}

impl Error for ScheduleError {}

impl Schedule {
    pub fn new(
        sample_rate: u32,
        lanes: Vec<LaneEntry>,
        tempos: Vec<TempoEntry>,
        notes: Vec<NoteEvent>,
    ) -> Result<Self, ScheduleError> {
        let total_samples = notes.iter().map(|n| n.end).max().unwrap_or(0);
        let s = Self {
            sample_rate,
            lanes,
            tempos,
            notes,
            total_samples,
        };
        s.validate()?;
        Ok(s)
    }

    /// Structural check. `from_bytes` treats its input as untrusted (R02.R04): every length,
    /// count and ordering is verified before the data is handed out.
    pub fn validate(&self) -> Result<(), ScheduleError> {
        if self.sample_rate == 0 {
            return Err(ScheduleError::ZeroSampleRate);
        }
        let lane_count = u16::try_from(self.lanes.len())
            .map_err(|_| ScheduleError::IntegerOverflow("lane_count"))?;

        for (index, note) in self.notes.iter().enumerate() {
            if note.end < note.start {
                return Err(ScheduleError::NoteEndBeforeStart { index });
            }
            if note.slot >= lane_count {
                return Err(ScheduleError::LaneSlotOutOfRange {
                    index,
                    slot: note.slot,
                    lane_count,
                });
            }
            if index > 0 {
                let prev = self.notes[index - 1];
                let order = (note.start, note.slot, note.pitch);
                let prev_order = (prev.start, prev.slot, prev.pitch);
                if order < prev_order {
                    return Err(ScheduleError::NoteNotSorted { index });
                }
            }
        }
        Ok(())
    }

    /// C3 little-endian, 8-byte aligned.
    pub fn to_bytes(&self) -> Vec<u8> {
        let note_count = u32::try_from(self.notes.len()).unwrap_or(u32::MAX);
        let lane_count = u32::try_from(self.lanes.len()).unwrap_or(u32::MAX);
        let tempo_count = u32::try_from(self.tempos.len()).unwrap_or(u32::MAX);

        let mut out = Vec::with_capacity(
            HEADER_LEN
                + self.lanes.len() * LANE_LEN
                + self.tempos.len() * TEMPO_LEN
                + self.notes.len() * NOTE_LEN,
        );
        out.extend_from_slice(&SCHEDULE_MAGIC);
        out.extend_from_slice(&SCHEDULE_VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&self.sample_rate.to_le_bytes());
        out.extend_from_slice(&note_count.to_le_bytes());
        out.extend_from_slice(&lane_count.to_le_bytes());
        out.extend_from_slice(&tempo_count.to_le_bytes());
        out.extend_from_slice(&self.total_samples.to_le_bytes());
        debug_assert_eq!(out.len(), HEADER_LEN);

        for lane in &self.lanes {
            out.extend_from_slice(&lane.slot.to_le_bytes());
            out.push(lane.program);
            out.push(lane.kind);
            out.push(lane.volume);
            out.push(lane.pan);
            out.push(lane.reverb_send);
            out.push(lane.flags);
            debug_assert_eq!(out.len() % LANE_LEN, HEADER_LEN % LANE_LEN);
        }
        for tempo in &self.tempos {
            out.extend_from_slice(&tempo.tick.to_le_bytes());
            out.extend_from_slice(&tempo.mpq.to_le_bytes());
            out.extend_from_slice(&tempo.sample.to_le_bytes());
        }
        for note in &self.notes {
            out.extend_from_slice(&note.start.to_le_bytes());
            out.extend_from_slice(&note.end.to_le_bytes());
            out.extend_from_slice(&note.slot.to_le_bytes());
            out.push(note.pitch);
            out.push(note.vel);
            out.extend_from_slice(&0u32.to_le_bytes()); // reserved
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ScheduleError> {
        if bytes.len() < HEADER_LEN {
            return Err(ScheduleError::TooShort {
                need: HEADER_LEN,
                got: bytes.len(),
            });
        }
        let magic: [u8; 4] = bytes[0..4]
            .try_into()
            .map_err(|_| ScheduleError::TooShort {
                need: 4,
                got: bytes.len(),
            })?;
        if magic != SCHEDULE_MAGIC {
            return Err(ScheduleError::BadMagic(magic));
        }
        let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        if version != SCHEDULE_VERSION {
            return Err(ScheduleError::BadVersion(version));
        }
        let flags = u16::from_le_bytes(bytes[6..8].try_into().unwrap());
        if flags != 0 {
            return Err(ScheduleError::BadFlags(flags));
        }
        let sample_rate = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if sample_rate == 0 {
            return Err(ScheduleError::ZeroSampleRate);
        }
        let note_count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let lane_count = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
        let tempo_count = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
        let total_samples = u64::from_le_bytes(bytes[24..32].try_into().unwrap());

        // Every multiplication and addition is checked: an untrusted header can ask for
        // usize::MAX * 24 and must produce an error, not a wrap (R02.R04).
        let table_len = |count: usize, width: usize| -> Result<usize, ScheduleError> {
            count
                .checked_mul(width)
                .ok_or(ScheduleError::IntegerOverflow("table size"))
        };
        let tables = table_len(lane_count, LANE_LEN)?
            .checked_add(table_len(tempo_count, TEMPO_LEN)?)
            .ok_or(ScheduleError::IntegerOverflow("tables"))?;
        let needed = tables
            .checked_add(table_len(note_count, NOTE_LEN)?)
            .and_then(|n| n.checked_add(HEADER_LEN))
            .ok_or(ScheduleError::IntegerOverflow("total length"))?;
        if bytes.len() < needed {
            return Err(ScheduleError::LengthMismatch {
                field: "table sizes",
                declared: needed as u32,
                needed,
            });
        }
        if bytes.len() > needed {
            return Err(ScheduleError::TrailingBytes {
                extra: bytes.len() - needed,
            });
        }

        let mut offset = HEADER_LEN;
        let mut lanes = Vec::with_capacity(lane_count);
        for _ in 0..lane_count {
            lanes.push(LaneEntry {
                slot: u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()),
                program: bytes[offset + 2],
                kind: bytes[offset + 3],
                volume: bytes[offset + 4],
                pan: bytes[offset + 5],
                reverb_send: bytes[offset + 6],
                flags: bytes[offset + 7],
            });
            offset += LANE_LEN;
        }

        let mut tempos = Vec::with_capacity(tempo_count);
        for _ in 0..tempo_count {
            tempos.push(TempoEntry {
                tick: u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
                mpq: u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()),
                sample: u64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().unwrap()),
            });
            offset += TEMPO_LEN;
        }

        let mut notes = Vec::with_capacity(note_count);
        for i in 0..note_count {
            let start = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
            let end = u64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().unwrap());
            let slot = u16::from_le_bytes(bytes[offset + 16..offset + 18].try_into().unwrap());
            let pitch = bytes[offset + 18];
            let vel = bytes[offset + 19];
            let reserved = u32::from_le_bytes(bytes[offset + 20..offset + 24].try_into().unwrap());
            if reserved != 0 {
                return Err(ScheduleError::NonZeroReserved { index: i });
            }
            notes.push(NoteEvent {
                start,
                end,
                slot,
                pitch,
                vel,
            });
            offset += NOTE_LEN;
        }

        let s = Self {
            sample_rate,
            lanes,
            tempos,
            notes,
            total_samples,
        };
        s.validate()?;
        Ok(s)
    }
}
