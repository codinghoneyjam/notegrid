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
        Some("render") => {
            // usage: ng-cli render <project.json> [--sr 44100] [--depth 16|24|f32] [--out NAME]
            let mut path: Option<String> = None;
            let mut sr = 44_100u32;
            let mut depth = ng_export::wav::WavDepth::Int16;
            let mut out_name: Option<String> = None;
            let mut args = args;
            while let Some(a) = args.next() {
                match a.as_str() {
                    "--sr" => {
                        sr = match args.next().and_then(|s| s.parse().ok()) {
                            Some(n) => n,
                            None => {
                                eprintln!("--sr needs a number");
                                exit(2);
                            }
                        }
                    }
                    "--depth" => {
                        depth = match args.next().as_deref() {
                            Some("16") => ng_export::wav::WavDepth::Int16,
                            Some("24") => ng_export::wav::WavDepth::Int24,
                            Some("f32" | "float32" | "float") => ng_export::wav::WavDepth::Float32,
                            _ => {
                                eprintln!("--depth must be 16, 24, or f32");
                                exit(2);
                            }
                        }
                    }
                    "--out" => {
                        out_name = args.next();
                    }
                    _ if path.is_none() => path = Some(a),
                    _ => {
                        eprintln!("unexpected arg: {a}");
                        exit(2);
                    }
                }
            }
            let Some(path) = path else {
                eprintln!(
                    "usage: ng-cli render <project.json> [--sr N] [--depth 16|24|f32] [--out NAME]"
                );
                exit(2);
            };
            let bytes = match fs::read(&path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("read error: {e}");
                    exit(1);
                }
            };
            match render_project(&bytes, sr, depth) {
                Ok(wav) => {
                    let project = ng_score::migrate(&bytes).ok();
                    let stem = match out_name {
                        Some(n) => n,
                        None => project
                            .as_ref()
                            .map(|p| slug(&p.title))
                            .unwrap_or_else(|| "untitled".to_string()),
                    };
                    let name = format!("{stem}.wav");
                    match write_atomic(&name, &wav) {
                        Ok(p) => println!("{}", p.display()),
                        Err(e) => {
                            eprintln!("write error: {e}");
                            exit(1);
                        }
                    }
                }
                Err((code, msg)) => {
                    eprintln!("render error {code}: {msg}");
                    exit(1);
                }
            }
        }
        _ => {
            eprintln!("usage: ng-cli <canonical|render> <args>");
            exit(2);
        }
    }
}

/// C12.1: lower-case [a-z0-9_-], <= 64 chars, empty -> "untitled". Runs of other
/// characters collapse into a single '-' so "Example: 4 lanes" does not become "a--b".
fn slug(title: &str) -> String {
    let mut out = String::new();
    for ch in title.chars() {
        if out.len() >= 64 {
            break;
        }
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed.to_string()
    }
}

/// C12.4 atomic write: .tmp then rename, inside ROOT/music_output only.
fn write_atomic(name: &str, bytes: &[u8]) -> std::io::Result<std::path::PathBuf> {
    let root = find_root();
    let out_dir = root.join("music_output");
    fs::create_dir_all(&out_dir)?;
    let tmp = out_dir.join(format!("{name}.tmp"));
    let final_path = out_dir.join(name);
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, &final_path)?;
    Ok(final_path)
}

// Nearest ancestor holding notegrid.root (C12.2); falls back to CWD.
fn find_root() -> std::path::PathBuf {
    let mut cur = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    loop {
        if cur.join("notegrid.root").exists() {
            return cur;
        }
        match cur.parent() {
            Some(p) => cur = p.to_path_buf(),
            None => return std::env::current_dir().unwrap_or_default(),
        }
    }
}

/// Composition glue for the native headless tool; a composition root per
/// 01_system.dsl, so it wires OscSynth into the Renderer and out through the encoder.
fn render_project(
    bytes: &[u8],
    sample_rate: u32,
    depth: ng_export::wav::WavDepth,
) -> Result<Vec<u8>, (&'static str, String)> {
    let project = ng_score::migrate(bytes).map_err(|e| (e.code, e.message))?;
    let report = ng_score::validate(&project);
    if !report.errors.is_empty() {
        return Err((
            "E_FORMAT",
            report
                .errors
                .iter()
                .map(|e| format!("{} at {}", e.code, e.path))
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    let schedule =
        ng_timeline::compile(&project, sample_rate).map_err(|e| ("E_FORMAT", format!("{e}")))?;
    let loop_points = match (
        ng_timeline::TempoMap::from_project(&project, sample_rate),
        project.loop_region,
    ) {
        (Ok(tm), Some(region)) => Some((
            tm.tick_to_sample(region.start) as u32,
            tm.tick_to_sample(region.end) as u32,
        )),
        _ => None,
    };
    let synth = ng_synth::osc::OscSynth::new(sample_rate, 64);
    let pcm = ng_render::render_offline(
        schedule,
        synth,
        &ng_render::OfflineOptions::default(),
        |_| true,
    )
    .map_err(|e| ("E_RENDER", format!("{e:?}")))?;
    let meta = ng_core::EncodeMeta {
        title: Some(project.title.clone()),
        loop_points,
    };
    let enc = ng_export::wav::WavEncoder::new();
    ng_core::AudioEncoder::encode(
        &enc,
        &pcm,
        &meta,
        &ng_export::wav::WavOptions {
            depth,
            ..Default::default()
        },
    )
    .map_err(|e| (e.code, e.message))
}
