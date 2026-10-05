// <META - ROLE : R07.AC2(partial) ABI fuzz: truncated/random JSON through validate/migrate/canonical, 0 traps. | L1-30>

#[test]
fn abi_fuzz_truncated_and_random_inputs_no_trap() {
    let mut state: u64 = 0xD1B54A32D192ED03;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..5000 {
        let mut data = vec![0u8; (next() % 512) as usize];
        for b in &mut data {
            *b = (next() % 256) as u8;
        }
        let _ = ng_score::validate_bytes(&data);
        let _ = ng_score::migrate(&data);
        // and canonical (must never panic)
        if let Ok(p) = ng_score::migrate(&data) {
            let _ = ng_score::to_canonical_json(&p);
        }
    }
    // also: truncated example project at every length
    let example = include_str!("../../../fixture/example_project.json");
    for cut in 0..example.len() {
        let _ = ng_score::validate_bytes(&example.as_bytes()[..cut]);
        let _ = ng_score::migrate(&example.as_bytes()[..cut]);
    }
}
