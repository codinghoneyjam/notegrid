// <META - FILE SUMMARY - Centralised test paths and helper macros for ng-score tests.>
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

pub fn spec_dir() -> PathBuf {
    root_dir().join("spec")
}

pub fn fixture_dir() -> PathBuf {
    spec_dir().join("fixture")
}

pub fn schema_dir() -> PathBuf {
    spec_dir().join("schema")
}

pub fn example_project_path() -> PathBuf {
    fixture_dir().join("example_project.json")
}

pub fn project_schema_path() -> PathBuf {
    schema_dir().join("project.schema.json")
}

pub fn validation_vectors_dir() -> PathBuf {
    fixture_dir().join("validation")
}

#[macro_export]
macro_rules! include_root_file {
    ($rel_path:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../", $rel_path))
    };
}
