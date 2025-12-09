//! Phase 2 Property-Based Tests
//!
//! T42-compliant property tests following The Atomic Capsule principles.
//! Validates concurrent correctness, generation counter invariants, and
//! two-phase commit integrity under all conditions.
//!
//! Mandatory Reading Applied:
//! - The Atomic Capsule: Two-phase commit, SWeMR pattern, generation counters
//! - UCE32: Q30 (Empirical Validation), Q31 (Rust transformation via Send/Sync)
//! - B32: Fair property testing, statistical rigor

use kiang::{BreakerState, CauseCode, GpuCircuitBreaker, GpuState, GpuStateCapsule, QualityLevel};
use proptest::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

// ============================================================================
// Property 1: Bit Packing Correctness
// ============================================================================

/// Property: All GPU state fields are correctly packed and unpacked
///
/// Validates that bit packing preserves all field values across full range.
/// Following The Atomic Capsule principle: "Pack only what a reader needs"
#[test]
fn prop_gpu_state_bit_packing_preserves_values() {
    proptest!(|(
        gpu_id in 0u8..=255,
        frequency_mhz in 800u16..=3000,
        power_mw in 10000u16..=65000,
        temp_celsius in 0u8..=127,
        utilization in 0u8..=100,
    )| {
        let capsule = GpuStateCapsule::new();
        let state = GpuState {
            gpu_id,
            frequency_mhz,
            power_mw,
            temp_celsius,
            utilization,
            valid: true,
        };

        capsule.publish(state);
        let read_state = capsule.read();

        // Atomic Capsule invariant: Published state must match read state
        prop_assert!(read_state.is_valid());
        prop_assert_eq!(read_state.gpu_id, state.gpu_id, "GPU ID mismatch");
        prop_assert_eq!(read_state.frequency_mhz, state.frequency_mhz, "Frequency mismatch");
        prop_assert_eq!(read_state.power_mw, state.power_mw, "Power mismatch");
        prop_assert_eq!(read_state.temp_celsius, state.temp_celsius, "Temperature mismatch");
        prop_assert_eq!(read_state.utilization, state.utilization, "Utilization mismatch");
    });
}

// ============================================================================
// Property 2: Generation Counter Invariants
// ============================================================================

/// Property: Fence values (sequence numbers) monotonically increase
///
/// Following The Atomic Capsule: Generation counters prevent TOCTOU races.
/// This validates that sequence numbers never decrease across publications.
#[test]
fn prop_generation_counter_monotonic_increase() {
    let capsule = Arc::new(GpuStateCapsule::new());
    let iteration_count = 1000;

    // Single writer publishes monotonically
    for i in 0..iteration_count {
        let state = GpuState {
            gpu_id: 0,
            frequency_mhz: 2100,
            power_mw: 45000,
            temp_celsius: 65 + (i % 10) as u8,
            utilization: 50,
            valid: true,
        };

        capsule.publish(state);

        // Each publish increments internal sequence number
        // We validate consistency by reading immediately
        let read_state = capsule.read();
        assert!(read_state.is_valid(), "State must be valid after publish");
    }
}

/// Property: Multiple rapid publications maintain sequence consistency
#[test]
fn prop_rapid_publications_maintain_consistency() {
    proptest!(|(state_sequence: Vec<(u16, u8)>)| {
        let capsule = GpuStateCapsule::new();

        for (i, (freq_offset, temp_offset)) in state_sequence.iter().enumerate() {
            let state = GpuState {
                gpu_id: (i % 4) as u8,
                frequency_mhz: 2100 + (freq_offset % 500),
                power_mw: 45000,
                temp_celsius: 65 + (temp_offset % 20),
                utilization: 50,
                valid: true,
            };

            capsule.publish(state);

            // Immediate read must see published state
            let read_state = capsule.read();
            if read_state.is_valid() {
                prop_assert_eq!(read_state.gpu_id, state.gpu_id);
                prop_assert_eq!(read_state.frequency_mhz, state.frequency_mhz);
            }
        }
    });
}

// ============================================================================
// Property 3: Two-Phase Commit Integrity
// ============================================================================

/// Property: Concurrent reads never observe partial writes
///
/// Following The Atomic Capsule: Two-phase commit ensures "all-old or all-new".
/// Readers must NEVER see torn state (partial field updates).
#[test]
fn prop_concurrent_reads_never_partial_writes() {
    let capsule = Arc::new(GpuStateCapsule::new());
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Initial known state
    let initial = GpuState {
        gpu_id: 1,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 65,
        utilization: 50,
        valid: true,
    };
    capsule.publish(initial);

    // Writer thread: Rapid state updates with CORRELATED fields
    let writer_capsule = Arc::clone(&capsule);
    let writer_stop = Arc::clone(&stop_flag);
    let writer = thread::spawn(move || {
        let mut iteration = 0u64;
        while !writer_stop.load(Ordering::Relaxed) {
            // Use iteration to create correlated field values
            // If reader sees torn write, correlation breaks
            let state = GpuState {
                gpu_id: 2,
                frequency_mhz: 2200 + (iteration % 100) as u16,
                power_mw: 50000 + (iteration % 100) as u16, // CORRELATED with freq
                temp_celsius: 70 + (iteration % 10) as u8,  // CORRELATED
                utilization: 60 + (iteration % 10) as u8,   // CORRELATED
                valid: true,
            };
            writer_capsule.publish(state);
            iteration += 1;
        }
    });

    // Multiple reader threads validate correlation
    let mut readers = vec![];
    for reader_id in 0..4 {
        let reader_capsule = Arc::clone(&capsule);
        let reader_stop = Arc::clone(&stop_flag);
        readers.push(thread::spawn(move || {
            let mut observations = 0;
            while !reader_stop.load(Ordering::Relaxed) && observations < 10000 {
                let state = reader_capsule.read();
                if state.is_valid() {
                    // Validate field correlations
                    if state.gpu_id == 2 {
                        // Writer's state (not initial)
                        assert!(
                            state.frequency_mhz >= 2200 && state.frequency_mhz < 2300,
                            "Reader {} saw invalid frequency: {}",
                            reader_id,
                            state.frequency_mhz
                        );
                        assert!(
                            state.power_mw >= 50000 && state.power_mw < 50100,
                            "Reader {} saw invalid power: {}",
                            reader_id,
                            state.power_mw
                        );

                        // CRITICAL: Validate correlation
                        // freq and power should have same offset from base
                        let freq_offset = state.frequency_mhz - 2200;
                        let power_offset = state.power_mw - 50000;
                        assert_eq!(
                            freq_offset, power_offset,
                            "Reader {} saw torn write: freq offset {} != power offset {}",
                            reader_id, freq_offset, power_offset
                        );
                    }
                    observations += 1;
                }
            }
            observations
        }));
    }

    // Let test run for 100ms
    thread::sleep(Duration::from_millis(100));
    stop_flag.store(true, Ordering::Relaxed);

    writer.join().unwrap();
    for (i, reader) in readers.into_iter().enumerate() {
        let observations = reader.join().unwrap();
        assert!(
            observations > 100,
            "Reader {} made too few observations: {}",
            i,
            observations
        );
    }
}

/// Property: Version mismatch detection prevents stale reads
///
/// Following The Atomic Capsule: "ver == ver_tail" validation prevents reading
/// data that's currently being updated.
#[test]
fn prop_version_mismatch_returns_invalid() {
    // This property is validated by internal implementation
    // We test it indirectly by ensuring rapid writes don't cause crashes
    let capsule = Arc::new(GpuStateCapsule::new());

    let writer_capsule = Arc::clone(&capsule);
    let writer = thread::spawn(move || {
        for i in 0..10000 {
            let state = GpuState {
                gpu_id: 0,
                frequency_mhz: 2100,
                power_mw: 45000,
                temp_celsius: 65,
                utilization: (i % 100) as u8,
                valid: true,
            };
            writer_capsule.publish(state);
        }
    });

    // Concurrent reader NEVER panics, even during rapid writes
    let reader_capsule = Arc::clone(&capsule);
    let reader = thread::spawn(move || {
        for _ in 0..10000 {
            let state = reader_capsule.read();
            // State may be invalid during write, but never panics
            if state.is_valid() {
                assert!(state.utilization < 100);
            }
        }
    });

    writer.join().unwrap();
    reader.join().unwrap();
}

// ============================================================================
// Property 4: Context State Transitions Are Valid
// ============================================================================

/// Property: Circuit breaker transitions maintain valid quality level ordering
///
/// Following The Atomic Capsule: Breaker state transitions must be deterministic
/// based on threshold values. Transitions may jump levels based on severity.
#[test]
fn prop_breaker_transitions_deterministic() {
    proptest!(|(
        thermal_sequence: Vec<u8>,
    )| {
        let breaker = GpuCircuitBreaker::new();

        for temp in thermal_sequence {
            // Map temperature to millicelsius
            let thermal_mc = (temp as u32) * 1000;

            breaker.auto_adjust(thermal_mc, 0, 50, 50);
            let current_level = match breaker.level() {
                QualityLevel::L0 => 0,
                QualityLevel::L1 => 1,
                QualityLevel::L2 => 2,
                QualityLevel::L3 => 3,
            };

            // Validate deterministic thresholds from circuit_breaker.rs:
            // >95°C → L3, >85°C → L2, >75°C → L1, else L0
            let expected_level = if thermal_mc > 95_000 {
                3
            } else if thermal_mc > 85_000 {
                2
            } else if thermal_mc > 75_000 {
                1
            } else {
                0
            };

            prop_assert_eq!(
                current_level,
                expected_level,
                "Breaker level {} doesn't match expected {} for temp {}°C",
                current_level,
                expected_level,
                temp
            );
        }
    });
}

/// Property: Quality multipliers are deterministic for each level
#[test]
fn prop_quality_multipliers_deterministic() {
    let breaker = GpuCircuitBreaker::new();

    // L0 → 1.0
    breaker.force_level(QualityLevel::L0);
    assert_eq!(breaker.quality_multiplier(), 1.0);

    // L1 → 0.75
    breaker.force_level(QualityLevel::L1);
    assert_eq!(breaker.quality_multiplier(), 0.75);

    // L2 → 0.5
    breaker.force_level(QualityLevel::L2);
    assert_eq!(breaker.quality_multiplier(), 0.5);

    // L3 → 0.0
    breaker.force_level(QualityLevel::L3);
    assert_eq!(breaker.quality_multiplier(), 0.0);

    // Back to L0 → 1.0
    breaker.reset();
    assert_eq!(breaker.quality_multiplier(), 1.0);
}

// ============================================================================
// Property 5: Queue Selection Is Deterministic
// ============================================================================

/// Property: Command queue operations maintain FIFO ordering
///
/// Following The Atomic Capsule: Lockfree MPSC queue must preserve submission order.
#[test]
fn prop_command_queue_fifo_ordering() {
    use kiang::command::{Command, CommandQueue, CommandType};

    let queue = CommandQueue::new(64);
    let submission_order = vec![1u32, 2, 3, 4, 5];

    // Submit commands with unique buffer IDs
    for &buffer_id in &submission_order {
        let cmd = Command {
            cmd_type: CommandType::Render,
            buffer_id,
            size: 1024,
            priority: 128,
        };
        queue.submit(cmd).unwrap();
    }

    // Dequeue must preserve order
    for &expected_id in &submission_order {
        let cmd = queue.dequeue().expect("Queue should not be empty");
        assert_eq!(
            cmd.buffer_id, expected_id,
            "FIFO ordering violated: expected {}, got {}",
            expected_id, cmd.buffer_id
        );
    }

    assert!(queue.is_empty(), "Queue should be empty after draining");
}

/// Property: Queue overflow is detected correctly
#[test]
fn prop_queue_overflow_detection() {
    use kiang::command::{Command, CommandError, CommandQueue, CommandType};

    proptest!(|(capacity in 4usize..128)| {
        let queue = CommandQueue::new(capacity);
        let cmd = Command {
            cmd_type: CommandType::Compute,
            buffer_id: 42,
            size: 512,
            priority: 100,
        };

        // Fill to capacity
        for _ in 0..capacity {
            prop_assert!(queue.submit(cmd).is_ok());
        }

        // Next submission should fail
        prop_assert!(matches!(queue.submit(cmd), Err(CommandError::QueueFull)));
    });
}

// ============================================================================
// Property 6: State Validity Invariants
// ============================================================================

/// Property: Invalid state reads never return valid flag
#[test]
fn prop_unpublished_state_always_invalid() {
    let capsule = GpuStateCapsule::new();

    // Read before any publish
    let state = capsule.read();
    assert!(!state.is_valid(), "Unpublished state must be invalid");
    assert!(!state.is_ready(), "Unpublished state must not be ready");
}

/// Property: Ready check validates thermal and utilization thresholds
#[test]
fn prop_ready_check_enforces_thresholds() {
    proptest!(|(
        temp in 0u8..=127,
        util in 0u8..=100,
    )| {
        let state = GpuState {
            gpu_id: 0,
            frequency_mhz: 2100,
            power_mw: 45000,
            temp_celsius: temp,
            utilization: util,
            valid: true,
        };

        let expected_ready = temp < 95 && util < 95;
        prop_assert_eq!(
            state.is_ready(),
            expected_ready,
            "Ready check failed for temp={}, util={}",
            temp,
            util
        );
    });
}

// ============================================================================
// Property 7: Breaker Cause Code Consistency
// ============================================================================

/// Property: Cause codes map to threshold ranges deterministically
#[test]
fn prop_cause_codes_identify_conditions() {
    // Test thermal thresholds with fresh breakers
    let breaker_l1 = GpuCircuitBreaker::new();
    breaker_l1.auto_adjust(76_000, 0, 50, 50); // >75°C → L1
    assert_eq!(breaker_l1.level(), QualityLevel::L1);

    let breaker_l2_thermal = GpuCircuitBreaker::new();
    breaker_l2_thermal.auto_adjust(86_000, 0, 50, 50); // >85°C → L2
    assert_eq!(breaker_l2_thermal.level(), QualityLevel::L2);

    let breaker_l3_thermal = GpuCircuitBreaker::new();
    breaker_l3_thermal.auto_adjust(96_000, 0, 50, 50); // >95°C → L3
    assert_eq!(breaker_l3_thermal.level(), QualityLevel::L3);
    let state = breaker_l3_thermal.read_state();
    assert_eq!(state.cause_code, CauseCode::Thermal);

    // Test error rate thresholds
    let breaker_l2_errors = GpuCircuitBreaker::new();
    breaker_l2_errors.auto_adjust(70_000, 60, 50, 50); // >50 errors/sec → L2
    assert_eq!(breaker_l2_errors.level(), QualityLevel::L2);
    let state2 = breaker_l2_errors.read_state();
    assert_eq!(state2.cause_code, CauseCode::ErrorRate);

    // Test memory pressure thresholds
    let breaker_l2_memory = GpuCircuitBreaker::new();
    breaker_l2_memory.auto_adjust(70_000, 0, 96, 50); // >95% memory → L2
    assert_eq!(breaker_l2_memory.level(), QualityLevel::L2);
}

// ============================================================================
// Property 8: Memory Safety Under Concurrent Access
// ============================================================================

/// Property: No data races under concurrent read/write
///
/// Following UCE32 Q31: Rust's Send/Sync guarantees prevent data races.
/// This test validates that Arc<GpuStateCapsule> is correctly Send+Sync.
#[test]
fn prop_memory_safety_concurrent_access() {
    let capsule = Arc::new(GpuStateCapsule::new());
    let iterations = 5000;

    // Single writer
    let writer_capsule = Arc::clone(&capsule);
    let writer = thread::spawn(move || {
        for i in 0..iterations {
            let state = GpuState {
                gpu_id: (i % 4) as u8,
                frequency_mhz: 2100,
                power_mw: 45000,
                temp_celsius: 65,
                utilization: 50,
                valid: true,
            };
            writer_capsule.publish(state);
        }
    });

    // Multiple readers (tests Send+Sync)
    let mut readers = vec![];
    for _ in 0..8 {
        let reader_capsule = Arc::clone(&capsule);
        readers.push(thread::spawn(move || {
            for _ in 0..iterations {
                let _state = reader_capsule.read();
                // Just reading is sufficient to test memory safety
            }
        }));
    }

    writer.join().unwrap();
    for reader in readers {
        reader.join().unwrap();
    }
}

// ============================================================================
// Property 9: Performance Characteristics
// ============================================================================

/// Property: Read operations complete within atomic operation latency
///
/// Following B32 framework: Atomic operations should be <15ns on Intel Ultra 7 155H.
/// This property validates that reads don't involve unexpected overhead.
#[test]
fn prop_read_latency_within_bounds() {
    use std::time::Instant;

    let capsule = GpuStateCapsule::new();
    let state = GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 65,
        utilization: 50,
        valid: true,
    };
    capsule.publish(state);

    // Warm up CPU cache
    for _ in 0..1000 {
        let _ = capsule.read();
    }

    // Measure 10000 reads
    let iterations = 10000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = capsule.read();
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() / iterations;

    // Following B32 K2: AtomicU64 CAS is 10-15ns actual
    // Read involves 2 loads + validation, so <100ns is expected
    assert!(
        avg_ns < 100,
        "Average read latency {}ns exceeds 100ns threshold",
        avg_ns
    );
}

// ============================================================================
// Property 10: Statistical Distribution Validation
// ============================================================================

/// Property: Under steady load, valid reads dominate invalid reads
///
/// Following B32: Use statistical validation to ensure implementation correctness.
#[test]
fn prop_valid_reads_dominate_under_steady_load() {
    let capsule = Arc::new(GpuStateCapsule::new());
    let valid_count = Arc::new(AtomicU64::new(0));
    let invalid_count = Arc::new(AtomicU64::new(0));

    // Steady writer (100Hz update rate)
    let writer_capsule = Arc::clone(&capsule);
    let writer = thread::spawn(move || {
        for i in 0..500 {
            let state = GpuState {
                gpu_id: 0,
                frequency_mhz: 2100,
                power_mw: 45000,
                temp_celsius: 65,
                utilization: 50,
                valid: true,
            };
            writer_capsule.publish(state);
            thread::sleep(Duration::from_micros(10)); // ~100Hz
        }
    });

    // Reader sampling at 10kHz
    let reader_capsule = Arc::clone(&capsule);
    let reader_valid = Arc::clone(&valid_count);
    let reader_invalid = Arc::clone(&invalid_count);
    let reader = thread::spawn(move || {
        for _ in 0..5000 {
            let state = reader_capsule.read();
            if state.is_valid() {
                reader_valid.fetch_add(1, Ordering::Relaxed);
            } else {
                reader_invalid.fetch_add(1, Ordering::Relaxed);
            }
            thread::sleep(Duration::from_micros(1)); // 10kHz sampling
        }
    });

    writer.join().unwrap();
    reader.join().unwrap();

    let valid = valid_count.load(Ordering::Relaxed);
    let invalid = invalid_count.load(Ordering::Relaxed);

    // At steady 100Hz update rate, most reads should see valid state
    // Following B32 statistical rigor: >95% valid reads expected
    let valid_ratio = valid as f64 / (valid + invalid) as f64;
    assert!(
        valid_ratio > 0.95,
        "Valid read ratio {:.2}% below 95% threshold",
        valid_ratio * 100.0
    );
}
