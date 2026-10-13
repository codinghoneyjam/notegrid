// <META - FILE SUMMARY - Bank errors: E_BANK_FORMAT | E_BANK_UNSUPPORTED (C9), with the structural detail behind E_BANK_FORMAT.>

use std::error::Error;
use std::fmt;

/// C9 error code for a structurally defective or unreadable bank.
pub const E_BANK_FORMAT: &str = "E_BANK_FORMAT";
/// C9 error code for a bank this version deliberately does not implement (SF3 Vorbis, ROM samples).
pub const E_BANK_UNSUPPORTED: &str = "E_BANK_UNSUPPORTED";

/// Why a source could not be read as a SoundFont. Every variant maps to `E_BANK_FORMAT`;
/// the split exists so tests and diagnostics can name the exact defect (R03.R01).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatError {
    /// A read ran past the end of the buffer (truncation).
    TooShort { need: usize, got: usize },
    /// Top-level FOURCC did not match what the format requires.
    BadMagic { expected: &'static str, got: [u8; 4] },
    /// A required RIFF/LIST form type is absent.
    MissingList(&'static str),
    /// A required pdta sub-chunk is absent.
    MissingChunk(&'static str),
    /// A chunk declared more bytes than the enclosing block can supply.
    ChunkSize { id: &'static str, declared: u32, available: usize },
    /// A pdta sub-chunk length is not a whole number of records.
    RecordSize { id: &'static str, len: usize, record: usize },
    /// A header index or a count implied by two headers does not fit its table.
    IndexOutOfRange { id: &'static str, index: u32, count: usize },
    /// Bag / generator indices must be non-decreasing; a decrease is a cycle (R03.R01).
    NotMonotonic { id: &'static str, index: usize },
    /// A terminal record (EOP / EOI / EOS) was expected to close the last real record.
    MissingTerminal(&'static str),
    /// A sample header points outside the smpl chunk or has end <= start.
    BadSampleRange { index: usize, start: u32, end: u32, frames: usize },
    /// A sample header declares a zero sample rate.
    ZeroSampleRate { index: usize },
    /// A C10.2 limit was exceeded. Reported as E_BANK_FORMAT: no third code exists in C9.
    Limit { what: &'static str, limit: u64, got: u64 },
    /// An offset arithmetic overflowed rather than wrapping.
    IntegerOverflow(&'static str),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { need, got } => write!(f, "need {need} bytes, have {got}"),
            Self::BadMagic { expected, got } => {
                write!(f, "expected {expected} form type, got {}", fourcc(*got))
            }
            Self::MissingList(id) => write!(f, "missing LIST '{id}'"),
            Self::MissingChunk(id) => write!(f, "missing pdta sub-chunk '{id}'"),
            Self::ChunkSize { id, declared, available } => {
                write!(f, "chunk '{id}' declares {declared} bytes, {available} available")
            }
            Self::RecordSize { id, len, record } => {
                write!(f, "chunk '{id}' is {len} bytes, not a multiple of {record}")
            }
            Self::IndexOutOfRange { id, index, count } => {
                write!(f, "'{id}' index {index} is outside {count} records")
            }
            Self::NotMonotonic { id, index } => {
                write!(f, "'{id}' index decreases at record {index}")
            }
            Self::MissingTerminal(id) => write!(f, "'{id}' has no terminal record"),
            Self::BadSampleRange { index, start, end, frames } => write!(
                f,
                "sample {index} spans [{start},{end}) which is not inside {frames} frames"
            ),
            Self::ZeroSampleRate { index } => write!(f, "sample {index} has sample rate 0"),
            Self::Limit { what, limit, got } => write!(f, "{what}: limit {limit}, got {got}"),
            Self::IntegerOverflow(what) => write!(f, "{what} overflowed"),
        }
    }
}

/// Errors of the whole crate. C9 defines exactly two bank codes, so the structural detail is
/// carried in `Format` rather than in a new code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BankError {
    /// The bytes are not a readable SoundFont.
    Format(FormatError),
    /// The bytes are a SoundFont this version does not implement.
    Unsupported(String),
}

impl BankError {
    /// The C9 error code for the ABI layer (C4 error detail = {code, message, path}).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Format(_) => E_BANK_FORMAT,
            Self::Unsupported(_) => E_BANK_UNSUPPORTED,
        }
    }

    pub fn message(&self) -> String {
        self.to_string()
    }

    pub(crate) fn limit(what: &'static str, limit: u64, got: u64) -> Self {
        Self::Format(FormatError::Limit { what, limit, got })
    }
}

impl fmt::Display for BankError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format(e) => write!(f, "{E_BANK_FORMAT}: {e}"),
            Self::Unsupported(what) => write!(f, "{E_BANK_UNSUPPORTED}: {what}"),
        }
    }
}

impl Error for BankError {}

impl From<FormatError> for BankError {
    fn from(e: FormatError) -> Self {
        Self::Format(e)
    }
}

/// Printable rendering of a FOURCC, always ASCII so the message survives a cp949 console.
pub(crate) fn fourcc(id: [u8; 4]) -> String {
    id.iter()
        .map(|b| {
            if (0x20..0x7f).contains(b) {
                char::from(*b)
            } else {
                '.'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_follow_c9() {
        assert_eq!(BankError::Format(FormatError::TooShort { need: 1, got: 0 }).code(), E_BANK_FORMAT);
        assert_eq!(BankError::Unsupported("x".into()).code(), E_BANK_UNSUPPORTED);
    }

    #[test]
    fn fourcc_is_printable_ascii() {
        assert_eq!(fourcc(*b"pdta"), "pdta");
        assert_eq!(fourcc([0x00, 0xff, b'a', 0x7f]), "..a.");
    }
}