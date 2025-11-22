//! Queue Instrumentation Module (Debug-Only)
//!
//! **Purpose**: Collect metrics on queue behavior for debugging livelock issues
//!
//! **Metrics Tracked**:
//! - Steal attempts (total CAS attempts on tail)
//! - Steal successes (CAS succeeded, task stolen)
//! - Steal CAS failures (contention or empty queue)
//! - Steal empty checks (queue was empty, no CAS attempted)
//! - Steal last-element skips (last element protected for owner)
//!
//! **Usage**:
//! ```rust
//! // In queue.rs steal() method:
//! record_steal_attempt();
//! if let Ok(_) = cas_result {
//!     record_steal_success();
//! } else {
//!     record_cas_failure();
//! }
//!
//! // At test end:
//! print_queue_stats();
//! ```
//!
//! **Thread Safety**: Uses lockfree atomics (safe for concurrent access)

use std::sync::atomic::{AtomicUsize, Ordering};

/// Global metrics (static for easy access from queue.rs)
static DEBUG_STEAL_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_STEAL_SUCCESSES: AtomicUsize = AtomicUsize::new(0);
static DEBUG_CAS_FAILURES: AtomicUsize = AtomicUsize::new(0);
static DEBUG_EMPTY_CHECKS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_LAST_ELEMENT_SKIPS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_POP_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_POP_SUCCESSES: AtomicUsize = AtomicUsize::new(0);
static DEBUG_PUSH_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_PUSH_FULL_ERRORS: AtomicUsize = AtomicUsize::new(0);

/// Record a steal attempt (before CAS)
#[inline(always)]
pub fn record_steal_attempt() {
    DEBUG_STEAL_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
}

/// Record a successful steal (CAS succeeded)
#[inline(always)]
pub fn record_steal_success() {
    DEBUG_STEAL_SUCCESSES.fetch_add(1, Ordering::Relaxed);
}

/// Record a CAS failure during steal (contention)
#[inline(always)]
pub fn record_cas_failure() {
    DEBUG_CAS_FAILURES.fetch_add(1, Ordering::Relaxed);
}

/// Record an empty queue check (no steal attempted)
#[inline(always)]
pub fn record_empty_check() {
    DEBUG_EMPTY_CHECKS.fetch_add(1, Ordering::Relaxed);
}

/// Record last-element skip (owner protection logic)
#[inline(always)]
pub fn record_last_element_skip() {
    DEBUG_LAST_ELEMENT_SKIPS.fetch_add(1, Ordering::Relaxed);
}

/// Record a pop attempt
#[inline(always)]
pub fn record_pop_attempt() {
    DEBUG_POP_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
}

/// Record a successful pop
#[inline(always)]
pub fn record_pop_success() {
    DEBUG_POP_SUCCESSES.fetch_add(1, Ordering::Relaxed);
}

/// Record a push attempt
#[inline(always)]
pub fn record_push_attempt() {
    DEBUG_PUSH_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
}

/// Record a push failure (queue full)
#[inline(always)]
pub fn record_push_full() {
    DEBUG_PUSH_FULL_ERRORS.fetch_add(1, Ordering::Relaxed);
}

/// Print queue statistics (call at test end)
pub fn print_queue_stats() {
    let steal_attempts = DEBUG_STEAL_ATTEMPTS.load(Ordering::SeqCst);
    let steal_successes = DEBUG_STEAL_SUCCESSES.load(Ordering::SeqCst);
    let cas_failures = DEBUG_CAS_FAILURES.load(Ordering::SeqCst);
    let empty_checks = DEBUG_EMPTY_CHECKS.load(Ordering::SeqCst);
    let last_elem_skips = DEBUG_LAST_ELEMENT_SKIPS.load(Ordering::SeqCst);
    let pop_attempts = DEBUG_POP_ATTEMPTS.load(Ordering::SeqCst);
    let pop_successes = DEBUG_POP_SUCCESSES.load(Ordering::SeqCst);
    let push_attempts = DEBUG_PUSH_ATTEMPTS.load(Ordering::SeqCst);
    let push_full = DEBUG_PUSH_FULL_ERRORS.load(Ordering::SeqCst);

    eprintln!("\n=== Queue Instrumentation Stats ===");

    // Steal metrics
    eprintln!("\n[STEAL METRICS]");
    eprintln!("  Steal attempts:       {:>10}", steal_attempts);
    eprintln!("  Steal successes:      {:>10}", steal_successes);
    eprintln!("  CAS failures:         {:>10}", cas_failures);
    eprintln!("  Empty checks:         {:>10}", empty_checks);
    eprintln!("  Last-element skips:   {:>10}", last_elem_skips);

    let success_rate = if steal_attempts > 0 {
        100.0 * steal_successes as f64 / steal_attempts as f64
    } else {
        0.0
    };
    eprintln!("  Success rate:         {:>9.2}%", success_rate);

    // Pop metrics
    eprintln!("\n[POP METRICS]");
    eprintln!("  Pop attempts:         {:>10}", pop_attempts);
    eprintln!("  Pop successes:        {:>10}", pop_successes);

    let pop_success_rate = if pop_attempts > 0 {
        100.0 * pop_successes as f64 / pop_attempts as f64
    } else {
        0.0
    };
    eprintln!("  Pop success rate:     {:>9.2}%", pop_success_rate);

    // Push metrics
    eprintln!("\n[PUSH METRICS]");
    eprintln!("  Push attempts:        {:>10}", push_attempts);
    eprintln!("  Push full errors:     {:>10}", push_full);

    let push_success_rate = if push_attempts > 0 {
        100.0 * (push_attempts - push_full) as f64 / push_attempts as f64
    } else {
        0.0
    };
    eprintln!("  Push success rate:    {:>9.2}%", push_success_rate);

    // Overall health indicators
    eprintln!("\n[HEALTH INDICATORS]");

    if success_rate < 1.0 && steal_attempts > 10_000 {
        eprintln!("  WARNING: Steal success rate <1% with high attempts");
        eprintln!("           Likely contention livelock!");
    }

    if cas_failures > steal_successes * 100 {
        eprintln!("  WARNING: CAS failures >> successes (100:1 ratio)");
        eprintln!("           Severe contention detected!");
    }

    if last_elem_skips > steal_successes * 10 {
        eprintln!("  WARNING: Last-element skips >> successes (10:1 ratio)");
        eprintln!("           Workers stuck waiting for owner pop!");
    }

    if empty_checks > steal_attempts * 2 {
        eprintln!("  INFO: Empty checks > steal attempts");
        eprintln!("        Queue frequently empty (normal for low load)");
    }

    eprintln!("\n===================================\n");
}

/// Reset all counters (call between tests)
pub fn reset_queue_stats() {
    DEBUG_STEAL_ATTEMPTS.store(0, Ordering::SeqCst);
    DEBUG_STEAL_SUCCESSES.store(0, Ordering::SeqCst);
    DEBUG_CAS_FAILURES.store(0, Ordering::SeqCst);
    DEBUG_EMPTY_CHECKS.store(0, Ordering::SeqCst);
    DEBUG_LAST_ELEMENT_SKIPS.store(0, Ordering::SeqCst);
    DEBUG_POP_ATTEMPTS.store(0, Ordering::SeqCst);
    DEBUG_POP_SUCCESSES.store(0, Ordering::SeqCst);
    DEBUG_PUSH_ATTEMPTS.store(0, Ordering::SeqCst);
    DEBUG_PUSH_FULL_ERRORS.store(0, Ordering::SeqCst);
}

/// Get current steal success rate (for programmatic checks)
pub fn get_steal_success_rate() -> f64 {
    let attempts = DEBUG_STEAL_ATTEMPTS.load(Ordering::SeqCst);
    let successes = DEBUG_STEAL_SUCCESSES.load(Ordering::SeqCst);

    if attempts > 0 {
        100.0 * successes as f64 / attempts as f64
    } else {
        0.0
    }
}

/// Get current CAS failure ratio (failures per success)
pub fn get_cas_failure_ratio() -> f64 {
    let successes = DEBUG_STEAL_SUCCESSES.load(Ordering::SeqCst);
    let failures = DEBUG_CAS_FAILURES.load(Ordering::SeqCst);

    if successes > 0 {
        failures as f64 / successes as f64
    } else {
        0.0
    }
}
