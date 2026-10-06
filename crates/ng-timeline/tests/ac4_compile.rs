// <META - ROLE : R02.AC4 slot mapping, mute/solo flags and ordering equal the golden schedule bytes. | L1-120>
#[macro_use]
mod common;

use ng_score::migrate;
use ng_timeline::compile;

fn project(text: &str) -> ng_score::Project {
    migrate(text.as_bytes()).unwrap_or_else(|e| panic!("project parses: {}", e.message))
}

fn golden_path(name: &str) -> std::path::PathBuf {
    common::golden_dir().join(format!("{name}.ngsc"))
}

#[test]
fn ac4_compile_is_deterministic() {
    let p = project(include_root_file!("spec/fixture/example_project.json"));
    let a = compile(&p, 44100).expect("compiles").to_bytes();
    let b = compile(&p, 44100).expect("compiles").to_bytes();
    assert_eq!(a, b, "R02.R01 same input must give the same bytes");
}

#[test]
fn ac4_slot_is_the_lane_array_index() {
    // C2.8: slot = lane index in the lanes array, not the lane id.
    let lanes = format!(
        "[{}, {}, {}]",
        common::melodic_lane(7),
        common::melodic_lane(3),
        common::melodic_lane(11)
    );
    // Note on lane id 3 must land in slot 1.
    let json = common::project_json(
        r#"[ { "tick": 0, "bpm": 120.0 } ]"#,
        &lanes,
        r#"[ [3, 0, 480, 60, 100] ]"#,
        480,
    );
    let s = compile(&project(&json), 44100).expect("compiles");
    assert_eq!(s.lanes.len(), 3);
    assert_eq!(s.lanes[1].slot, 1);
    assert_eq!(s.notes[0].slot, 1, "lane id 3 is at array index 1");
}

#[test]
fn ac4_mute_and_solo_flags_land_in_the_lane_table() {
    let lanes = r#"[
        { "id": 1, "name": "a", "kind": "melodic", "program": 0, "mute": true,  "solo": false },
        { "id": 2, "name": "b", "kind": "melodic", "program": 0, "mute": false, "solo": true  },
        { "id": 3, "name": "c", "kind": "melodic", "program": 0 }
    ]"#;
    let json = common::project_json(
        r#"[ { "tick": 0, "bpm": 120.0 } ]"#,
        lanes,
        r#"[ [1, 0, 480, 60, 100], [2, 0, 480, 62, 100], [3, 0, 480, 64, 100] ]"#,
        480,
    );
    let s = compile(&project(&json), 44100).expect("compiles");
    assert!(s.lanes[0].is_mute() && !s.lanes[0].is_solo());
    assert!(!s.lanes[1].is_mute() && s.lanes[1].is_solo());
    assert!(!s.lanes[2].is_mute() && !s.lanes[2].is_solo());
}

#[test]
fn ac4_drum_lane_carries_the_kit_as_its_program() {
    let lanes = r#"[
        { "id": 1, "name": "a", "kind": "melodic", "program": 33 },
        { "id": 2, "name": "d", "kind": "drum", "kit": 0 }
    ]"#;
    let json = common::project_json(
        r#"[ { "tick": 0, "bpm": 120.0 } ]"#,
        lanes,
        r#"[ [2, 0, 120, 36, 110] ]"#,
        480,
    );
    let s = compile(&project(&json), 44100).expect("compiles");
    assert_eq!(s.lanes[0].kind, 0, "melodic");
    assert_eq!(s.lanes[0].program, 33);
    assert_eq!(s.lanes[1].kind, 1, "drum");
    assert_eq!(s.lanes[1].program, 0, "kit goes in the program field");
}

#[test]
fn ac4_notes_are_sorted_by_start_slot_pitch() {
    // Deliberately out of order in the source, and overlapping starts across slots.
    let lanes = format!("[{}, {}]", common::melodic_lane(1), common::melodic_lane(2));
    let json = common::project_json(
        r#"[ { "tick": 0, "bpm": 120.0 } ]"#,
        &lanes,
        r#"[ [2, 0, 240, 72, 90], [1, 0, 240, 60, 90], [1, 0, 240, 64, 90] ]"#,
        480,
    );
    let s = compile(&project(&json), 44100).expect("compiles");
    let keys: Vec<(u64, u16, u8)> = s.notes.iter().map(|n| (n.start, n.slot, n.pitch)).collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "C3 ORDER notes sorted (start, slot, pitch)");
    // slot 0 first at the equal start, then pitches ascending inside it.
    assert_eq!(keys[0], (0, 0, 60));
    assert_eq!(keys[1], (0, 0, 64));
    assert_eq!(keys[2].2, 72);
    assert_eq!(keys[2].1, 1);
}

#[test]
fn ac4_matches_the_golden_bytes() {
    let p = project(include_root_file!("spec/fixture/example_project.json"));
    let bytes = compile(&p, 44100).expect("compiles").to_bytes();

    let path = golden_path("example_44100");
    match std::fs::read(&path) {
        Ok(golden) => {
            assert_eq!(
                bytes,
                golden,
                "schedule bytes drifted from {}; regenerate it and add a dated entry to \
                 docs/golden_changes.md (gate g12)",
                path.display()
            );
        }
        Err(_) => {
            // First run: write the golden so the comparison exists from now on.
            std::fs::create_dir_all(common::golden_dir()).expect("golden dir");
            std::fs::write(&path, &bytes).expect("golden written");
            panic!(
                "golden {} did not exist and has been created from this run; re-run the test \
                 and record the reason in docs/golden_changes.md (gate g12)",
                path.display()
            );
        }
    }
}

#[test]
fn ac4_round_trips_through_bytes() {
    let p = project(include_root_file!("spec/fixture/example_project.json"));
    let s = compile(&p, 44100).expect("compiles");
    let back = ng_timeline::Schedule::from_bytes(&s.to_bytes()).expect("re-parses");
    assert_eq!(back, s);
}
