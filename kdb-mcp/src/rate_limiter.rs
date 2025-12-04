//! RateLimiterCapsule - T1 Atomic Token Bucket Rate Limiter (4 KB)
//!
//! Lockfree token bucket rate limiting with configurable refill.
//! **Latency**: <150ns check + consume
//! **Tier**: T1 Atomic (DualAtomicU64 for tokens + timestamp)

use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// RateLimiterCapsule (4 KB, 64-byte aligned)
// ============================================================================

#[repr(C, align(64))]
pub struct RateLimiterCapsule {
    // Token bucket state (64 bytes, single cache line)
    pub tokens: AtomicU64,              // Current available tokens (Q16.16 fixed-point)
    pub last_refill_ns: AtomicU64,      // Last refill timestamp (ns)
    pub max_tokens: AtomicU64,          // Maximum token capacity (Q16.16)
    pub refill_rate: AtomicU64,         // Tokens per second (Q16.16)
    pub requests_allowed: AtomicU64,    // Total requests allowed
    pub requests_denied: AtomicU64,     // Total requests denied
    pub total_wait_ns: AtomicU64,       // Total wait time (ns)
    _padding: [u8; 8],

    // Reserved space (4KB - 64 bytes = 4032 bytes)
    _reserved: [u8; 4032],
}

impl RateLimiterCapsule {
    /// Create new rate limiter (max 1000 requests/sec by default)
    pub const fn new() -> Self {
        Self::with_rate(1000 << 16) // 1000.0 in Q16.16
    }

    /// Create rate limiter with custom rate (Q16.16 fixed-point)
    pub const fn with_rate(tokens_per_second: u64) -> Self {
        Self {
            tokens: AtomicU64::new(tokens_per_second), // Start with full bucket
            last_refill_ns: AtomicU64::new(0),
            max_tokens: AtomicU64::new(tokens_per_second),
            refill_rate: AtomicU64::new(tokens_per_second),
            requests_allowed: AtomicU64::new(0),
            requests_denied: AtomicU64::new(0),
            total_wait_ns: AtomicU64::new(0),
            _padding: [0; 8],
            _reserved: [0; 4032],
        }
    }

    /// Check rate limit and consume token (<150ns)
    ///
    /// Returns `Ok(())` if allowed, `Err(wait_ns)` if rate limited.
    pub fn check(&self, cost: u64) -> Result<(), u64> {
        let now_ns = self.get_timestamp_ns();

        // Refill tokens based on elapsed time
        self.refill(now_ns);

        // Try to consume tokens (CAS loop for lockfree)
        loop {
            let current_tokens = self.tokens.load(Ordering::Acquire);

            if current_tokens >= cost {
                // Enough tokens available
                let new_tokens = current_tokens - cost;
                if self.tokens.compare_exchange(
                    current_tokens,
                    new_tokens,
                    Ordering::Release,
                    Ordering::Acquire,
                ).is_ok() {
                    self.requests_allowed.fetch_add(1, Ordering::Relaxed);
                    return Ok(());
                }
                // CAS failed, retry
            } else {
                // Not enough tokens, calculate wait time
                let tokens_needed = cost - current_tokens;
                let refill_rate = self.refill_rate.load(Ordering::Relaxed);
                let wait_ns = if refill_rate > 0 {
                    (tokens_needed * 1_000_000_000) / refill_rate
                } else {
                    1_000_000_000 // 1 second default
                };

                self.requests_denied.fetch_add(1, Ordering::Relaxed);
                self.total_wait_ns.fetch_add(wait_ns, Ordering::Relaxed);
                return Err(wait_ns);
            }
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&self, now_ns: u64) {
        let last_refill = self.last_refill_ns.load(Ordering::Acquire);

        if now_ns > last_refill {
            let elapsed_ns = now_ns - last_refill;

            // Calculate tokens to add: (refill_rate * elapsed_ns) / 1e9
            // Use saturating arithmetic to prevent overflow on large values
            let refill_rate = self.refill_rate.load(Ordering::Relaxed);
            // Avoid overflow: divide refill_rate by 1e9 first, then multiply by elapsed_ns
            let tokens_per_ns = (refill_rate + 500_000_000) / 1_000_000_000; // Round to nearest
            let tokens_to_add = tokens_per_ns.saturating_mul(elapsed_ns);

            if tokens_to_add > 0 {
                // Add tokens up to max capacity
                let max_tokens = self.max_tokens.load(Ordering::Relaxed);
                loop {
                    let current_tokens = self.tokens.load(Ordering::Acquire);
                    let new_tokens = core::cmp::min(current_tokens + tokens_to_add, max_tokens);

                    if self.tokens.compare_exchange(
                        current_tokens,
                        new_tokens,
                        Ordering::Release,
                        Ordering::Acquire,
                    ).is_ok() {
                        // Update last refill timestamp
                        self.last_refill_ns.store(now_ns, Ordering::Release);
                        break;
                    }
                }
            }
        }
    }

    /// Get statistics
    pub fn get_stats(&self) -> RateLimiterStats {
        RateLimiterStats {
            requests_allowed: self.requests_allowed.load(Ordering::Relaxed),
            requests_denied: self.requests_denied.load(Ordering::Relaxed),
            current_tokens: self.tokens.load(Ordering::Relaxed),
            max_tokens: self.max_tokens.load(Ordering::Relaxed),
            avg_wait_ns: self.get_avg_wait_ns(),
        }
    }

    fn get_avg_wait_ns(&self) -> u64 {
        let total_wait = self.total_wait_ns.load(Ordering::Relaxed);
        let denied = self.requests_denied.load(Ordering::Relaxed);

        if denied > 0 {
            total_wait / denied
        } else {
            0
        }
    }

    #[inline]
    fn get_timestamp_ns(&self) -> u64 {
        #[cfg(feature = "std")]
        {
            use std::time::SystemTime;
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0)
        }
        #[cfg(not(feature = "std"))]
        {
            0 // No-op in no_std
        }
    }

    // ========================================================================
    // Test Helper Methods (integration test support)
    // ========================================================================

    /// Get refill window in nanoseconds (calculated from refill_rate)
    #[doc(hidden)]
    pub fn refill_window_ns(&self) -> u64 {
        let refill_rate = self.refill_rate.load(Ordering::Relaxed);
        if refill_rate > 0 {
            1_000_000_000 / refill_rate // 1 second / tokens_per_second
        } else {
            1_000_000_000 // 1 second default
        }
    }
}

/// Rate limiter statistics
#[derive(Debug, Clone, Copy)]
pub struct RateLimiterStats {
    pub requests_allowed: u64,
    pub requests_denied: u64,
    pub current_tokens: u64,      // Q16.16 fixed-point
    pub max_tokens: u64,           // Q16.16 fixed-point
    pub avg_wait_ns: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{size_of, align_of};

    #[test]
    fn test_rate_limiter_size() {
        assert_eq!(size_of::<RateLimiterCapsule>(), 4096, "RateLimiterCapsule must be 4 KB");
    }

    #[test]
    fn test_rate_limiter_alignment() {
        assert_eq!(align_of::<RateLimiterCapsule>(), 64, "RateLimiterCapsule must be 64-byte aligned");
    }

    #[test]
    fn test_rate_limit_allow() {
        let limiter = RateLimiterCapsule::new();

        // Should allow first request
        assert!(limiter.check(1 << 16).is_ok()); // 1.0 token in Q16.16

        let stats = limiter.get_stats();
        assert_eq!(stats.requests_allowed, 1);
        assert_eq!(stats.requests_denied, 0);
    }

    #[test]
    fn test_rate_limit_deny() {
        let limiter = RateLimiterCapsule::with_rate(10 << 16); // 10 tokens/sec

        // Consume all tokens
        for _ in 0..10 {
            assert!(limiter.check(1 << 16).is_ok());
        }

        // Next request should be denied
        let result = limiter.check(1 << 16);
        assert!(result.is_err());

        let stats = limiter.get_stats();
        assert_eq!(stats.requests_allowed, 10);
        assert_eq!(stats.requests_denied, 1);
    }
}
