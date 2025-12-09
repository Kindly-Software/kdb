//! Minimal distributed cache profile (quorum replication) without external deps.
//! Uses multiple sharded nodes and consistent hashing to choose primary/replicas.

use crate::sharded::ShardedCache;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use atomic_capsule::collections::StatsSnapshot;

struct Node {
    id: u64,
    cache: ShardedCache<String>,
}

impl Node {
    fn new(id: u64, shards: usize, capacity_per_shard: usize) -> Self {
        Self {
            id,
            cache: ShardedCache::new(shards, capacity_per_shard),
        }
    }
}

/// Simple quorum-based distributed cache (in-process, no network).
pub struct DistributedCache {
    nodes: Vec<Node>,
    replication_factor: usize,
    quorum: usize,
}

impl DistributedCache {
    /// Create a distributed cache with `node_count` nodes.
    /// Each node is a sharded cache (per-node shards = `shards_per_node`).
    pub fn new(node_count: usize, shards_per_node: usize, capacity_per_shard: usize, replication_factor: usize) -> Self {
        let count = node_count.max(1);
        let rep = replication_factor.clamp(1, count);
        let quorum = rep / 2 + 1;
        let nodes = (0..count as u64)
            .map(|id| Node::new(id, shards_per_node, capacity_per_shard))
            .collect();
        Self {
            nodes,
            replication_factor: rep,
            quorum,
        }
    }

    fn route_replicas(&self, key: &str) -> Vec<&Node> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let mut idx = (hasher.finish() as usize) % self.nodes.len();
        let mut out = Vec::with_capacity(self.replication_factor);
        for _ in 0..self.replication_factor {
            out.push(&self.nodes[idx]);
            idx = (idx + 1) % self.nodes.len();
        }
        out
    }

    /// Insert across replicas; succeeds if quorum succeeds.
    pub fn insert(&self, key: String, value: String, ttl: Duration) -> Result<(), &'static str> {
        let replicas = self.route_replicas(&key);
        let mut ok = 0;
        for node in replicas {
            if node.cache.insert(key.clone(), value.clone(), ttl).is_ok() {
                ok += 1;
            }
        }
        if ok >= self.quorum {
            Ok(())
        } else {
            Err("ERR quorum failed")
        }
    }

    /// Get from primary first, then replicas.
    pub fn get(&self, key: &str) -> Option<String> {
        let replicas = self.route_replicas(key);
        for node in replicas {
            if let Some(v) = node.cache.get(&key.to_string()) {
                return Some(v);
            }
        }
        None
    }

    /// Remove across replicas; returns number removed.
    pub fn remove(&self, key: &str) -> usize {
        let replicas = self.route_replicas(key);
        let mut removed = 0;
        for node in replicas {
            if node.cache.remove(&key.to_string()).is_some() {
                removed += 1;
            }
        }
        removed
    }

    /// TTL from the first replica that has the key.
    pub fn ttl_remaining(&self, key: &str) -> Option<Duration> {
        let replicas = self.route_replicas(key);
        for node in replicas {
            if let Some(ttl) = node.cache.ttl_remaining(&key.to_string()) {
                return Some(ttl);
            }
        }
        None
    }

    /// Update TTL on replicas; returns true if quorum success.
    pub fn expire(&self, key: &str, ttl: Duration) -> bool {
        let replicas = self.route_replicas(key);
        let mut ok = 0;
        for node in replicas {
            if node.cache.expire(&key.to_string(), ttl) {
                ok += 1;
            }
        }
        ok >= self.quorum
    }

    /// Increment across replicas (best-effort, returns first success).
    pub fn incr(&self, key: &str, delta: i64) -> Result<i64, &'static str> {
        let replicas = self.route_replicas(key);
        for node in replicas {
            if let Ok(v) = node.cache.incr(&key.to_string(), delta) {
                // propagate updated value to other replicas
                let ttl = node.cache.ttl_remaining(&key.to_string()).unwrap_or(Duration::from_secs(365 * 24 * 3600));
                for other in self.route_replicas(key) {
                    let _ = other.cache.insert(key.to_string(), v.to_string(), ttl);
                }
                return Ok(v);
            }
        }
        Err("ERR incr quorum failed")
    }

    /// Aggregate stats and percentiles.
    pub fn stats(&self) -> (StatsSnapshot, (Option<u64>, Option<u64>, Option<u64>, Option<u64>)) {
        let mut total_requests = 0;
        let mut successful = 0;
        let mut failed = 0;
        let mut total_latency_ns = 0;
        let mut min_latency_ns = u64::MAX;
        let mut max_latency_ns = 0;
        let mut p50 = None;
        let mut p95 = None;
        let mut p99 = None;
        let mut p999 = None;

        for node in &self.nodes {
            let (snap, pct) = node.cache.stats();
            total_requests += snap.total_requests;
            successful += snap.successful;
            failed += snap.failed;
            total_latency_ns += snap.total_latency_ns;
            min_latency_ns = min_latency_ns.min(snap.min_latency_ns);
            max_latency_ns = max_latency_ns.max(snap.max_latency_ns);
            p50 = p50.or(pct.0);
            p95 = p95.or(pct.1);
            p99 = p99.or(pct.2);
            p999 = p999.or(pct.3);
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
            (p50, p95, p99, p999),
        )
    }
}
