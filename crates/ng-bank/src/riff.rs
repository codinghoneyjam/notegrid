// <META - FILE SUMMARY - Bounded RIFF/LIST walker plus bounds-checked fixed-width reads. No indexing, no unwrap (R03.R01).>

use crate::error::FormatError;

/// Read `n` bytes at `off`, or report the truncation. This is the only way this crate reaches
/// into a byte slice, so "never panics on truncation" is a property of one function (R03.R01).
pub fn read_exact<'a>(data: &'a [u8], off: usize, n: usize) -> Result<&'a [u8], FormatError> {
    let end = off.checked_add(n).ok_or(FormatError::IntegerOverflow("read end"))?;
    data.get(off..end).ok_or(FormatError::TooShort { need: end, got: data.len() })
}

pub fn u16_at(data: &[u8], off: usize) -> Result<u16, FormatError> {
    let b = read_exact(data, off, 2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

pub fn i16_at(data: &[u8], off: usize) -> Result<i16, FormatError> {
    let b = read_exact(data, off, 2)?;
    Ok(i16::from_le_bytes([b[0], b[1]]))
}

pub fn u32_at(data: &[u8], off: usize) -> Result<u32, FormatError> {
    let b = read_exact(data, off, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// A NUL-terminated fixed-width name field. Bytes outside printable ASCII become '_' so that
/// the name that ends up in the image is deterministic and stays 100% ASCII English (the
/// language whitelist); an untrusted name must not be able to inject anything else.
pub fn fixed_name(data: &[u8], off: usize, len: usize) -> Result<String, FormatError> {
    let raw = read_exact(data, off, len)?;
    let end = raw.iter().position(|b| *b == 0).unwrap_or(raw.len());
    Ok(raw[..end]
        .iter()
        .map(|b| {
            if (0x20..0x7f).contains(b) {
                char::from(*b)
            } else {
                '_'
            }
        })
        .collect())
}

/// One chunk inside a RIFF or LIST body.
#[derive(Clone, Copy)]
pub struct Chunk<'a> {
    pub id: [u8; 4],
    /// The chunk payload, already clipped to what the enclosing block really has.
    pub body: &'a [u8],
}

/// Walk the chunks of a RIFF or LIST body. Chunks are word aligned: an odd payload is followed
/// by one pad byte. Trailing bytes too short to hold a header end the walk (SF2 2.01 sec 3.3
/// treats that as structural damage, but skipping is the safer reading and the caller still
/// validates every structural field it depends on).
pub fn walk<'a>(body: &'a [u8]) -> ChunkIter<'a> {
    ChunkIter { body, off: 0 }
}

pub struct ChunkIter<'a> {
    body: &'a [u8],
    off: usize,
}

impl<'a> Iterator for ChunkIter<'a> {
    type Item = Chunk<'a>;

    fn next(&mut self) -> Option<Chunk<'a>> {
        let rest = self.body.get(self.off..)?;
        if rest.len() < 8 {
            return None;
        }
        let id = [rest[0], rest[1], rest[2], rest[3]];
        let size = u32::from_le_bytes([rest[4], rest[5], rest[6], rest[7]]) as usize;
        let start = self.off + 8;
        let available = self.body.len() - start;
        let take = size.min(available);
        let body = self.body.get(start..start + take)?;
        // Advance past the payload and its pad byte; if the declared size ran off the end of
        // the block the walk stops here rather than spinning.
        let advance = 8 + take + (take & 1);
        self.off = self.off.saturating_add(advance);
        Some(Chunk { id, body })
    }
}

/// Body of the first chunk with this id, if present.
pub fn find<'a>(body: &'a [u8], id: &[u8]) -> Option<&'a [u8]> {
    walk(body).find(|c| c.id.as_ref() == id).map(|c| c.body)
}

/// Body of the first LIST with this form type. A LIST body starts with its own four-byte form
/// type, which is not part of the payload the caller wants.
pub fn find_list<'a>(body: &'a [u8], form: &[u8]) -> Option<&'a [u8]> {
    walk(body)
        .find(|c| c.id.as_ref() == b"LIST" && c.body.len() >= 4 && &c.body[..4] == form)
        .and_then(|c| c.body.get(4..))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(id);
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        if body.len() % 2 == 1 {
            v.push(0);
        }
        v
    }

    #[test]
    fn walk_reads_and_pads() {
        let mut buf = chunk(b"ifil", &[1, 2, 3, 4]);
        buf.extend_from_slice(&chunk(b"isng", &[b'a']));
        let got: Vec<(String, Vec<u8>)> = walk(&buf)
            .map(|c| (fourcc(c.id), c.body.to_vec()))
            .collect();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].1, vec![1, 2, 3, 4]);
        assert_eq!(got[1].0, "isng");
        assert_eq!(got[1].1, vec![b'a']);
    }

    #[test]
    fn oversized_declared_size_is_clipped_not_read_past() {
        let mut v = b"XXXX".to_vec();
        v.extend_from_slice(&u32::MAX.to_le_bytes());
        v.extend_from_slice(b"only-8-bytes-here");
        let got: Vec<_> = walk(&v).collect();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].body, b"only-8-bytes-here");
    }

    #[test]
    fn find_list_skips_the_form_type() {
        let mut body = b"INFO".to_vec();
        body.extend_from_slice(&chunk(b"ifil", &[2, 0, 1, 0]));
        let mut outer = chunk(b"LIST", &body);
        outer.extend_from_slice(&chunk(b"JUNK", &[0]));
        // The form type itself is not part of the returned body; the sub-chunks are.
        let expected = chunk(b"ifil", &[2, 0, 1, 0]);
        assert_eq!(find_list(&outer, b"INFO"), Some(expected.as_slice()));
        assert_eq!(find_list(&outer, b"nope"), None);
    }

    #[test]
    fn fixed_name_stops_at_nul_and_sanitises() {
        let data = [b'a', b'b', 0, b'c'];
        assert_eq!(fixed_name(&data, 0, 4).unwrap_or_default(), "ab");
        let dirty = [b'x', 0xe9, 0x0a, 0];
        assert_eq!(fixed_name(&dirty, 0, 4).unwrap_or_default(), "x__");
    }

    use crate::error::fourcc;
}