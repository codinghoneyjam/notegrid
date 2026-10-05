// <META - ROLE : Structural checks (schema-equivalent) + semantic invariants I-01..I-10, W-1..W-3; collects all issues. | L1-200>

use crate::model::{LaneKindSerde, Project};
use serde_json::Value;
use std::collections::{BTreeSet, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub code: &'static str,
    pub message: String,
    pub path: String,
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub errors: Vec<Issue>,
    pub warnings: Vec<Issue>,
}

impl Report {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn validate(project: &Project) -> Report {
    let mut r = Report::default();
    validate_semantics(project, &mut r);
    r
}

/// Full validation from raw JSON text: structural rules first, then semantics.
pub fn validate_bytes(bytes: &[u8]) -> Report {
    let mut r = Report::default();
    let value: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(e) => {
            r.errors.push(Issue {
                code: "E_FORMAT",
                message: format!("invalid JSON: {e}"),
                path: String::new(),
            });
            return r;
        }
    };
    if let Some(project) = validate_structure(&value, &mut r) {
        validate_semantics(&project, &mut r);
    }
    r
}

fn err(r: &mut Report, code: &'static str, path: &str, msg: impl Into<String>) {
    r.errors.push(Issue {
        code,
        message: msg.into(),
        path: path.to_string(),
    });
}
fn warn(r: &mut Report, code: &'static str, path: &str, msg: impl Into<String>) {
    r.warnings.push(Issue {
        code,
        message: msg.into(),
        path: path.to_string(),
    });
}

const TOP_KEYS: &[&str] = &[
    "format_version",
    "title",
    "ppq",
    "tempo",
    "meter",
    "loop",
    "length_ticks",
    "lanes",
    "notes",
];
const LANE_KEYS: &[&str] = &[
    "id",
    "name",
    "kind",
    "program",
    "kit",
    "volume",
    "pan",
    "reverb_send",
    "mute",
    "solo",
    "color",
];

fn check_unknown(
    r: &mut Report,
    obj: &serde_json::Map<String, Value>,
    allowed: &[&str],
    path: &str,
) {
    for k in obj.keys() {
        if !allowed.contains(&k.as_str()) && !k.starts_with("x_") {
            err(r, "E_UNKNOWN_FIELD", path, format!("unknown field `{k}`"));
        }
    }
}

fn require<'v>(
    r: &mut Report,
    obj: &'v serde_json::Map<String, Value>,
    key: &str,
    path: &str,
) -> Option<&'v Value> {
    match obj.get(key) {
        Some(v) => Some(v),
        None => {
            err(
                r,
                "E_FORMAT",
                path,
                format!("missing required field `{key}`"),
            );
            None
        }
    }
}

fn as_u64(v: &Value) -> Option<u64> {
    v.as_u64()
}

fn validate_structure(v: &Value, r: &mut Report) -> Option<Project> {
    let Some(root) = v.as_object() else {
        err(r, "E_FORMAT", "", "root must be an object");
        return None;
    };
    check_unknown(r, root, TOP_KEYS, "");

    let mut ok = true;
    let fv = require(r, root, "format_version", "/format_version");
    match fv {
        Some(Value::Number(n)) if n.as_u64().is_some() => {
            let n = n.as_u64().unwrap();
            if n > crate::model::FORMAT_VERSION_SUPPORTED as u64 {
                err(
                    r,
                    "E_VERSION_TOO_NEW",
                    "/format_version",
                    "version newer than supported",
                );
                ok = false;
            } else if n != 1 {
                err(r, "E_FORMAT", "/format_version", "format_version must be 1");
                ok = false;
            }
        }
        Some(_) => {
            err(
                r,
                "E_FORMAT",
                "/format_version",
                "format_version must be an integer",
            );
            ok = false;
        }
        None => ok = false,
    }
    match require(r, root, "title", "/title") {
        Some(Value::String(s)) if !s.is_empty() => {}
        Some(_) => {
            err(r, "E_FORMAT", "/title", "title must be a non-empty string");
            ok = false;
        }
        None => ok = false,
    }
    match require(r, root, "ppq", "/ppq") {
        Some(Value::Number(n)) if n.as_u64() == Some(480) => {}
        Some(_) => {
            err(r, "E_FORMAT", "/ppq", "ppq must be 480");
            ok = false;
        }
        None => ok = false,
    }
    let tempo = require(r, root, "tempo", "/tempo");
    let meter = require(r, root, "meter", "/meter");
    if let Some(Value::Array(t)) = tempo {
        if t.is_empty() {
            err(r, "E_FORMAT", "/tempo", "tempo must be non-empty");
            ok = false;
        }
        for (i, item) in t.iter().enumerate() {
            let p = format!("/tempo/{i}");
            let Some(obj) = item.as_object() else {
                err(r, "E_FORMAT", &p, "tempo item must be an object");
                ok = false;
                continue;
            };
            check_unknown(r, obj, &["tick", "bpm"], &p);
            match obj.get("tick").and_then(as_u64) {
                Some(_) => {}
                None => {
                    err(
                        r,
                        "E_FORMAT",
                        &format!("{p}/tick"),
                        "tick must be a non-negative integer",
                    );
                    ok = false;
                }
            }
            match obj.get("bpm").and_then(|n| n.as_f64()) {
                Some(b) if (20.0..=400.0).contains(&b) => {}
                Some(_) => {
                    err(r, "E_RANGE", &format!("{p}/bpm"), "bpm out of [20,400]");
                    ok = false;
                }
                None => {
                    err(r, "E_FORMAT", &format!("{p}/bpm"), "bpm must be a number");
                    ok = false;
                }
            }
        }
    } else {
        err(r, "E_FORMAT", "/tempo", "tempo must be an array");
        ok = false;
    }
    if let Some(Value::Array(m)) = meter {
        if m.is_empty() {
            err(r, "E_FORMAT", "/meter", "meter must be non-empty");
            ok = false;
        }
        for (i, item) in m.iter().enumerate() {
            let p = format!("/meter/{i}");
            let Some(obj) = item.as_object() else {
                err(r, "E_FORMAT", &p, "meter item must be an object");
                ok = false;
                continue;
            };
            check_unknown(r, obj, &["tick", "num", "den"], &p);
            match obj.get("tick").and_then(as_u64) {
                Some(_) => {}
                None => {
                    err(
                        r,
                        "E_FORMAT",
                        &format!("{p}/tick"),
                        "tick must be a non-negative integer",
                    );
                    ok = false;
                }
            }
            match obj.get("num").and_then(as_u64) {
                Some(n) if (1..=32).contains(&n) => {}
                _ => {
                    err(r, "E_RANGE", &format!("{p}/num"), "num out of [1,32]");
                    ok = false;
                }
            }
            match obj.get("den").and_then(as_u64) {
                Some(d) if [1, 2, 4, 8, 16, 32].contains(&d) => {}
                _ => {
                    err(
                        r,
                        "E_FORMAT",
                        &format!("{p}/den"),
                        "den must be a power of two in [1,32]",
                    );
                    ok = false;
                }
            }
        }
    } else {
        err(r, "E_FORMAT", "/meter", "meter must be an array");
        ok = false;
    }
    match require(r, root, "lanes", "/lanes") {
        Some(Value::Array(lanes)) => {
            if lanes.is_empty() {
                err(r, "E_RANGE", "/lanes", "at least one lane required");
                ok = false;
            } else if lanes.len() > 128 {
                err(r, "E_RANGE", "/lanes", "more than 128 lanes");
                ok = false;
            }
            for (i, lane) in lanes.iter().enumerate() {
                let p = format!("/lanes/{i}");
                let Some(obj) = lane.as_object() else {
                    err(r, "E_FORMAT", &p, "lane must be an object");
                    ok = false;
                    continue;
                };
                check_unknown(r, obj, LANE_KEYS, &p);
                match obj.get("id").and_then(as_u64) {
                    Some(v) if v >= 1 => {}
                    _ => {
                        err(r, "E_RANGE", &format!("{p}/id"), "id must be >= 1");
                        ok = false;
                    }
                }
                match obj.get("name") {
                    Some(Value::String(s)) if !s.is_empty() => {}
                    _ => {
                        err(
                            r,
                            "E_FORMAT",
                            &format!("{p}/name"),
                            "name must be a non-empty string",
                        );
                        ok = false;
                    }
                }
                let kind = obj.get("kind").and_then(|k| k.as_str());
                match kind {
                    Some("melodic") => match obj.get("program").and_then(as_u64) {
                        Some(v) if v <= 127 => {}
                        _ => {
                            err(r, "E_RANGE", &format!("{p}/program"), "program in [0,127]");
                            ok = false;
                        }
                    },
                    Some("drum") => match obj.get("kit").and_then(as_u64) {
                        Some(v) if v <= 127 => {}
                        _ => {
                            err(r, "E_RANGE", &format!("{p}/kit"), "kit in [0,127]");
                            ok = false;
                        }
                    },
                    _ => {
                        err(
                            r,
                            "E_FORMAT",
                            &format!("{p}/kind"),
                            "kind must be melodic|drum",
                        );
                        ok = false;
                    }
                }
                for key in ["volume", "pan", "reverb_send"] {
                    if let Some(v) = obj.get(key) {
                        match as_u64(v) {
                            Some(x) if x <= 127 => {}
                            _ => {
                                err(r, "E_RANGE", &format!("{p}/{key}"), "0..=127");
                                ok = false;
                            }
                        }
                    }
                }
            }
        }
        Some(_) => {
            err(r, "E_FORMAT", "/lanes", "lanes must be an array");
            ok = false;
        }
        None => ok = false,
    }
    if let Some(Value::Object(lp)) = root.get("loop") {
        match lp.get("start").and_then(as_u64) {
            Some(_) => {}
            None => {
                err(
                    r,
                    "E_FORMAT",
                    "/loop/start",
                    "start must be a non-negative integer",
                );
                ok = false;
            }
        }
        match lp.get("end").and_then(as_u64) {
            Some(v) if v >= 1 => {}
            _ => {
                err(r, "E_RANGE", "/loop/end", "end must be >= 1");
                ok = false;
            }
        }
    } else if let Some(v) = root.get("loop") {
        if !v.is_null() {
            err(r, "E_FORMAT", "/loop", "loop must be null or an object");
            ok = false;
        }
    }
    if let Some(v) = root.get("length_ticks") {
        match as_u64(v) {
            Some(n) if n >= 1 => {}
            _ => {
                err(r, "E_RANGE", "/length_ticks", "must be >= 1");
                ok = false;
            }
        }
    }
    match require(r, root, "notes", "/notes") {
        Some(Value::Array(notes)) => {
            if notes.len() > 1_000_000 {
                err(r, "E_RANGE", "/notes", "more than 1_000_000 notes");
                ok = false;
            }
            for (i, n) in notes.iter().enumerate() {
                let p = format!("/notes/{i}");
                let Some(arr) = n.as_array() else {
                    err(r, "E_FORMAT", &p, "note must be a 5-tuple");
                    ok = false;
                    continue;
                };
                if arr.len() != 5 {
                    err(r, "E_FORMAT", &p, "note must have exactly 5 elements");
                    ok = false;
                    continue;
                }
                match as_u64(&arr[0]) {
                    Some(v) if v >= 1 => {}
                    _ => {
                        err(r, "E_RANGE", &format!("{p}/0"), "lane id >= 1");
                        ok = false;
                    }
                }
                match as_u64(&arr[1]) {
                    Some(_) => {}
                    None => {
                        err(r, "E_FORMAT", &format!("{p}/1"), "start must be an integer");
                        ok = false;
                    }
                }
                match as_u64(&arr[2]) {
                    Some(v) if v >= 1 && v <= u32::MAX as u64 => {}
                    _ => {
                        err(r, "E_RANGE", &format!("{p}/2"), "duration in [1,u32::MAX]");
                        ok = false;
                    }
                }
                match as_u64(&arr[3]) {
                    Some(v) if v <= 127 => {}
                    _ => {
                        err(r, "E_RANGE", &format!("{p}/3"), "pitch 0..=127");
                        ok = false;
                    }
                }
                match as_u64(&arr[4]) {
                    Some(v) if (1..=127).contains(&v) => {}
                    _ => {
                        err(r, "E_RANGE", &format!("{p}/4"), "velocity 1..=127");
                        ok = false;
                    }
                }
            }
        }
        Some(_) => {
            err(r, "E_FORMAT", "/notes", "notes must be an array");
            ok = false;
        }
        None => ok = false,
    }
    if !ok {
        return None;
    }
    match serde_json::from_value::<Project>(v.clone()) {
        Ok(p) => Some(p),
        Err(e) => {
            err(r, "E_FORMAT", "", format!("model parse failed: {e}"));
            None
        }
    }
}

fn bar_ticks(num: u64, den: u64) -> u64 {
    num * (1920 / den)
}

fn validate_semantics(p: &Project, r: &mut Report) {
    if p.lanes.is_empty() {
        return;
    }
    let mut ids = HashSet::new();
    for (i, lane) in p.lanes.iter().enumerate() {
        if !ids.insert(lane.id) {
            err(
                r,
                "E_LANE_DUP",
                &format!("/lanes/{i}/id"),
                "duplicate lane id",
            );
        }
    }
    if p.lanes.len() > 128 {
        err(r, "E_RANGE", "/lanes", "more than 128 lanes");
    }
    if p.tempo.is_empty() || p.tempo[0].tick != 0 {
        err(r, "E_TEMPO", "/tempo/0/tick", "tempo[0].tick must be 0");
    }
    for w in p.tempo.windows(2).enumerate() {
        if w.1[1].tick <= w.1[0].tick {
            err(
                r,
                "E_TEMPO",
                &format!("/tempo/{}/tick", w.0 + 1),
                "ticks strictly increasing",
            );
        }
    }
    for (i, t) in p.tempo.iter().enumerate() {
        if (t.normalized_bpm() - t.bpm).abs() > 0.0005 {
            warn(
                r,
                "W_BPM_ROUNDED",
                &format!("/tempo/{i}/bpm"),
                "bpm changed by mpq normalisation",
            );
        }
    }
    if p.meter.is_empty() || p.meter[0].tick != 0 {
        err(r, "E_METER", "/meter/0/tick", "meter[0].tick must be 0");
    }
    for w in p.meter.windows(2).enumerate() {
        if w.1[1].tick <= w.1[0].tick {
            err(
                r,
                "E_METER",
                &format!("/meter/{}/tick", w.0 + 1),
                "ticks increasing",
            );
        }
        let prev = &w.1[0];
        let bar = bar_ticks(prev.num as u64, prev.den as u64);
        if bar > 0 && !(w.1[1].tick as u64).is_multiple_of(bar) {
            err(
                r,
                "E_METER",
                &format!("/meter/{}/tick", w.0 + 1),
                "change must sit on a bar boundary of the previous meter",
            );
        }
    }
    let lane_ids: HashSet<u32> = p.lanes.iter().map(|l| l.id).collect();
    let mut max_end: u64 = 0;
    for (i, n) in p.notes.iter().enumerate() {
        if !lane_ids.contains(&n.lane()) {
            err(
                r,
                "E_LANE_MISSING",
                &format!("/notes/{i}/0"),
                "note references unknown lane id",
            );
        }
        let end = n.end();
        if end > u32::MAX as u64 + n.start() as u64 {
            // start itself is u32; overflow guard
        }
        if n.start() as u64 + n.duration() as u64 > u32::MAX as u64 {
            err(
                r,
                "E_RANGE",
                &format!("/notes/{i}"),
                "start + duration overflows u32",
            );
        }
        max_end = max_end.max(end);
        let kind = p.lanes.iter().find(|l| l.id == n.lane()).map(|l| l.kind);
        if kind == Some(LaneKindSerde::Drum) && !(27..=87).contains(&n.pitch()) {
            warn(
                r,
                "W_DRUM_KEY_RANGE",
                &format!("/notes/{i}/3"),
                "drum key outside 27-87",
            );
        }
    }
    // Overlap per (lane, pitch)
    let mut buckets: BTreeSet<(u32, u8, u32, u32)> = BTreeSet::new();
    let mut per_lane_pitch: std::collections::HashMap<(u32, u8), Vec<(u32, u32)>> =
        std::collections::HashMap::new();
    for (i, n) in p.notes.iter().enumerate() {
        let v = per_lane_pitch.entry((n.lane(), n.pitch())).or_default();
        v.push((n.start(), n.end().min(u32::MAX as u64) as u32));
        buckets.insert((n.lane(), n.pitch(), n.start(), i as u32));
    }
    for ((lane, pitch), mut spans) in per_lane_pitch {
        spans.sort_by_key(|s| s.0);
        for w in spans.windows(2) {
            if w[1].0 < w[0].1 {
                err(
                    r,
                    "E_OVERLAP",
                    &format!("/notes/{lane}/{pitch}"),
                    "same lane+pitch overlapping notes",
                );
            }
        }
    }
    // W-2 unused lanes
    let used: HashSet<u32> = p.notes.iter().map(|n| n.lane()).collect();
    for (i, l) in p.lanes.iter().enumerate() {
        if !used.contains(&l.id) {
            warn(
                r,
                "W_UNUSED_LANE",
                &format!("/lanes/{i}"),
                "lane without notes",
            );
        }
    }
    // Effective length (I-07)
    let eff_len = match p.length_ticks {
        Some(lt) => lt,
        None => {
            let m = p
                .meter
                .iter()
                .rev()
                .find(|m| (m.tick as u64) <= max_end)
                .unwrap_or(&p.meter[0]);
            let bar = bar_ticks(m.num as u64, m.den as u64).max(1);
            max_end.div_ceil(bar) * bar
        }
    };
    if let Some(lt) = p.length_ticks {
        if max_end > lt {
            warn(
                r,
                "W_NOTE_PAST_LENGTH",
                "/length_ticks",
                "notes extend past declared length_ticks",
            );
        }
    }
    if let Some(lp) = p.loop_region {
        if lp.start >= lp.end || lp.end as u64 > eff_len {
            err(
                r,
                "E_LOOP",
                "/loop",
                "require start < end <= effective length",
            );
        }
    }
}
