// <META - FILE SUMMARY - R03 ng-bank: SF2/SF3 -> SoundbankImage, image load, preset lookup and coverage.>

pub mod error;
pub mod flatten;
pub mod image;
pub mod image_load;
pub mod limits;
pub mod lookup;
pub mod record;
pub mod riff;
pub mod sf2;
pub mod sf2_builder;
pub mod timecents;

pub use error::{BankError, FormatError, E_BANK_FORMAT, E_BANK_UNSUPPORTED};
pub use image::{LoopMode, PresetEntry, SampleEntry, SoundbankImage, ZoneEntry};
pub use image::{HEADER_LEN, IMAGE_MAGIC, IMAGE_VERSION, PRESET_LEN, SAMPLE_LEN, ZONE_LEN};
pub use image_load::from_bytes as image_from_bytes;
pub use lookup::{lookup_chain, Coverage, DRUM_BANK};
pub use limits::{
    source_is_oversized, MAX_IMAGE_BYTES, MAX_NAME_BYTES, MAX_RECORDS, MAX_SAMPLES, MAX_SOURCE_BYTES,
    WARN_SOURCE_BYTES,
};
pub use sf2::Sf2File;
pub use sf2_builder::{
    builder_with, demo_bank, sine_at, sine_amplitude, sine_sample, InstrumentSpec, PresetSpec,
    SampleSpec, Sf2Builder, SplitSpec,
};

/// Build the image for a SoundFont. This is the crate's main entry point and the one C4
/// `ng_bank_parse` calls; it runs on a worker thread, never on the audio thread (R-RT-5).
///
/// Errors are `E_BANK_FORMAT` for a structurally damaged or over-limit source and
/// `E_BANK_UNSUPPORTED` for a well-formed bank this version does not implement (SF3 Vorbis,
/// ROM samples, an `ifil` major other than 2).
pub fn image_from_sf2(bytes: &[u8]) -> Result<SoundbankImage, BankError> {
    let file = sf2::parse_sf2(bytes)?;
    let image = flatten::flatten(&file)?;
    image::validate(&image)?;
    Ok(image)
}

/// Serialise an image to the NGSB container (C10.1). The same image always produces the same
/// bytes: every offset is computed, every name is sanitised, and no iteration order is involved
/// (R03.R02).
pub fn image_to_bytes(image: &SoundbankImage) -> Result<Vec<u8>, BankError> {
    image::to_bytes(image)
}

/// Load a previously built image. Validates the version and every range before handing anything
/// out, then copies once: this is the "minimal copy, no parsing" path C10.1 asks for.
pub fn image_from_image(bytes: &[u8]) -> Result<SoundbankImage, BankError> {
    image_load::from_bytes(bytes)
}
