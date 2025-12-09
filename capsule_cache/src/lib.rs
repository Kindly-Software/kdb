//! Capsule-cache: Redis-style cache built with computational capsule architecture (UCE34 + Chaos).
//! - Lockfree, cache-line-aligned capsules (no mutex/RwLock).
//! - Generation counters + commit-flip for TOCTOU safety.
//! - Optional integrity/encryption/multi-tenant features forwarded to `atomic_capsule`.

#![forbid(unsafe_code)]

use std::hash::Hash;
use std::time::Duration;

use atomic_capsule::collections::cache::LockfreeCacheCapsule;
use atomic_capsule::collections::error::MapError;
use atomic_capsule::collections::{HistogramCapsule, StatsCapsule64, StatsSnapshot};

pub mod persistence;
pub mod sharded;
pub mod slowlog;
pub mod distributed;

const DEFAULT_TTL: Duration = Duration::from_secs(365 * 24 * 3600); // 1 year baseline for counters with no TTL

/// Thin orchestration wrapper around `LockfreeCacheCapsule` with UCE34 defaults.
pub struct CapsuleCache<K>
where
    K: Hash + Eq,
{
    inner: LockfreeCacheCapsule<K, String>,
    stats: StatsCapsule64,
    histogram: HistogramCapsule,
}

impl<K> CapsuleCache<K>
where
    K: Hash + Eq,
{
    /// Create a cache with the default 16K slots (8MB, 512B per slot).
    pub fn new() -> Self {
        Self {
            inner: LockfreeCacheCapsule::new(),
            stats: StatsCapsule64::new(),
            histogram: HistogramCapsule::new(),
        }
    }

    /// Create a cache with a custom capacity (rounded up to next power of two).
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: LockfreeCacheCapsule::with_capacity(capacity),
            stats: StatsCapsule64::new(),
            histogram: HistogramCapsule::new(),
        }
    }

    /// Fetch a value if present and not expired.
    pub fn get(&self, key: &K) -> Option<String> {
        self.stats.increment_requests();
        let start = std::time::Instant::now();
        let res = self.inner.get(key);
        let elapsed = start.elapsed().as_nanos() as u64;
        self.stats.record_latency_ns(elapsed);
        self.histogram.record(elapsed);
        if res.is_some() {
            self.stats.record_success();
        } else {
            self.stats.record_failure();
        }
        res
    }

    /// Insert or replace a value with a TTL.
    pub fn insert(&self, key: K, value: String, ttl: Duration) -> Result<(), MapError> {
        self.stats.increment_requests();
        let start = std::time::Instant::now();
        let res = self.inner.insert(key, value, ttl);
        let elapsed = start.elapsed().as_nanos() as u64;
        self.stats.record_latency_ns(elapsed);
        self.histogram.record(elapsed);
        if res.is_ok() {
            self.stats.record_success();
        } else {
            self.stats.record_failure();
        }
        res
    }

    /// Remove an entry, returning the value if it existed.
    pub fn remove(&self, key: &K) -> Option<String> {
        self.stats.increment_requests();
        let start = std::time::Instant::now();
        let res = self.inner.remove(key);
        let elapsed = start.elapsed().as_nanos() as u64;
        self.stats.record_latency_ns(elapsed);
        self.histogram.record(elapsed);
        if res.is_some() {
            self.stats.record_success();
        } else {
            self.stats.record_failure();
        }
        res
    }

    /// Remaining TTL for a key, if present and unexpired.
    pub fn ttl_remaining(&self, key: &K) -> Option<Duration> {
        self.inner.ttl(key)
    }

    /// Set a new TTL for an existing key; returns true if updated.
    pub fn expire(&self, key: &K, ttl: Duration) -> bool
    where
        K: Clone,
    {
        if let Some(value) = self.inner.get(key) {
            let _ = self.inner.insert(key.clone(), value, ttl);
            true
        } else {
            false
        }
    }

    /// Increment a numeric value by `delta`, preserving TTL if present.
    pub fn incr(&self, key: &K, delta: i64) -> Result<i64, &'static str>
    where
        K: Clone,
    {
        let current = match self.inner.get(key) {
            Some(v) => v,
            None => {
                let new_val = delta.to_string();
                let _ = self
                    .inner
                    .insert(key.clone(), new_val.clone(), DEFAULT_TTL);
                return Ok(delta);
            }
        };

        let ttl = self.inner.ttl(key).unwrap_or(DEFAULT_TTL);

        let parsed: i64 = current
            .parse()
            .map_err(|_| "ERR value is not an integer")?;
        let new_val = parsed
            .checked_add(delta)
            .ok_or("ERR increment would overflow i64")?;
        let new_str = new_val.to_string();
        let _ = self.inner.insert(key.clone(), new_str, ttl);
        Ok(new_val)
    }

    /// Evict all expired entries; returns the number removed.
    pub fn evict_expired(&self) -> usize {
        self.inner.evict_expired()
    }

    /// Clear all entries (full flush). Returns number of entries cleared.
    pub fn clear_all(&self) -> usize {
        self.inner.clear_all()
    }

    pub fn stats(&self) -> (StatsSnapshot, (Option<u64>, Option<u64>, Option<u64>, Option<u64>)) {
        let mut snap = self.stats.get_stats();
        if snap.min_latency_ns == u64::MAX {
            snap.min_latency_ns = 0;
        }

        let pct = (
            self.histogram.p50(),
            self.histogram.p95(),
            self.histogram.p99(),
            self.histogram.p999(),
        );

        (snap, pct)
    }

    /// Histogram counters (count, overflow).
    pub fn hist_counts(&self) -> (u64, u64) {
        (self.histogram.total_count(), self.histogram.overflow_count())
    }

    /// Return up to `limit` key hashes (non-expired slots only).
    pub fn scan_hashes(&self, limit: usize) -> Vec<u64> {
        self.inner.scan_hashes(limit)
    }
}
