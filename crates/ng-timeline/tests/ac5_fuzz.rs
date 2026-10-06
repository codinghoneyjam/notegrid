// <META - ROLE : R02.AC5 Schedule::from_bytes treats its input as untrusted: truncated, tampered and random bytes are rejected, never panicking. | L1-140>
use ng_timeline::{Schedule, ScheduleError};

fn valid_bytes() -> Vec<u8> {
    let s = Schedule::new(
        44100,
        vec![ng_timeline::LaneEntry {
            slot: 0,
            program: 0,
            kind: 0,
            volume: 100,
            pan: 64,
            reverb_send: 0,
            flags: 0,
        }],
        vec![ng_timeline::TempoEntry {
            tick: 0,
            mpq: 500_000,
            sample: 0,
        }],
        vec![ng_timeline::NoteEvent {
            start: 0,
            end: 22050,
            slot: 0,
            pitch: 60,
            vel: 100,
        }],
    )
    .expect("fixture is valid");
    s.to_bytes()
}

#[test]
fn ac5_truncation_at_every_length_is_rejected_without_panicking() {
    let bytes = valid_bytes();
    for cut in 0..bytes.len() {
        // Must be an Err, never a panic and never a silent success.
        let result = Schedule::from_bytes(&bytes[..cut]);
        assert!(result.is_err(), "truncation at {cut} was accepted");
    }
}

#[test]
fn ac5_trailing_bytes_are_rejected() {
    let mut bytes = valid_bytes();
    bytes.push(0x00);
    assert!(matches!(
        Schedule::from_bytes(&bytes),
        Err(ScheduleError::TrailingBytes { .. })
    ));
}

#[test]
fn ac5_bad_magic_version_and_flags_are_rejected() {
    let good = valid_bytes();

    let mut bad_magic = good.clone();
    bad_magic[0] = b'X';
    assert!(matches!(
        Schedule::from_bytes(&bad_magic),
        Err(ScheduleError::BadMagic(_))
    ));

    let mut bad_version = good.clone();
    bad_version[4] = 2;
    assert!(matches!(
        Schedule::from_bytes(&bad_version),
        Err(ScheduleError::BadVersion(2))
    ));

    let mut bad_flags = good.clone();
    bad_flags[6] = 1;
    assert!(matches!(
        Schedule::from_bytes(&bad_flags),
        Err(ScheduleError::BadFlags(_))
    ));

    let mut zero_sr = good.clone();
    zero_sr[8..12].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        Schedule::from_bytes(&zero_sr),
        Err(ScheduleError::ZeroSampleRate)
    ));
}

#[test]
fn ac5_lying_counts_are_rejected() {
    // note_count inflated to a value the buffer cannot back.
    let mut bytes = valid_bytes();
    bytes[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Schedule::from_bytes(&bytes).is_err());

    // A count whose table size overflows usize on a 32-bit target.
    let mut bytes = valid_bytes();
    bytes[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Schedule::from_bytes(&bytes).is_err());

    // u32::MAX note count is a multiply overflow, not a panic.
    let mut bytes = valid_bytes();
    bytes[12..16].copy_from_slice(&(u32::MAX / 4).to_le_bytes());
    assert!(Schedule::from_bytes(&bytes).is_err());
}

#[test]
fn ac5_non_zero_reserved_bytes_are_rejected() {
    let mut bytes = valid_bytes();
    let note_reserved = 32 + 8 + 16 + 20;
    bytes[note_reserved] = 1;
    assert!(matches!(
        Schedule::from_bytes(&bytes),
        Err(ScheduleError::NonZeroReserved { index: 0 })
    ));
}

#[test]
fn ac5_note_slot_beyond_the_lane_table_is_rejected() {
    let mut bytes = valid_bytes();
    let note_slot = 32 + 8 + 16 + 16;
    bytes[note_slot..note_slot + 2].copy_from_slice(&9u16.to_le_bytes());
    assert!(matches!(
        Schedule::from_bytes(&bytes),
        Err(ScheduleError::LaneSlotOutOfRange { .. })
    ));
}

#[test]
fn ac5_random_bytes_never_panic() {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..20_000 {
        let len = (next() % 96) as usize;
        let mut buf = vec![0u8; len];
        for b in &mut buf {
            *b = (next() % 256) as u8;
        }
        let _ = Schedule::from_bytes(&buf);
    }
}

#[test]
fn ac5_single_bit_flips_never_panic() {
    let good = valid_bytes();
    for byte_index in 0..good.len() {
        for bit in 0..8 {
            let mut bytes = good.clone();
            bytes[byte_index] ^= 1 << bit;
            // Either an error, or a value that still validates -- but never a panic.
            let _ = Schedule::from_bytes(&bytes);
        }
    }
}

#[test]
fn ac5_empty_input_is_rejected() {
    assert!(Schedule::from_bytes(&[]).is_err());
}
