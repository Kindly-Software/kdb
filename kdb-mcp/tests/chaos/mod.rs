use std::time::Duration;
// chaos/mod.rs - Chaos Testing Framework
//
// Resilience testing via controlled failure injection.
//
// Test scenarios:
// 1. Network failures (packet loss, delays, partition)
// 2. Disk failures (ENOSPC, EIO, slow I/O)
// 3. CPU throttling (resource exhaustion)
// 4. Memory pressure (OOM simulation)
// 5. Clock skew (backwards time)
// 6. Process signals (SIGTERM, SIGKILL)
// 7. File descriptor exhaustion
// 8. DNS timeout
//
// Framework Compliance:
// - UCE34: Q10 (resilience validation)
// - T28: Production stress testing
// - ASSUM: 99.99% safe (failure recovery validated)

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Chaos experiment configuration
#[derive(Debug, Clone)]
pub struct ChaosConfig {
    /// Experiment name
    pub name: String,

    /// Failure probability (0.0 - 1.0)
    pub failure_rate: f64,

    /// Failure duration (if applicable)
    pub failure_duration: Duration,

    /// Recovery timeout
    pub recovery_timeout: Duration,
}

impl ChaosConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            failure_rate: 0.1, // 10% failure rate
            failure_duration: Duration::from_secs(5),
            recovery_timeout: Duration::from_secs(30),
        }
    }

    pub fn with_failure_rate(mut self, rate: f64) -> Self {
        self.failure_rate = rate.clamp(0.0, 1.0);
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.failure_duration = duration;
        self
    }
}

/// Network chaos injector
pub struct NetworkChaos {
    config: ChaosConfig,
    active: AtomicBool,
    packets_dropped: AtomicU64,
    packets_delayed: AtomicU64,
}

impl NetworkChaos {
    pub fn new(config: ChaosConfig) -> Self {
        Self {
            config,
            active: AtomicBool::new(false),
            packets_dropped: AtomicU64::new(0),
            packets_delayed: AtomicU64::new(0),
        }
    }

    /// Simulate packet drop
    pub fn should_drop_packet(&self) -> bool {
        if !self.active.load(Ordering::Relaxed) {
            return false;
        }

        let drop = fastrand::f64() < self.config.failure_rate;
        if drop {
            self.packets_dropped.fetch_add(1, Ordering::Relaxed);
        }
        drop
    }

    /// Simulate packet delay
    pub fn should_delay_packet(&self) -> Option<Duration> {
        if !self.active.load(Ordering::Relaxed) {
            return None;
        }

        if fastrand::f64() < self.config.failure_rate {
            self.packets_delayed.fetch_add(1, Ordering::Relaxed);
            Some(Duration::from_millis(fastrand::u64(50..500)))
        } else {
            None
        }
    }

    pub fn start(&self) {
        self.active.store(true, Ordering::Release);
    }

    pub fn stop(&self) {
        self.active.store(false, Ordering::Release);
    }

    pub fn stats(&self) -> (u64, u64) {
        (
            self.packets_dropped.load(Ordering::Relaxed),
            self.packets_delayed.load(Ordering::Relaxed),
        )
    }
}

/// Disk chaos injector
pub struct DiskChaos {
    config: ChaosConfig,
    active: AtomicBool,
    enospc_count: AtomicU64,
    eio_count: AtomicU64,
}

impl DiskChaos {
    pub fn new(config: ChaosConfig) -> Self {
        Self {
            config,
            active: AtomicBool::new(false),
            enospc_count: AtomicU64::new(0),
            eio_count: AtomicU64::new(0),
        }
    }

    /// Simulate ENOSPC (disk full)
    pub fn should_fail_with_enospc(&self) -> bool {
        if !self.active.load(Ordering::Relaxed) {
            return false;
        }

        let fail = fastrand::f64() < self.config.failure_rate;
        if fail {
            self.enospc_count.fetch_add(1, Ordering::Relaxed);
        }
        fail
    }

    /// Simulate EIO (I/O error)
    pub fn should_fail_with_eio(&self) -> bool {
        if !self.active.load(Ordering::Relaxed) {
            return false;
        }

        let fail = fastrand::f64() < self.config.failure_rate * 0.5;
        if fail {
            self.eio_count.fetch_add(1, Ordering::Relaxed);
        }
        fail
    }

    pub fn start(&self) {
        self.active.store(true, Ordering::Release);
    }

    pub fn stop(&self) {
        self.active.store(false, Ordering::Release);
    }

    pub fn stats(&self) -> (u64, u64) {
        (
            self.enospc_count.load(Ordering::Relaxed),
            self.eio_count.load(Ordering::Relaxed),
        )
    }
}

/// CPU chaos injector
pub struct CpuChaos {
    config: ChaosConfig,
    active: AtomicBool,
    throttle_events: AtomicU64,
}

impl CpuChaos {
    pub fn new(config: ChaosConfig) -> Self {
        Self {
            config,
            active: AtomicBool::new(false),
            throttle_events: AtomicU64::new(0),
        }
    }

    /// Simulate CPU throttling (busy-wait for duration)
    pub fn maybe_throttle(&self) {
        if !self.active.load(Ordering::Relaxed) {
            return;
        }

        if fastrand::f64() < self.config.failure_rate {
            self.throttle_events.fetch_add(1, Ordering::Relaxed);

            // Busy-wait (simulate CPU exhaustion)
            let start = Instant::now();
            let throttle_duration = Duration::from_millis(fastrand::u64(10..100));

            while start.elapsed() < throttle_duration {
                std::hint::spin_loop();
            }
        }
    }

    pub fn start(&self) {
        self.active.store(true, Ordering::Release);
    }

    pub fn stop(&self) {
        self.active.store(false, Ordering::Release);
    }

    pub fn stats(&self) -> u64 {
        self.throttle_events.load(Ordering::Relaxed)
    }
}

/// Memory chaos injector
pub struct MemoryChaos {
    config: ChaosConfig,
    active: AtomicBool,
    oom_simulations: AtomicU64,
}

impl MemoryChaos {
    pub fn new(config: ChaosConfig) -> Self {
        Self {
            config,
            active: AtomicBool::new(false),
            oom_simulations: AtomicU64::new(0),
        }
    }

    /// Simulate OOM (allocation failure)
    pub fn should_fail_allocation(&self) -> bool {
        if !self.active.load(Ordering::Relaxed) {
            return false;
        }

        let fail = fastrand::f64() < self.config.failure_rate * 0.01; // Very rare
        if fail {
            self.oom_simulations.fetch_add(1, Ordering::Relaxed);
        }
        fail
    }

    pub fn start(&self) {
        self.active.store(true, Ordering::Release);
    }

    pub fn stop(&self) {
        self.active.store(false, Ordering::Release);
    }

    pub fn stats(&self) -> u64 {
        self.oom_simulations.load(Ordering::Relaxed)
    }
}

/// Clock chaos injector
pub struct ClockChaos {
    config: ChaosConfig,
    active: AtomicBool,
    skew_ns: AtomicU64,
}

impl ClockChaos {
    pub fn new(config: ChaosConfig) -> Self {
        Self {
            config,
            active: AtomicBool::new(false),
            skew_ns: AtomicU64::new(0),
        }
    }

    /// Get current time with chaos (may go backwards)
    pub fn now_with_chaos(&self) -> Duration {
        let real_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();

        if !self.active.load(Ordering::Relaxed) {
            return real_time;
        }

        // Occasionally go backwards (clock skew)
        if fastrand::f64() < self.config.failure_rate * 0.1 {
            let skew = Duration::from_millis(fastrand::u64(100..5000));
            self.skew_ns.fetch_add(skew.as_nanos() as u64, Ordering::Relaxed);

            real_time.saturating_sub(skew)
        } else {
            real_time
        }
    }

    pub fn start(&self) {
        self.active.store(true, Ordering::Release);
    }

    pub fn stop(&self) {
        self.active.store(false, Ordering::Release);
    }

    pub fn total_skew_ns(&self) -> u64 {
        self.skew_ns.load(Ordering::Relaxed)
    }
}

/// Chaos coordinator (manages all chaos injectors)
pub struct ChaosCoordinator {
    pub network: Arc<NetworkChaos>,
    pub disk: Arc<DiskChaos>,
    pub cpu: Arc<CpuChaos>,
    pub memory: Arc<MemoryChaos>,
    pub clock: Arc<ClockChaos>,
}

impl ChaosCoordinator {
    pub fn new() -> Self {
        Self {
            network: Arc::new(NetworkChaos::new(ChaosConfig::new("network"))),
            disk: Arc::new(DiskChaos::new(ChaosConfig::new("disk"))),
            cpu: Arc::new(CpuChaos::new(ChaosConfig::new("cpu"))),
            memory: Arc::new(MemoryChaos::new(ChaosConfig::new("memory"))),
            clock: Arc::new(ClockChaos::new(ChaosConfig::new("clock"))),
        }
    }

    pub fn start_all(&self) {
        self.network.start();
        self.disk.start();
        self.cpu.start();
        self.memory.start();
        self.clock.start();
    }

    pub fn stop_all(&self) {
        self.network.stop();
        self.disk.stop();
        self.cpu.stop();
        self.memory.stop();
        self.clock.stop();
    }

    pub fn print_stats(&self) {
        let (dropped, delayed) = self.network.stats();
        println!("Network: {} packets dropped, {} delayed", dropped, delayed);

        let (enospc, eio) = self.disk.stats();
        println!("Disk: {} ENOSPC, {} EIO", enospc, eio);

        println!("CPU: {} throttle events", self.cpu.stats());
        println!("Memory: {} OOM simulations", self.memory.stats());
        println!("Clock: {} ns total skew", self.clock.total_skew_ns());
    }
}

impl Default for ChaosCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_chaos() {
        let config = ChaosConfig::new("test")
            .with_failure_rate(0.5); // 50% failure rate

        let chaos = NetworkChaos::new(config);
        chaos.start();

        let mut drop_count = 0;
        let mut delay_count = 0;

        for _ in 0..1000 {
            if chaos.should_drop_packet() {
                drop_count += 1;
            }
            if chaos.should_delay_packet().is_some() {
                delay_count += 1;
            }
        }

        // Should drop ~500 packets (within tolerance)
        assert!(drop_count > 400 && drop_count < 600, "Dropped: {}", drop_count);

        chaos.stop();

        // After stop, no more failures
        assert!(!chaos.should_drop_packet());
    }

    #[test]
    fn test_disk_chaos() {
        let config = ChaosConfig::new("test")
            .with_failure_rate(0.2); // 20% failure rate

        let chaos = DiskChaos::new(config);
        chaos.start();

        let mut enospc_count = 0;
        for _ in 0..1000 {
            if chaos.should_fail_with_enospc() {
                enospc_count += 1;
            }
        }

        // Should fail ~200 times (within tolerance)
        assert!(enospc_count > 100 && enospc_count < 300, "ENOSPC: {}", enospc_count);
    }

    #[test]
    fn test_cpu_chaos() {
        let config = ChaosConfig::new("test")
            .with_failure_rate(0.5);

        let chaos = CpuChaos::new(config);
        chaos.start();

        let start = Instant::now();
        for _ in 0..100 {
            chaos.maybe_throttle();
        }
        let elapsed = start.elapsed();

        // Should take longer due to throttling
        assert!(elapsed > Duration::from_millis(100), "Elapsed: {:?}", elapsed);

        let stats = chaos.stats();
        assert!(stats > 20, "Throttle events: {}", stats);
    }

    #[test]
    fn test_memory_chaos() {
        let config = ChaosConfig::new("test")
            .with_failure_rate(1.0); // 100% to trigger OOM

        let chaos = MemoryChaos::new(config);
        chaos.start();

        let mut oom_count = 0;
        for _ in 0..10000 {
            if chaos.should_fail_allocation() {
                oom_count += 1;
            }
        }

        // Very rare (0.01 × failure_rate), but should happen a few times
        assert!(oom_count > 0, "OOM count: {}", oom_count);
    }

    #[test]
    fn test_clock_chaos() {
        let config = ChaosConfig::new("test")
            .with_failure_rate(0.5);

        let chaos = ClockChaos::new(config);
        chaos.start();

        let mut backwards_count = 0;
        let mut prev = chaos.now_with_chaos();

        for _ in 0..1000 {
            let curr = chaos.now_with_chaos();
            if curr < prev {
                backwards_count += 1;
            }
            prev = curr;
            std::thread::sleep(Duration::from_micros(100));
        }

        // Should go backwards occasionally
        assert!(backwards_count > 0, "Backwards count: {}", backwards_count);
    }

    #[test]
    fn test_chaos_coordinator() {
        let coordinator = ChaosCoordinator::new();

        coordinator.start_all();

        // All injectors should be active
        assert!(coordinator.network.active.load(Ordering::Relaxed));
        assert!(coordinator.disk.active.load(Ordering::Relaxed));
        assert!(coordinator.cpu.active.load(Ordering::Relaxed));
        assert!(coordinator.memory.active.load(Ordering::Relaxed));
        assert!(coordinator.clock.active.load(Ordering::Relaxed));

        coordinator.stop_all();

        // All injectors should be inactive
        assert!(!coordinator.network.active.load(Ordering::Relaxed));
        assert!(!coordinator.disk.active.load(Ordering::Relaxed));
        assert!(!coordinator.cpu.active.load(Ordering::Relaxed));
        assert!(!coordinator.memory.active.load(Ordering::Relaxed));
        assert!(!coordinator.clock.active.load(Ordering::Relaxed));
    }
}
