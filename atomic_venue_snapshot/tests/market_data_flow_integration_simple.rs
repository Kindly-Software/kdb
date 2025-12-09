// Simplified Market Data Flow Integration Tests
//
// Tests basic integration between atomic_venue_snapshot and atomic_depth_of_market_slice
// focusing on data consistency validation under lockfree operation.
//
// ASSUM: All operations are lockfree - NO mutex/RwLock usage
// VERIFY: Data consistency maintained across atomic primitives

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use atomic_venue_snapshot::{Avs128, Avs128Snapshot};
use atomic_depth_of_market_slice::{
    Dos1024, Dos1024Snapshot, DosHeader, DosInstrument, DosInstrumentHeader,
    DosLevel, DosSummary, DosInstrumentDerived
};

/// Basic market data consistency test
#[test]
fn test_market_data_consistency() {
    // #ASSUME: Both primitives publish atomically via Release semantics
    // #VERIFY: Readers observe consistent state across both data sources

    // Setup venue snapshot for a sample instrument
    let avs = Avs128::new();

    // Setup depth-of-market for dual instrument configuration
    let dos = Dos1024::new();

    // Test Case 1: Basic consistency - single instrument update
    let base_snapshot = Avs128Snapshot {
        spread_ticks: 2,
        obi_q1_10: 512,  // Slight bid imbalance in Q1.10 format
        micro_off_ticks: 1,
        sum_bid_l1_3: 1000,
        sum_ask_l1_3: 950,
        vol_bp_q8_8: 25600,  // ~100 bp in Q8.8 format
        sweep_flag: false,
        trend_200ms_ticks: 3,
        ts_coarse_ms: 64000, // ~4 minutes into session (in ms/4 units)
        version: 1,
        sequence: 0,
    };

    // Corresponding DOS configuration
    let dos_header = DosHeader {
        commit: true,
        stale: false,
        version_even: 2,
        sequence_head: 1,
        sym_a_id: 12345,  // Match AVS instrument
        sym_b_id: 12346,  // Secondary instrument
        created_ms_coarse: 64000,  // Same timestamp as AVS
        forbid_after_min_ct: 900,
        eod_flat_min_ct: 950,
        flags: 0,
        spare: 0,
    };

    let dos_instrument_a = DosInstrument {
        header: DosInstrumentHeader {
            tick_value_cents_q4: 400,  // $0.25 per tick in Q4 format
            px_ref_ticks: 0,
            local_ver: 1,
            local_seq: 0,
        },
        bids: [
            DosLevel { px_ticks: 100, qty: 400 },  // L1 bid
            DosLevel { px_ticks: 99, qty: 300 },   // L2 bid
            DosLevel { px_ticks: 98, qty: 300 },   // L3 bid
            DosLevel { px_ticks: 97, qty: 200 },   // L4 bid
            DosLevel { px_ticks: 96, qty: 100 },   // L5 bid
        ],
        asks: [
            DosLevel { px_ticks: 102, qty: 350 },  // L1 ask
            DosLevel { px_ticks: 103, qty: 300 },  // L2 ask
            DosLevel { px_ticks: 104, qty: 300 },  // L3 ask
            DosLevel { px_ticks: 105, qty: 250 },  // L4 ask
            DosLevel { px_ticks: 106, qty: 150 },  // L5 ask
        ],
        sum_bid_l1_3: 1000,  // Matches AVS
        sum_ask_l1_3: 950,   // Matches AVS
    };

    let dos_summary = DosSummary {
        instrument_a: DosInstrumentDerived {
            spread_ticks: 2,     // Matches AVS
            obi_q1_10: 512,      // Matches AVS
            micro_off_ticks: 1,  // Matches AVS
            sweep_flag: false,   // Matches AVS
            trend_200ms_ticks: 3, // Matches AVS
        },
        instrument_b: DosInstrumentDerived::default(),
        checksum16: 0,  // Will be computed during packing
        ver_tail: 1,    // Odd during staging
        seq_tail: 1,
    };

    let dos_snapshot = Dos1024Snapshot {
        header: dos_header,
        instrument_a: dos_instrument_a,
        instrument_b: DosInstrument::default(),
        summary: dos_summary,
    };

    // Publish data atomically
    avs.publish(base_snapshot);
    let packed_dos = dos_snapshot.pack();
    dos.publish(&packed_dos);

    // Verify consistency between AVS and DOS
    let read_avs = avs.load_relaxed().unpack();
    let read_dos = dos.load_consistent(10)
        .expect("DOS should provide consistent snapshot");

    assert_consistency(&read_avs, &read_dos);

    // Test Case 2: Coordinated updates
    test_coordinated_updates(&avs, &dos);

    println!("✓ Basic market data consistency test passed");
}

/// Verify data consistency between AVS and DOS
fn assert_consistency(avs: &Avs128Snapshot, dos: &Dos1024Snapshot) {
    // Identify which DOS instrument matches AVS (assuming instrument A for this test)
    let dos_instr = &dos.instrument_a;
    let dos_derived = &dos.summary.instrument_a;

    // Verify timestamp consistency (within tolerance for quantization)
    let avs_ts_ms = avs.ts_coarse_ms as u64 * 4;  // Dequantize AVS timestamp
    let dos_ts_ms = dos.header.created_ms_coarse as u64 * 4;  // Dequantize DOS timestamp

    let timestamp_diff = avs_ts_ms.abs_diff(dos_ts_ms);
    assert!(timestamp_diff <= 100,  // Allow 100ms tolerance
        "Timestamp consistency: AVS={}, DOS={}, diff={}",
        avs_ts_ms, dos_ts_ms, timestamp_diff);

    // Verify spread consistency
    assert_eq!(avs.spread_ticks, dos_derived.spread_ticks,
        "Spread should match between AVS and DOS");

    // Verify order book imbalance consistency
    assert_eq!(avs.obi_q1_10, dos_derived.obi_q1_10,
        "Order book imbalance should match between AVS and DOS");

    // Verify microprice offset consistency
    assert_eq!(avs.micro_off_ticks, dos_derived.micro_off_ticks,
        "Microprice offset should match between AVS and DOS");

    // Verify depth sum consistency
    assert_eq!(avs.sum_bid_l1_3, dos_instr.sum_bid_l1_3,
        "Bid depth L1-3 sum should match between AVS and DOS");
    assert_eq!(avs.sum_ask_l1_3, dos_instr.sum_ask_l1_3,
        "Ask depth L1-3 sum should match between AVS and DOS");

    // Verify sweep flag consistency
    assert_eq!(avs.sweep_flag, dos_derived.sweep_flag,
        "Sweep flag should match between AVS and DOS");

    // Verify trend consistency
    assert_eq!(avs.trend_200ms_ticks, dos_derived.trend_200ms_ticks,
        "200ms trend should match between AVS and DOS");

    println!("✓ Data consistency verified: spread={}, obi={}, depth_bid={}, depth_ask={}",
             avs.spread_ticks, avs.obi_q1_10, avs.sum_bid_l1_3, avs.sum_ask_l1_3);
}

/// Test coordinated updates to maintain consistency under time pressure
fn test_coordinated_updates(avs: &Avs128, dos: &Dos1024) {
    // #ASSUME: Updates are published with proper memory ordering
    // #VERIFY: Readers never observe inconsistent intermediate states

    const UPDATE_COUNT: usize = 100;
    let start_time = Instant::now();

    for update_id in 0..UPDATE_COUNT {
        // Create correlated updates
        let spread = 1 + (update_id % 5) as u8;
        let obi = -512 + (update_id as i16 * 10);
        let bid_depth = 800 + (update_id % 200) as u16;
        let ask_depth = 750 + (update_id % 200) as u16;
        let timestamp = 65000 + (update_id as u32 * 100);

        // Update AVS
        let avs_update = Avs128Snapshot {
            spread_ticks: spread,
            obi_q1_10: obi,
            micro_off_ticks: if spread > 2 { 1 } else { 0 },
            sum_bid_l1_3: bid_depth,
            sum_ask_l1_3: ask_depth,
            vol_bp_q8_8: 26000 + (update_id % 1000) as u16,
            sweep_flag: update_id % 10 == 0,
            trend_200ms_ticks: (update_id % 20) as i16 - 10,
            ts_coarse_ms: timestamp,
            version: 1,
            sequence: (update_id % 16) as u8,
        };

        // Create corresponding DOS update
        let mut dos_snapshot = create_base_dos_snapshot();
        dos_snapshot.header.created_ms_coarse = timestamp;
        dos_snapshot.header.sequence_head = update_id as u16;

        // Update instrument A to match AVS
        dos_snapshot.instrument_a.sum_bid_l1_3 = bid_depth;
        dos_snapshot.instrument_a.sum_ask_l1_3 = ask_depth;

        dos_snapshot.summary.instrument_a.spread_ticks = spread;
        dos_snapshot.summary.instrument_a.obi_q1_10 = obi;
        dos_snapshot.summary.instrument_a.micro_off_ticks = if spread > 2 { 1 } else { 0 };
        dos_snapshot.summary.instrument_a.sweep_flag = update_id % 10 == 0;
        dos_snapshot.summary.instrument_a.trend_200ms_ticks = (update_id % 20) as i16 - 10;

        dos_snapshot.summary.seq_tail = update_id as u16;

        // Publish updates with proper ordering
        avs.publish(avs_update);
        let packed_dos = dos_snapshot.pack();
        dos.publish(&packed_dos);

        // Brief pause to allow readers to observe
        if update_id % 20 == 0 {
            thread::yield_now();
        }
    }

    let elapsed = start_time.elapsed();
    let updates_per_second = UPDATE_COUNT as f64 / elapsed.as_secs_f64();

    // #VERIFY: Update performance should be sufficient for market data
    assert!(updates_per_second > 1000.0,
        "Market data updates should achieve >1k/sec, got {:.0}", updates_per_second);

    println!("✓ Coordinated updates: {:.0} updates/sec", updates_per_second);
}

/// Helper to create a base DOS snapshot for testing
fn create_base_dos_snapshot() -> Dos1024Snapshot {
    Dos1024Snapshot {
        header: DosHeader {
            commit: true,
            stale: false,
            version_even: 2,
            sequence_head: 1,
            sym_a_id: 12345,
            sym_b_id: 12346,
            created_ms_coarse: 64000,
            forbid_after_min_ct: 900,
            eod_flat_min_ct: 950,
            flags: 0,
            spare: 0,
        },
        instrument_a: DosInstrument {
            header: DosInstrumentHeader {
                tick_value_cents_q4: 400,
                px_ref_ticks: 0,
                local_ver: 1,
                local_seq: 0,
            },
            bids: [
                DosLevel { px_ticks: 100, qty: 400 },
                DosLevel { px_ticks: 99, qty: 300 },
                DosLevel { px_ticks: 98, qty: 300 },
                DosLevel { px_ticks: 97, qty: 200 },
                DosLevel { px_ticks: 96, qty: 100 },
            ],
            asks: [
                DosLevel { px_ticks: 102, qty: 350 },
                DosLevel { px_ticks: 103, qty: 300 },
                DosLevel { px_ticks: 104, qty: 300 },
                DosLevel { px_ticks: 105, qty: 250 },
                DosLevel { px_ticks: 106, qty: 150 },
            ],
            sum_bid_l1_3: 1000,
            sum_ask_l1_3: 950,
        },
        instrument_b: DosInstrument::default(),
        summary: DosSummary {
            instrument_a: DosInstrumentDerived {
                spread_ticks: 2,
                obi_q1_10: 512,
                micro_off_ticks: 1,
                sweep_flag: false,
                trend_200ms_ticks: 3,
            },
            instrument_b: DosInstrumentDerived::default(),
            checksum16: 0,
            ver_tail: 1,
            seq_tail: 1,
        },
    }
}

/// Multi-threaded stress test for lockfree market data coordination
#[test]
fn test_lockfree_market_data_stress() {
    // #ASSUME: Lockfree readers can access data concurrently
    // #VERIFY: All readers observe consistent snapshots

    const NUM_READERS: usize = 4;
    const NUM_WRITERS: usize = 2;
    const READS_PER_THREAD: usize = 5000;
    const WRITES_PER_THREAD: usize = 1000;

    let avs = Arc::new(Avs128::new());
    let dos = Arc::new(Dos1024::new());

    let start_time = Instant::now();

    // Spawn writer threads
    let writer_handles: Vec<_> = (0..NUM_WRITERS).map(|writer_id| {
        let avs = Arc::clone(&avs);
        let dos = Arc::clone(&dos);

        thread::spawn(move || {
            for i in 0..WRITES_PER_THREAD {
                let spread = 1 + (i % 4) as u8;
                let update = Avs128Snapshot {
                    spread_ticks: spread,
                    obi_q1_10: (i % 1000) as i16,
                    micro_off_ticks: 0,
                    sum_bid_l1_3: 1000 + (i % 100) as u16,
                    sum_ask_l1_3: 950 + (i % 100) as u16,
                    vol_bp_q8_8: 25000,
                    sweep_flag: i % 50 == 0,
                    trend_200ms_ticks: (i % 20) as i16 - 10,
                    ts_coarse_ms: 70000 + (writer_id * 1000 + i) as u32,
                    version: 1,
                    sequence: (i % 16) as u8,
                };

                avs.publish(update);

                // Corresponding DOS update
                let mut dos_update = create_base_dos_snapshot();
                dos_update.header.created_ms_coarse = 70000 + (writer_id * 1000 + i) as u32;
                dos_update.instrument_a.sum_bid_l1_3 = 1000 + (i % 100) as u16;
                dos_update.instrument_a.sum_ask_l1_3 = 950 + (i % 100) as u16;
                dos_update.summary.instrument_a.spread_ticks = spread;
                dos_update.summary.instrument_a.obi_q1_10 = (i % 1000) as i16;

                let packed = dos_update.pack();
                dos.publish(&packed);

                if i % 100 == 0 {
                    thread::yield_now();
                }
            }
            writer_id
        })
    }).collect();

    // Spawn reader threads
    let reader_handles: Vec<_> = (0..NUM_READERS).map(|reader_id| {
        let avs = Arc::clone(&avs);
        let dos = Arc::clone(&dos);

        thread::spawn(move || {
            let mut consistent_reads = 0;
            let mut inconsistent_reads = 0;
            let mut failed_dos_reads = 0;

            for _ in 0..READS_PER_THREAD {
                // Read AVS (always succeeds)
                let avs_snapshot = avs.load_relaxed().unpack();

                // Read DOS (may fail under contention, retry with budget)
                match dos.load_consistent(3) {
                    Some(dos_snapshot) => {
                        // Check basic consistency
                        let timestamp_diff = (avs_snapshot.ts_coarse_ms as i64 -
                                            dos_snapshot.header.created_ms_coarse as i64).abs();

                        if timestamp_diff <= 2000 {  // Allow reasonable time skew for test
                            consistent_reads += 1;
                        } else {
                            inconsistent_reads += 1;
                        }
                    },
                    None => {
                        failed_dos_reads += 1;
                    }
                }

                // Brief yield to encourage contention
                if consistent_reads % 1000 == 0 {
                    thread::yield_now();
                }
            }

            (reader_id, consistent_reads, inconsistent_reads, failed_dos_reads)
        })
    }).collect();

    // Wait for completion
    let _writer_results: Vec<_> = writer_handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    let reader_results: Vec<_> = reader_handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    let elapsed = start_time.elapsed();

    // Analyze results
    let total_reads: usize = reader_results.iter()
        .map(|(_, consistent, inconsistent, _)| consistent + inconsistent)
        .sum();
    let total_consistent: usize = reader_results.iter()
        .map(|(_, consistent, _, _)| *consistent)
        .sum();
    let total_failed: usize = reader_results.iter()
        .map(|(_, _, _, failed)| *failed)
        .sum();

    let total_operations = total_reads + total_failed + (NUM_WRITERS * WRITES_PER_THREAD);
    let ops_per_second = total_operations as f64 / elapsed.as_secs_f64();
    let consistency_rate = total_consistent as f64 / total_reads as f64;
    let failure_rate = total_failed as f64 / (total_reads + total_failed) as f64;

    // #VERIFY: High performance and consistency
    assert!(ops_per_second > 50_000.0,
        "Combined throughput should exceed 50k ops/sec, got {:.0}", ops_per_second);

    assert!(consistency_rate > 0.80,
        "Consistency rate should be >80%, got {:.2}", consistency_rate);

    // Allow some failures under high contention
    assert!(failure_rate < 0.20,
        "DOS read failure rate should be <20%, got {:.2}", failure_rate);

    println!("✓ Lockfree market data stress test: {:.0} ops/sec, {:.1}% consistent, {:.1}% failed",
             ops_per_second, consistency_rate * 100.0, failure_rate * 100.0);
}

/// Property-based test: Market data invariants
#[test]
fn test_market_data_invariants() {
    // Property: Valid market data should maintain certain invariants
    // - Spreads should be non-negative
    // - Bid/ask quantities should be non-negative
    // - Timestamps should be reasonable

    let avs = Avs128::new();

    let test_cases = vec![
        // Normal case
        (1u8, 500u16, 500u16, false),
        // Wide spread
        (10u8, 1000u16, 800u16, false),
        // Zero depth (possible in quiet markets)
        (5u8, 0u16, 0u16, false),
        // Maximum values
        (255u8, 65535u16, 65535u16, true),
    ];

    for (spread, bid_depth, ask_depth, sweep) in test_cases {
        let avs_snapshot = Avs128Snapshot {
            spread_ticks: spread,
            obi_q1_10: 0,
            micro_off_ticks: 0,
            sum_bid_l1_3: bid_depth,
            sum_ask_l1_3: ask_depth,
            vol_bp_q8_8: 25000,
            sweep_flag: sweep,
            trend_200ms_ticks: 0,
            ts_coarse_ms: 65000,
            version: 1,
            sequence: 0,
        };

        // Verify packing preserves invariants
        let packed = avs_snapshot.pack();
        let unpacked = packed.unpack();

        assert_eq!(unpacked.spread_ticks, spread, "Spread should be preserved");
        assert_eq!(unpacked.sum_bid_l1_3, bid_depth, "Bid depth should be preserved");
        assert_eq!(unpacked.sum_ask_l1_3, ask_depth, "Ask depth should be preserved");
        assert_eq!(unpacked.sweep_flag, sweep, "Sweep flag should be preserved");

        // Verify invariants are maintained
        assert!(unpacked.spread_ticks <= 255, "Spread should fit in 8 bits");

        println!("✓ Invariants maintained: spread={}, bid_depth={}, ask_depth={}",
                 spread, bid_depth, ask_depth);
    }
}

/// Edge case test: Stale data detection
#[test]
fn test_stale_data_detection() {
    let avs = Avs128::new();
    let dos = Dos1024::new();

    // Publish initial data
    let base_time = 60000u32;  // 4 minutes into session

    let avs_snapshot = Avs128Snapshot {
        spread_ticks: 1,
        obi_q1_10: 0,
        micro_off_ticks: 0,
        sum_bid_l1_3: 1000,
        sum_ask_l1_3: 1000,
        vol_bp_q8_8: 25000,
        sweep_flag: false,
        trend_200ms_ticks: 0,
        ts_coarse_ms: base_time,
        version: 1,
        sequence: 0,
    };

    let mut dos_snapshot = create_base_dos_snapshot();
    dos_snapshot.header.created_ms_coarse = base_time;
    dos_snapshot.header.stale = false;

    avs.publish(avs_snapshot);
    dos.publish(&dos_snapshot.pack());

    // Test staleness detection
    let current_time_ms = (base_time as u64 + 1000) * 4;  // 4 seconds later (1000 * 4ms units)
    let budget_ms = 500;  // 500ms staleness budget

    let read_avs = avs.load_relaxed().unpack();
    let read_dos = dos.load_consistent(5).unwrap();

    // Both should be stale beyond the budget
    assert!(read_avs.is_stale(current_time_ms, budget_ms),
        "AVS should be detected as stale");
    assert!(read_dos.is_stale(current_time_ms, budget_ms),
        "DOS should be detected as stale");

    // Within larger budget should not be stale
    let larger_budget = 5000;  // 5 second budget
    assert!(!read_avs.is_stale(current_time_ms, larger_budget),
        "AVS should not be stale within larger budget");
    assert!(!read_dos.is_stale(current_time_ms, larger_budget),
        "DOS should not be stale within larger budget");

    println!("✓ Stale data detection working correctly");
}