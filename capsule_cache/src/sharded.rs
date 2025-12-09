//! Sharded cache wrapper for multi-core scaling. No external deps; simple modulo hashing.

use crate::CapsuleCache;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use atomic_capsule::collections::{HistogramCapsule, StatsSnapshot};

pub struct ShardedCache<K>
where
    K: Hash + Eq + Clone,
{
    shards: Vec<CapsuleCache<K>>,
    histogram: HistogramCapsule,
}

impl<K> ShardedCache<K>
where
    K: Hash + Eq + Clone,
{
    pub fn new(num_shards: usize, capacity_per_shard: usize) -> Self {
        let count = num_shards.max(1);
        let shards = (0..count)
            .map(|_| CapsuleCache::with_capacity(capacity_per_shard))
            .collect();
        Self {
            shards,
            histogram: HistogramCapsule::new(),
        }
    }

    fn shard_for(&self, key: &K) -> &CapsuleCache<K> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let idx = (hasher.finish() as usize) % self.shards.len();
        &self.shards[idx]
    }

    pub fn insert(&self, key: K, value: String, ttl: Duration) -> Result<(), crate::MapError> {
        let start = std::time::Instant::now();
        let res = self.shard_for(&key).insert(key, value, ttl);
        let elapsed = start.elapsed().as_nanos() as u64;
        self.histogram.record(elapsed);
        res
    }

    pub fn get(&self, key: &K) -> Option<String> {
        let start = std::time::Instant::now();
        let res = self.shard_for(key).get(key);
        let elapsed = start.elapsed().as_nanos() as u64;
        self.histogram.record(elapsed);
        res
    }

    pub fn remove(&self, key: &K) -> Option<String> {
        let start = std::time::Instant::now();
        let res = self.shard_for(key).remove(key);
        let elapsed = start.elapsed().as_nanos() as u64;
        self.histogram.record(elapsed);
        res
    }

    pub fn ttl_remaining(&self, key: &K) -> Option<Duration> {
        self.shard_for(key).ttl_remaining(key)
    }

    pub fn expire(&self, key: &K, ttl: Duration) -> bool {
        self.shard_for(key).expire(key, ttl)
    }

    pub fn incr(&self, key: &K, delta: i64) -> Result<i64, &'static str> {
        let start = std::time::Instant::now();
        let res = self.shard_for(key).incr(key, delta);
        let elapsed = start.elapsed().as_nanos() as u64;
        self.histogram.record(elapsed);
        res
    }

    pub fn evict_expired(&self) -> usize {
        self.shards.iter().map(|s| s.evict_expired()).sum()
    }

    pub fn clear_all(&self) -> usize {
        self.shards.iter().map(|s| s.clear_all()).sum()
    }

    pub fn stats(&self) -> (StatsSnapshot, (Option<u64>, Option<u64>, Option<u64>, Option<u64>)) {
        let mut total_requests = 0;
        let mut successful = 0;
        let mut failed = 0;
        let mut total_latency_ns = 0;
        let mut min_latency_ns = u64::MAX;
        let mut max_latency_ns = 0;

        for shard in &self.shards {
            let (snap, _) = shard.stats();
            total_requests += snap.total_requests;
            successful += snap.successful;
            failed += snap.failed;
            total_latency_ns += snap.total_latency_ns;
            min_latency_ns = min_latency_ns.min(snap.min_latency_ns);
            max_latency_ns = max_latency_ns.max(snap.max_latency_ns);
        }

        if min_latency_ns == u64::MAX {
            min_latency_ns = 0;
        }

        (
            StatsSnapshot {
                total_requests,
                successful,
                failed,
                total_latency_ns,
                min_latency_ns,
                max_latency_ns,
            },
            (
                self.histogram.p50(),
                self.histogram.p95(),
                self.histogram.p99(),
                self.histogram.p999(),
            ),
        )
    }

    pub fn hist_counts(&self) -> (u64, u64) {
        (self.histogram.total_count(), self.histogram.overflow_count())
    }

    pub fn flush(&mut self, capacity_per_shard: usize) {
        self.shards = (0..self.shards.len())
            .map(|_| CapsuleCache::with_capacity(capacity_per_shard))
            .collect();
        self.histogram = HistogramCapsule::new();
    }

    /// Scan key hashes across shards (limit distributed evenly).
    pub fn scan_hashes(&self, limit: usize) -> Vec<u64> {
        if self.shards.is_empty() || limit == 0 {
            return Vec::new();
        }
        let per = (limit / self.shards.len()).max(1);
        let mut out = Vec::with_capacity(limit.min(self.shards.len() * per));
        for shard in &self.shards {
            if out.len() >= limit {
                break;
            }
            let remaining = limit - out.len();
            let take = per.min(remaining);
            out.extend(shard.scan_hashes(take));
        }
        out.truncate(limit);
        out
    }
}
