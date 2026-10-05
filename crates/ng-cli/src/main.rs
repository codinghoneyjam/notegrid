// <META - FILE SUMMARY - ng-cli entry; P0: `canonical` subcommand only; render arrives in P1.>

use std::env;
use std::fs;
use std::process::exit;

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("canonical") => {
            let Some(path) = args.next() else {
                eprintln!("usage: ng-cli canonical <project.json>");
                exit(2);
            };
            let bytes = match fs::read(&path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("read error: {e}");
                    exit(1);
                }
            };
            match ng_score::migrate(&bytes) {
                Ok(project) => {
                    let report = ng_score::validate(&project);
                    // warnings do not block canonical output; report to stderr
                    for w in &report.warnings {
                        eprintln!("WARN {} at {}: {}", w.code, w.path, w.message);
                    }
                    print!("{}", ng_score::to_canonical_json(&project));
                }
                Err(e) => {
                    eprintln!("migrate error {}: {}", e.code, e.message);
                    exit(1);
                }
            }
        }
        _ => {
            eprintln!("usage: ng-cli <canonical> <args>");
            exit(2);
        }
    }
}
