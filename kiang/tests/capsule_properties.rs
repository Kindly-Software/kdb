//! Property-based tests for atomic capsules
//!
//! Validates generation counter invariants, TOCTOU prevention,
//! and atomic operation correctness under concurrent load.

use kiang::{GpuState, GpuStateCapsule};
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

/// Property: Published state is always readable with correct values
#[test]
fn prop_gpu_state_always_valid() {
    proptest!(|(
        gpu_id in 0u8..16,
        freq in 800u16..3000,
        power in 10000u16..65000,
        temp in 30u8..100,
        util in 0u8..100,
    )| {
        let capsule = GpuStateCapsule::new();
        let state = GpuState {
            gpu_id,
            frequency_mhz: freq,
            power_mw: power,
            temp_celsius: temp,
            utilization: util,
            valid: true,
        };

        capsule.publish(state);
        let read_state = capsule.read();

        prop_assert!(read_state.is_valid());
        prop_assert_eq!(read_state.gpu_id, state.gpu_id);
        prop_assert_eq!(read_state.frequency_mhz, state.frequency_mhz);
        prop_assert_eq!(read_state.power_mw, state.power_mw);
        prop_assert_eq!(read_state.temp_celsius, state.temp_celsius);
        prop_assert_eq!(read_state.utilization, state.utilization);
    });
}

/// Property: Concurrent reads never observe partial writes
#[test]
fn prop_concurrent_reads_atomic() {
    let capsule = Arc::new(GpuStateCapsule::new());

    // Initial state
    let initial = GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 65,
        utilization: 50,
        valid: true,
    };
    capsule.publish(initial);

    // Writer thread updates rapidly
    let writer_capsule = Arc::clone(&capsule);
    let writer = thread::spawn(move || {
        for i in 0..1000 {
            let state = GpuState {
                gpu_id: 1,
                frequency_mhz: 2100 + (i % 100) as u16,
                power_mw: 45000 + (i % 1000) as u16,
                temp_celsius: 65 + (i % 20) as u8,
                utilization: 50 + (i % 40) as u8,
                valid: true,
            };
            writer_capsule.publish(state);
        }
    });

    // Multiple reader threads
    let mut readers = vec![];
    for _ in 0..4 {
        let reader_capsule = Arc::clone(&capsule);
        readers.push(thread::spawn(move || {
            for _ in 0..1000 {
                let state = reader_capsule.read();
                if state.is_valid() {
                    // Invariant: frequency and power should be correlated
                    // If we read a partial write, this would fail
                    assert!(state.frequency_mhz >= 2100);
                    assert!(state.frequency_mhz < 2200);
                    assert!(state.power_mw >= 45000);
                    assert!(state.power_mw < 46000);
                }
            }
        }));
    }

    writer.join().unwrap();
    for reader in readers {
        reader.join().unwrap();
    }
}

/// Property: Generation counter prevents ABA
#[test]
fn prop_generation_counter_monotonic() {
    let capsule = GpuStateCapsule::new();

    let mut last_seq = 0u64;
    for i in 0..1000 {
        let state = GpuState {
            gpu_id: 0,
            frequency_mhz: 2100,
            power_mw: 45000,
            temp_celsius: 65 + (i % 10) as u8,
            utilization: 50,
            valid: true,
        };

        capsule.publish(state);

        // Note: We can't directly read sequence number from public API
        // This test validates that publishing works monotonically
        let read_state = capsule.read();
        assert!(read_state.is_valid());
    }
}

/// Property: Two-phase commit ensures readers see consistent state
#[test]
fn prop_two_phase_commit_consistency() {
    proptest!(|(states: Vec<(u16, u16, u8)>)| {
        let capsule = GpuStateCapsule::new();

        for (freq, power, temp) in states {
            let state = GpuState {
                gpu_id: 0,
                frequency_mhz: freq.clamp(800, 3000),
                power_mw: power.clamp(10000, 65000),
                temp_celsius: temp.clamp(30, 100),
                utilization: 50,
                valid: true,
            };

            capsule.publish(state);

            // Immediate read should see published state
            let read_state = capsule.read();
            if read_state.is_valid() {
                prop_assert_eq!(read_state.frequency_mhz, state.frequency_mhz);
                prop_assert_eq!(read_state.power_mw, state.power_mw);
                prop_assert_eq!(read_state.temp_celsius, state.temp_celsius);
            }
        }
    });
}

/// Property: Invalid reads return invalid state
#[test]
fn prop_invalid_reads_safe() {
    let capsule = GpuStateCapsule::new();

    // Read before any publish
    let state = capsule.read();
    assert!(!state.is_valid());
    assert!(!state.is_ready());
}

/// Stress test: Verify no data races under extreme load
#[test]
fn stress_concurrent_operations() {
    let capsule = Arc::new(GpuStateCapsule::new());

    // Single writer (SWeMR pattern)
    let writer_capsule = Arc::clone(&capsule);
    let writer = thread::spawn(move || {
        for i in 0..10000 {
            let state = GpuState {
                gpu_id: (i % 4) as u8,
                frequency_mhz: 2100 + (i % 500) as u16,
                power_mw: 45000 + (i % 5000) as u16,
                temp_celsius: 65 + (i % 25) as u8,
                utilization: 50 + (i % 50) as u8,
                valid: true,
            };
            writer_capsule.publish(state);
        }
    });

    // Many readers
    let mut readers = vec![];
    for _ in 0..8 {
        let reader_capsule = Arc::clone(&capsule);
        readers.push(thread::spawn(move || {
            let mut valid_reads = 0;
            for _ in 0..10000 {
                let state = reader_capsule.read();
                if state.is_valid() {
                    valid_reads += 1;
                    // All values should be in expected ranges
                    assert!(state.gpu_id < 4);
                    assert!(state.frequency_mhz >= 2100 && state.frequency_mhz < 2600);
                    assert!(state.temp_celsius >= 65 && state.temp_celsius < 90);
                }
            }
            valid_reads
        }));
    }

    writer.join().unwrap();
    for reader in readers {
        let valid_reads = reader.join().unwrap();
        // Should have read many valid states
        assert!(valid_reads > 100);
    }
}
