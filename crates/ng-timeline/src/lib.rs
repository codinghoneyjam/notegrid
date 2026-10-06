// <META - FILE SUMMARY - R02: TempoMap (the only tick<->sample conversion in the system) and compile(project,sr) -> Schedule.>
mod compile;
mod schedule;
mod tempo_map;

pub use compile::{compile, CompileError};
pub use schedule::{LaneEntry, NoteEvent, Schedule, ScheduleError, TempoEntry, SCHEDULE_VERSION};
pub use tempo_map::{TempoMap, TempoMapError};

/// Bytes of an f64 sample position crossing the ABI are exact below 2^53 (D12).
pub const MAX_EXACT_SAMPLE: f64 = 9_007_199_254_740_992.0;
