// <META - ROLE : Project model mirroring schema/project.schema.json; x_ keys preserved via ext maps. | L1-60>

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub use ng_core::LaneKind;

pub const PPQ: u32 = 480;
pub const FORMAT_VERSION_SUPPORTED: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub format_version: u32,
    pub title: String,
    pub ppq: u32,
    pub tempo: Vec<TempoItem>,
    pub meter: Vec<MeterItem>,
    #[serde(rename = "loop", default, skip_serializing_if = "Option::is_none")]
    pub loop_region: Option<LoopRegion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length_ticks: Option<u64>,
    pub lanes: Vec<Lane>,
    pub notes: Vec<Note>,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ext: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TempoItem {
    pub tick: u32,
    pub bpm: f64,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ext: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeterItem {
    pub tick: u32,
    pub num: u32,
    pub den: u32,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ext: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct LoopRegion {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lane {
    pub id: u32,
    pub name: String,
    pub kind: LaneKindSerde,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pan: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverb_send: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mute: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solo: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ext: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaneKindSerde {
    Melodic,
    Drum,
}

/// Note tuple encoding (C1 NOTE_ENCODING): [lane, start, duration, pitch, velocity].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note(pub u32, pub u32, pub u32, pub u8, pub u8);

impl Note {
    pub fn lane(&self) -> u32 {
        self.0
    }
    pub fn start(&self) -> u32 {
        self.1
    }
    pub fn duration(&self) -> u32 {
        self.2
    }
    pub fn pitch(&self) -> u8 {
        self.3
    }
    pub fn velocity(&self) -> u8 {
        self.4
    }
    pub fn end(&self) -> u64 {
        self.1 as u64 + self.2 as u64
    }
}

impl TempoItem {
    /// C0.2: mpq = round(60_000_000 / bpm).
    pub fn mpq(&self) -> u32 {
        (60_000_000.0 / self.bpm).round() as u32
    }
    /// Saved display bpm after mpq normalisation (3 decimals).
    pub fn normalized_bpm(&self) -> f64 {
        let raw = 60_000_000.0 / self.mpq() as f64;
        (raw * 1000.0).round() / 1000.0
    }
}
