# SP-4 spike report -- SF3 decoder and memory expansion

Date: 2026-10-12 | Owner: P3/T3.0

## Question
Can a 64 MB-class SF3 be decoded within memory budget?

## Background
SF3 (SoundFont 3) is an unofficial extension of SF2 that uses Ogg Vorbis compression for sample data.
The project's R03 spec says: "SF3 = Ogg-Vorbis-compressed samples: decode to i16 PCM at parse time
with a pure-Rust Vorbis decoder."

## Memory analysis

### Compression ratio
SF3 achieves roughly 10x compression over SF2:
- FluidR3 GM: 141 MB SF2 -> 15 MB SF3 (9.4x)
- Typical ratio: 8-12x depending on Vorbis quality setting

### Memory expansion during decode
Decoding SF3 requires holding both the compressed data and the uncompressed PCM in memory:
- Compressed SF3 file: ~6-10 MB (for a 64 MB image)
- Uncompressed PCM: ~64 MB (the target image size)
- Peak memory during decode: ~70-75 MB

This is within typical browser memory limits (512 MB+ available in AudioWorklet context).

### Post-decode memory
After decode, the SoundbankImage holds the uncompressed PCM:
- 64 MB image = 64 MB RAM
- This is the recommended maximum (R03 RISK line: "image <= 64 MB recommended")

### Decode time
lewton (pure-Rust Vorbis decoder) is not the fastest decoder. Estimated decode time for 64 MB:
- ~2-5 seconds on modern hardware (rough estimate)
- This is a one-time cost when loading a new bank
- The `bank_load_cached` budget (<= 1 s) applies to loading from cached NGSB image, not SF3 decode

## Decoder options

### lewton (v0.10.2, MIT/BSD, ~291 stars)
- Pure Rust Vorbis decoder
- Used by rustysynth-ext for SF3 support
- Mature, well-tested
- Streaming decode possible (decode per-packet, not all at once)
- wasm-compatible

### Alternative: SF2 only
- No Vorbis decoding needed
- SF2 is uncompressed: file size ~= image size
- Simpler, faster loads
- But: SF2 files are ~10x larger for same quality

## Lazy vs eager decode

### Eager (at parse time, per R03 design)
- Decode all samples when loading the bank
- Peak memory: compressed + uncompressed
- Simpler implementation
- Faster playback (no decode during render)

### Lazy (on-demand)
- Decode samples only when played
- Lower peak memory
- More complex (decode during render, caching)
- Risk of audio glitches during decode

R03 spec mandates eager decode at parse time. This is the simpler approach and avoids
render-path complexity.

## Decision
**PASS**.

SF3 is supported using lewton as the Vorbis decoder. The memory expansion is manageable:
- A 64 MB image comes from a ~6-10 MB SF3 file
- Peak memory during decode: ~70-75 MB (within browser limits)
- Post-decode: 64 MB image (within recommended budget)

Conditions:
1. Use lewton for Vorbis decoding (pure Rust, wasm-compatible)
2. Decode at parse time (eager, per R03 design)
3. Enforce image size limit: uncompressed SoundbankImage <= 64 MB
4. SF3 file size limit: ~10 MB recommended (yields ~64-100 MB uncompressed)
5. Decode time is a one-time cost; document expected load time for large banks

## Fallback
If decode time proves unacceptable (> 5 s for a 64 MB image), switch to SF2-only:
- SF2 files are uncompressed (no decode step)
- File size ~= image size (max ~64 MB)
- SF3 users convert externally using sf3convert or similar

## Risks
- lewton decode speed not benchmarked yet (measure in T3.1)
- Vorbis quality affects both file size and decode time
- 24-bit samples (sm24) ignored in v1 per R03 spec
- Memory pressure on low-end devices (mitigation: image size limit)
