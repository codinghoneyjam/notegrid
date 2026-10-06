// <META - FILE SUMMARY - Fractional-position table lookup: linear (v1) and Catmull-Rom cubic (v2). | L1-120>
// HOTPATH: no allocation, no panic, no indexing. Callers get `Option` and decide; a hot path
// that knows its table length can unwrap-free match on it without the protections firing.

/// Which interpolation rule a lookup uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterpMode {
    /// Straight line between the two neighbouring samples (R03 sample playback default).
    Linear,
    /// Catmull-Rom cubic through the two neighbours and one sample on each side (R04 "v2 cubic").
    Cubic,
}

/// Fractional-position lookup over a fixed sample table.
///
/// `pos` is in table coordinates: `pos == 3.25` means 25% of the way from sample 3 to sample 4.
/// Positions outside `[0, len - 1]` or NaN return `None`; clamping is the caller's job, so a
/// clipped loop end never reads as a valid sample by accident.
#[derive(Clone, Copy, Debug)]
pub struct LinearInterp<'a> {
    table: &'a [f32],
}

impl<'a> LinearInterp<'a> {
    pub fn new(table: &'a [f32]) -> Self {
        Self { table }
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    pub fn at(&self, pos: f32, mode: InterpMode) -> Option<f32> {
        match mode {
            InterpMode::Linear => self.linear(pos),
            InterpMode::Cubic => self.cubic(pos),
        }
    }

    pub fn linear(&self, pos: f32) -> Option<f32> {
        let (i, t) = self.segment(pos)?;
        let p1 = *self.table.get(i)?;
        let p2 = *self.table.get(i + 1)?;
        Some(p1 + t * (p2 - p1))
    }

    /// Catmull-Rom: passes through every table sample exactly, and its first difference is
    /// continuous — no derivative jump at sample boundaries, which is what a linear lookup
    /// clicks on at fractional positions.
    pub fn cubic(&self, pos: f32) -> Option<f32> {
        let (i, t) = self.segment(pos)?;
        // One sample of context on each side; at the edges the endpoint is duplicated, which
        // is the standard clamped-Catmull-Rom boundary and keeps the value finite.
        let p0 = *self.table.get(i.saturating_sub(1))?;
        let p1 = *self.table.get(i)?;
        let p2 = *self.table.get(i + 1)?;
        let p3 = *self.table.get(i + 2).unwrap_or(&p2);
        let t2 = t * t;
        let t3 = t2 * t;
        Some(
            0.5 * ((2.0 * p1)
                + (-p0 + p2) * t
                + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3),
        )
    }

    /// Floor the position to a segment index and the in-segment fraction. `pos == len - 1`
    /// resolves to the last sample exactly, t = 0.
    fn segment(&self, pos: f32) -> Option<(usize, f32)> {
        let len = self.table.len();
        if len < 2 || pos.is_nan() || pos < 0.0 || pos > (len - 1) as f32 {
            return None;
        }
        let i = pos as usize;
        // `pos` can land exactly on `len - 1`; that is the last sample, t = 0 of the final segment.
        let i = if i + 1 >= len { len - 2 } else { i };
        let t = pos - i as f32;
        Some((i, t))
    }
}
