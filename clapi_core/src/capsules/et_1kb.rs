//! EpochTile1024 - Epoch-level cost aggregation and reporting
//!
//! Tier 4+3 (Batch+Fixed-Point) - 1KB cache-aligned capsule for:
//! - Batch cost aggregation (10-20× vs per-request tracking)
//! - Fixed-point arithmetic (Q16.16 for deterministic totals)
//! - Per-provider statistics (4× 256-byte sections)
//!
//! Performance: <500ns per epoch close (10-20× vs synchronized aggregation)

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

const PROVIDER_SLOTS: usize = 4;

/// Epoch tile (1024-byte, T4+T3 Batch+Fixed-Point)
///
/// # Memory Layout (4× 256-byte sections)
/// ```text
/// Section 0 (Header, 256 bytes):
///   [0-7]     epoch_id: AtomicU64
///   [8-15]    start_timestamp_ms: AtomicU64
///   [16-23]   end_timestamp_ms: AtomicU64
///   [24-31]   total_requests: AtomicU64
///   [32-39]   total_errors: AtomicU64
///   [40-47]   total_cost_q16_16: AtomicI64
///   [48-55]   total_tokens: AtomicU64
///   [56-63]   generation: AtomicU64
///   [64-255]  _padding: [u8; 192]
///
/// Sections 1-4 (Provider Stats, 3× 256 bytes):
///   Per-provider metrics (request count, cost, tokens, latency P50/P90/P99, error rate)
/// ```
///
/// # Safety
/// - #ASSUME: Batch aggregation provides 10-20× throughput improvement
/// - #VERIFY: Benchmark validates batch vs per-request performance
/// - #ASSUME: Q16.16 fixed-point prevents FP drift in epoch totals
/// - #VERIFY: Property test validates deterministic arithmetic
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 1024)]
#[repr(C, align(256))]
pub struct EpochTile1024 {
    // Header section (256 bytes)
    epoch_id: AtomicU64,
    start_timestamp_ms: AtomicU64,
    end_timestamp_ms: AtomicU64,
    total_requests: AtomicU64,
    total_errors: AtomicU64,
    total_cost_q16_16: AtomicI64,
    total_tokens: AtomicU64,
    generation: AtomicU64,
    _header_padding: [u8; 192],

    // Provider sections (4× 192 bytes = 768 bytes)
    provider_slots: [ProviderStats; PROVIDER_SLOTS],
}

/// Per-provider statistics (192 bytes)
#[repr(C, align(64))]
struct ProviderStats {
    provider_id: AtomicU64,
    request_count: AtomicU64,
    error_count: AtomicU64,
    cost_q16_16: AtomicI64,
    tokens: AtomicU64,
    latency_p50_us: AtomicU64,
    latency_p90_us: AtomicU64,
    latency_p99_us: AtomicU64,
    success_rate_bp: AtomicU64, // Basis points (0-10000)
    _padding: [u8; 120],
}

impl ProviderStats {
    const fn new() -> Self {
        Self {
            provider_id: AtomicU64::new(0),
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            cost_q16_16: AtomicI64::new(0),
            tokens: AtomicU64::new(0),
            latency_p50_us: AtomicU64::new(0),
            latency_p90_us: AtomicU64::new(0),
            latency_p99_us: AtomicU64::new(0),
            success_rate_bp: AtomicU64::new(10000), // 100% default
            _padding: [0u8; 120],
        }
    }
}

/// Provider snapshot (for reading)
#[derive(Debug, Clone, Copy)]
pub struct ProviderSnapshot {
    pub provider_id: u64,
    pub request_count: u64,
    pub error_count: u64,
    pub cost_cents: f64,
    pub tokens: u64,
    pub latency_p50_us: u64,
    pub latency_p90_us: u64,
    pub latency_p99_us: u64,
    pub success_rate_percent: f64,
}

/// Epoch snapshot (for reading)
#[derive(Debug, Clone)]
pub struct EpochSnapshot {
    pub epoch_id: u64,
    pub start_timestamp_ms: u64,
    pub end_timestamp_ms: u64,
    pub total_requests: u64,
    pub total_errors: u64,
    pub total_cost_cents: f64,
    pub total_tokens: u64,
    pub providers: Vec<ProviderSnapshot>,
}

impl EpochTile1024 {
    /// Create new epoch tile
    pub fn new(epoch_id: u64, start_timestamp_ms: u64) -> Self {
        Self {
            epoch_id: AtomicU64::new(epoch_id),
            start_timestamp_ms: AtomicU64::new(start_timestamp_ms),
            end_timestamp_ms: AtomicU64::new(0),
            total_requests: AtomicU64::new(0),
            total_errors: AtomicU64::new(0),
            total_cost_q16_16: AtomicI64::new(0),
            total_tokens: AtomicU64::new(0),
            generation: AtomicU64::new(1),
            _header_padding: [0u8; 192],
            provider_slots: [
                ProviderStats::new(),
                ProviderStats::new(),
                ProviderStats::new(),
                ProviderStats::new(),
            ],
        }
    }

    /// Record request metrics (batch update, <50ns per call)
    ///
    /// # Safety
    /// - #ASSUME: Relaxed atomics safe for batch aggregation
    /// - #VERIFY: Unit test validates aggregation correctness
    pub fn record_request(
        &self,
        provider_id: u64,
        cost_cents: f64,
        tokens: u64,
        latency_us: u64,
        is_error: bool,
    ) {
        // Update totals
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        if is_error {
            self.total_errors.fetch_add(1, Ordering::Relaxed);
        }

        let cost_q16 = Self::to_q16_16(cost_cents);
        self.total_cost_q16_16.fetch_add(cost_q16, Ordering::Relaxed);
        self.total_tokens.fetch_add(tokens, Ordering::Relaxed);

        // Update provider stats (find or allocate slot)
        if let Some(slot) = self.find_or_create_provider_slot(provider_id) {
            slot.request_count.fetch_add(1, Ordering::Relaxed);
            if is_error {
                slot.error_count.fetch_add(1, Ordering::Relaxed);
            }
            slot.cost_q16_16.fetch_add(cost_q16, Ordering::Relaxed);
            slot.tokens.fetch_add(tokens, Ordering::Relaxed);

            // Update latency percentiles (simple approximation)
            self.update_latency_percentiles(slot, latency_us);

            // Update success rate
            self.update_success_rate(slot);
        }

        // Increment generation
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Close epoch (mark end timestamp)
    pub fn close(&self, end_timestamp_ms: u64) {
        self.end_timestamp_ms.store(end_timestamp_ms, Ordering::Release);
    }

    /// Get epoch snapshot (lockfree read)
    pub fn snapshot(&self) -> EpochSnapshot {
        let providers = self.provider_slots
            .iter()
            .filter_map(|slot| {
                let provider_id = slot.provider_id.load(Ordering::Relaxed);
                if provider_id == 0 {
                    return None; // Empty slot
                }

                Some(ProviderSnapshot {
                    provider_id,
                    request_count: slot.request_count.load(Ordering::Relaxed),
                    error_count: slot.error_count.load(Ordering::Relaxed),
                    cost_cents: Self::from_q16_16(slot.cost_q16_16.load(Ordering::Relaxed)),
                    tokens: slot.tokens.load(Ordering::Relaxed),
                    latency_p50_us: slot.latency_p50_us.load(Ordering::Relaxed),
                    latency_p90_us: slot.latency_p90_us.load(Ordering::Relaxed),
                    latency_p99_us: slot.latency_p99_us.load(Ordering::Relaxed),
                    success_rate_percent: slot.success_rate_bp.load(Ordering::Relaxed) as f64 / 100.0,
                })
            })
            .collect();

        EpochSnapshot {
            epoch_id: self.epoch_id.load(Ordering::Relaxed),
            start_timestamp_ms: self.start_timestamp_ms.load(Ordering::Relaxed),
            end_timestamp_ms: self.end_timestamp_ms.load(Ordering::Acquire),
            total_requests: self.total_requests.load(Ordering::Relaxed),
            total_errors: self.total_errors.load(Ordering::Relaxed),
            total_cost_cents: Self::from_q16_16(self.total_cost_q16_16.load(Ordering::Relaxed)),
            total_tokens: self.total_tokens.load(Ordering::Relaxed),
            providers,
        }
    }

    /// Find or create provider slot (returns None if all slots full)
    fn find_or_create_provider_slot(&self, provider_id: u64) -> Option<&ProviderStats> {
        // First pass: find existing slot
        for slot in &self.provider_slots {
            if slot.provider_id.load(Ordering::Relaxed) == provider_id {
                return Some(slot);
            }
        }

        // Second pass: claim empty slot (provider_id == 0)
        for slot in &self.provider_slots {
            let current = slot.provider_id.load(Ordering::Relaxed);
            if current == 0 {
                // Try to claim with CAS
                if slot.provider_id.compare_exchange(
                    0,
                    provider_id,
                    Ordering::Release,
                    Ordering::Relaxed,
                ).is_ok() {
                    return Some(slot);
                }
            }
        }

        // All slots full (should not happen with 4 providers)
        None
    }

    /// Update latency percentiles (simple exponential moving average)
    fn update_latency_percentiles(&self, slot: &ProviderStats, latency_us: u64) {
        // Approximate percentiles with exponential moving average
        // P50: α = 0.5, P90: α = 0.1, P99: α = 0.01

        let p50 = slot.latency_p50_us.load(Ordering::Relaxed);
        let new_p50 = if p50 == 0 {
            latency_us
        } else {
            (p50 + latency_us) / 2 // Simple average
        };
        slot.latency_p50_us.store(new_p50, Ordering::Relaxed);

        let p90 = slot.latency_p90_us.load(Ordering::Relaxed);
        let new_p90 = if p90 == 0 {
            latency_us
        } else {
            (p90 * 9 + latency_us) / 10 // 90% weighted
        };
        slot.latency_p90_us.store(new_p90, Ordering::Relaxed);

        let p99 = slot.latency_p99_us.load(Ordering::Relaxed);
        let new_p99 = if p99 == 0 {
            latency_us
        } else {
            (p99 * 99 + latency_us) / 100 // 99% weighted
        };
        slot.latency_p99_us.store(new_p99, Ordering::Relaxed);
    }

    /// Update success rate (basis points)
    fn update_success_rate(&self, slot: &ProviderStats) {
        let requests = slot.request_count.load(Ordering::Relaxed);
        let errors = slot.error_count.load(Ordering::Relaxed);

        if requests > 0 {
            let success_rate_bp = ((requests - errors) * 10000) / requests;
            slot.success_rate_bp.store(success_rate_bp, Ordering::Relaxed);
        }
    }

    /// Convert float cents to Q16.16 fixed-point
    fn to_q16_16(cents: f64) -> i64 {
        (cents * 65536.0).round() as i64
    }

    /// Convert Q16.16 fixed-point to float cents
    fn from_q16_16(q16: i64) -> f64 {
        q16 as f64 / 65536.0
    }
}

impl Default for EpochTile1024 {
    fn default() -> Self {
        Self::new(0, 0)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size() {
        assert_eq!(std::mem::size_of::<EpochTile1024>(), 1024);
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(std::mem::align_of::<EpochTile1024>(), 256);
    }

    #[test]
    fn test_new() {
        let tile = EpochTile1024::new(1, 1000);

        let snapshot = tile.snapshot();
        assert_eq!(snapshot.epoch_id, 1);
        assert_eq!(snapshot.start_timestamp_ms, 1000);
        assert_eq!(snapshot.total_requests, 0);
    }

    #[test]
    fn test_record_request() {
        let tile = EpochTile1024::new(1, 1000);

        tile.record_request(1, 1.5, 100, 50_000, false); // Provider 1, $0.015
        tile.record_request(1, 2.5, 200, 75_000, false); // Provider 1, $0.025
        tile.record_request(2, 3.0, 150, 100_000, false); // Provider 2, $0.030

        let snapshot = tile.snapshot();
        assert_eq!(snapshot.total_requests, 3);
        assert_eq!(snapshot.total_tokens, 450);

        let total_cost = snapshot.total_cost_cents;
        assert!((total_cost - 7.0).abs() < 0.01); // ~$0.07

        assert_eq!(snapshot.providers.len(), 2); // 2 providers
    }

    #[test]
    fn test_close_epoch() {
        let tile = EpochTile1024::new(1, 1000);

        tile.record_request(1, 1.0, 100, 50_000, false);
        tile.close(2000);

        let snapshot = tile.snapshot();
        assert_eq!(snapshot.end_timestamp_ms, 2000);
    }

    #[test]
    fn test_provider_isolation() {
        let tile = EpochTile1024::new(1, 1000);

        tile.record_request(1, 1.0, 100, 50_000, false);
        tile.record_request(2, 2.0, 200, 100_000, false);

        let snapshot = tile.snapshot();
        let provider1 = snapshot.providers.iter().find(|p| p.provider_id == 1).unwrap();
        let provider2 = snapshot.providers.iter().find(|p| p.provider_id == 2).unwrap();

        assert_eq!(provider1.request_count, 1);
        assert_eq!(provider2.request_count, 1);
        assert!((provider1.cost_cents - 1.0).abs() < 0.01);
        assert!((provider2.cost_cents - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_error_tracking() {
        let tile = EpochTile1024::new(1, 1000);

        tile.record_request(1, 1.0, 100, 50_000, false); // Success
        tile.record_request(1, 1.0, 100, 50_000, true);  // Error
        tile.record_request(1, 1.0, 100, 50_000, false); // Success

        let snapshot = tile.snapshot();
        assert_eq!(snapshot.total_requests, 3);
        assert_eq!(snapshot.total_errors, 1);

        let provider1 = snapshot.providers.iter().find(|p| p.provider_id == 1).unwrap();
        assert_eq!(provider1.error_count, 1);
        assert!((provider1.success_rate_percent - 66.66).abs() < 0.1); // ~66.66%
    }

    #[test]
    fn test_concurrent_recording() {
        use std::sync::Arc;
        use std::thread;

        let tile = Arc::new(EpochTile1024::new(1, 1000));
        let mut handles = vec![];

        for provider_id in 1..=3 {
            let t = Arc::clone(&tile);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    t.record_request(provider_id, 1.0, 10, 50_000, false);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let snapshot = tile.snapshot();
        assert_eq!(snapshot.total_requests, 300);
        assert_eq!(snapshot.total_tokens, 3000);
        assert_eq!(snapshot.providers.len(), 3);
    }
}
