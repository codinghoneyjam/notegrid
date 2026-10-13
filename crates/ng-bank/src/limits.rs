// <META - FILE SUMMARY - C10.2 source and image limits, plus the structural caps that keep an untrusted header from allocating.>

/// C10.2: a source bank may not exceed 256 MB. The parse API rejects above this.
pub const MAX_SOURCE_BYTES: u64 = 256 * 1024 * 1024;

/// C10.2: a source above 64 MB is legal but should be warned about.
pub const WARN_SOURCE_BYTES: u64 = 64 * 1024 * 1024;

/// C10.2: the recommended ceiling for a built image; each engine thread holds a copy.
pub const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

/// Longest bank name accepted from an INFO/INAM string (SF2 2.01 sec 5.3 caps it at 256 bytes
/// including the terminator).
pub const MAX_NAME_BYTES: usize = 255;

/// Structural cap on any one pdta sub-chunk. 2^22 records of the widest record (shdr, 46 B) is
/// already 192 MB, so this only ever bites a header that claims more records than C10.2 could
/// possibly carry. It exists so a corrupt count cannot turn into a huge `Vec::with_capacity`.
pub const MAX_RECORDS: usize = 1 << 22;

/// Structural cap on the number of samples in a bank (Realistic GM banks are well under 5000).
pub const MAX_SAMPLES: usize = 1 << 20;

/// Structural cap on the number of presets in a bank (C0.6: 128 melodic programs per bank).
pub const MAX_PRESETS: usize = 1 << 16;

/// Every table in the image starts on this boundary (C10.1 "aligned sample blob").
pub const IMAGE_ALIGN: usize = 16;

/// True when a source of `bytes` bytes is over the warn line but still acceptable (C10.2).
pub fn source_is_oversized(bytes: usize) -> bool {
    bytes as u64 > WARN_SOURCE_BYTES
}