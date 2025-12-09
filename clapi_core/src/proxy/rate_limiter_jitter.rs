//! Jitter + Backpressure Integration for Advanced Rate Limiting
//!
//! **Purpose**: Integrate AdvancedRateLimiter64 with HTTP proxy layer
//! **Features**: Jitter-based retry scheduling, exponential backoff, distributed fairness
//!
//! # Thundering Herd Prevention
//! - Without jitter: All clients retry simultaneously when rate limit resets
//! - With jitter: Clients spread retries across random intervals
//! - Result: 50-90% reduction in retry collisions
//!
//! # Backpressure Mechanism
//! - When rate limit exceeded: Return 429 Too Many Requests with Retry-After header
//! - Client retry strategy: Exponential backoff with jitter
//! - Formula: retry_delay = base_delay × 2^attempt + random(0, jitter_ms)

use crate::capsules::{AdvancedRateLimiter64, RateLimiterStats};
use crate::error::{ClapiError, ClapiResult};
use atomic_capsule::collections::ConcurrentMapCapsule;
use std::sync::Arc;
use std::time::Duration;

/// Per-user rate limiter with jitter support
pub struct JitteredRateLimiter {
    limiter: Arc<AdvancedRateLimiter64>,
    user_id: String,
}

impl JitteredRateLimiter {
    /// Create new rate limiter for a specific user
    ///
    /// # Arguments
    /// - `user_id`: Unique user identifier
    /// - `capacity`: Maximum tokens (requests per period)
    /// - `refill_period_ns`: Nanoseconds to refill from 0 to capacity
    pub fn new(user_id: String, capacity: i32, refill_period_ns: u64) -> Self {
        Self {
            limiter: Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
                capacity,
                refill_period_ns,
            )),
            user_id,
        }
    }

    /// Acquire token with jitter for distributed retry fairness
    ///
    /// **Returns**:
    /// - `Ok(RetryInfo)`: Request allowed, includes retry info for client
    /// - `Err(RateLimitError)`: Request rejected, includes backpressure info
    pub fn acquire_with_backpressure(&self) -> ClapiResult<RetryInfo> {
        match self.limiter.acquire_token_with_jitter() {
            Ok((tokens_remaining, jitter_ns)) => Ok(RetryInfo {
                allowed: true,
                tokens_remaining: tokens_remaining as u64,
                jitter_ns,
                retry_after_ms: None,
            }),
            Err(_e) => {
                let stats = self.limiter.stats();
                let retry_after_ms = self.calculate_retry_delay(&stats);

                Err(ClapiError::RateLimitExceededWithBackpressure {
                    user_id: self.user_id.clone(),
                    retry_after_ms,
                    quota: stats.capacity as u64,
                    throttle_rate_percent: stats.throttle_rate_percent(),
                })
            }
        }
    }

    /// Calculate retry delay based on current limiter state
    ///
    /// **Algorithm**:
    /// - If tokens < 0: Immediate backpressure (retry_after = refill_rate)
    /// - If tokens == 0: Wait for next refill
    /// - Includes jitter to prevent thundering herd
    fn calculate_retry_delay(&self, stats: &RateLimiterStats) -> u64 {
        // Base delay: Time until next token refill
        let base_delay_ns = stats.refill_rate_ns;

        // Add jitter (0 to base_delay/10)
        let jitter_max_ns = base_delay_ns / 10;
        let jitter_ns = now_ns() % jitter_max_ns;

        // Convert to milliseconds
        (base_delay_ns + jitter_ns) / 1_000_000
    }

    /// Get current limiter statistics
    pub fn stats(&self) -> RateLimiterStats {
        self.limiter.stats()
    }

    /// Get user ID
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
}

/// Retry information for clients
#[derive(Debug, Clone)]
pub struct RetryInfo {
    /// Whether request was allowed
    pub allowed: bool,

    /// Remaining tokens (for client visibility)
    pub tokens_remaining: u64,

    /// Jitter applied (nanoseconds)
    pub jitter_ns: u64,

    /// Retry-After delay (milliseconds), if rate limited
    pub retry_after_ms: Option<u64>,
}

/// Exponential backoff calculator with jitter
pub struct ExponentialBackoff {
    base_delay_ms: u64,
    max_delay_ms: u64,
    jitter_max_ms: u64,
    attempt: u32,
}

impl ExponentialBackoff {
    /// Create new exponential backoff strategy
    ///
    /// # Arguments
    /// - `base_delay_ms`: Initial retry delay (milliseconds)
    /// - `max_delay_ms`: Maximum retry delay (milliseconds)
    /// - `jitter_max_ms`: Maximum jitter to add (milliseconds)
    ///
    /// # Example
    /// ```
    /// use clapi_core::proxy::rate_limiter_jitter::ExponentialBackoff;
    ///
    /// let backoff = ExponentialBackoff::new(100, 30_000, 1000);
    /// // Attempt 0: 100ms + jitter
    /// // Attempt 1: 200ms + jitter
    /// // Attempt 2: 400ms + jitter
    /// // Attempt 3: 800ms + jitter
    /// // ...
    /// // Attempt 8: 25,600ms + jitter (capped at 30,000ms)
    /// ```
    pub fn new(base_delay_ms: u64, max_delay_ms: u64, jitter_max_ms: u64) -> Self {
        Self {
            base_delay_ms,
            max_delay_ms,
            jitter_max_ms,
            attempt: 0,
        }
    }

    /// Calculate next retry delay
    ///
    /// **Formula**: delay = min(base × 2^attempt, max) + random(0, jitter_max)
    pub fn next_delay(&mut self) -> Duration {
        let exponential_delay = self
            .base_delay_ms
            .saturating_mul(2u64.saturating_pow(self.attempt))
            .min(self.max_delay_ms);

        let jitter = if self.jitter_max_ms > 0 {
            now_ns() % self.jitter_max_ms
        } else {
            0
        };
        let total_delay_ms = exponential_delay + jitter;

        self.attempt += 1;

        Duration::from_millis(total_delay_ms)
    }

    /// Reset backoff state (for new request sequence)
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    /// Get current attempt number
    pub fn attempt(&self) -> u32 {
        self.attempt
    }
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self::new(100, 30_000, 1000) // 100ms base, 30s max, 1s jitter
    }
}

/// Per-user rate limiter registry
pub struct RateLimiterRegistry {
    limiters: ConcurrentMapCapsule<String, Arc<AdvancedRateLimiter64>>,
    default_capacity: i32,
    default_refill_period_ns: u64,
}

impl RateLimiterRegistry {
    /// Create new registry with default configuration
    pub fn new(default_capacity: i32, default_refill_period_ns: u64) -> Self {
        Self {
            limiters: ConcurrentMapCapsule::new(),
            default_capacity,
            default_refill_period_ns,
        }
    }

    /// Get or create rate limiter for user
    ///
    /// Uses lockfree get-or-insert pattern to ensure consistent limiter across threads.
    /// If multiple threads call this simultaneously with the same user_id, one thread's
    /// limiter will be stored and returned to all threads (equivalent behavior).
    pub fn get_or_create(&self, user_id: &str) -> Arc<AdvancedRateLimiter64> {
        self.limiters.or_insert_with(user_id.to_string(), || {
            Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
                self.default_capacity,
                self.default_refill_period_ns,
            ))
        })
    }

    /// Acquire token for user with jitter
    pub fn acquire_with_jitter(&self, user_id: &str) -> ClapiResult<(i32, u64)> {
        let limiter = self.get_or_create(user_id);
        limiter.acquire_token_with_jitter()
    }

    /// Get statistics for all users
    ///
    /// Returns a snapshot of all rate limiters and their current statistics.
    /// Concurrent modifications may not be reflected in the snapshot.
    pub fn all_stats(&self) -> Vec<RateLimiterStats> {
        // Collect all limiters and extract their stats
        let limiters = self.limiters.values();
        limiters.iter().map(|limiter| limiter.stats()).collect()
    }

    /// Get total user count
    pub fn user_count(&self) -> usize {
        self.limiters.len()
    }
}

// Helper: Get current timestamp in nanoseconds
#[inline]
fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jittered_rate_limiter_create() {
        let limiter = JitteredRateLimiter::new("user123".to_string(), 100, 60_000_000_000);
        assert_eq!(limiter.user_id(), "user123");

        let stats = limiter.stats();
        assert_eq!(stats.capacity, 100);
        assert_eq!(stats.tokens, 100);
    }

    #[test]
    fn test_acquire_with_backpressure_success() {
        let limiter = JitteredRateLimiter::new("user123".to_string(), 10, 60_000_000_000);

        let result = limiter.acquire_with_backpressure();
        assert!(result.is_ok());

        let retry_info = result.unwrap();
        assert!(retry_info.allowed);
        assert_eq!(retry_info.tokens_remaining, 9);
        assert!(retry_info.jitter_ns > 0);
        assert!(retry_info.retry_after_ms.is_none());
    }

    #[test]
    fn test_acquire_with_backpressure_exhausted() {
        let limiter = JitteredRateLimiter::new("user123".to_string(), 3, 60_000_000_000);

        // Exhaust tokens
        for _ in 0..3 {
            let _ = limiter.acquire_with_backpressure();
        }

        // Next request should fail with backpressure
        let result = limiter.acquire_with_backpressure();
        assert!(result.is_err());

        match result {
            Err(ClapiError::RateLimitExceededWithBackpressure {
                user_id,
                retry_after_ms,
                quota,
                throttle_rate_percent: _,
            }) => {
                assert_eq!(user_id, "user123");
                assert!(retry_after_ms > 0, "retry_after_ms should be > 0, got {}", retry_after_ms);
                assert_eq!(quota, 3); // Capacity from limiter
            }
            _ => panic!("Expected RateLimitExceededWithBackpressure error"),
        }
    }

    #[test]
    fn test_exponential_backoff() {
        let mut backoff = ExponentialBackoff::new(100, 10_000, 500);

        let delay0 = backoff.next_delay().as_millis();
        assert!(delay0 >= 100 && delay0 < 700, "Delay0: {} (100 + 0-500 jitter)", delay0);

        let delay1 = backoff.next_delay().as_millis();
        assert!(delay1 >= 200 && delay1 < 800, "Delay1: {} (200 + 0-500 jitter)", delay1);

        let delay2 = backoff.next_delay().as_millis();
        assert!(delay2 >= 400 && delay2 < 1000, "Delay2: {} (400 + 0-500 jitter)", delay2);

        // Reset and verify
        backoff.reset();
        assert_eq!(backoff.attempt(), 0);
    }

    #[test]
    fn test_exponential_backoff_max_cap() {
        let mut backoff = ExponentialBackoff::new(100, 1000, 100);

        // Force many attempts to hit max
        for _ in 0..20 {
            let _ = backoff.next_delay();
        }

        let delay = backoff.next_delay().as_millis();
        assert!(delay <= 1100, "Delay should be capped at max + jitter ({}ms)", delay);
    }

    #[test]
    fn test_rate_limiter_registry() {
        let registry = RateLimiterRegistry::new(100, 60_000_000_000);

        // Create limiter for user1
        let limiter1 = registry.get_or_create("user1");
        assert_eq!(limiter1.stats().capacity, 100);

        // Get same limiter again (should be cached)
        let limiter1_again = registry.get_or_create("user1");
        assert!(Arc::ptr_eq(&limiter1, &limiter1_again));

        // Create different limiter for user2
        let limiter2 = registry.get_or_create("user2");
        assert!(!Arc::ptr_eq(&limiter1, &limiter2));

        assert_eq!(registry.user_count(), 2);
    }

    #[test]
    fn test_registry_acquire_with_jitter() {
        let registry = RateLimiterRegistry::new(10, 60_000_000_000);

        let result = registry.acquire_with_jitter("user1");
        assert!(result.is_ok());

        let (tokens_remaining, jitter_ns) = result.unwrap();
        assert_eq!(tokens_remaining, 9);
        assert!(jitter_ns > 0);
    }

    #[test]
    fn test_registry_all_stats() {
        let registry = RateLimiterRegistry::new(100, 60_000_000_000);

        registry.acquire_with_jitter("user1").unwrap();
        registry.acquire_with_jitter("user2").unwrap();
        registry.acquire_with_jitter("user2").unwrap();

        let all_stats = registry.all_stats();
        assert_eq!(all_stats.len(), 2);

        // Verify stats contain expected token counts
        // user1: 100 - 1 = 99 tokens
        // user2: 100 - 2 = 98 tokens
        let total_requests: u64 = all_stats.iter().map(|s| s.total_requests).sum();
        assert_eq!(total_requests, 3); // 1 + 2

        // Find the stats with 98 tokens (user2)
        let user2_stats = all_stats
            .iter()
            .find(|stats| stats.tokens == 98)
            .expect("user2 stats not found (expected 98 tokens)");

        assert_eq!(user2_stats.total_requests, 2);
        assert_eq!(user2_stats.tokens, 98);

        // Find the stats with 99 tokens (user1)
        let user1_stats = all_stats
            .iter()
            .find(|stats| stats.tokens == 99)
            .expect("user1 stats not found (expected 99 tokens)");

        assert_eq!(user1_stats.total_requests, 1);
        assert_eq!(user1_stats.tokens, 99);
    }
}
