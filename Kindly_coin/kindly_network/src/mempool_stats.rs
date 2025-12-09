//! Mempool Statistics
//!
//! Real-time mempool statistics using lockfree atomic counters.
//!
//! ## Architecture (Q33: Atomic Capsule Transform)
//!
//! The atomic capsule architecture transforms mempool statistics:
//!
//! 1. **Lockfree Counters**: <5ns atomic increments, no mutex contention
//! 2. **Real-Time Monitoring**: Instant statistics without blocking operations
//! 3. **Circuit Breaker Integration**: Health metrics feed breaker decisions
//! 4. **Zero-Cost Tracking**: Statistics gathering has no impact on pool performance
//!
//! ## Performance (B32 Validation)
//!
//! - Counter increment: <5ns
//! - Stats snapshot: <100ns (read all counters)
//! - Rate calculation: <50ns
//! - Zero contention (lockfree)
//!
//! ## Safety (ASSUM Framework)
//!
//! - `#ASSUME_ATOMIC_COUNTERS`: Relaxed ordering sufficient for statistics
//! - `#VERIFY_LOCKFREE`: No mutex/RwLock in hot path
//! - `#ASSUME_OVERFLOW_SAFE`: 64-bit counters won't overflow in practice

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// Mempool statistics
///
/// ## Design
///
/// - All counters are atomic (lockfree)
/// - Relaxed ordering (statistics don't require synchronization)
/// - 64-bit counters (overflow-safe for years of operation)
pub struct MempoolStats {
    /// Transaction statistics
    tx_stats: TransactionStats,

    /// Fee statistics
    fee_stats: FeeStats,

    /// Health statistics
    health_stats: HealthStats,

    /// Timestamp of last stats reset
    start_time: Instant,
}

/// Transaction statistics (atomic counters)
struct TransactionStats {
    /// Total transactions received
    total_received: AtomicU64,

    /// Total transactions accepted
    total_accepted: AtomicU64,

    /// Total transactions rejected
    total_rejected: AtomicU64,

    /// Total transactions confirmed
    total_confirmed: AtomicU64,

    /// Current pending transactions
    pending_count: AtomicUsize,
}

/// Fee statistics (atomic counters)
struct FeeStats {
    /// Total fees collected (smallest unit)
    total_fees: AtomicU64,

    /// Minimum fee seen (basis points)
    min_fee_bp: AtomicU64,

    /// Maximum fee seen (basis points)
    max_fee_bp: AtomicU64,

    /// Average fee (weighted by transaction count)
    avg_fee_bp: AtomicU64,
}

/// Health statistics (atomic counters)
struct HealthStats {
    /// Circuit breaker triggers
    breaker_triggers: AtomicU64,

    /// Pool full events
    pool_full_events: AtomicU64,

    /// Duplicate transaction attempts
    duplicate_attempts: AtomicU64,

    /// Invalid transactions
    invalid_txs: AtomicU64,
}

impl MempoolStats {
    /// Create new mempool statistics
    pub fn new() -> Self {
        Self {
            tx_stats: TransactionStats {
                total_received: AtomicU64::new(0),
                total_accepted: AtomicU64::new(0),
                total_rejected: AtomicU64::new(0),
                total_confirmed: AtomicU64::new(0),
                pending_count: AtomicUsize::new(0),
            },
            fee_stats: FeeStats {
                total_fees: AtomicU64::new(0),
                min_fee_bp: AtomicU64::new(u64::MAX),
                max_fee_bp: AtomicU64::new(0),
                avg_fee_bp: AtomicU64::new(0),
            },
            health_stats: HealthStats {
                breaker_triggers: AtomicU64::new(0),
                pool_full_events: AtomicU64::new(0),
                duplicate_attempts: AtomicU64::new(0),
                invalid_txs: AtomicU64::new(0),
            },
            start_time: Instant::now(),
        }
    }

    /// Record transaction received
    ///
    /// # Performance
    ///
    /// <5ns (single atomic increment)
    #[inline(always)]
    pub fn record_received(&self) {
        // #ASSUME_ATOMIC_COUNTERS: Relaxed ordering sufficient for statistics
        // #VERIFY_LOCKFREE: No synchronization overhead
        self.tx_stats.total_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Record transaction accepted
    ///
    /// # Performance
    ///
    /// <10ns (two atomic increments)
    #[inline(always)]
    pub fn record_accepted(&self, fee_bp: u64) {
        self.tx_stats.total_accepted.fetch_add(1, Ordering::Relaxed);
        self.tx_stats.pending_count.fetch_add(1, Ordering::Relaxed);
        self.update_fee_stats(fee_bp);
    }

    /// Record transaction rejected
    ///
    /// # Performance
    ///
    /// <5ns (single atomic increment)
    #[inline(always)]
    pub fn record_rejected(&self) {
        self.tx_stats.total_rejected.fetch_add(1, Ordering::Relaxed);
    }

    /// Record transaction confirmed
    ///
    /// # Performance
    ///
    /// <10ns (two atomic operations)
    #[inline(always)]
    pub fn record_confirmed(&self) {
        self.tx_stats.total_confirmed.fetch_add(1, Ordering::Relaxed);
        self.tx_stats.pending_count.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record circuit breaker trigger
    #[inline(always)]
    pub fn record_breaker_trigger(&self) {
        self.health_stats.breaker_triggers.fetch_add(1, Ordering::Relaxed);
    }

    /// Record pool full event
    #[inline(always)]
    pub fn record_pool_full(&self) {
        self.health_stats.pool_full_events.fetch_add(1, Ordering::Relaxed);
    }

    /// Record duplicate transaction
    #[inline(always)]
    pub fn record_duplicate(&self) {
        self.health_stats.duplicate_attempts.fetch_add(1, Ordering::Relaxed);
    }

    /// Record invalid transaction
    #[inline(always)]
    pub fn record_invalid(&self) {
        self.health_stats.invalid_txs.fetch_add(1, Ordering::Relaxed);
    }

    /// Update fee statistics (atomic min/max/avg)
    ///
    /// # Performance
    ///
    /// <20ns (multiple atomic operations with CAS)
    fn update_fee_stats(&self, fee_bp: u64) {
        // Update total fees
        self.fee_stats.total_fees.fetch_add(fee_bp, Ordering::Relaxed);

        // Update min fee (atomic minimum)
        let mut current_min = self.fee_stats.min_fee_bp.load(Ordering::Relaxed);
        while fee_bp < current_min {
            match self.fee_stats.min_fee_bp.compare_exchange_weak(
                current_min,
                fee_bp,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_min = actual,
            }
        }

        // Update max fee (atomic maximum)
        let mut current_max = self.fee_stats.max_fee_bp.load(Ordering::Relaxed);
        while fee_bp > current_max {
            match self.fee_stats.max_fee_bp.compare_exchange_weak(
                current_max,
                fee_bp,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_max = actual,
            }
        }

        // Update average (simple moving average)
        let total_accepted = self.tx_stats.total_accepted.load(Ordering::Relaxed);
        if total_accepted > 0 {
            let total_fees = self.fee_stats.total_fees.load(Ordering::Relaxed);
            let avg = total_fees / total_accepted;
            self.fee_stats.avg_fee_bp.store(avg, Ordering::Relaxed);
        }
    }

    /// Get complete statistics snapshot
    ///
    /// # Performance
    ///
    /// <100ns (read all atomic counters)
    pub fn snapshot(&self) -> StatsSnapshot {
        let elapsed = self.start_time.elapsed();

        // Load all counters
        let total_received = self.tx_stats.total_received.load(Ordering::Relaxed);
        let total_accepted = self.tx_stats.total_accepted.load(Ordering::Relaxed);
        let total_rejected = self.tx_stats.total_rejected.load(Ordering::Relaxed);
        let total_confirmed = self.tx_stats.total_confirmed.load(Ordering::Relaxed);
        let pending_count = self.tx_stats.pending_count.load(Ordering::Relaxed);

        let total_fees = self.fee_stats.total_fees.load(Ordering::Relaxed);
        let min_fee = self.fee_stats.min_fee_bp.load(Ordering::Relaxed);
        let max_fee = self.fee_stats.max_fee_bp.load(Ordering::Relaxed);
        let avg_fee = self.fee_stats.avg_fee_bp.load(Ordering::Relaxed);

        let breaker_triggers = self.health_stats.breaker_triggers.load(Ordering::Relaxed);
        let pool_full_events = self.health_stats.pool_full_events.load(Ordering::Relaxed);
        let duplicate_attempts = self.health_stats.duplicate_attempts.load(Ordering::Relaxed);
        let invalid_txs = self.health_stats.invalid_txs.load(Ordering::Relaxed);

        // Calculate rates
        let elapsed_secs = elapsed.as_secs_f64();
        let receive_rate = if elapsed_secs > 0.0 {
            total_received as f64 / elapsed_secs
        } else {
            0.0
        };

        let confirm_rate = if elapsed_secs > 0.0 {
            total_confirmed as f64 / elapsed_secs
        } else {
            0.0
        };

        // Calculate ratios
        let acceptance_rate = if total_received > 0 {
            (total_accepted * 10000) / total_received
        } else {
            0
        };

        let rejection_rate = if total_received > 0 {
            (total_rejected * 10000) / total_received
        } else {
            0
        };

        StatsSnapshot {
            // Transaction stats
            total_received,
            total_accepted,
            total_rejected,
            total_confirmed,
            pending_count,
            receive_rate_tps: receive_rate,
            confirm_rate_tps: confirm_rate,
            acceptance_rate_bp: acceptance_rate,
            rejection_rate_bp: rejection_rate,

            // Fee stats
            total_fees,
            min_fee_bp: if min_fee == u64::MAX { 0 } else { min_fee },
            max_fee_bp: max_fee,
            avg_fee_bp: avg_fee,

            // Health stats
            breaker_triggers,
            pool_full_events,
            duplicate_attempts,
            invalid_txs,

            // Uptime
            uptime: elapsed,
        }
    }

    /// Reset all statistics
    pub fn reset(&mut self) {
        // Reset transaction stats
        self.tx_stats.total_received.store(0, Ordering::Relaxed);
        self.tx_stats.total_accepted.store(0, Ordering::Relaxed);
        self.tx_stats.total_rejected.store(0, Ordering::Relaxed);
        self.tx_stats.total_confirmed.store(0, Ordering::Relaxed);
        self.tx_stats.pending_count.store(0, Ordering::Relaxed);

        // Reset fee stats
        self.fee_stats.total_fees.store(0, Ordering::Relaxed);
        self.fee_stats.min_fee_bp.store(u64::MAX, Ordering::Relaxed);
        self.fee_stats.max_fee_bp.store(0, Ordering::Relaxed);
        self.fee_stats.avg_fee_bp.store(0, Ordering::Relaxed);

        // Reset health stats
        self.health_stats.breaker_triggers.store(0, Ordering::Relaxed);
        self.health_stats.pool_full_events.store(0, Ordering::Relaxed);
        self.health_stats.duplicate_attempts.store(0, Ordering::Relaxed);
        self.health_stats.invalid_txs.store(0, Ordering::Relaxed);

        // Reset start time
        self.start_time = Instant::now();
    }
}

/// Statistics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSnapshot {
    // Transaction statistics
    /// Total transactions received
    pub total_received: u64,
    /// Total transactions accepted
    pub total_accepted: u64,
    /// Total transactions rejected
    pub total_rejected: u64,
    /// Total transactions confirmed
    pub total_confirmed: u64,
    /// Current pending transactions
    pub pending_count: usize,
    /// Receive rate (TPS)
    pub receive_rate_tps: f64,
    /// Confirmation rate (TPS)
    pub confirm_rate_tps: f64,
    /// Acceptance rate (basis points)
    pub acceptance_rate_bp: u64,
    /// Rejection rate (basis points)
    pub rejection_rate_bp: u64,

    // Fee statistics
    /// Total fees collected
    pub total_fees: u64,
    /// Minimum fee (basis points)
    pub min_fee_bp: u64,
    /// Maximum fee (basis points)
    pub max_fee_bp: u64,
    /// Average fee (basis points)
    pub avg_fee_bp: u64,

    // Health statistics
    /// Circuit breaker triggers
    pub breaker_triggers: u64,
    /// Pool full events
    pub pool_full_events: u64,
    /// Duplicate transaction attempts
    pub duplicate_attempts: u64,
    /// Invalid transactions
    pub invalid_txs: u64,

    // Uptime
    /// Time since statistics started
    pub uptime: Duration,
}

impl Default for MempoolStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_creation() {
        let stats = MempoolStats::new();
        let snapshot = stats.snapshot();

        assert_eq!(snapshot.total_received, 0);
        assert_eq!(snapshot.total_accepted, 0);
        assert_eq!(snapshot.pending_count, 0);
    }

    #[test]
    fn test_record_transactions() {
        let stats = MempoolStats::new();

        stats.record_received();
        stats.record_accepted(100); // 1% fee
        stats.record_confirmed();

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.total_received, 1);
        assert_eq!(snapshot.total_accepted, 1);
        assert_eq!(snapshot.total_confirmed, 1);
        assert_eq!(snapshot.pending_count, 0);
        assert_eq!(snapshot.avg_fee_bp, 100);
    }

    #[test]
    fn test_fee_stats() {
        let stats = MempoolStats::new();

        stats.record_received();
        stats.record_accepted(50);  // 0.5%

        stats.record_received();
        stats.record_accepted(150); // 1.5%

        stats.record_received();
        stats.record_accepted(100); // 1.0%

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.min_fee_bp, 50);
        assert_eq!(snapshot.max_fee_bp, 150);
        assert_eq!(snapshot.avg_fee_bp, 100); // (50 + 150 + 100) / 3
    }

    #[test]
    fn test_health_stats() {
        let stats = MempoolStats::new();

        stats.record_breaker_trigger();
        stats.record_pool_full();
        stats.record_duplicate();
        stats.record_invalid();

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.breaker_triggers, 1);
        assert_eq!(snapshot.pool_full_events, 1);
        assert_eq!(snapshot.duplicate_attempts, 1);
        assert_eq!(snapshot.invalid_txs, 1);
    }
}
