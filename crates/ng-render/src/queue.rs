// Note-off queue for sample-accurate timing.

use ng_core::Slot;

/// Fixed note-off queue capacity (R05.DESIGN).
pub const OFF_QUEUE_CAP: usize = 512;

#[derive(Clone, Copy, Debug)]
pub struct OffEvent {
    /// Output-clock position (monotonic, not transport position).
    pub at: u64,
    pub slot: Slot,
    pub key: u8,
}

/// Fixed-capacity min queue keyed by output clock (R05.DESIGN). Full -> force the earliest
/// off early, and count it; "dropped_offs" never panics and never waits (AC4).
#[derive(Clone, Debug, Default)]
pub struct OffQueue {
    pub events: Vec<OffEvent>,
    pub dropped_offs: u64,
}

impl OffQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            events: Vec::with_capacity(capacity),
            dropped_offs: 0,
        }
    }

    /// Returns the forced-early off when the queue was full (R05.DESIGN).
    pub fn push(&mut self, ev: OffEvent) -> Option<OffEvent> {
        let pos = self
            .events
            .iter()
            .position(|e| e.at >= ev.at)
            .unwrap_or(self.events.len());
        if self.events.len() >= self.events.capacity() {
            let forced = self.events.remove(0);
            self.dropped_offs = self.dropped_offs.saturating_add(1);
            self.events.insert(pos.min(self.events.len()), ev);
            return Some(forced);
        }
        self.events.insert(pos, ev);
        None
    }

    pub fn front_at(&self) -> Option<u64> {
        self.events.first().map(|e| e.at)
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}
