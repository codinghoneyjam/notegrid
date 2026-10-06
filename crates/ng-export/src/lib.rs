// <META - FILE SUMMARY - ng-export: WavEncoder (P1); MidiExporter/MidiImporter land in P4. | L1-4>
// NOT a hotpath crate: encoding is block/IO work, not the audio thread.
pub mod wav;
