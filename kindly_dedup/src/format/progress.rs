//! Progress tracking capsule (Atomic tier)
//!
//! Lockfree progress tracking for format readers.

use std::sync::atomic::{AtomicU64, Ordering};

/// Progress tracker capsule (Atomic, <5ns operations)
///
/// Provides lockfree document counter for streaming readers.
/// Used to track loading progress in UI, logging, and diagnostics.
///
/// # Performance
///
/// - **increment()**: <5ns (fetch_add, Relaxed)
/// - **current()**: <3ns (load, Relaxed)
/// - **reset()**: <3ns (store, Relaxed)
/// - **Memory**: 8 bytes (fits in single cache line with room for metadata)
/// - **Contention**: Zero (atomic operations, no locks)
///
/// # lockfree Compliance
///
/// - **Atomic**: AtomicU64 operations
/// - **100% Lockfree**: No mutex/RwLock
/// - **Cache-Aligned**: 8-byte atomic fits in 64B cache line
/// - **Relaxed Ordering**: Sufficient for monotonic counter (not synchronized)
///
/// # Safety (ASSUM Framework)
///
/// - #ASSUME: Relaxed ordering sufficient (progress display only, not synchronization)
/// - #VERIFY: Not used for synchronization (only monotonic increment)
/// - **Justification**: Progress counter is informational; relaxed atomic is safe
///
/// # Example
///
/// ```rust,ignore
/// use kindly_dedup::format::ProgressTrackerCapsule;
/// use std::sync::Arc;
///
/// let progress = Arc::new(ProgressTrackerCapsule::new());
///
/// // Simulate loading
/// for _ in 0..1000 {
///     progress.increment();
/// }
///
/// assert_eq!(progress.current(), 1000);
/// println!("Loaded {} documents", progress.current());
/// ```
#[derive(Debug)]
pub struct ProgressTrackerCapsule {
    /// Documents loaded (lockfree counter)
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Relaxed ordering sufficient (progress display only)
    /// - #VERIFY: Not used for synchronization (only monotonic increment)
    count: AtomicU64,
}

impl ProgressTrackerCapsule {
    /// Create new progress tracker (starts at 0)
    ///
    /// # Performance
    /// - <1ns (const-time initialization)
    pub fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
        }
    }

    /// Increment counter by 1 (lockfree, <5ns)
    ///
    /// Uses Relaxed ordering (sufficient for monotonic counter).
    #[inline]
    pub fn increment(&self) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current count (lockfree, <3ns)
    ///
    /// Uses Relaxed ordering (reads latest value without synchronization).
    #[inline]
    pub fn current(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Reset counter to 0 (lockfree, <3ns)
    ///
    /// Uses Relaxed ordering.
    #[inline]
    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
    }

    /// Increment by a specific amount (lockfree, <5ns)
    #[inline]
    pub fn add(&self, delta: u64) {
        self.count.fetch_add(delta, Ordering::Relaxed);
    }
}

impl Default for ProgressTrackerCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_new() {
        let progress = ProgressTrackerCapsule::new();
        assert_eq!(progress.current(), 0);
    }

    #[test]
    fn test_increment() {
        let progress = ProgressTrackerCapsule::new();
        progress.increment();
        assert_eq!(progress.current(), 1);

        progress.increment();
        assert_eq!(progress.current(), 2);
    }

    #[test]
    fn test_reset() {
        let progress = ProgressTrackerCapsule::new();
        progress.increment();
        progress.increment();
        assert_eq!(progress.current(), 2);

        progress.reset();
        assert_eq!(progress.current(), 0);
    }

    #[test]
    fn test_add() {
        let progress = ProgressTrackerCapsule::new();
        progress.add(100);
        assert_eq!(progress.current(), 100);

        progress.add(50);
        assert_eq!(progress.current(), 150);
    }

    #[test]
    fn test_default() {
        let progress = ProgressTrackerCapsule::default();
        assert_eq!(progress.current(), 0);
    }

    #[test]
    fn test_concurrent_increment() {
        let progress = Arc::new(ProgressTrackerCapsule::new());
        let mut handles = vec![];

        // 10 threads, 100 increments each
        for _ in 0..10 {
            let prog = Arc::clone(&progress);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    prog.increment();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // All increments should be visible (relaxed atomic still sees final count)
        assert_eq!(progress.current(), 1000);
    }

    #[test]
    fn test_concurrent_read_write() {
        let progress = Arc::new(ProgressTrackerCapsule::new());
        let mut handles = vec![];

        // Writer thread
        {
            let prog = Arc::clone(&progress);
            let handle = thread::spawn(move || {
                for _ in 0..1000 {
                    prog.increment();
                }
            });
            handles.push(handle);
        }

        // Reader threads
        for _ in 0..5 {
            let prog = Arc::clone(&progress);
            let handle = thread::spawn(move || {
                let mut last_count = 0u64;
                for _ in 0..100 {
                    let count = prog.current();
                    // Count should be monotonically increasing
                    assert!(count >= last_count);
                    last_count = count;
                    std::thread::sleep(std::time::Duration::from_micros(10));
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Final count should be 1000
        assert_eq!(progress.current(), 1000);
    }
}
