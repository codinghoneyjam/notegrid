// <META - ROLE : C0.3 tempo segments; tick<->sample conversion. The ONLY place in the workspace that multiplies ticks by a rate. | L1-130>

use ng_core::Tick;

const PPQ: u128 = 480;
const MICROS_PER_QUARTER: u128 = 1_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TempoMapError {
    /// C1 I-04: tempo[0].tick must be 0 and ticks must strictly increase.
    NonMonotonicTempo { index: usize },
    /// C1 I-04: the tempo list must not be empty.
    EmptyTempo,
    /// mpq must fit in u32; a bpm outside this range cannot be represented (C0.2).
    MpqOutOfRange { index: usize },
    /// sample_rate must be nonzero; 0 would make every conversion meaningless.
    ZeroSampleRate,
}

/// One tempo segment. `start_scaled` is the accumulated microseconds at `tick`, kept in u128
/// so that tick->sample accumulates in u128 as C0.3 requires and rounds exactly once at the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TempoSegment {
    pub tick: Tick,
    pub mpq: u32,
    /// Accumulated microseconds at `tick` (u128, per C0.3).
    pub start_scaled: u128,
}

/// Tempo map for one sample rate: the only tick<->sample conversion in the system (R02.R02).
///
/// The conversion is a pure function of the tempo list and the sample rate, so the sample rate
/// is fixed at construction. Nothing outside this type can turn a tick into a sample, because
/// `tick_to_sample` and `sample_to_tick` are the only such operations and both need a TempoMap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TempoMap {
    sample_rate: u32,
    segments: Vec<TempoSegment>,
}

impl TempoMap {
    /// Build from mpq (microseconds per quarter) values, already converted from bpm.
    ///
    /// Callers holding a `Project` should use `TempoMap::from_project`, which converts bpm
    /// through the single definition in ng-score. Nothing in this crate sees a float bpm, so
    /// R02.R02 holds by construction: the only tick<->sample operations live on this type.
    pub fn from_mpq_segments(
        sample_rate: u32,
        points: &[(Tick, u32)],
    ) -> Result<Self, TempoMapError> {
        if sample_rate == 0 {
            return Err(TempoMapError::ZeroSampleRate);
        }
        let first = points.first().ok_or(TempoMapError::EmptyTempo)?;
        if first.0 != 0 {
            return Err(TempoMapError::NonMonotonicTempo { index: 0 });
        }

        let mut segments = Vec::with_capacity(points.len());
        let mut scaled: u128 = 0;
        // Segment i spans [points[i].tick, points[i+1].tick) at points[i].mpq, so the stretch
        // that ends at points[i+1] is priced with points[i].mpq -- the tempo in force before
        // the change, not the one after it.
        let mut prev: Option<(Tick, u32)> = None;
        for (index, (tick, mpq)) in points.iter().copied().enumerate() {
            if let Some((prev_tick, prev_mpq)) = prev {
                if tick <= prev_tick {
                    return Err(TempoMapError::NonMonotonicTempo { index });
                }
                scaled += u128::from(tick - prev_tick) * u128::from(prev_mpq);
            }
            if mpq == 0 {
                return Err(TempoMapError::MpqOutOfRange { index });
            }
            segments.push(TempoSegment {
                tick,
                mpq,
                start_scaled: scaled,
            });
            prev = Some((tick, mpq));
        }

        Ok(Self {
            sample_rate,
            segments,
        })
    }

    /// Build from a project's tempo list. bpm -> mpq goes through ng-score so that C0.2 has a
    /// single definition; this type never sees a float bpm.
    pub fn from_project(
        project: &ng_score::Project,
        sample_rate: u32,
    ) -> Result<Self, TempoMapError> {
        let points: Vec<(Tick, u32)> = project.tempo.iter().map(|t| (t.tick, t.mpq())).collect();
        Self::from_mpq_segments(sample_rate, &points)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn segments(&self) -> &[TempoSegment] {
        &self.segments
    }

    /// C0.3: sample = round_half_up(T * sr / (480 * 1e6)) with T accumulated in u128 across
    /// segments, rounded exactly once.
    pub fn tick_to_sample(&self, tick: Tick) -> u64 {
        let micros = self.micros_at(tick);
        round_half_up_div(
            u128::from(self.sample_rate) * micros,
            PPQ * MICROS_PER_QUARTER,
        )
    }

    /// Inverse conversion, floored (C2 declares sample_to_tick as floor, for display).
    ///
    /// Defined as the largest tick whose sample position does not exceed `sample`. That is
    /// computed by binary search over `tick_to_sample`, which makes the inverse consistent
    /// with the forward direction by construction -- the round-trip property R02.AC3 asserts
    /// holds because of how the forward function is defined, not because both were written to
    /// agree. Sample 0 and the empty segment range are handled explicitly.
    ///
    /// Not a hot path: used for display and for C2.6 (set_schedule keeps the tick position).
    pub fn sample_to_tick(&self, sample: u64) -> Tick {
        if sample == 0 {
            return 0;
        }
        let (lo, hi) = self.search_bounds(sample);
        let mut low = lo;
        let mut high = hi;
        // Invariant: tick_to_sample(low) <= sample < tick_to_sample(high), or high is exclusive.
        while low < high {
            let mid = low + (high - low).div_ceil(2);
            if self.tick_to_sample_u64(mid) <= sample {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        low as Tick
    }

    /// Tick range to search for `sample`: the segment that covers it.
    fn search_bounds(&self, sample: u64) -> (u64, u64) {
        let idx = self
            .segments
            .iter()
            .rposition(|seg| self.tick_to_sample_u64(u64::from(seg.tick)) <= sample)
            .unwrap_or(0);
        let seg = self.segments[idx];
        let lo = u64::from(seg.tick);
        let hi = match self.segments.get(idx + 1) {
            Some(next) => u64::from(next.tick) - 1,
            None => u64::from(u32::MAX),
        };
        (lo, hi)
    }

    /// Accumulated microseconds at `tick`, across all segments (C0.3).
    fn micros_at(&self, tick: Tick) -> u128 {
        match self.segments.iter().rposition(|seg| seg.tick <= tick) {
            Some(idx) => {
                let seg = self.segments[idx];
                seg.start_scaled + u128::from(tick - seg.tick) * u128::from(seg.mpq)
            }
            None => 0,
        }
    }

    /// tick_to_sample widened to u64 ticks so the binary search can address u32::MAX.
    fn tick_to_sample_u64(&self, tick: u64) -> u64 {
        let micros = if u128::from(tick) > u128::from(u32::MAX) {
            // Beyond the last tick there is no segment; hold the final value.
            let seg = self.segments[self.segments.len() - 1];
            let span = u128::from(u32::MAX) - u128::from(seg.tick);
            seg.start_scaled + span * u128::from(seg.mpq)
        } else {
            self.micros_at(tick as Tick)
        };
        round_half_up_div(
            u128::from(self.sample_rate) * micros,
            PPQ * MICROS_PER_QUARTER,
        )
    }
}

/// Integer division rounding halves up: (n + d/2) / d for non-negative n, done in integers.
fn round_half_up_div(n: u128, d: u128) -> u64 {
    debug_assert!(d > 0);
    let q = (2 * n + d) / (2 * d);
    if q > u128::from(u64::MAX) {
        u64::MAX
    } else {
        q as u64
    }
}
