//! Diagnostics helpers built around a lightweight seqlock.

use core::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

#[cfg(feature = "circuit-breaker-serde")]
use serde::{Deserialize, Serialize};

/// Seqlock-based diagnostics record capturing slow-path information.
#[derive(Default)]
pub struct Diag {
    sequence: AtomicU32,
    /// Timestamp of last breaker update (milliseconds).
    pub last_update_ms: AtomicU32,
    /// Last recorded cause flags.
    pub last_reason: AtomicU8,
    /// Long-running error counter.
    pub long_err: AtomicU64,
}

impl Diag {
    fn begin_write(&self) -> u32 {
        loop {
            let seq = self.sequence.load(Ordering::Relaxed);
            if seq & 1 == 0 {
                if self
                    .sequence
                    .compare_exchange(seq, seq + 1, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    return seq + 1;
                }
            } else {
                core::hint::spin_loop();
            }
        }
    }

    fn end_write(&self, ticket: u32) {
        self.sequence.store(ticket + 1, Ordering::Release);
    }

    /// Update the diagnostics record.
    pub fn write(&self, timestamp_ms: u32, cause: u8, err_delta: u64) {
        let ticket = self.begin_write();
        self.last_update_ms.store(timestamp_ms, Ordering::Relaxed);
        self.last_reason.store(cause, Ordering::Relaxed);
        self.long_err.fetch_add(err_delta, Ordering::Relaxed);
        self.end_write(ticket);
    }

    /// Attempt to snapshot the diagnostics record; returns `None` if contended.
    pub fn read(&self) -> Option<DiagSnapshot> {
        let mut spins = 0u32;
        loop {
            let start = self.sequence.load(Ordering::Acquire);
            if start & 1 == 1 {
                core::hint::spin_loop();
                spins += 1;
                if spins > 32 {
                    return None;
                }
                continue;
            }

            let ts = self.last_update_ms.load(Ordering::Acquire);
            let reason = self.last_reason.load(Ordering::Acquire);
            let err = self.long_err.load(Ordering::Acquire);

            let end = self.sequence.load(Ordering::Relaxed);
            if start == end {
                return Some(DiagSnapshot {
                    last_update_ms: ts,
                    last_reason: reason,
                    long_err: err,
                });
            }

            spins += 1;
            if spins > 32 {
                return None;
            }
        }
    }
}

/// Immutable snapshot of diagnostics data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "circuit-breaker-serde", derive(Serialize, Deserialize))]
pub struct DiagSnapshot {
    /// Timestamp of the last breaker update in milliseconds.
    pub last_update_ms: u32,
    /// Cause flags associated with the last change.
    pub last_reason: u8,
    /// Long-running error counter.
    pub long_err: u64,
}

impl DiagSnapshot {
    /// Determine if the snapshot is stale relative to the current time.
    #[must_use]
    pub fn is_stale(&self, now_ms: u32, stale_window_ms: u32) -> bool {
        now_ms.wrapping_sub(self.last_update_ms) > stale_window_ms
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn seqlock_write_and_read() {
        let diag = Diag::default();
        diag.write(100, 0b1010, 7);
        let snapshot = diag.read().expect("snapshot should be available");
        assert_eq!(snapshot.last_update_ms, 100);
        assert_eq!(snapshot.last_reason, 0b1010);
        assert_eq!(snapshot.long_err, 7);
    }

    #[test]
    fn staleness_detection() {
        let snapshot = DiagSnapshot {
            last_update_ms: 10,
            last_reason: 0,
            long_err: 0,
        };
        assert!(snapshot.is_stale(25, 10));
        assert!(!snapshot.is_stale(18, 10));
    }

    #[cfg(feature = "circuit-breaker-serde")]
    #[test]
    fn snapshot_round_trips_with_serde() {
        let snapshot = DiagSnapshot {
            last_update_ms: 42,
            last_reason: 0b1100,
            long_err: 1234,
        };
        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let parsed: DiagSnapshot = serde_json::from_str(&json).expect("deserialize snapshot");
        assert_eq!(snapshot, parsed);
    }

    #[test]
    fn read_retries_when_sequence_is_odd() {
        let diag = Diag::default();
        diag.sequence.store(1, Ordering::Relaxed);
        assert!(diag.read().is_none());
        diag.sequence.store(0, Ordering::Relaxed);
    }
}
