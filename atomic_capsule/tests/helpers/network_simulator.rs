// Network Simulator for Chaos Testing
// Injects latency, packet loss, corruption, partitions

use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::collections::HashMap;

/// Network simulator for chaos engineering
pub struct NetworkSimulator {
    /// Latency to inject (in ms)
    latency_ms: Arc<Mutex<u64>>,

    /// Packet loss percentage (0-100)
    packet_loss_pct: Arc<Mutex<u8>>,

    /// Corruption percentage (0-100)
    corruption_pct: Arc<Mutex<u8>>,

    /// Network partitions (set of blocked connections)
    partitions: Arc<Mutex<HashMap<(u16, u16), bool>>>,
}

impl NetworkSimulator {
    /// Create new network simulator (no faults by default)
    pub fn new() -> Self {
        Self {
            latency_ms: Arc::new(Mutex::new(0)),
            packet_loss_pct: Arc::new(Mutex::new(0)),
            corruption_pct: Arc::new(Mutex::new(0)),
            partitions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Inject latency (all RPC calls delayed by this amount)
    ///
    /// # T28 Chaos Test Support
    /// - Add artificial latency to all network calls
    /// - Simulates slow network, congestion
    pub fn inject_latency(&self, latency_ms: u64) {
        *self.latency_ms.lock().unwrap() = latency_ms;
    }

    /// Inject packet loss (drop % of packets)
    ///
    /// # T28 Chaos Test Support
    /// - Randomly drop packets
    /// - Tests retry logic, idempotency
    pub fn inject_packet_loss(&self, percent: u8) {
        assert!(percent <= 100, "Packet loss must be 0-100%");
        *self.packet_loss_pct.lock().unwrap() = percent;
    }

    /// Inject corruption (flip bits in % of packets)
    ///
    /// # T28 Chaos Test Support
    /// - Corrupt payload data
    /// - Tests error detection (checksums)
    pub fn inject_corruption(&self, percent: u8) {
        assert!(percent <= 100, "Corruption must be 0-100%");
        *self.corruption_pct.lock().unwrap() = percent;
    }

    /// Partition network (block traffic between shard sets)
    ///
    /// # T28 Chaos Test Support
    /// - Simulates network split-brain
    /// - Tests partition tolerance (CAP theorem)
    pub fn partition(&self, set1: &[u16], set2: &[u16]) {
        let mut partitions = self.partitions.lock().unwrap();

        // Block all traffic between set1 ↔ set2
        for &id1 in set1 {
            for &id2 in set2 {
                partitions.insert((id1, id2), true);
                partitions.insert((id2, id1), true);
            }
        }
    }

    /// Clear all network faults
    pub fn clear_faults(&self) {
        *self.latency_ms.lock().unwrap() = 0;
        *self.packet_loss_pct.lock().unwrap() = 0;
        *self.corruption_pct.lock().unwrap() = 0;
        self.partitions.lock().unwrap().clear();
    }

    /// Apply network effects to RPC (returns false if packet dropped)
    ///
    /// # Returns
    /// - true: Packet delivered (possibly delayed/corrupted)
    /// - false: Packet dropped
    pub fn apply_effects(&self, from_shard: u16, to_shard: u16) -> bool {
        // Check partition
        let partitions = self.partitions.lock().unwrap();
        if partitions.get(&(from_shard, to_shard)) == Some(&true) {
            return false; // Blocked by partition
        }
        drop(partitions);

        // Check packet loss
        let packet_loss = *self.packet_loss_pct.lock().unwrap();
        if packet_loss > 0 {
            let random = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() % 100) as u8;

            if random < packet_loss {
                return false; // Packet dropped
            }
        }

        // Apply latency
        let latency = *self.latency_ms.lock().unwrap();
        if latency > 0 {
            std::thread::sleep(Duration::from_millis(latency));
        }

        // Apply corruption (simplified: just return false to simulate checksum failure)
        let corruption = *self.corruption_pct.lock().unwrap();
        if corruption > 0 {
            let random = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() % 100) as u8;

            if random < corruption {
                return false; // Corruption detected, packet rejected
            }
        }

        true // Packet delivered successfully
    }

    /// Get current latency
    pub fn latency_ms(&self) -> u64 {
        *self.latency_ms.lock().unwrap()
    }

    /// Get current packet loss percentage
    pub fn packet_loss_pct(&self) -> u8 {
        *self.packet_loss_pct.lock().unwrap()
    }
}

impl Default for NetworkSimulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inject_latency() {
        let sim = NetworkSimulator::new();
        sim.inject_latency(100);

        let start = std::time::Instant::now();
        assert!(sim.apply_effects(0, 1));
        let elapsed = start.elapsed();

        // Should have added ~100ms latency
        assert!(elapsed.as_millis() >= 90 && elapsed.as_millis() <= 150);
    }

    #[test]
    fn test_packet_loss() {
        let sim = NetworkSimulator::new();
        sim.inject_packet_loss(100); // Drop all packets

        // Should drop packet
        assert!(!sim.apply_effects(0, 1));
    }

    #[test]
    fn test_partition() {
        let sim = NetworkSimulator::new();
        sim.partition(&[0, 1], &[2, 3]);

        // Should block traffic between sets
        assert!(!sim.apply_effects(0, 2));
        assert!(!sim.apply_effects(2, 0));

        // Should allow traffic within sets
        assert!(sim.apply_effects(0, 1));
        assert!(sim.apply_effects(2, 3));
    }

    #[test]
    fn test_clear_faults() {
        let sim = NetworkSimulator::new();
        sim.inject_latency(100);
        sim.inject_packet_loss(50);

        sim.clear_faults();

        assert_eq!(sim.latency_ms(), 0);
        assert_eq!(sim.packet_loss_pct(), 0);
    }
}
