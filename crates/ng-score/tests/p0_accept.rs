// <META - ROLE : P0 acceptance tests: AC1 example project, AC2 vectors vs JSON Schema, AC3 fuzz-smoke, AC4 GM tables, canonical stability. | L1-90>

const EXAMPLE: &str = include_str!("../../../fixture/example_project.json");

#[test]
fn ac1_example_project_passes_and_canonical_stable() {
    let project = ng_score::migrate(EXAMPLE.as_bytes()).expect("example parses");
    let report = ng_score::validate(&project);
    assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
    let c1 = ng_score::to_canonical_json(&project);
    let p2 = ng_score::migrate(c1.as_bytes()).expect("canonical re-parses");
    let c2 = ng_score::to_canonical_json(&p2);
    assert_eq!(c1, c2, "canonical form must be load->save stable");
}

#[test]
fn ac2_validation_vectors_match_schema_verdict() {
    let schema_text = include_str!("../../../schema/project.schema.json");
    let schema_value: serde_json::Value = serde_json::from_str(schema_text).unwrap();
    let validator = jsonschema::validator_for(&schema_value).expect("schema compiles");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixture/validation");
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        count += 1;
        let text = std::fs::read_to_string(entry.path()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let input = &v["input"];
        let expected: Vec<&str> = v["expect_codes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap())
            .collect();
        let schema_invalid = v["schema_invalid"].as_bool().unwrap();
        let input_bytes = serde_json::to_vec(input).unwrap();
        let report = ng_score::validate_bytes(&input_bytes);
        for code in &expected {
            assert!(
                report.errors.iter().any(|e| e.code == *code)
                    || report.warnings.iter().any(|w| w.code == *code),
                "{}: expected code {} in {:?}",
                entry.path().display(),
                code,
                report.errors
            );
        }
        let schema_failed = !validator.is_valid(input);
        assert_eq!(
            schema_failed,
            schema_invalid,
            "{}: schema verdict mismatch",
            entry.path().display()
        );
        // g09 verdict rule: schema failure <=> Rust has at least one error
        if schema_invalid && expected.iter().all(|c| !c.starts_with("W_")) {
            assert!(
                !report.errors.is_empty(),
                "{}: schema failed but Rust accepted",
                entry.path().display()
            );
        }
    }
    assert!(count >= 40, "need >= 40 vectors, got {count}");
}

#[test]
fn ac3_fuzz_smoke_no_panic() {
    // AC3 fuzz: 1e6 iterations is run via NOTEGRID_FUZZ_N; default smoke is fast.
    let n: usize = std::env::var("NOTEGRID_FUZZ_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000);
    let mut state: u64 = 0x9E3779B97F4A7C15;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..n {
        let len = (next() % 256) as usize;
        let mut buf = Vec::with_capacity(len);
        for _ in 0..len {
            buf.push((next() % 256) as u8);
        }
        let _ = ng_score::validate_bytes(&buf);
        let _ = ng_score::migrate(&buf);
    }
}

#[test]
fn ac4_gm_tables_sizes() {
    let t = ng_score::gm_tables();
    assert_eq!(t["programs"].as_array().unwrap().len(), 128);
    assert_eq!(t["families"].as_array().unwrap().len(), 16);
}

#[test]
fn proptest_random_bytes_roundtrip_no_panic() {
    use proptest::prelude::*;
    proptest!(|(bytes in proptest::collection::vec(any::<u8>(), 0..512))| {
        let _ = ng_score::validate_bytes(&bytes);
        let _ = ng_score::migrate(&bytes);
    });
}
