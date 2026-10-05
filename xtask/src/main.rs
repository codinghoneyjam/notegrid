// <META - FILE SUMMARY - xtask entry: test-all runs fmt/clippy/tests/gates dry-run.>

use std::env;
use std::process::{exit, Command};

fn run(program: &str, args: &[&str]) {
    let status = Command::new(program)
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn {program}: {e}"));
    if !status.success() {
        eprintln!("command failed: {program} {args:?}");
        exit(1);
    }
}

fn main() {
    let task = env::args().nth(1).unwrap_or_else(|| "help".to_string());
    match task.as_str() {
        "build-wasm" => {
            run(
                "cargo",
                &[
                    "build",
                    "--target",
                    "wasm32-unknown-unknown",
                    "--release",
                    "-p",
                    "ng-wasm",
                ],
            );
            let src = "target/wasm32-unknown-unknown/release/ng_wasm.wasm";
            let dst = "packages/abi/dist/ng-wasm.wasm";
            std::fs::create_dir_all("packages/abi/dist").ok();
            std::fs::copy(src, dst).unwrap_or_else(|e| panic!("copy wasm: {e}"));
        }
        "test-all" => {
            run("cargo", &["fmt", "--check"]);
            run(
                "cargo",
                &[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings",
                ],
            );
            run("cargo", &["test", "--workspace"]);
        }
        _ => {
            eprintln!("usage: cargo xtask <test-all>");
            exit(2);
        }
    }
}
