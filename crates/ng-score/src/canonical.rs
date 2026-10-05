// <META - ROLE : Canonical project serialisation (C1 CANONICAL): schema key order, 2-space indent, notes sorted, LF + final newline. | L1-90>

use crate::model::{Lane, LaneKindSerde, Project};
use std::collections::BTreeMap;

pub fn to_canonical_json(p: &Project) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"format_version\": {},\n", p.format_version));
    out.push_str(&format!("  \"title\": {},\n", jstr(&p.title)));
    out.push_str(&format!("  \"ppq\": {},\n", p.ppq));
    out.push_str("  \"tempo\": [\n");
    for (i, t) in p.tempo.iter().enumerate() {
        let comma = if i + 1 < p.tempo.len() { "," } else { "" };
        out.push_str(&format!(
            "    {{ \"tick\": {}, \"bpm\": {} }}{}\n",
            t.tick,
            jbpm(t.normalized_bpm()),
            comma
        ));
    }
    out.push_str("  ],\n");
    out.push_str("  \"meter\": [\n");
    for (i, m) in p.meter.iter().enumerate() {
        let comma = if i + 1 < p.meter.len() { "," } else { "" };
        out.push_str(&format!(
            "    {{ \"tick\": {}, \"num\": {}, \"den\": {} }}{}\n",
            m.tick, m.num, m.den, comma
        ));
    }
    out.push_str("  ],\n");
    if let Some(lp) = p.loop_region {
        out.push_str(&format!(
            "  \"loop\": {{ \"start\": {}, \"end\": {} }},\n",
            lp.start, lp.end
        ));
    }
    if let Some(lt) = p.length_ticks {
        out.push_str(&format!("  \"length_ticks\": {lt},\n"));
    }
    out.push_str("  \"lanes\": [\n");
    for (i, l) in p.lanes.iter().enumerate() {
        out.push_str(&lane_json(l));
        if i + 1 < p.lanes.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ],\n");
    out.push_str("  \"notes\": [\n");
    let mut notes: Vec<_> = p.notes.iter().collect();
    notes.sort_by_key(|n| (n.start(), n.lane(), n.pitch()));
    let total = notes.len();
    for (i, n) in notes.iter().enumerate() {
        let comma = if i + 1 < total { "," } else { "" };
        out.push_str(&format!(
            "    [{}, {}, {}, {}, {}]{}\n",
            n.lane(),
            n.start(),
            n.duration(),
            n.pitch(),
            n.velocity(),
            comma
        ));
    }
    out.push_str("  ]");
    for (k, v) in &p.ext {
        // x_ fields at the end, compact
        if i_want_ext(k) {
            out.push_str(&format!(
                ",\n  {}: {}",
                jstr(k),
                serde_json::to_string(v).unwrap_or_default()
            ));
        }
    }
    out.push_str("\n}\n");
    out
}

fn i_want_ext(k: &str) -> bool {
    k.starts_with("x_")
}

fn lane_json(l: &Lane) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!("\"id\": {}", l.id));
    parts.push(format!("\"name\": {}", jstr(&l.name)));
    match l.kind {
        LaneKindSerde::Melodic => {
            parts.push("\"kind\": \"melodic\"".to_string());
            if let Some(p) = l.program {
                parts.push(format!("\"program\": {p}"));
            }
        }
        LaneKindSerde::Drum => {
            parts.push("\"kind\": \"drum\"".to_string());
            if let Some(k) = l.kit {
                parts.push(format!("\"kit\": {k}"));
            }
        }
    }
    if let Some(v) = l.volume {
        parts.push(format!("\"volume\": {v}"));
    }
    if let Some(v) = l.pan {
        parts.push(format!("\"pan\": {v}"));
    }
    if let Some(v) = l.reverb_send {
        parts.push(format!("\"reverb_send\": {v}"));
    }
    if let Some(v) = l.mute {
        parts.push(format!("\"mute\": {v}"));
    }
    if let Some(v) = l.solo {
        parts.push(format!("\"solo\": {v}"));
    }
    if let Some(c) = &l.color {
        parts.push(format!("\"color\": {}", jstr(c)));
    }
    for (k, v) in &l.ext {
        if i_want_ext(k) {
            parts.push(format!(
                "{}: {}",
                jstr(k),
                serde_json::to_string(v).unwrap_or_default()
            ));
        }
    }
    format!("    {{ {} }}", parts.join(", "))
}

fn jstr(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

fn jbpm(b: f64) -> String {
    let v = (b * 1000.0).round() / 1000.0;
    if v.fract() == 0.0 {
        format!("{:.1}", v)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Keep BTreeMap import used.
#[allow(dead_code)]
fn _marker(_: &BTreeMap<String, serde_json::Value>) {}
