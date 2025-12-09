use crate::layout::{ActSnapshot, ActWord};

#[cfg(target_has_atomic = "128")]
use core::sync::atomic::{AtomicU128, Ordering};
#[cfg(not(target_has_atomic = "128"))]
use std::sync::Mutex;

/// Thread-safe storage for a single ACT-128 snapshot.
///
/// When the target supports native 128-bit atomics the slot uses an
/// `AtomicU128` with release/relaxed semantics in keeping with the ACT-128
/// design contract. On targets without 128-bit atomics the implementation
/// transparently falls back to a mutex; this preserves correctness for testing
/// but does not offer atomic update semantics.
pub struct ActSlot {
    #[cfg(target_has_atomic = "128")]
    inner: AtomicU128,
    #[cfg(not(target_has_atomic = "128"))]
    inner: Mutex<u128>,
}

impl ActSlot {
    /// Create a slot initialized with the provided snapshot.
    pub fn new(initial: ActSnapshot) -> Self {
        let raw = ActWord::pack(&initial).raw();
        Self {
            #[cfg(target_has_atomic = "128")]
            inner: AtomicU128::new(raw),
            #[cfg(not(target_has_atomic = "128"))]
            inner: Mutex::new(raw),
        }
    }

    /// Publish a new snapshot using release semantics when native atomics exist.
    pub fn publish(&self, snapshot: &ActSnapshot) {
        let packed = ActWord::pack(snapshot);
        self.publish_raw(packed);
    }

    /// Publish a raw word without re-packing.
    pub fn publish_raw(&self, word: ActWord) {
        let raw = word.raw();
        #[cfg(target_has_atomic = "128")]
        {
            self.inner.store(raw, Ordering::Release);
        }
        #[cfg(not(target_has_atomic = "128"))]
        {
            *self.inner.lock().expect("mutex poisoned") = raw;
        }
    }

    /// Obtain the latest snapshot with relaxed ordering.
    pub fn load_relaxed(&self) -> ActWord {
        #[cfg(target_has_atomic = "128")]
        {
            ActWord::from_raw(self.inner.load(Ordering::Relaxed))
        }
        #[cfg(not(target_has_atomic = "128"))]
        {
            ActWord::from_raw(*self.inner.lock().expect("mutex poisoned"))
        }
    }

    /// Obtain the latest snapshot with acquire semantics.
    pub fn load_acquire(&self) -> ActWord {
        #[cfg(target_has_atomic = "128")]
        {
            ActWord::from_raw(self.inner.load(Ordering::Acquire))
        }
        #[cfg(not(target_has_atomic = "128"))]
        {
            // Mutex lock already provides acquire semantics.
            ActWord::from_raw(*self.inner.lock().expect("mutex poisoned"))
        }
    }
}

impl Default for ActSlot {
    fn default() -> Self {
        Self::new(ActSnapshot::empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{ActFlags, FixedQ8_8};

    #[test]
    fn publishes_and_loads_snapshot() {
        let slot = ActSlot::default();
        let mut snapshot = ActSnapshot::empty();
        snapshot.net = FixedQ8_8::saturating_from_bp(3.25);
        snapshot.min_required = FixedQ8_8::saturating_from_bp(2.0);
        snapshot.flags = ActFlags::OK;
        snapshot.version = 1;
        snapshot.seq = 42;
        snapshot.age_ms_bucket = 3;

        slot.publish(&snapshot);
        let loaded = slot.load_relaxed().unpack();
        assert_eq!(loaded, snapshot);
    }
}
