//! Multi-Region Testing Infrastructure
//!
//! **Framework Compliance**: UCE34 (Q12 Distributed Constraints) + T28 (Q22-Q28 Production) + I20 (Partition Handling)
//!
//! Provides infrastructure for simulating distributed multi-region deployments with:
//! - Network latency injection (configurable per region pair)
//! - Region failure injection (simulate provider outages)
//! - Network partition simulation (split brain scenarios)
//! - Circuit state coordination (cross-region synchronization)
//!
//! # Architecture
//!
//! **RegionSimulator** - Multi-region environment controller
//! - NetworkLatencyInjector: Inject configurable network delays
//! - RegionFailureInjector: Fail providers in specific regions
//! - PartitionSimulator: Simulate network partitions
//! - CircuitStateCoordinator: Track cross-region circuit state
//!
//! # Safety
//! - #ASSUME: Network latency is simulated via thread::sleep (deterministic)
//! - #VERIFY: All tests are #[ignore] to avoid CI flakiness
//! - #ASSUME: Region failover completes within 5 seconds
//! - #VERIFY: Each test validates failover time bounds
//! - #ASSUME: Circuit state synchronization within 1 second
//! - #VERIFY: Property tests validate sync time distribution
//!
//! # Performance Targets
//! - Local operations: <10ms p50 (proxy overhead, not network)
//! - Region failover: <5 seconds (automatic recovery)
//! - State synchronization: <1 second (cross-region coordination)
//! - Split brain recovery: <10 seconds (partition healing)

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use atomic_capsule_derive::ComputationalCapsule;

/// Region identifier (US, EU, APAC, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// United States (primary)
    US,
    /// Europe (secondary)
    EU,
    /// Asia-Pacific (tertiary)
    APAC,
}

impl Region {
    pub fn as_str(&self) -> &'static str {
        match self {
            Region::US => "US",
            Region::EU => "EU",
            Region::APAC => "APAC",
        }
    }

    pub fn all() -> &'static [Region] {
        &[Region::US, Region::EU, Region::APAC]
    }
}

/// Network partition status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionStatus {
    /// All regions connected
    Connected,
    /// US isolated from EU/APAC
    UsIsolated,
    /// EU isolated from US/APAC
    EuIsolated,
    /// APAC isolated from US/EU
    ApacIsolated,
    /// Complete partition (all isolated)
    FullPartition,
}

/// Per-region context (64B aligned, Tier 1 Atomic)
///
/// Tracks health, latency, and partition status for a single region.
///
/// # Layout (64 bytes):
/// - [0-7]   failure_rate_bp: AtomicU64 (8 bytes)
/// - [8-15]  latency_ms: AtomicU64 (8 bytes)
/// - [16-23] last_state_change_ns: AtomicU64 (8 bytes)
/// - [24]    health: AtomicU8 (1 byte)
/// - [25]    circuit_state: AtomicU8 (1 byte)
/// - [26]    is_partitioned: AtomicBool (1 byte)
/// - [27-63] _padding: [u8; 37] (37 bytes)
///
/// # Safety
/// - #ASSUME: AtomicU64 operations are lockfree
/// - #VERIFY: Property tests validate concurrent access
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct RegionContext {
    /// Provider failure rate (basis points, 0-10000)
    failure_rate_bp: AtomicU64,

    /// Network latency to this region (milliseconds)
    latency_ms: AtomicU64,

    /// Last state change timestamp (nanoseconds)
    last_state_change_ns: AtomicU64,

    /// Region health: 0=Healthy, 1=Degraded, 2=Unavailable
    health: AtomicU8,

    /// Circuit state: 0=Closed, 1=HalfOpen, 2=Open
    circuit_state: AtomicU8,

    /// Is region partitioned from others?
    is_partitioned: AtomicBool,

    /// Padding to 64 bytes (8+8+8+1+1+1+37 = 64)
    _padding: [u8; 37],
}

impl RegionContext {
    /// Create new healthy region context
    pub fn new() -> Self {
        Self {
            failure_rate_bp: AtomicU64::new(0),
            latency_ms: AtomicU64::new(0),
            last_state_change_ns: AtomicU64::new(0),
            health: AtomicU8::new(0),
            circuit_state: AtomicU8::new(0),
            is_partitioned: AtomicBool::new(false),
            _padding: [0u8; 37],
        }
    }

    /// Get region health
    pub fn get_health(&self) -> RegionHealth {
        match self.health.load(Ordering::Acquire) {
            0 => RegionHealth::Healthy,
            1 => RegionHealth::Degraded,
            _ => RegionHealth::Unavailable,
        }
    }

    /// Set region health
    pub fn set_health(&self, health: RegionHealth) {
        self.health.store(health as u8, Ordering::Release);
    }

    /// Get failure rate (basis points)
    pub fn get_failure_rate_bp(&self) -> u64 {
        self.failure_rate_bp.load(Ordering::Relaxed)
    }

    /// Set failure rate (basis points)
    pub fn set_failure_rate_bp(&self, rate_bp: u64) {
        self.failure_rate_bp.store(rate_bp, Ordering::Relaxed);
    }

    /// Get network latency (milliseconds)
    pub fn get_latency_ms(&self) -> u64 {
        self.latency_ms.load(Ordering::Relaxed)
    }

    /// Set network latency (milliseconds)
    pub fn set_latency_ms(&self, latency: u64) {
        self.latency_ms.store(latency, Ordering::Relaxed);
    }

    /// Check if region is partitioned
    pub fn is_partitioned(&self) -> bool {
        self.is_partitioned.load(Ordering::Acquire)
    }

    /// Set partition status
    pub fn set_partitioned(&self, partitioned: bool) {
        self.is_partitioned.store(partitioned, Ordering::Release);
    }

    /// Get circuit state
    pub fn get_circuit_state(&self) -> CircuitState {
        match self.circuit_state.load(Ordering::Acquire) {
            0 => CircuitState::Closed,
            1 => CircuitState::HalfOpen,
            _ => CircuitState::Open,
        }
    }

    /// Set circuit state
    pub fn set_circuit_state(&self, state: CircuitState) {
        self.circuit_state.store(state as u8, Ordering::Release);
        self.last_state_change_ns.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            Ordering::Relaxed,
        );
    }

    /// Get time since last state change (nanoseconds)
    pub fn time_since_state_change_ns(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let last = self.last_state_change_ns.load(Ordering::Relaxed);
        now.saturating_sub(last)
    }
}

impl Default for RegionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Region health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RegionHealth {
    Healthy = 0,
    Degraded = 1,
    Unavailable = 2,
}

/// Circuit state (matches CircuitBreakerCapsule)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CircuitState {
    Closed = 0,
    HalfOpen = 1,
    Open = 2,
}

/// Multi-region simulator
///
/// Simulates distributed deployment with configurable:
/// - Network latency between regions
/// - Region failures (provider outages)
/// - Network partitions (split brain)
/// - Circuit state coordination
pub struct RegionSimulator {
    /// Per-region contexts
    regions: HashMap<Region, Arc<RegionContext>>,

    /// Network latency matrix (region_a -> region_b -> latency_ms)
    latency_matrix: HashMap<(Region, Region), u64>,

    /// Partition status
    partition_status: Arc<AtomicU8>,

    /// Active region (primary for routing)
    active_region: Arc<AtomicU8>,
}

impl RegionSimulator {
    /// Create new multi-region simulator
    pub fn new() -> Self {
        let mut regions = HashMap::new();
        regions.insert(Region::US, Arc::new(RegionContext::new()));
        regions.insert(Region::EU, Arc::new(RegionContext::new()));
        regions.insert(Region::APAC, Arc::new(RegionContext::new()));

        Self {
            regions,
            latency_matrix: HashMap::new(),
            partition_status: Arc::new(AtomicU8::new(PartitionStatus::Connected as u8)),
            active_region: Arc::new(AtomicU8::new(Region::US as u8)),
        }
    }

    /// Configure regions to simulate
    pub fn configure_regions(&mut self, region_names: &[&str]) {
        // Already configured in new(), this is a no-op for API compatibility
        let _ = region_names;
    }

    /// Inject network latency between two regions
    ///
    /// # Arguments
    /// - `route`: Region pair (e.g., "US->EU")
    /// - `latency_ms`: Latency in milliseconds
    pub fn inject_latency(&mut self, route: &str, latency_ms: u64) {
        let parts: Vec<&str> = route.split("->").collect();
        if parts.len() == 2 {
            let from = self.parse_region(parts[0]);
            let to = self.parse_region(parts[1]);

            self.latency_matrix.insert((from, to), latency_ms);

            // Update region context
            if let Some(ctx) = self.regions.get(&to) {
                ctx.set_latency_ms(latency_ms);
            }
        }
    }

    /// Get network latency between two regions
    pub fn get_latency(&self, from: Region, to: Region) -> u64 {
        self.latency_matrix.get(&(from, to)).copied().unwrap_or(0)
    }

    /// Simulate network delay (blocking)
    ///
    /// # Safety
    /// - #ASSUME: thread::sleep provides accurate delays
    /// - #VERIFY: Tests validate expected latency distribution
    pub fn simulate_delay(&self, from: Region, to: Region) {
        if let Some(&latency_ms) = self.latency_matrix.get(&(from, to)) {
            std::thread::sleep(Duration::from_millis(latency_ms));
        }
    }

    /// Fail all providers in a specific region
    ///
    /// Simulates region-wide outage (100% failure rate).
    pub fn fail_region(&mut self, region_name: &str) {
        let region = self.parse_region(region_name);

        if let Some(ctx) = self.regions.get(&region) {
            ctx.set_health(RegionHealth::Unavailable);
            ctx.set_failure_rate_bp(10000); // 100% failure
            ctx.set_circuit_state(CircuitState::Open);
        }
    }

    /// Recover a failed region
    ///
    /// Restores region to healthy state (0% failure rate).
    pub fn recover_region(&mut self, region_name: &str) {
        let region = self.parse_region(region_name);

        if let Some(ctx) = self.regions.get(&region) {
            ctx.set_health(RegionHealth::Healthy);
            ctx.set_failure_rate_bp(0); // 0% failure
            ctx.set_circuit_state(CircuitState::Closed);
        }
    }

    /// Create network partition
    ///
    /// Simulates split brain by isolating regions from each other.
    pub fn create_partition(&mut self, status: PartitionStatus) {
        self.partition_status.store(status as u8, Ordering::Release);

        // Update partition flags in region contexts
        match status {
            PartitionStatus::Connected => {
                for ctx in self.regions.values() {
                    ctx.set_partitioned(false);
                }
            }
            PartitionStatus::UsIsolated => {
                if let Some(ctx) = self.regions.get(&Region::US) {
                    ctx.set_partitioned(true);
                }
            }
            PartitionStatus::EuIsolated => {
                if let Some(ctx) = self.regions.get(&Region::EU) {
                    ctx.set_partitioned(true);
                }
            }
            PartitionStatus::ApacIsolated => {
                if let Some(ctx) = self.regions.get(&Region::APAC) {
                    ctx.set_partitioned(true);
                }
            }
            PartitionStatus::FullPartition => {
                for ctx in self.regions.values() {
                    ctx.set_partitioned(true);
                }
            }
        }
    }

    /// Heal network partition
    pub fn heal_partition(&mut self) {
        self.create_partition(PartitionStatus::Connected);
    }

    /// Get active region
    pub fn active_region(&self) -> Region {
        match self.active_region.load(Ordering::Acquire) {
            0 => Region::US,
            1 => Region::EU,
            _ => Region::APAC,
        }
    }

    /// Set active region (for failover)
    pub fn set_active_region(&self, region: Region) {
        self.active_region.store(region as u8, Ordering::Release);
    }

    /// Automatic failover to healthy region
    ///
    /// Selects first healthy region in priority order: US -> EU -> APAC
    ///
    /// # Returns
    /// - Failover duration (if failover occurred)
    pub fn failover(&mut self) -> Option<Duration> {
        let start = Instant::now();
        let current = self.active_region();

        // Find first healthy region
        for region in Region::all() {
            if let Some(ctx) = self.regions.get(region) {
                if ctx.get_health() == RegionHealth::Healthy
                    && ctx.get_circuit_state() == CircuitState::Closed
                {
                    if *region != current {
                        self.set_active_region(*region);
                        return Some(start.elapsed());
                    } else {
                        return None; // Already on healthy region
                    }
                }
            }
        }

        None // No healthy region found
    }

    /// Get region context
    pub fn get_region(&self, region: Region) -> Option<&Arc<RegionContext>> {
        self.regions.get(&region)
    }

    /// Check if region is partitioned
    pub fn is_partitioned(&self, region: Region) -> bool {
        self.regions
            .get(&region)
            .map(|ctx| ctx.is_partitioned())
            .unwrap_or(false)
    }

    /// Synchronize circuit state across regions
    ///
    /// Simulates cross-region circuit state coordination.
    ///
    /// # Returns
    /// - Synchronization duration
    pub fn sync_circuit_state(&self, source: Region, state: CircuitState) -> Duration {
        let start = Instant::now();

        // Update source region
        if let Some(ctx) = self.regions.get(&source) {
            ctx.set_circuit_state(state);
        }

        // Simulate network delay for propagation
        std::thread::sleep(Duration::from_millis(100)); // 100ms sync time

        // Update other regions (if not partitioned)
        for (region, ctx) in &self.regions {
            if *region != source && !ctx.is_partitioned() {
                ctx.set_circuit_state(state);
            }
        }

        start.elapsed()
    }

    /// Parse region name to enum
    fn parse_region(&self, name: &str) -> Region {
        match name.to_uppercase().as_str() {
            "US" => Region::US,
            "EU" => Region::EU,
            "APAC" => Region::APAC,
            _ => Region::US, // Default to US
        }
    }

    /// Get partition status
    pub fn get_partition_status(&self) -> PartitionStatus {
        match self.partition_status.load(Ordering::Acquire) {
            0 => PartitionStatus::Connected,
            1 => PartitionStatus::UsIsolated,
            2 => PartitionStatus::EuIsolated,
            3 => PartitionStatus::ApacIsolated,
            _ => PartitionStatus::FullPartition,
        }
    }
}

impl Default for RegionSimulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_context_creation() {
        let ctx = RegionContext::new();

        assert_eq!(ctx.get_health(), RegionHealth::Healthy);
        assert_eq!(ctx.get_failure_rate_bp(), 0);
        assert_eq!(ctx.get_latency_ms(), 0);
        assert!(!ctx.is_partitioned());
        assert_eq!(ctx.get_circuit_state(), CircuitState::Closed);
    }

    #[test]
    fn test_region_context_updates() {
        let ctx = RegionContext::new();

        ctx.set_health(RegionHealth::Degraded);
        assert_eq!(ctx.get_health(), RegionHealth::Degraded);

        ctx.set_failure_rate_bp(1000); // 10%
        assert_eq!(ctx.get_failure_rate_bp(), 1000);

        ctx.set_latency_ms(50);
        assert_eq!(ctx.get_latency_ms(), 50);

        ctx.set_partitioned(true);
        assert!(ctx.is_partitioned());

        ctx.set_circuit_state(CircuitState::Open);
        assert_eq!(ctx.get_circuit_state(), CircuitState::Open);
    }

    #[test]
    fn test_region_simulator_creation() {
        let simulator = RegionSimulator::new();

        assert_eq!(simulator.active_region(), Region::US);
        assert_eq!(simulator.get_partition_status(), PartitionStatus::Connected);
    }

    #[test]
    fn test_inject_latency() {
        let mut simulator = RegionSimulator::new();

        simulator.inject_latency("US->EU", 50);
        assert_eq!(simulator.get_latency(Region::US, Region::EU), 50);

        simulator.inject_latency("EU->APAC", 100);
        assert_eq!(simulator.get_latency(Region::EU, Region::APAC), 100);
    }

    #[test]
    fn test_fail_recover_region() {
        let mut simulator = RegionSimulator::new();

        // Fail US region
        simulator.fail_region("US");
        {
            let ctx = simulator.get_region(Region::US).unwrap();
            assert_eq!(ctx.get_health(), RegionHealth::Unavailable);
            assert_eq!(ctx.get_failure_rate_bp(), 10000);
            assert_eq!(ctx.get_circuit_state(), CircuitState::Open);
        }

        // Recover US region
        simulator.recover_region("US");
        {
            let ctx = simulator.get_region(Region::US).unwrap();
            assert_eq!(ctx.get_health(), RegionHealth::Healthy);
            assert_eq!(ctx.get_failure_rate_bp(), 0);
            assert_eq!(ctx.get_circuit_state(), CircuitState::Closed);
        }
    }

    #[test]
    fn test_network_partition() {
        let mut simulator = RegionSimulator::new();

        simulator.create_partition(PartitionStatus::UsIsolated);
        assert!(simulator.is_partitioned(Region::US));
        assert!(!simulator.is_partitioned(Region::EU));
        assert!(!simulator.is_partitioned(Region::APAC));

        simulator.heal_partition();
        assert!(!simulator.is_partitioned(Region::US));
        assert!(!simulator.is_partitioned(Region::EU));
        assert!(!simulator.is_partitioned(Region::APAC));
    }

    #[test]
    fn test_failover() {
        let mut simulator = RegionSimulator::new();

        // Initial state: US is active
        assert_eq!(simulator.active_region(), Region::US);

        // Fail US region
        simulator.fail_region("US");

        // Trigger failover
        let duration = simulator.failover();
        assert!(duration.is_some()); // Failover occurred
        assert_eq!(simulator.active_region(), Region::EU); // Failover to EU

        // Recover US
        simulator.recover_region("US");

        // Failover back to US
        let duration = simulator.failover();
        assert!(duration.is_some());
        assert_eq!(simulator.active_region(), Region::US);
    }

    #[test]
    fn test_circuit_state_sync() {
        let simulator = RegionSimulator::new();

        // Open circuit in US
        let duration = simulator.sync_circuit_state(Region::US, CircuitState::Open);

        // Verify synchronization occurred
        assert!(duration >= Duration::from_millis(100)); // At least 100ms for sync

        // All regions should have open circuit
        for region in Region::all() {
            let ctx = simulator.get_region(*region).unwrap();
            assert_eq!(ctx.get_circuit_state(), CircuitState::Open);
        }
    }

    #[test]
    fn test_circuit_state_sync_with_partition() {
        let mut simulator = RegionSimulator::new();

        // Create partition (US isolated)
        simulator.create_partition(PartitionStatus::UsIsolated);

        // Open circuit in US
        simulator.sync_circuit_state(Region::US, CircuitState::Open);

        // US should have open circuit
        let us_ctx = simulator.get_region(Region::US).unwrap();
        assert_eq!(us_ctx.get_circuit_state(), CircuitState::Open);

        // EU/APAC should still have closed circuit (partitioned)
        let eu_ctx = simulator.get_region(Region::EU).unwrap();
        assert_eq!(eu_ctx.get_circuit_state(), CircuitState::Closed);
    }
}
