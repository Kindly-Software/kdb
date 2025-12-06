//! T5 Streaming - Ring buffer trace (192 KB)
//! Reduced from 256 KB to accommodate T4 Batch tier (64 KB)
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

#[repr(C, align(64))]
pub struct TraceEvent {
    pub event_type: AtomicU8,
    pub tid: AtomicU32,
    pub timestamp_ns: AtomicU64,
    pub data: AtomicU64,
    pub context: AtomicU64,
    _padding: [u8; 64 - 1 - 4 - 3 * 8 - 3],
}

impl TraceEvent {
    pub const fn empty() -> Self {
        Self {
            event_type: AtomicU8::new(0),
            tid: AtomicU32::new(0),
            timestamp_ns: AtomicU64::new(0),
            data: AtomicU64::new(0),
            context: AtomicU64::new(0),
            _padding: [0; 64 - 1 - 4 - 3 * 8 - 3],
        }
    }

    pub fn set(&self, event_type: u8, tid: u32, timestamp_ns: u64, data: u64) {
        self.event_type.store(event_type, Ordering::Relaxed);
        self.tid.store(tid, Ordering::Relaxed);
        self.timestamp_ns.store(timestamp_ns, Ordering::Relaxed);
        self.data.store(data, Ordering::Relaxed);
    }
}

/// Ring buffer trace capsule (192 KB)
///
/// **REDUCED**: 3072 events (was 4096) to accommodate T4 Batch tier
/// **Size**: 3072 × 64B + 256B metadata = 196,864 bytes (192 KB)
#[repr(C, align(256))]
pub struct RingBufferTraceCapsule {
    pub head: AtomicU64,
    pub tail: AtomicU64,
    pub total_events: AtomicU64,
    pub dropped_events: AtomicU64,
    _padding: [u8; 256 - 4 * 8],
    pub events: [TraceEvent; 3072], // REDUCED from 4096 to 3072
}

impl RingBufferTraceCapsule {
    pub fn new() -> Self {
        const EMPTY: TraceEvent = TraceEvent::empty();
        Self {
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            total_events: AtomicU64::new(0),
            dropped_events: AtomicU64::new(0),
            _padding: [0; 256 - 4 * 8],
            events: [EMPTY; 3072], // REDUCED from 4096 to 3072
        }
    }

    pub fn record(&self, event_type: u8, tid: u32, data: u64) {
        const CAPACITY: u64 = 3072; // REDUCED from 4096
        let timestamp_ns = self.total_events.load(Ordering::Relaxed) * 1000;
        let head = self.head.fetch_add(1, Ordering::Relaxed);
        let index = (head % CAPACITY) as usize;

        self.events[index].set(event_type, tid, timestamp_ns, data);
        self.total_events.fetch_add(1, Ordering::Relaxed);

        let tail = self.tail.load(Ordering::Relaxed);
        if head >= tail + CAPACITY {
            self.dropped_events.fetch_add(1, Ordering::Relaxed);
            self.tail.store(head - (CAPACITY - 1), Ordering::Relaxed);
        }
    }

    pub fn drain_recent(&self, max_count: usize) -> Vec<(u8, u32, u64, u64)> {
        const CAPACITY: u64 = 3072; // REDUCED from 4096
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        let available = (head - tail).min(CAPACITY) as usize;
        let count = available.min(max_count);

        let mut result = Vec::with_capacity(count);
        for i in 0..count {
            let index = ((head - count as u64 + i as u64) % CAPACITY) as usize;
            let event = &self.events[index];
            result.push((
                event.event_type.load(Ordering::Relaxed),
                event.tid.load(Ordering::Relaxed),
                event.timestamp_ns.load(Ordering::Relaxed),
                event.data.load(Ordering::Relaxed),
            ));
        }

        result
    }

    pub fn get_stats(&self) -> (u64, u64) {
        (
            self.total_events.load(Ordering::Relaxed),
            self.dropped_events.load(Ordering::Relaxed),
        )
    }
}
