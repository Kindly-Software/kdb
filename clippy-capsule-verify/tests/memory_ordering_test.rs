//! # Memory Ordering Lint Test
//!
//! Demonstrates CAPSULE_MEMORY_ORDERING lint detection.
//!
//! ## Expected Behavior
//!
//! This file should trigger warnings when run with:
//! ```bash
//! cargo clippy --test memory_ordering_test -- -W clippy::capsule_memory_ordering
//! ```
//!
//! ## Test Cases
//!
//! 1. **Relaxed load** → Should warn (suggest Acquire)
//! 2. **Relaxed store** → Should warn (suggest Release)
//! 3. **Relaxed swap** → Should warn (suggest AcqRel/SeqCst)
//! 4. **Relaxed compare_exchange** → Should warn (suggest SeqCst)
//! 5. **Acquire load** → Should NOT warn (correct)
//! 6. **Release store** → Should NOT warn (correct)

use std::sync::atomic::{AtomicU64, Ordering};

/// Example capsule with atomic fields
#[repr(C, align(64))]
struct ExampleCapsule {
    state: AtomicU64,
    count: AtomicU64,
}

impl ExampleCapsule {
    fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    /// BAD: Relaxed load breaks synchronization
    #[allow(dead_code)]
    fn bad_load(&self) -> u64 {
        self.state.load(Ordering::Relaxed) // Should warn
    }

    /// BAD: Relaxed store breaks synchronization
    #[allow(dead_code)]
    fn bad_store(&self, value: u64) {
        self.state.store(value, Ordering::Relaxed); // Should warn
    }

    /// BAD: Relaxed swap breaks synchronization
    #[allow(dead_code)]
    fn bad_swap(&self, value: u64) -> u64 {
        self.state.swap(value, Ordering::Relaxed) // Should warn
    }

    /// BAD: Relaxed compare_exchange breaks synchronization
    #[allow(dead_code)]
    fn bad_compare_exchange(&self, old: u64, new: u64) -> Result<u64, u64> {
        self.state.compare_exchange(
            old,
            new,
            Ordering::Relaxed, // Should warn
            Ordering::Relaxed,
        )
    }

    /// BAD: Relaxed fetch_add breaks synchronization
    #[allow(dead_code)]
    fn bad_fetch_add(&self, value: u64) -> u64 {
        self.count.fetch_add(value, Ordering::Relaxed) // Should warn
    }

    /// GOOD: Acquire load (correct synchronization)
    #[allow(dead_code)]
    fn good_load(&self) -> u64 {
        self.state.load(Ordering::Acquire) // No warning
    }

    /// GOOD: Release store (correct synchronization)
    #[allow(dead_code)]
    fn good_store(&self, value: u64) {
        self.state.store(value, Ordering::Release); // No warning
    }

    /// GOOD: SeqCst swap (correct synchronization)
    #[allow(dead_code)]
    fn good_swap(&self, value: u64) -> u64 {
        self.state.swap(value, Ordering::SeqCst) // No warning
    }

    /// GOOD: SeqCst compare_exchange (correct synchronization)
    #[allow(dead_code)]
    fn good_compare_exchange(&self, old: u64, new: u64) -> Result<u64, u64> {
        self.state.compare_exchange(
            old,
            new,
            Ordering::SeqCst, // No warning
            Ordering::SeqCst,
        )
    }

    /// GOOD: AcqRel fetch_add (correct synchronization)
    #[allow(dead_code)]
    fn good_fetch_add(&self, value: u64) -> u64 {
        self.count.fetch_add(value, Ordering::AcqRel) // No warning
    }

    /// Intentional Relaxed use (with suppression)
    #[allow(dead_code)]
    #[allow(clippy::capsule_memory_ordering)]
    fn intentional_relaxed_counter(&self) -> u64 {
        // This is a non-coordinating counter (metrics only)
        // Relaxed is intentional for performance
        self.count.load(Ordering::Relaxed) // Suppressed, no warning
    }
}

#[test]
fn test_memory_ordering_examples() {
    let capsule = ExampleCapsule::new();

    // These calls demonstrate the lint in action
    // The warnings appear at compile-time, not runtime
    let _ = capsule.good_load();
    capsule.good_store(42);
    let _ = capsule.good_swap(100);
    let _ = capsule.good_compare_exchange(42, 100);
    let _ = capsule.good_fetch_add(1);
}

#[test]
fn test_intentional_relaxed() {
    let capsule = ExampleCapsule::new();

    // This demonstrates proper suppression
    let _ = capsule.intentional_relaxed_counter();
}
