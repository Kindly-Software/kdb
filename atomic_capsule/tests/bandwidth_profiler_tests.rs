//! Bandwidth Profiler Capsule Tests
//!
//! # Test Coverage (30+ tests across 4 tiers)
//!
//! ## Unit Tests (Q1-Q7)
//! - Bandwidth snapshot creation and conversions
//! - Memory domain properties
//! - DualAtomicU64 consistency
//! - Domain counter operations
//! - Profiler initialization
//!
//! ## Property Tests (Q8-Q14)
//! - Utilization bounds (0-100%)
//! - Bandwidth monotonicity
//! - Peak tracking correctness
//! - Ring buffer wraparound
//!
//! ## Integration Tests (Q15-Q21)
//! - Multi-domain profiling
//! - Concurrent sampling
//! - Rolling window behavior
//!
//! ## Production Tests (Q22-Q28)
//! - Sustained bandwidth monitoring
//! - High-frequency sampling
//! - Peak detection accuracy

#![cfg(test)]
#![cfg(feature = "std")]

use atomic_capsule::gpu::kgpu_driver::bandwidth_profiler::{
    BandwidthProfilerCapsule, BandwidthSnapshot, MemoryDomain,
};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ============================================================================
// Unit Tests (Q1-Q7)
// ============================================================================

#[test]
fn test_q1_bandwidth_snapshot_zero() {
    let snapshot = BandwidthSnapshot::zero();
    assert_eq!(snapshot.read_bytes_per_sec, 0);
    assert_eq!(snapshot.write_bytes_per_sec, 0);
    assert_eq!(snapshot.total_bytes_per_sec, 0);
    assert_eq!(snapshot.utilization_percent, 0);
    assert_eq!(snapshot.timestamp_ns, 0);
}

#[test]
fn test_q2_bandwidth_snapshot_creation() {
    let snapshot = BandwidthSnapshot::new(1_000_000_000, 500_000_000, 12800, 1000);

    assert_eq!(snapshot.read_bytes_per_sec, 1_000_000_000);
    assert_eq!(snapshot.write_bytes_per_sec, 500_000_000);
    assert_eq!(snapshot.total_bytes_per_sec, 1_500_000_000);
    assert_eq!(snapshot.utilization_percent, 12800); // 50.0% in Q24.8
    assert_eq!(snapshot.timestamp_ns, 1000);
}

#[test]
fn test_q3_bandwidth_snapshot_conversions() {
    let snapshot = BandwidthSnapshot::new(2_000_000_000, 1_000_000_000, 25600, 2000);

    assert_eq!(snapshot.utilization_f32(), 100.0);
    assert_eq!(snapshot.total_gbps(), 3.0);
    assert_eq!(snapshot.read_gbps(), 2.0);
    assert_eq!(snapshot.write_gbps(), 1.0);
}

#[test]
fn test_q4_memory_domain_all() {
    let domains = MemoryDomain::all();
    assert_eq!(domains.len(), 5);
    assert_eq!(domains[0], MemoryDomain::Vram);
    assert_eq!(domains[1], MemoryDomain::Gtt);
    assert_eq!(domains[2], MemoryDomain::Pcie);
    assert_eq!(domains[3], MemoryDomain::L2Cache);
    assert_eq!(domains[4], MemoryDomain::SharedMemory);
}

#[test]
fn test_q5_memory_domain_names() {
    assert_eq!(MemoryDomain::Vram.name(), "VRAM");
    assert_eq!(MemoryDomain::Gtt.name(), "GTT");
    assert_eq!(MemoryDomain::Pcie.name(), "PCIe");
    assert_eq!(MemoryDomain::L2Cache.name(), "L2Cache");
    assert_eq!(MemoryDomain::SharedMemory.name(), "SharedMemory");
}

#[test]
fn test_q6_memory_domain_theoretical_peaks() {
    // HBM3 reference: 819 GB/s per stack
    assert_eq!(MemoryDomain::Vram.theoretical_peak_gbps(), 819);

    // DDR5-4800: 76.8 GB/s per channel, 2 channels
    assert_eq!(MemoryDomain::Gtt.theoretical_peak_gbps(), 154);

    // PCIe Gen5 x16: 64 GB/s bidirectional
    assert_eq!(MemoryDomain::Pcie.theoretical_peak_gbps(), 64);

    // L2 cache: ~10× VRAM (internal estimate)
    assert_eq!(MemoryDomain::L2Cache.theoretical_peak_gbps(), 8192);

    // Shared memory: ~100× VRAM (internal estimate)
    assert_eq!(MemoryDomain::SharedMemory.theoretical_peak_gbps(), 81920);
}

#[test]
fn test_q7_profiler_creation() {
    let profiler = BandwidthProfilerCapsule::new();
    assert_eq!(profiler.generation(), 0);
    assert_eq!(profiler.get_total_samples(), 0);

    // Verify zero initialization
    let snapshot = profiler.get_current_bandwidth();
    assert_eq!(snapshot.read_bytes_per_sec, 0);
    assert_eq!(snapshot.write_bytes_per_sec, 0);
}

// ============================================================================
// Property Tests (Q8-Q14)
// ============================================================================

#[test]
fn test_q8_utilization_bounds_zero() {
    let profiler = BandwidthProfilerCapsule::new();

    for domain in MemoryDomain::all() {
        let util = profiler.get_utilization(domain);
        assert!(
            util >= 0.0 && util <= 100.0,
            "Utilization out of bounds: {}",
            util
        );
    }
}

#[test]
fn test_q9_utilization_bounds_after_sampling() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record samples for all domains
    for domain in MemoryDomain::all() {
        profiler.record_sample(domain, 1_000_000_000, 500_000_000, 1_000_000_000);
    }

    // Verify all utilizations are in bounds
    for domain in MemoryDomain::all() {
        let util = profiler.get_utilization(domain);
        assert!(
            util >= 0.0 && util <= 100.0,
            "Utilization out of bounds for {:?}: {}",
            domain,
            util
        );
    }
}

#[test]
fn test_q10_utilization_saturation() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record 10× theoretical peak (should saturate at 100%)
    let theoretical_bps = (MemoryDomain::Vram.theoretical_peak_gbps() as u64) * 1_000_000_000;
    profiler.record_sample(MemoryDomain::Vram, theoretical_bps * 5, theoretical_bps * 5, 1_000_000_000);

    let util = profiler.get_utilization(MemoryDomain::Vram);
    assert!(
        util <= 100.0,
        "Utilization exceeds 100%: {}",
        util
    );
}

#[test]
fn test_q11_peak_monotonicity() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record increasing bandwidth samples
    for i in 1..=10 {
        profiler.record_sample(
            MemoryDomain::Vram,
            i * 100_000_000,
            i * 50_000_000,
            1_000_000_000,
        );

        let peak = profiler.get_peak_bandwidth();
        assert!(
            peak.read_bytes_per_sec >= (i * 100_000_000) as u64,
            "Peak read decreased"
        );
        assert!(
            peak.write_bytes_per_sec >= (i * 50_000_000) as u64,
            "Peak write decreased"
        );
    }
}

#[test]
fn test_q12_peak_never_decreases() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record high bandwidth
    profiler.record_sample(MemoryDomain::Vram, 10_000_000_000, 5_000_000_000, 1_000_000_000);
    let peak_high = profiler.get_peak_bandwidth();

    // Record low bandwidth
    profiler.record_sample(MemoryDomain::Vram, 100_000_000, 50_000_000, 1_000_000_000);
    let peak_low = profiler.get_peak_bandwidth();

    // Peak should remain at high value
    assert_eq!(peak_high.read_bytes_per_sec, peak_low.read_bytes_per_sec);
    assert_eq!(peak_high.write_bytes_per_sec, peak_low.write_bytes_per_sec);
}

#[test]
fn test_q13_ring_buffer_wraparound() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record 20 samples (ring buffer capacity is 8)
    for i in 1..=20 {
        profiler.record_sample(
            MemoryDomain::Vram,
            i * 100_000_000,
            i * 50_000_000,
            1_000_000_000,
        );
    }

    let snapshots = profiler.get_recent_snapshots();

    // Most recent should be sample 20
    assert_eq!(snapshots[0].read_bytes_per_sec, 2_000_000_000);

    // Oldest in window should be sample 13 (20 - 7)
    assert_eq!(snapshots[7].read_bytes_per_sec, 1_300_000_000);
}

#[test]
fn test_q14_sample_count_accuracy() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    let expected_samples = 42;
    for i in 0..expected_samples {
        profiler.record_sample(
            MemoryDomain::Vram,
            (i + 1) * 100_000_000,
            (i + 1) * 50_000_000,
            1_000_000_000,
        );
    }

    assert_eq!(profiler.get_total_samples(), expected_samples);
}

// ============================================================================
// Integration Tests (Q15-Q21)
// ============================================================================

#[test]
fn test_q15_multi_domain_profiling() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record samples for each domain
    profiler.record_sample(MemoryDomain::Vram, 1_000_000_000, 500_000_000, 1_000_000_000);
    profiler.record_sample(MemoryDomain::Gtt, 200_000_000, 100_000_000, 1_000_000_000);
    profiler.record_sample(MemoryDomain::Pcie, 50_000_000, 25_000_000, 1_000_000_000);
    profiler.record_sample(MemoryDomain::L2Cache, 5_000_000_000, 2_500_000_000, 1_000_000_000);
    profiler.record_sample(MemoryDomain::SharedMemory, 10_000_000_000, 5_000_000_000, 1_000_000_000);

    // Verify all domains have non-zero utilization
    for domain in MemoryDomain::all() {
        let util = profiler.get_utilization(domain);
        assert!(util > 0.0, "Domain {:?} has zero utilization", domain);
    }
}

#[test]
fn test_q16_concurrent_sampling() {
    let profiler = Arc::new(BandwidthProfilerCapsule::new());
    profiler.start_sampling(1000);

    let mut handles = vec![];

    // Spawn 8 threads, each recording samples
    for thread_id in 0..8 {
        let profiler_clone = Arc::clone(&profiler);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let domain = match thread_id % 5 {
                    0 => MemoryDomain::Vram,
                    1 => MemoryDomain::Gtt,
                    2 => MemoryDomain::Pcie,
                    3 => MemoryDomain::L2Cache,
                    _ => MemoryDomain::SharedMemory,
                };

                profiler_clone.record_sample(
                    domain,
                    (i + 1) * 10_000_000,
                    (i + 1) * 5_000_000,
                    1_000_000_000,
                );
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify samples were recorded
    assert_eq!(profiler.get_total_samples(), 800); // 8 threads × 100 samples
}

#[test]
fn test_q17_rolling_window_consistency() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record 8 samples (fill ring buffer)
    for i in 1..=8 {
        profiler.record_sample(
            MemoryDomain::Vram,
            i * 100_000_000,
            i * 50_000_000,
            1_000_000_000,
        );
    }

    let snapshots = profiler.get_recent_snapshots();

    // Verify ordering (most recent first)
    for i in 0..8 {
        let expected_read = ((8 - i) * 100_000_000) as u64;
        assert_eq!(snapshots[i].read_bytes_per_sec, expected_read);
    }
}

#[test]
fn test_q18_start_stop_sampling() {
    let profiler = BandwidthProfilerCapsule::new();

    // Start sampling
    profiler.start_sampling(1000);
    profiler.record_sample(MemoryDomain::Vram, 1_000_000_000, 500_000_000, 1_000_000_000);
    let samples_started = profiler.get_total_samples();
    assert_eq!(samples_started, 1);

    // Stop sampling
    profiler.stop_sampling();

    // Recording after stop should still work (API doesn't prevent it)
    profiler.record_sample(MemoryDomain::Vram, 1_000_000_000, 500_000_000, 1_000_000_000);
    let samples_stopped = profiler.get_total_samples();
    assert_eq!(samples_stopped, 2);
}

#[test]
fn test_q19_reset_functionality() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record samples
    for i in 1..=10 {
        profiler.record_sample(
            MemoryDomain::Vram,
            i * 100_000_000,
            i * 50_000_000,
            1_000_000_000,
        );
    }

    assert_eq!(profiler.get_total_samples(), 10);
    let gen_before = profiler.generation();

    // Reset
    profiler.reset();

    assert_eq!(profiler.get_total_samples(), 0);
    assert_eq!(profiler.generation(), gen_before + 1);

    let peak = profiler.get_peak_bandwidth();
    assert_eq!(peak.read_bytes_per_sec, 0);
    assert_eq!(peak.write_bytes_per_sec, 0);
}

#[test]
fn test_q20_generation_counter_increments() {
    let profiler = BandwidthProfilerCapsule::new();
    let gen0 = profiler.generation();

    profiler.start_sampling(1000);
    let gen1 = profiler.generation();
    assert_eq!(gen1, gen0 + 1);

    profiler.reset();
    let gen2 = profiler.generation();
    assert_eq!(gen2, gen1 + 1);
}

#[test]
fn test_q21_bandwidth_calculation_accuracy() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record exactly 1 GB/s read, 500 MB/s write over 1 second
    profiler.record_sample(MemoryDomain::Vram, 1_000_000_000, 500_000_000, 1_000_000_000);

    let snapshot = profiler.get_current_bandwidth();
    assert_eq!(snapshot.read_bytes_per_sec, 1_000_000_000);
    assert_eq!(snapshot.write_bytes_per_sec, 500_000_000);
    assert_eq!(snapshot.total_bytes_per_sec, 1_500_000_000);

    // Verify GB/s conversions
    assert_eq!(snapshot.read_gbps(), 1.0);
    assert_eq!(snapshot.write_gbps(), 0.5);
    assert_eq!(snapshot.total_gbps(), 1.5);
}

// ============================================================================
// Production Tests (Q22-Q28)
// ============================================================================

#[test]
fn test_q22_sustained_monitoring_100_samples() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record 100 samples with varying bandwidth
    for i in 1..=100 {
        let read_bps = (i % 20 + 1) * 100_000_000;
        let write_bps = (i % 20 + 1) * 50_000_000;
        profiler.record_sample(MemoryDomain::Vram, read_bps, write_bps, 1_000_000_000);
    }

    assert_eq!(profiler.get_total_samples(), 100);

    // Peak should be 20× the base rate
    let peak = profiler.get_peak_bandwidth();
    assert_eq!(peak.read_bytes_per_sec, 2_000_000_000);
    assert_eq!(peak.write_bytes_per_sec, 1_000_000_000);
}

#[test]
fn test_q23_high_frequency_sampling() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(100); // 100 microsecond interval

    // Record 1000 samples rapidly
    for i in 1..=1000 {
        profiler.record_sample(
            MemoryDomain::Vram,
            (i % 100 + 1) * 10_000_000,
            (i % 100 + 1) * 5_000_000,
            100_000, // 100 microseconds
        );
    }

    assert_eq!(profiler.get_total_samples(), 1000);
}

#[test]
fn test_q24_peak_detection_multiple_domains() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record peak for each domain at different times
    profiler.record_sample(MemoryDomain::Vram, 10_000_000_000, 5_000_000_000, 1_000_000_000);
    profiler.record_sample(MemoryDomain::Gtt, 1_000_000_000, 500_000_000, 1_000_000_000);
    profiler.record_sample(MemoryDomain::Pcie, 500_000_000, 250_000_000, 1_000_000_000);

    // Global peak should be from VRAM
    let global_peak = profiler.get_peak_bandwidth();
    assert_eq!(global_peak.read_bytes_per_sec, 10_000_000_000);
    assert_eq!(global_peak.write_bytes_per_sec, 5_000_000_000);
}

#[test]
fn test_q25_utilization_accuracy() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record exactly 50% of HBM3 theoretical peak (819 GB/s)
    let half_peak = (MemoryDomain::Vram.theoretical_peak_gbps() as u64) * 500_000_000; // 50%
    profiler.record_sample(MemoryDomain::Vram, half_peak, 0, 1_000_000_000);

    let util = profiler.get_utilization(MemoryDomain::Vram);

    // Should be close to 50% (within 1% tolerance)
    assert!((util - 50.0).abs() < 1.0, "Utilization: {}", util);
}

#[test]
fn test_q26_concurrent_domain_sampling() {
    let profiler = Arc::new(BandwidthProfilerCapsule::new());
    profiler.start_sampling(1000);

    let mut handles = vec![];

    // Spawn 5 threads, one per domain
    for (i, domain) in MemoryDomain::all().iter().enumerate() {
        let profiler_clone = Arc::clone(&profiler);
        let domain_copy = *domain;
        let handle = thread::spawn(move || {
            for j in 0..200 {
                profiler_clone.record_sample(
                    domain_copy,
                    (j + 1) * 10_000_000,
                    (j + 1) * 5_000_000,
                    1_000_000_000,
                );
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all domains have samples
    assert_eq!(profiler.get_total_samples(), 1000); // 5 domains × 200 samples
}

#[test]
fn test_q27_bandwidth_overflow_protection() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record maximum u64 values (should not panic)
    profiler.record_sample(MemoryDomain::Vram, u64::MAX, u64::MAX, 1_000_000_000);

    // Should saturate gracefully
    let snapshot = profiler.get_current_bandwidth();
    assert!(snapshot.read_bytes_per_sec > 0);
    assert!(snapshot.write_bytes_per_sec > 0);
}

#[test]
fn test_q28_zero_elapsed_time_handling() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record with zero elapsed time (should handle gracefully)
    profiler.record_sample(MemoryDomain::Vram, 1_000_000_000, 500_000_000, 0);

    let snapshot = profiler.get_current_bandwidth();
    // Should be zero (bandwidth calculation failed)
    assert_eq!(snapshot.read_bytes_per_sec, 0);
    assert_eq!(snapshot.write_bytes_per_sec, 0);
}

// ============================================================================
// Size and Alignment Tests
// ============================================================================

#[test]
fn test_size_constraints() {
    assert!(core::mem::size_of::<BandwidthProfilerCapsule>() <= 1024);
    assert_eq!(core::mem::align_of::<BandwidthProfilerCapsule>(), 256);
}

#[test]
fn test_snapshot_size() {
    assert_eq!(core::mem::size_of::<BandwidthSnapshot>(), 32);
}

// ============================================================================
// Documentation Tests
// ============================================================================

#[test]
fn test_readme_example_basic_usage() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record VRAM bandwidth: 1 GB/s read, 500 MB/s write
    profiler.record_sample(MemoryDomain::Vram, 1_000_000_000, 500_000_000, 1_000_000_000);

    // Get current bandwidth
    let current = profiler.get_current_bandwidth();
    assert_eq!(current.total_gbps(), 1.5);

    // Get utilization
    let util = profiler.get_utilization(MemoryDomain::Vram);
    assert!(util > 0.0);
}

#[test]
fn test_readme_example_multi_domain() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Profile all memory domains
    for domain in MemoryDomain::all() {
        profiler.record_sample(domain, 100_000_000, 50_000_000, 1_000_000_000);
    }

    // Check utilization for each domain
    for domain in MemoryDomain::all() {
        let util = profiler.get_utilization(domain);
        println!("{}: {:.2}%", domain.name(), util);
    }
}

#[test]
fn test_readme_example_peak_tracking() {
    let profiler = BandwidthProfilerCapsule::new();
    profiler.start_sampling(1000);

    // Record varying bandwidth
    for i in 1..=10 {
        profiler.record_sample(
            MemoryDomain::Vram,
            i * 200_000_000,
            i * 100_000_000,
            1_000_000_000,
        );
    }

    // Get peak bandwidth
    let peak = profiler.get_peak_bandwidth();
    assert_eq!(peak.read_gbps(), 2.0);
    assert_eq!(peak.write_gbps(), 1.0);
}
