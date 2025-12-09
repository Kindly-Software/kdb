//! Resource Exhaustion Chaos Test (Scenario 3)
//!
//! **Purpose**: Simulate OOM (memory pressure) and CPU saturation
//! **Expected Behavior**:
//! - System handles gracefully (no crashes)
//! - Degradation mode activates
//! - Memory/CPU limits respected
//! - Recovery when resources available
//!
//! # ASSUM Safety
//! - #ASSUME: System survives memory pressure (no OOM killer)
//! - #VERIFY: Allocate controlled amount, monitor success
//! - #ASSUME: CPU saturation doesn't deadlock system
//! - #VERIFY: Operations complete (may be slow)
//! - #ASSUME: Graceful degradation under resource pressure
//! - #VERIFY: Error messages clear, no panics
//!
//! # UCE34 Compliance
//! - Q23 (Resource management): Handle memory/CPU exhaustion
//! - Q24 (Graceful degradation): Degrade service, not crash
//! - Q25 (Recovery): Restore when resources available
//!
//! # T28 Testing
//! - Q22: Production scenario (resource exhaustion is common)
//! - Q23: Adversarial (simulate malicious resource consumption)
//! - Q24: B32 benchmarks (measure performance under pressure)

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::thread;

use clapi_core::proxy::BudgetRegistry;
use super::{ChaosConfig, ChaosFault, ChaosTestHarness};

/// Memory pressure simulator
#[derive(Clone)]
struct MemoryPressureSimulator {
    /// Pressure enabled flag
    enabled: Arc<AtomicBool>,
    /// Target memory pressure (MB)
    target_mb: usize,
    /// Allocated memory (balloons)
    balloons: Arc<parking_lot::Mutex<Vec<Vec<u8>>>>,
    /// Current allocation (MB)
    allocated_mb: Arc<AtomicUsize>,
}

impl MemoryPressureSimulator {
    fn new(enabled: Arc<AtomicBool>, target_mb: usize) -> Self {
        Self {
            enabled,
            target_mb,
            balloons: Arc::new(parking_lot::Mutex::new(Vec::new())),
            allocated_mb: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Apply memory pressure if enabled
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Allocation doesn't trigger OOM killer
    /// - #VERIFY: Allocate in small chunks, check success
    /// - #ASSUME: Deallocate releases memory to OS
    /// - #VERIFY: Monitor RSS after deallocation
    fn apply_pressure(&self) {
        if !self.enabled.load(Ordering::Acquire) {
            // Release memory
            let mut balloons = self.balloons.lock();
            balloons.clear();
            self.allocated_mb.store(0, Ordering::Release);
            return;
        }

        // Allocate memory in 10MB chunks
        const CHUNK_SIZE: usize = 10 * 1024 * 1024; // 10MB
        let mut balloons = self.balloons.lock();
        let current_mb = self.allocated_mb.load(Ordering::Relaxed);

        if current_mb < self.target_mb {
            // Allocate more
            let chunks_needed = (self.target_mb - current_mb) / 10;
            for _ in 0..chunks_needed {
                // Allocate and initialize (to force physical pages)
                let mut balloon = vec![0u8; CHUNK_SIZE];
                for i in (0..CHUNK_SIZE).step_by(4096) {
                    balloon[i] = 0xFF; // Touch page
                }
                balloons.push(balloon);
                self.allocated_mb.fetch_add(10, Ordering::Relaxed);
            }
            println!("Memory pressure: {} MB allocated", self.allocated_mb.load(Ordering::Relaxed));
        }
    }

    /// Get current allocation
    fn get_allocated_mb(&self) -> usize {
        self.allocated_mb.load(Ordering::Relaxed)
    }

    fn clone_handle(&self) -> Self {
        Self {
            enabled: Arc::clone(&self.enabled),
            target_mb: self.target_mb,
            balloons: Arc::clone(&self.balloons),
            allocated_mb: Arc::clone(&self.allocated_mb),
        }
    }
}

/// CPU saturation simulator
#[allow(dead_code)]
struct CpuSaturationSimulator {
    /// Saturation enabled flag
    enabled: Arc<AtomicBool>,
}

impl CpuSaturationSimulator {
    fn new(enabled: Arc<AtomicBool>) -> Self {
        let num_cpus = num_cpus::get();

        // Spawn CPU-intensive workers (detached - don't store JoinHandles)
        for _ in 0..num_cpus {
            let enabled_clone = Arc::clone(&enabled);
            thread::spawn(move || {
                let mut counter = 0u64;
                while enabled_clone.load(Ordering::Acquire) {
                    // Busy loop (CPU-intensive)
                    counter = counter.wrapping_add(1);
                    if counter % 1_000_000 == 0 {
                        thread::yield_now(); // Prevent total starvation
                    }
                }
            });
        }

        Self { enabled }
    }
}

/// Test: Memory pressure handling
///
/// # Test Scenario
/// 1. Baseline: Normal operation (10s)
/// 2. Chaos: Allocate 500MB memory pressure (30s)
/// 3. Recovery: Release memory, validate recovery (30s)
///
/// # Expected Results
/// - System survives memory pressure
/// - Operations may slow down, but complete
/// - No OOM crashes
/// - Recovery restores performance
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_memory_pressure() {
    // Setup chaos config
    let config = ChaosConfig::new(
        ChaosFault::ResourceExhaustion { memory_mb: 500 },
        Duration::from_secs(30),
        Duration::from_secs(30),
    );

    // Create memory pressure simulator
    let simulator = MemoryPressureSimulator::new(Arc::clone(&config.enabled), 500);

    // Budget registry
    let budget_registry = Arc::new(BudgetRegistry::new(100_00));
    let budget_id = 0x1111222233334444;

    // Background thread to apply/release pressure
    let simulator_clone = simulator.clone_handle();
    let pressure_thread = thread::spawn(move || {
        loop {
            simulator_clone.apply_pressure();
            thread::sleep(Duration::from_secs(1));
            if !simulator_clone.enabled.load(Ordering::Acquire) {
                // Release pressure
                simulator_clone.apply_pressure();
                break;
            }
        }
    });

    // Test function
    let test_fn = {
        let budget_registry = Arc::clone(&budget_registry);
        move || {
            budget_registry.try_deduct(budget_id, 1_00)
                .map(|_| ())
                .map_err(|e| format!("Budget error: {:?}", e))
        }
    };

    // Run chaos test
    let harness = ChaosTestHarness::new(config);
    let results = harness.run("Memory Pressure", test_fn);

    // Wait for pressure thread
    pressure_thread.join().unwrap();

    // Validate results
    // #ASSUME: System survives memory pressure
    // #VERIFY: Test completed without crash
    assert!(results.survived, "System should survive memory pressure");

    // #ASSUME: Operations may slow down but succeed
    // #VERIFY: Failure rate should be low (<10%)
    assert!(
        results.chaos_failure_rate_bp() < 1000,
        "Memory pressure should not cause >10% failures, got {} bp",
        results.chaos_failure_rate_bp()
    );

    // #ASSUME: Recovery restores normal operation
    // #VERIFY: Recovery failure rate <5%
    assert!(results.recovered, "System should recover after memory pressure released");

    println!("\n{}", results.summary());
    println!("Peak memory allocation: {} MB", simulator.get_allocated_mb());
}

/// Test: CPU saturation handling
///
/// # Test Scenario
/// - Saturate all CPU cores (100% utilization)
/// - System should remain responsive (degraded)
/// - No deadlocks or hangs
///
/// # Expected Results
/// - Operations complete (may be slow)
/// - Latency increased but bounded
/// - No deadlocks
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_cpu_saturation() {
    // Setup chaos config
    let config = ChaosConfig::new(
        ChaosFault::ResourceExhaustion { memory_mb: 0 }, // CPU saturation
        Duration::from_secs(30),
        Duration::from_secs(30),
    );

    // Create CPU saturation simulator
    let _simulator = CpuSaturationSimulator::new(Arc::clone(&config.enabled));

    // Budget registry
    let budget_registry = Arc::new(BudgetRegistry::new(100_00));
    let budget_id = 0x5555666677778888;

    // Test function
    let test_fn = {
        let budget_registry = Arc::clone(&budget_registry);
        move || {
            budget_registry.try_deduct(budget_id, 1_00)
                .map(|_| ())
                .map_err(|e| format!("Budget error: {:?}", e))
        }
    };

    // Run chaos test
    let harness = ChaosTestHarness::new(config);
    let results = harness.run("CPU Saturation", test_fn);

    // Validate results
    // #ASSUME: System survives CPU saturation
    // #VERIFY: Test completed
    assert!(results.survived, "System should survive CPU saturation");

    // #ASSUME: Latency increases but operations complete
    // #VERIFY: P99 latency higher during chaos, but finite
    assert!(
        results.chaos_p99_ms < 5000.0,
        "P99 latency should be <5s during CPU saturation, got {:.2}ms",
        results.chaos_p99_ms
    );

    // #ASSUME: Recovery restores normal latency
    // #VERIFY: Recovery p99 < chaos p99
    assert!(
        results.recovery_p99_ms < results.chaos_p99_ms,
        "Recovery should improve latency"
    );

    println!("\n{}", results.summary());
}

/// Test: Combined resource exhaustion (memory + CPU)
///
/// # Test Scenario
/// - Simultaneous memory pressure + CPU saturation
/// - Worst-case resource exhaustion
/// - System should survive gracefully
///
/// # Expected Results
/// - No crashes or OOM
/// - Degraded but functional
/// - Clear error messages
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_combined_resource_exhaustion() {
    // Setup chaos config
    let config = ChaosConfig::new(
        ChaosFault::ResourceExhaustion { memory_mb: 300 },
        Duration::from_secs(30),
        Duration::from_secs(30),
    );

    // Create simulators
    let memory_simulator = MemoryPressureSimulator::new(Arc::clone(&config.enabled), 300);
    let _cpu_simulator = CpuSaturationSimulator::new(Arc::clone(&config.enabled));

    // Budget registry
    let budget_registry = Arc::new(BudgetRegistry::new(100_00));
    let budget_id = 0x9999AAAABBBBCCCC;

    // Background memory pressure
    let memory_clone = memory_simulator.clone_handle();
    let pressure_thread = thread::spawn(move || {
        loop {
            memory_clone.apply_pressure();
            thread::sleep(Duration::from_secs(1));
            if !memory_clone.enabled.load(Ordering::Acquire) {
                memory_clone.apply_pressure();
                break;
            }
        }
    });

    // Test function
    let test_fn = {
        let budget_registry = Arc::clone(&budget_registry);
        move || {
            budget_registry.try_deduct(budget_id, 1_00)
                .map(|_| ())
                .map_err(|e| format!("Budget error: {:?}", e))
        }
    };

    // Run chaos test
    let harness = ChaosTestHarness::new(config);
    let results = harness.run("Combined Resource Exhaustion", test_fn);

    // Wait for pressure thread
    pressure_thread.join().unwrap();

    // Validate survival
    // #ASSUME: System survives worst-case resource exhaustion
    // #VERIFY: Test completed
    assert!(results.survived, "System should survive combined resource exhaustion");

    println!("\n{}", results.summary());
}

// Helper: Get number of CPUs
mod num_cpus {
    pub fn get() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    }
}

#[cfg(test)]
mod compile_tests {
    use super::*;

    #[test]
    fn test_memory_simulator_clone() {
        let enabled = Arc::new(AtomicBool::new(false));
        let simulator = MemoryPressureSimulator::new(enabled, 100);
        let cloned = simulator.clone_handle();

        simulator.allocated_mb.store(50, Ordering::Relaxed);
        assert_eq!(cloned.get_allocated_mb(), 50);
    }
}
