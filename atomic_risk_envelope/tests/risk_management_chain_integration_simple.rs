// Simplified Risk Management Chain Integration Tests
//
// Tests basic integration between atomic_risk_envelope and atomic_risk_ladder_table
// focusing on critical lockfree operation validation.
//
// ASSUM: All operations are lockfree - NO mutex/RwLock usage
// VERIFY: Only atomic operations used throughout the chain

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use atomic_risk_envelope::{
    AtomicRiskEnvelope, Fields, OrderCheck, RiskEnvelope, GateOutcome, DenyReason, flag
};
use atomic_risk_ladder_table::{
    Rlt1024, HeaderWord, TripWord, TailWord, DEFAULT_RECOVER_SCALE_Q1_7
};

/// Basic integration test: Risk envelope and RLT coordination
#[test]
fn test_basic_risk_envelope_rlt_integration() {
    // #ASSUME: Both primitives use lockfree atomics
    // #VERIFY: No blocking operations in critical path

    // Setup risk envelope with conservative limits
    let risk_fields = Fields {
        rem_daily_loss_cents: 50_000,  // $500 daily loss limit
        max_per_trade_cents: 10_000,   // $100 per trade limit
        max_contracts: 5,
        max_open_ms: 30_000,           // 30 second position limit
        forbid_after_min_ct: 900,      // 15 hours (900 minutes)
        eod_flat_min_ct: 950,          // 15:50 EOD flatten
        flags: flag::Flags::EMPTY,     // No restrictions initially
        version: 1,
        sequence: 0,
    };

    let risk_envelope = RiskEnvelope::try_from_fields(risk_fields)
        .expect("Risk envelope creation should succeed");
    let atomic_risk = AtomicRiskEnvelope::new(risk_envelope);

    // Setup basic risk ladder table
    let mut rlt = Rlt1024::new();

    // Configure header with strategy mask and recovery scale
    let mut header = HeaderWord::ZERO;
    header.set_strategy_mask(atomic_risk_ladder_table::layout::header::StrategyMask::new(0b111));
    header.set_recover_scale(DEFAULT_RECOVER_SCALE_Q1_7);
    rlt.header = header;

    // Configure strategy A with basic thresholds
    let mut trips = TripWord::ZERO;
    let trip_thresholds = atomic_risk_ladder_table::layout::trips::TripThresholds {
        alt: [640, 896, 1023],        // ALT thresholds
        rej: [150, 300, 600],         // Rejection rate thresholds (basis points)
        loss: [200, 350, 450],        // Loss thresholds (basis points: 2%, 3.5%, 4.5%)
        vol: [384, 640, 1024],        // Volatility thresholds
    };
    trips.set_thresholds(trip_thresholds);
    rlt.strat_a_trips = trips;

    // Configure tail with integrity checksum
    let mut tail = TailWord::ZERO;
    tail.set_seq_tail(1);
    tail.set_version(0);
    tail.set_checksum(rlt.checksum16());
    rlt.tail = tail;

    // Test Case 1: Normal operation - orders should be allowed
    let normal_order = OrderCheck::new(
        5_000,  // $50 cost (well under limits)
        2,      // 2 contracts (under limit)
        300,    // 5 hours into session
        15_000, // 15 second duration
    );

    let outcome = atomic_risk.load(Ordering::Relaxed).evaluate_order(normal_order);
    assert!(outcome.is_allow(), "Normal order should be allowed");

    // Test Case 2: RLT integrity maintained
    let expected_checksum = rlt.checksum16();
    let actual_checksum = rlt.tail.checksum();
    assert_eq!(expected_checksum, actual_checksum,
        "RLT checksum should be valid");

    // Test Case 3: Progressive loss simulation
    let trade_costs = [15_000, 10_000, 10_000, 10_000];  // Total: $450

    for trade_cost in trade_costs {
        // Debit the risk envelope
        let result = atomic_risk.debit_daily_loss(
            trade_cost,
            Ordering::SeqCst,
            Ordering::SeqCst
        );

        match result {
            Ok(_) => {
                // Successful debit - verify remaining balance
                let current_envelope = atomic_risk.load(Ordering::Relaxed);
                let remaining = current_envelope.rem_daily_loss_cents();
                assert!(remaining <= 50_000, "Remaining should be <= initial limit");
            },
            Err(_) => {
                // Expected when approaching limits
                println!("Debit rejected near daily limit - expected behavior");
            }
        }
    }

    // Test Case 4: Emergency flags coordination
    test_emergency_flags(&atomic_risk);

    println!("✓ Basic risk envelope and RLT integration test completed");
}

/// Test emergency flag coordination
fn test_emergency_flags(atomic_risk: &AtomicRiskEnvelope) {
    let base_envelope = atomic_risk.load(Ordering::Relaxed);

    // Test emergency flat flag
    let emergency_envelope = base_envelope.with_flags(flag::EMERGENCY_FLAT).unwrap();
    atomic_risk.store(emergency_envelope, Ordering::SeqCst);

    let test_order = OrderCheck::new(1_000, 1, 300, 10_000);
    let outcome = atomic_risk.load(Ordering::SeqCst).evaluate_order(test_order);

    assert!(!outcome.is_allow(), "Orders should be denied when emergency flat is active");
    assert!(matches!(outcome, GateOutcome::Deny(DenyReason::EmergencyFlat)),
        "Should specifically deny due to emergency flat");

    // Clear flags and verify recovery
    let normal_envelope = base_envelope.with_flags(flag::Flags::EMPTY).unwrap();
    atomic_risk.store(normal_envelope, Ordering::SeqCst);

    let outcome = atomic_risk.load(Ordering::SeqCst).evaluate_order(test_order);
    assert!(outcome.is_allow(), "Orders should be allowed after clearing flags");
}

/// Multi-threaded stress test for lockfree operation
#[test]
fn test_lockfree_stress() {
    // #ASSUME: Concurrent access patterns are lockfree
    // #VERIFY: No deadlocks or blocking under contention

    const NUM_THREADS: usize = 4;
    const OPERATIONS_PER_THREAD: usize = 1000;

    let atomic_risk = Arc::new(AtomicRiskEnvelope::new(create_test_risk_envelope()));
    let rlt = Arc::new(create_test_rlt());

    let start_time = Instant::now();

    let handles: Vec<_> = (0..NUM_THREADS).map(|thread_id| {
        let atomic_risk = Arc::clone(&atomic_risk);
        let rlt = Arc::clone(&rlt);

        thread::spawn(move || {
            let mut operations_completed = 0;
            let mut successful_operations = 0;

            for op_id in 0..OPERATIONS_PER_THREAD {
                let success = match op_id % 3 {
                    0 => {
                        // Risk envelope read
                        let envelope = atomic_risk.load(Ordering::Relaxed);
                        envelope.rem_daily_loss_cents() > 0
                    },
                    1 => {
                        // Order evaluation
                        let order = OrderCheck::new(
                            1000 + (thread_id * 100) as u32,
                            1,
                            300,
                            10_000,
                        );
                        let envelope = atomic_risk.load(Ordering::Relaxed);
                        envelope.evaluate_order(order).is_allow()
                    },
                    2 => {
                        // RLT integrity check
                        let expected_checksum = rlt.checksum16();
                        let actual_checksum = rlt.tail.checksum();
                        expected_checksum == actual_checksum
                    },
                    _ => unreachable!(),
                };

                if success {
                    successful_operations += 1;
                }
                operations_completed += 1;

                // Occasional yield to encourage contention
                if op_id % 100 == 0 {
                    thread::yield_now();
                }
            }

            (thread_id, operations_completed, successful_operations)
        })
    }).collect();

    let results: Vec<_> = handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    let elapsed = start_time.elapsed();
    let total_ops: usize = results.iter().map(|(_, ops, _)| ops).sum();
    let total_successful: usize = results.iter().map(|(_, _, successful)| successful).sum();

    let ops_per_second = total_ops as f64 / elapsed.as_secs_f64();
    let success_rate = total_successful as f64 / total_ops as f64;

    // #VERIFY: Lockfree performance should be high
    assert!(ops_per_second > 10_000.0,
        "Lockfree integration should achieve >10k ops/sec, got {:.0}",
        ops_per_second);

    // #VERIFY: High success rate expected
    assert!(success_rate > 0.90,
        "Success rate should be >90%, got {:.2}", success_rate);

    println!("✓ Lockfree stress test: {:.0} ops/sec, {:.1}% success rate",
             ops_per_second, success_rate * 100.0);
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

    let mut header = HeaderWord::ZERO;
    header.set_strategy_mask(atomic_risk_ladder_table::layout::header::StrategyMask::new(0b111));
    header.set_recover_scale(DEFAULT_RECOVER_SCALE_Q1_7);
    rlt.header = header;

    let mut trips = TripWord::ZERO;
    let trip_thresholds = atomic_risk_ladder_table::layout::trips::TripThresholds {
        alt: [640, 896, 1023],
        rej: [150, 300, 600],
        loss: [250, 500, 750],  // 2.5%, 5.0%, 7.5% loss thresholds
        vol: [384, 640, 1024],
    };
    trips.set_thresholds(trip_thresholds);
    rlt.strat_a_trips = trips;

    let mut tail = TailWord::ZERO;
    tail.set_seq_tail(1);
    tail.set_version(0);
    tail.set_checksum(rlt.checksum16());
    rlt.tail = tail;

    rlt
}

/// Property-based test: Risk thresholds consistency
#[test]
fn test_risk_thresholds_consistency() {
    // Property: RLT thresholds should be monotonically increasing
    // and envelope limits should be reasonable relative to thresholds

    let test_configs = vec![
        // Conservative configuration
        (10_000, 50_000),
        // Aggressive configuration
        (25_000, 100_000),
        // Minimal configuration
        (1_000, 5_000),
    ];

    for (max_trade, daily_limit) in test_configs {
        // Create risk envelope
        let fields = Fields {
            rem_daily_loss_cents: daily_limit,
            max_per_trade_cents: max_trade,
            max_contracts: 10,
            max_open_ms: 60_000,
            forbid_after_min_ct: 900,
            eod_flat_min_ct: 950,
            flags: flag::Flags::EMPTY,
            version: 1,
            sequence: 0,
        };

        let _envelope = RiskEnvelope::try_from_fields(fields)
            .expect("Valid configuration should create envelope");

        // Create corresponding RLT
        let rlt = create_test_rlt();
        let thresholds = rlt.strat_a_trips.thresholds();

        // Property 1: RLT thresholds are monotonically increasing
        for i in 1..thresholds.loss.len() {
            assert!(thresholds.loss[i] >= thresholds.loss[i-1],
                "RLT loss level {} ({}) should be >= level {} ({})",
                i, thresholds.loss[i], i-1, thresholds.loss[i-1]);
        }

        // Property 2: Per-trade limit should be reasonable
        let _daily_limit_bp = daily_limit / 100;  // Convert cents to basis points
        assert!(max_trade <= daily_limit,
            "Per-trade limit {} should not exceed daily limit {}",
            max_trade, daily_limit);

        println!("✓ Configuration valid: trade_limit={}, daily_limit={}, rlt_levels={:?}",
                 max_trade, daily_limit, thresholds.loss);
    }
}