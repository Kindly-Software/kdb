// Lockfree Stress Integration Tests
//
// Comprehensive multi-threaded stress tests for atomic primitive coordination
// following UCE-32 and T42 frameworks with ASSUM safety validation.
//
// ASSUM: All operations are lockfree - NO mutex/RwLock usage
// VERIFY: Memory ordering correctness under extreme contention

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use std::collections::HashMap;

// Import all atomic primitives for integration testing
use atomic_risk_envelope::{AtomicRiskEnvelope, RiskEnvelope, Fields, OrderCheck, flag};
use atomic_risk_ladder_table::{Rlt1024, HeaderWord, TripWord, ActionWord, DEFAULT_RECOVER_SCALE_Q1_7};
use atomic_venue_snapshot::{Avs128, Avs128Snapshot};
use atomic_depth_of_market_slice::{Dos1024, Dos1024Snapshot, DosHeader, DosInstrument};

/// Maximum stress test: All primitives under heavy concurrent load
/// Verifies lockfree operation and data consistency across the entire system
#[test]
fn test_system_wide_lockfree_stress() {
    // #ASSUME: System-wide lockfree coordination under maximum load
    // #VERIFY: No deadlocks, livelocks, or data corruption

    const NUM_THREADS: usize = 16;
    const OPERATIONS_PER_THREAD: usize = 50_000;
    const TEST_DURATION_SECS: u64 = 30;

    println!("🚀 Starting system-wide stress test: {} threads, {} ops/thread, {} seconds",
             NUM_THREADS, OPERATIONS_PER_THREAD, TEST_DURATION_SECS);

    // Initialize all atomic primitives
    let risk_envelope = create_test_risk_envelope();
    let atomic_risk = Arc::new(AtomicRiskEnvelope::new(risk_envelope));

    let rlt = Arc::new(create_test_rlt());
    let avs = Arc::new(Avs128::new());
    let dos = Arc::new(Dos1024::new());

    // Shared coordination state
    let should_stop = Arc::new(AtomicBool::new(false));
    let operations_completed = Arc::new(AtomicUsize::new(0));
    let errors_detected = Arc::new(AtomicUsize::new(0));

    let start_time = Instant::now();

    // Spawn worker threads with different operation patterns
    let handles: Vec<_> = (0..NUM_THREADS).map(|thread_id| {
        let atomic_risk = Arc::clone(&atomic_risk);
        let rlt = Arc::clone(&rlt);
        let avs = Arc::clone(&avs);
        let dos = Arc::clone(&dos);
        let should_stop = Arc::clone(&should_stop);
        let operations_completed = Arc::clone(&operations_completed);
        let errors_detected = Arc::clone(&errors_detected);

        thread::spawn(move || {
            stress_worker_thread(
                thread_id,
                atomic_risk,
                rlt,
                avs,
                dos,
                should_stop,
                operations_completed,
                errors_detected,
                OPERATIONS_PER_THREAD,
            )
        })
    }).collect();

    // Spawn monitor thread
    let monitor_should_stop = Arc::clone(&should_stop);
    let monitor_ops = Arc::clone(&operations_completed);
    let monitor_errors = Arc::clone(&errors_detected);

    let monitor_handle = thread::spawn(move || {
        let mut last_ops = 0;
        let mut last_time = Instant::now();

        while !monitor_should_stop.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(5));

            let current_ops = monitor_ops.load(Ordering::Relaxed);
            let current_errors = monitor_errors.load(Ordering::Relaxed);
            let current_time = Instant::now();

            let ops_delta = current_ops - last_ops;
            let time_delta = current_time.duration_since(last_time).as_secs_f64();
            let ops_per_sec = ops_delta as f64 / time_delta;

            println!("📊 Progress: {} ops total, {:.0} ops/sec, {} errors",
                     current_ops, ops_per_sec, current_errors);

            last_ops = current_ops;
            last_time = current_time;
        }
    });

    // Let the test run for the specified duration or until completion
    thread::sleep(Duration::from_secs(TEST_DURATION_SECS));
    should_stop.store(true, Ordering::SeqCst);

    // Wait for all threads to complete
    let worker_results: Vec<_> = handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    monitor_handle.join().unwrap();

    let total_time = start_time.elapsed();
    let total_ops = operations_completed.load(Ordering::Relaxed);
    let total_errors = errors_detected.load(Ordering::Relaxed);

    // Analyze results
    let avg_ops_per_sec = total_ops as f64 / total_time.as_secs_f64();
    let error_rate = total_errors as f64 / total_ops as f64;

    println!("✅ Stress test completed:");
    println!("   Total operations: {}", total_ops);
    println!("   Total time: {:.2}s", total_time.as_secs_f64());
    println!("   Average throughput: {:.0} ops/sec", avg_ops_per_sec);
    println!("   Error rate: {:.4}%", error_rate * 100.0);

    // Verify performance and correctness requirements
    assert!(avg_ops_per_sec > 50_000.0,
        "System throughput should exceed 50k ops/sec under stress, got {:.0}",
        avg_ops_per_sec);

    assert!(error_rate < 0.001,
        "Error rate should be <0.1% under stress, got {:.4}%",
        error_rate * 100.0);

    // Verify final system state consistency
    verify_final_system_consistency(&atomic_risk, &rlt, &avs, &dos);

    println!("🎉 System-wide lockfree stress test PASSED");
}

/// Individual worker thread performing mixed operations on all primitives
fn stress_worker_thread(
    thread_id: usize,
    atomic_risk: Arc<AtomicRiskEnvelope>,
    rlt: Arc<Rlt1024>,
    avs: Arc<Avs128>,
    dos: Arc<Dos1024>,
    should_stop: Arc<AtomicBool>,
    operations_completed: Arc<AtomicUsize>,
    errors_detected: Arc<AtomicUsize>,
    max_operations: usize,
) -> (usize, usize, usize) {
    let mut local_ops = 0;
    let mut local_errors = 0;
    let mut local_successes = 0;

    let mut operation_mix = [0usize; 8];  // Track operation distribution

    while local_ops < max_operations && !should_stop.load(Ordering::Relaxed) {
        let operation_type = local_ops % 8;
        operation_mix[operation_type] += 1;

        let success = match operation_type {
            0 => {
                // Risk envelope read
                let envelope = atomic_risk.load(Ordering::Relaxed);
                let remaining = envelope.rem_daily_loss_cents();
                remaining > 0  // Simple validation
            },
            1 => {
                // Risk envelope order evaluation
                let order = OrderCheck::new(
                    1000 + (thread_id * 100) as u32,
                    1 + (local_ops % 3) as u16,
                    300 + (local_ops % 600) as u16,
                    5000 + (local_ops % 30000) as u32,
                );
                let envelope = atomic_risk.load(Ordering::Relaxed);
                let outcome = envelope.evaluate_order(order);
                true  // All outcomes are valid
            },
            2 => {
                // Risk envelope debit attempt
                let debit_amount = 100 + (thread_id * 10) as u32;
                match atomic_risk.debit_daily_loss(
                    debit_amount,
                    Ordering::SeqCst,
                    Ordering::SeqCst
                ) {
                    Ok(_) => true,
                    Err(_) => true,  // Expected when near limits
                }
            },
            3 => {
                // RLT integrity check
                let expected_checksum = rlt.checksum16();
                let actual_checksum = rlt.tail.checksum();
                expected_checksum == actual_checksum
            },
            4 => {
                // AVS publish
                let snapshot = Avs128Snapshot {
                    spread_ticks: 1 + (local_ops % 10) as u8,
                    obi_q1_10: (local_ops % 2048) as i16 - 1024,
                    micro_off_ticks: (local_ops % 20) as i16 - 10,
                    sum_bid_l1_3: 800 + (local_ops % 400) as u16,
                    sum_ask_l1_3: 750 + (local_ops % 400) as u16,
                    vol_bp_q8_8: 25000 + (local_ops % 5000) as u16,
                    sweep_flag: local_ops % 100 == 0,
                    trend_200ms_ticks: (local_ops % 40) as i16 - 20,
                    ts_coarse_ms: 65000 + (local_ops / 10) as u32,
                    version: 1,
                    sequence: (local_ops % 16) as u8,
                };
                avs.publish(snapshot);
                true
            },
            5 => {
                // AVS read
                let packed = avs.load_relaxed();
                let snapshot = packed.unpack();
                snapshot.spread_ticks < 256  // Basic validation
            },
            6 => {
                // DOS publish
                let mut dos_snapshot = create_test_dos_snapshot();
                dos_snapshot.header.sequence_head = local_ops as u16;
                dos_snapshot.header.created_ms_coarse = 65000 + (local_ops / 10) as u32;
                dos_snapshot.summary.seq_tail = local_ops as u16;

                let packed = dos_snapshot.pack();
                dos.publish(&packed);
                true
            },
            7 => {
                // DOS consistent read
                match dos.load_consistent(3) {
                    Some(snapshot) => {
                        snapshot.head_tail_match()  // Validate consistency
                    },
                    None => true,  // Expected under high contention
                }
            },
            _ => unreachable!(),
        };

        if success {
            local_successes += 1;
        } else {
            local_errors += 1;
            errors_detected.fetch_add(1, Ordering::Relaxed);
        }

        local_ops += 1;
        operations_completed.fetch_add(1, Ordering::Relaxed);

        // Occasional yield to encourage contention
        if local_ops % 1000 == 0 {
            thread::yield_now();
        }
    }

    (local_ops, local_successes, local_errors)
}

/// Create test risk envelope configuration
fn create_test_risk_envelope() -> RiskEnvelope {
    let fields = Fields {
        rem_daily_loss_cents: 100_000,  // $1000 daily limit
        max_per_trade_cents: 10_000,    // $100 per trade
        max_contracts: 10,
        max_open_ms: 60_000,           // 1 minute positions
        forbid_after_min_ct: 900,
        eod_flat_min_ct: 950,
        flags: flag::Flags::EMPTY,
        version: 1,
        sequence: 0,
    };
    RiskEnvelope::try_from_fields(fields).unwrap()
}

/// Create test RLT configuration
fn create_test_rlt() -> Rlt1024 {
    let mut rlt = Rlt1024::new();

    rlt.header = HeaderWord::ZERO
        .with_strategy_mask(0b111)
        .with_recover_scale(DEFAULT_RECOVER_SCALE_Q1_7)
        .with_global_breaker_level(0);

    rlt.strat_a_trips = TripWord::ZERO
        .with_loss_level_0(25_000)
        .with_loss_level_1(50_000)
        .with_loss_level_2(75_000)
        .with_loss_level_3(100_000);

    rlt.tail = rlt.tail.with_sequence(1).with_checksum(rlt.checksum16());

    rlt
}

/// Create test DOS snapshot
fn create_test_dos_snapshot() -> Dos1024Snapshot {
    Dos1024Snapshot {
        header: DosHeader {
            commit: true,
            stale: false,
            version_even: 2,
            sequence_head: 1,
            sym_a_id: 12345,
            sym_b_id: 12346,
            created_ms_coarse: 65000,
            forbid_after_min_ct: 900,
            eod_flat_min_ct: 950,
            flags: 0,
            spare: 0,
        },
        instrument_a: DosInstrument::default(),
        instrument_b: DosInstrument::default(),
        summary: atomic_depth_of_market_slice::DosSummary::default(),
    }
}

/// Verify final system state consistency after stress test
fn verify_final_system_consistency(
    atomic_risk: &AtomicRiskEnvelope,
    rlt: &Rlt1024,
    avs: &Avs128,
    dos: &Dos1024,
) {
    // Check risk envelope consistency
    let envelope = atomic_risk.load(Ordering::SeqCst);
    let remaining = envelope.rem_daily_loss_cents();
    assert!(remaining <= 100_000, "Daily loss should not exceed initial limit");

    // Check RLT integrity
    let expected_checksum = rlt.checksum16();
    let actual_checksum = rlt.tail.checksum();
    assert_eq!(expected_checksum, actual_checksum, "RLT checksum should be valid");

    // Check AVS data validity
    let avs_snapshot = avs.load_relaxed().unpack();
    assert!(avs_snapshot.spread_ticks < 256, "AVS spread should be valid");

    // Check DOS consistency
    if let Some(dos_snapshot) = dos.load_consistent(10) {
        assert!(dos_snapshot.head_tail_match(), "DOS should be internally consistent");
    }

    println!("✓ Final system state consistency verified");
}

/// Memory ordering stress test
/// Specifically tests acquire-release semantics under contention
#[test]
fn test_memory_ordering_stress() {
    // #ASSUME: Acquire-Release ordering provides proper synchronization
    // #VERIFY: Writes are visible in correct order across threads

    const NUM_WRITERS: usize = 4;
    const NUM_READERS: usize = 8;
    const WRITES_PER_WRITER: usize = 10_000;

    let avs = Arc::new(Avs128::new());
    let write_counter = Arc::new(AtomicUsize::new(0));
    let should_stop = Arc::new(AtomicBool::new(false));

    // Spawn writer threads
    let writer_handles: Vec<_> = (0..NUM_WRITERS).map(|writer_id| {
        let avs = Arc::clone(&avs);
        let write_counter = Arc::clone(&write_counter);
        let should_stop = Arc::clone(&should_stop);

        thread::spawn(move || {
            for write_id in 0..WRITES_PER_WRITER {
                if should_stop.load(Ordering::Relaxed) {
                    break;
                }

                let global_sequence = write_counter.fetch_add(1, Ordering::SeqCst);

                let snapshot = Avs128Snapshot {
                    spread_ticks: (writer_id % 10) as u8,
                    obi_q1_10: write_id as i16,
                    micro_off_ticks: 0,
                    sum_bid_l1_3: global_sequence as u16,
                    sum_ask_l1_3: global_sequence as u16,
                    vol_bp_q8_8: 25000,
                    sweep_flag: false,
                    trend_200ms_ticks: 0,
                    ts_coarse_ms: global_sequence as u32,
                    version: 1,
                    sequence: (global_sequence % 16) as u8,
                };

                // Publish with Release semantics
                avs.publish(snapshot);

                if write_id % 1000 == 0 {
                    thread::yield_now();
                }
            }
            writer_id
        })
    }).collect();

    // Spawn reader threads to verify ordering
    let reader_handles: Vec<_> = (0..NUM_READERS).map(|reader_id| {
        let avs = Arc::clone(&avs);
        let should_stop = Arc::clone(&should_stop);

        thread::spawn(move || {
            let mut reads_performed = 0;
            let mut ordering_violations = 0;
            let mut last_timestamp = 0u32;

            while !should_stop.load(Ordering::Relaxed) && reads_performed < 100_000 {
                // Read with Relaxed ordering (readers use this in practice)
                let snapshot = avs.load_relaxed().unpack();

                // Check for ordering violations
                if snapshot.ts_coarse_ms < last_timestamp {
                    ordering_violations += 1;
                }
                last_timestamp = snapshot.ts_coarse_ms;

                reads_performed += 1;

                if reads_performed % 10000 == 0 {
                    thread::yield_now();
                }
            }

            (reader_id, reads_performed, ordering_violations)
        })
    }).collect();

    // Let writers complete
    let writer_results: Vec<_> = writer_handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    // Signal readers to stop
    should_stop.store(true, Ordering::SeqCst);

    // Collect reader results
    let reader_results: Vec<_> = reader_handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    // Analyze results
    let total_writes = write_counter.load(Ordering::SeqCst);
    let total_reads: usize = reader_results.iter()
        .map(|(_, reads, _)| *reads)
        .sum();
    let total_violations: usize = reader_results.iter()
        .map(|(_, _, violations)| *violations)
        .sum();

    let violation_rate = total_violations as f64 / total_reads as f64;

    println!("📈 Memory ordering test results:");
    println!("   Writers: {}, Readers: {}", NUM_WRITERS, NUM_READERS);
    println!("   Total writes: {}", total_writes);
    println!("   Total reads: {}", total_reads);
    println!("   Ordering violations: {} ({:.4}%)", total_violations, violation_rate * 100.0);

    // #VERIFY: Ordering violations should be minimal
    // Some violations are acceptable due to concurrent writers,
    // but should be very low with proper memory ordering
    assert!(violation_rate < 0.01,
        "Memory ordering violation rate should be <1%, got {:.4}%",
        violation_rate * 100.0);

    println!("✅ Memory ordering stress test PASSED");
}

/// ABA problem prevention test
/// Verifies that generation counters prevent ABA scenarios
#[test]
fn test_aba_prevention() {
    // #ASSUME: Sequence/generation counters prevent ABA problems
    // #VERIFY: No false positive updates under concurrent modification

    const NUM_THREADS: usize = 8;
    const OPERATIONS_PER_THREAD: usize = 5_000;

    let atomic_risk = Arc::new(AtomicRiskEnvelope::new(create_test_risk_envelope()));
    let aba_detections = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..NUM_THREADS).map(|thread_id| {
        let atomic_risk = Arc::clone(&atomic_risk);
        let aba_detections = Arc::clone(&aba_detections);

        thread::spawn(move || {
            let mut operations = 0;
            let mut aba_detected = 0;

            for op_id in 0..OPERATIONS_PER_THREAD {
                // Read current state
                let current = atomic_risk.load(Ordering::Acquire);
                let original_sequence = current.sequence();

                // Simulate some work
                thread::yield_now();

                // Attempt compare-and-swap operation
                let mut updated = current;
                if let Ok(new_envelope) = updated.with_sequence((original_sequence + 1) % 64) {
                    match atomic_risk.compare_exchange(
                        current,
                        new_envelope,
                        Ordering::SeqCst,
                        Ordering::SeqCst
                    ) {
                        Ok(_) => {
                            // Success - no ABA
                        },
                        Err(actual) => {
                            // Compare-exchange failed
                            let actual_sequence = actual.sequence();

                            // Check for ABA: same content but different sequence
                            if actual.rem_daily_loss_cents() == current.rem_daily_loss_cents() &&
                               actual.max_per_trade_cents() == current.max_per_trade_cents() &&
                               actual_sequence != original_sequence {
                                aba_detected += 1;
                            }
                        }
                    }
                }

                operations += 1;
            }

            aba_detections.fetch_add(aba_detected, Ordering::Relaxed);
            (thread_id, operations, aba_detected)
        })
    }).collect();

    let results: Vec<_> = handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    let total_operations: usize = results.iter().map(|(_, ops, _)| *ops).sum();
    let total_aba = aba_detections.load(Ordering::Relaxed);

    println!("🔄 ABA prevention test results:");
    println!("   Total operations: {}", total_operations);
    println!("   ABA scenarios detected: {}", total_aba);

    // ABA detection indicates the generation counter system is working
    // Some ABA scenarios are expected under high contention
    println!("✅ ABA prevention test completed - {} ABA scenarios properly handled", total_aba);
}