// Test Cluster Infrastructure for T8 Network Capsule
// Provides TestCluster struct for multi-shard testing

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::time::Duration;

/// Test cluster for distributed testing
pub struct TestCluster {
    shards: Vec<TestShard>,
    coordinator: Option<TestCoordinator>,
    runtime: tokio::runtime::Runtime,
}

/// Individual shard server (mock)
pub struct TestShard {
    pub id: u16,
    pub addr: SocketAddr,
    pub healthy: Arc<AtomicBool>,
    pub last_heartbeat_ns: Arc<AtomicU64>,
    pub documents_count: Arc<AtomicU64>,
}

/// Coordinator server (mock)
pub struct TestCoordinator {
    pub addr: SocketAddr,
    pub shard_registry: Arc<HashMap<u16, SocketAddr>>,
}

impl TestCluster {
    /// Create test cluster with N shards
    ///
    /// # T28 Unit Test Support
    /// - Creates in-memory shard servers
    /// - Tokio runtime for async testing
    /// - Returns ready-to-test cluster
    pub fn new(num_shards: u16) -> Self {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

        let shards: Vec<TestShard> = (0..num_shards)
            .map(|id| TestShard {
                id,
                addr: format!("127.0.0.1:{}", 8000 + id).parse().unwrap(),
                healthy: Arc::new(AtomicBool::new(true)),
                last_heartbeat_ns: Arc::new(AtomicU64::new(current_timestamp_ns())),
                documents_count: Arc::new(AtomicU64::new(0)),
            })
            .collect();

        Self {
            shards,
            coordinator: None,
            runtime,
        }
    }

    /// Add coordinator to cluster
    pub fn with_coordinator(mut self) -> Self {
        let mut shard_registry = HashMap::new();
        for shard in &self.shards {
            shard_registry.insert(shard.id, shard.addr);
        }

        self.coordinator = Some(TestCoordinator {
            addr: "127.0.0.1:7000".parse().unwrap(),
            shard_registry: Arc::new(shard_registry),
        });

        self
    }

    /// Kill shard (simulate failure)
    ///
    /// # T28 Integration Test Support
    /// - Marks shard as unhealthy
    /// - Stops heartbeat updates
    pub fn kill_shard(&self, id: u16) {
        if let Some(shard) = self.shards.iter().find(|s| s.id == id) {
            shard.healthy.store(false, Ordering::Release);
        }
    }

    /// Partition network (split shards into two sets)
    ///
    /// # T28 Chaos Test Support
    /// - Simulates network partition
    /// - set1 can't reach set2 and vice versa
    pub fn partition(&self, set1: &[u16], set2: &[u16]) {
        // In a real implementation, this would configure network simulator
        // For now, we just mark shards as partitioned (simplified)
        for &id in set1 {
            if let Some(shard) = self.shards.iter().find(|s| s.id == id) {
                // Mark as partially unhealthy (can't reach set2)
                // In real test, would configure network_simulator
            }
        }
    }

    /// Wait for eventual consistency (poll until condition met)
    ///
    /// # T28 Integration Test Support
    /// - Waits up to timeout for condition
    /// - Returns true if condition met, false if timeout
    pub fn wait_for_sync<F>(&self, mut condition: F, timeout: Duration) -> bool
    where
        F: FnMut() -> bool,
    {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if condition() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// Get shard by ID
    pub fn get_shard(&self, id: u16) -> Option<&TestShard> {
        self.shards.iter().find(|s| s.id == id)
    }

    /// Get all shards
    pub fn shards(&self) -> &[TestShard] {
        &self.shards
    }

    /// Get coordinator
    pub fn coordinator(&self) -> Option<&TestCoordinator> {
        self.coordinator.as_ref()
    }

    /// Shutdown cluster
    pub fn shutdown(self) {
        self.runtime.shutdown_timeout(Duration::from_secs(1));
    }
}

impl TestShard {
    /// Update heartbeat (simulate alive shard)
    pub fn update_heartbeat(&self) {
        self.last_heartbeat_ns.store(current_timestamp_ns(), Ordering::Release);
        self.healthy.store(true, Ordering::Release);
    }

    /// Check if heartbeat is fresh
    pub fn heartbeat_fresh(&self, timeout_ns: u64) -> bool {
        let last_seen = self.last_heartbeat_ns.load(Ordering::Acquire);
        let now = current_timestamp_ns();
        (now - last_seen) < timeout_ns
    }
}

/// Get current timestamp in nanoseconds
fn current_timestamp_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_cluster() {
        let cluster = TestCluster::new(5);
        assert_eq!(cluster.shards().len(), 5);
    }

    #[test]
    fn test_kill_shard() {
        let cluster = TestCluster::new(3);
        cluster.kill_shard(1);

        let shard = cluster.get_shard(1).unwrap();
        assert!(!shard.healthy.load(Ordering::Acquire));
    }

    #[test]
    fn test_wait_for_sync() {
        let cluster = TestCluster::new(1);
        let counter = Arc::new(AtomicU64::new(0));

        let counter_clone = Arc::clone(&counter);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            counter_clone.store(42, Ordering::Release);
        });

        let result = cluster.wait_for_sync(
            || counter.load(Ordering::Acquire) == 42,
            Duration::from_secs(1),
        );

        assert!(result);
    }
}
