// <META - FILE SUMMARY - Test path helpers for ng-wasm integration tests; mirrors the ng-score test helpers.>
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
