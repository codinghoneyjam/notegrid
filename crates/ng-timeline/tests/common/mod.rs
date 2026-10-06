// <META - ROLE : Shared paths and helpers for ng-timeline acceptance tests (R02.AC1..AC5).>
#![allow(dead_code)]
use std::path::{Path, PathBuf};

pub fn root_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .parent()
        .expect("workspace root dir")
        .to_path_buf()
}

pub fn tempo_vectors_dir() -> PathBuf {
    root_dir().join("spec").join("fixture").join("tempo")
}

pub fn golden_dir() -> PathBuf {
    root_dir()
        .join("spec")
        .join("fixture")
        .join("golden")
        .join("schedule")
}

pub fn example_project_path() -> PathBuf {
    root_dir()
        .join("spec")
        .join("fixture")
        .join("example_project.json")
}

#[macro_export]
macro_rules! include_root_file {
    ($rel_path:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../", $rel_path))
    };
}

/// A minimal valid project: `lanes` melodic lanes with ids 1..=n, `notes` as (lane, start,
/// duration, pitch, velocity), one tempo at 124 bpm. Kept literal rather than derived so the
/// tests do not depend on ng-score's model for building inputs.
pub fn project_json(tempo: &str, lanes: &str, notes: &str, length_ticks: u32) -> String {
    format!(
        r#"{{
  "format_version": 1,
  "title": "t",
  "ppq": 480,
  "tempo": {tempo},
  "meter": [ {{ "tick": 0, "num": 4, "den": 4 }} ],
  "length_ticks": {length_ticks},
  "lanes": {lanes},
  "notes": {notes}
}}"#
    )
}

pub fn melodic_lane(id: u32) -> String {
    format!(r#"{{ "id": {id}, "name": "L{id}", "kind": "melodic", "program": 0 }}"#)
}
