# QEC Integration Layer - T28 Test Plan

**Phase**: Q3.6-C Specialized Surface Code Simulator - Testing Strategy
**Version**: 1.0.0
**Date**: 2025-11-21
**Framework**: T28 Comprehensive Testing (Unit, Property, Integration, Production)

---

## Table of Contents

1. [T28 Framework Overview](#t28-framework-overview)
2. [Q1-Q7: Unit Tests](#q1-q7-unit-tests)
3. [Q8-Q14: Property Tests](#q8-q14-property-tests)
4. [Q15-Q21: Integration Tests](#q15-q21-integration-tests)
5. [Q22-Q28: Production Tests](#q22-q28-production-tests)
6. [Test Infrastructure](#test-infrastructure)
7. [Coverage Analysis](#coverage-analysis)
8. [CI/CD Integration](#cicd-integration)

---

## T28 Framework Overview

### Testing Tiers

**T28 Framework**: 4 tiers × 7 questions = 28 comprehensive tests

| Tier | Questions | Focus | Coverage |
|------|-----------|-------|----------|
| **Unit** | Q1-Q7 | Component isolation | Individual functions, data structures |
| **Property** | Q8-Q14 | Invariant verification | Correctness guarantees, edge cases |
| **Integration** | Q15-Q21 | Component interaction | Full pipeline, decoder comparison |
| **Production** | Q22-Q28 | Real-world validation | Stress testing, performance, accuracy |

### Success Criteria

**Unit Tests** (Q1-Q7):
- ✅ 100% pass rate
- ✅ <1ms per test (fast feedback)
- ✅ Isolated (no external dependencies)

**Property Tests** (Q8-Q14):
- ✅ 1000+ random inputs per property
- ✅ No counterexamples found
- ✅ Shrinking to minimal failing case (if failure)

**Integration Tests** (Q15-Q21):
- ✅ Full pipeline coverage (syndrome → decode → correct)
- ✅ Decoder comparison (Union-Find vs MWPM accuracy)
- ✅ Realistic workloads (d=5, 1000 cycles)

**Production Tests** (Q22-Q28):
- ✅ 10,000+ QEC cycles sustained
- ✅ <100μs P99 latency (meets target)
- ✅ >90% logical error suppression (meets target)
- ✅ <1% overflow rate (buffer well-provisioned)

---

## Q1-Q7: Unit Tests

### Q1: Are individual components isolated?

**Test: Ring Buffer Initialization**

```rust
#[test]
fn test_ring_buffer_initialization() {
    let buffer = SyndromeRingBuffer::<256>::new();

    // Verify initial state
    assert_eq!(buffer.head.load(Ordering::Relaxed), 0);
    assert_eq!(buffer.tail.load(Ordering::Relaxed), 0);
    assert_eq!(buffer.overflow_count.load(Ordering::Relaxed), 0);

    // Verify capacity
    assert_eq!(buffer.capacity(), 256);

    // Verify power-of-two
    assert!(256_usize.is_power_of_two());
}
```

**Expected Result**: ✅ Pass (0ns runtime, compile-time verified)

---

### Q2: Do basic operations work correctly?

**Test: Ring Buffer Push/Pop**

```rust
#[test]
fn test_ring_buffer_push_pop() {
    let buffer = SyndromeRingBuffer::<256>::new();

    // Create test syndrome
    let syndrome = SyndromeEntry {
        syndrome_bits: [1, 2, 3, 4, 5, 6, 7, 8],
        syndrome_weight: 10,
        generation: 0,
        ..Default::default()
    };

    // Push syndrome
    let result = buffer.push(syndrome);
    assert!(result.is_ok());

    // Verify head advanced
    assert_eq!(buffer.head.load(Ordering::Relaxed), 1);

    // Pop syndrome
    let popped = buffer.pop();
    assert!(popped.is_some());

    let popped_syndrome = popped.unwrap();
    assert_eq!(popped_syndrome.syndrome_bits, syndrome.syndrome_bits);
    assert_eq!(popped_syndrome.syndrome_weight, 10);

    // Verify tail advanced
    assert_eq!(buffer.tail.load(Ordering::Relaxed), 1);
}
```

**Expected Result**: ✅ Pass (<100μs runtime)

---

### Q3: Does the component handle edge cases?

**Test: Ring Buffer Empty/Full**

```rust
#[test]
fn test_ring_buffer_empty() {
    let buffer = SyndromeRingBuffer::<256>::new();

    // Pop from empty buffer
    let result = buffer.pop();
    assert!(result.is_none());
}

#[test]
fn test_ring_buffer_full() {
    let buffer = SyndromeRingBuffer::<256>::new();

    // Fill buffer to capacity
    for i in 0..256 {
        let syndrome = SyndromeEntry {
            generation: i,
            ..Default::default()
        };
        buffer.push(syndrome).expect("Buffer should accept 256 entries");
    }

    // Try to push one more (should fail)
    let overflow_syndrome = SyndromeEntry::default();
    let result = buffer.push(overflow_syndrome);
    assert!(result.is_err());

    // Verify overflow counter
    assert_eq!(buffer.overflow_count.load(Ordering::Relaxed), 1);
}
```

**Expected Result**: ✅ Pass (<1ms runtime)

---

### Q4: Does decoder selection work correctly?

**Test: Adaptive Decoder Selection**

```rust
#[test]
fn test_decoder_selection_empty() {
    let config = QECConfig::default(); // d=5, threshold=12
    let capsule = QECIntegrationCapsule::new_with_config(config);

    let syndrome = SyndromeEntry {
        syndrome_weight: 0, // Empty syndrome
        ..Default::default()
    };

    let decoder = capsule.select_decoder(&syndrome);
    assert_eq!(decoder, DecoderType::None);
}

#[test]
fn test_decoder_selection_sparse() {
    let config = QECConfig::default(); // d=5, threshold=12
    let capsule = QECIntegrationCapsule::new_with_config(config);

    let syndrome = SyndromeEntry {
        syndrome_weight: 8, // Sparse syndrome (< threshold)
        ..Default::default()
    };

    let decoder = capsule.select_decoder(&syndrome);
    assert_eq!(decoder, DecoderType::UnionFind);
}

#[test]
fn test_decoder_selection_dense() {
    let config = QECConfig::default(); // d=5, threshold=12
    let capsule = QECIntegrationCapsule::new_with_config(config);

    let syndrome = SyndromeEntry {
        syndrome_weight: 18, // Dense syndrome (>= threshold)
        ..Default::default()
    };

    let decoder = capsule.select_decoder(&syndrome);
    assert_eq!(decoder, DecoderType::MWPM);
}
```

**Expected Result**: ✅ Pass (<10μs runtime per test)

---

### Q5: Does state machine enforce transitions?

**Test: State Machine Transitions**

```rust
#[test]
fn test_state_machine_idle_to_busy() {
    let capsule = QECIntegrationCapsule::new();

    // Initial state: IDLE
    assert_eq!(
        capsule.pipeline_state.decoder_state.load(Ordering::Relaxed),
        IDLE
    );

    // Transition: IDLE → UNION_FIND_BUSY
    let result = capsule.start_decoding(DecoderType::UnionFind);
    assert!(result.is_ok());
    assert_eq!(
        capsule.pipeline_state.decoder_state.load(Ordering::Relaxed),
        UNION_FIND_BUSY
    );
}

#[test]
fn test_state_machine_busy_to_idle() {
    let capsule = QECIntegrationCapsule::new();

    // Transition: IDLE → BUSY
    capsule.start_decoding(DecoderType::UnionFind).unwrap();

    // Transition: BUSY → IDLE
    let result = capsule.finish_decoding();
    assert!(result.is_ok());
    assert_eq!(
        capsule.pipeline_state.decoder_state.load(Ordering::Relaxed),
        IDLE
    );
}

#[test]
fn test_state_machine_reject_concurrent() {
    let capsule = QECIntegrationCapsule::new();

    // Transition: IDLE → UNION_FIND_BUSY
    capsule.start_decoding(DecoderType::UnionFind).unwrap();

    // Try to start MWPM (should fail, already busy)
    let result = capsule.start_decoding(DecoderType::MWPM);
    assert!(result.is_err());

    // State unchanged
    assert_eq!(
        capsule.pipeline_state.decoder_state.load(Ordering::Relaxed),
        UNION_FIND_BUSY
    );
}
```

**Expected Result**: ✅ Pass (<50μs runtime per test)

---

### Q6: Does telemetry track metrics correctly?

**Test: Telemetry Recording**

```rust
#[test]
fn test_telemetry_record_latency() {
    let telemetry = QECTelemetryCapsule::new();

    // Record syndrome latency
    telemetry.record_syndrome_latency(25_000); // 25μs

    // Verify histogram updated (exact value depends on HistogramCapsule API)
    let snapshot = telemetry.syndrome_latency_hist.snapshot();
    assert!(snapshot.count > 0);
}

#[test]
fn test_telemetry_decoder_usage() {
    let telemetry = QECTelemetryCapsule::new();

    // Record decoder usage
    telemetry.record_decoder_usage(DecoderType::UnionFind);
    telemetry.record_decoder_usage(DecoderType::UnionFind);
    telemetry.record_decoder_usage(DecoderType::MWPM);

    // Verify counters
    assert_eq!(
        telemetry.decoder_stats.union_find_count.load(Ordering::Relaxed),
        2
    );
    assert_eq!(
        telemetry.decoder_stats.mwpm_count.load(Ordering::Relaxed),
        1
    );
}
```

**Expected Result**: ✅ Pass (<100μs runtime per test)

---

### Q7: Does error handling propagate correctly?

**Test: Error Propagation**

```rust
#[test]
fn test_buffer_full_error() {
    let buffer = SyndromeRingBuffer::<256>::new();

    // Fill buffer
    for i in 0..256 {
        buffer.push(SyndromeEntry { generation: i, ..Default::default() }).unwrap();
    }

    // Push one more (should return BufferFull error)
    let result = buffer.push(SyndromeEntry::default());
    match result {
        Err(BufferFull) => {}, // Expected
        _ => panic!("Expected BufferFull error"),
    }
}

#[test]
fn test_decoder_timeout_error() {
    let capsule = QECIntegrationCapsule::new();

    // Mock decoder that always times out
    let syndrome = SyndromeEntry::default();
    let result = capsule.decode_with_timeout(&syndrome, DecoderType::MWPM, 1); // 1ns timeout

    match result {
        Err(DecoderTimeout { .. }) => {}, // Expected
        _ => panic!("Expected DecoderTimeout error"),
    }
}
```

**Expected Result**: ✅ Pass (<1ms runtime per test)

---

## Q8-Q14: Property Tests

### Q8: Are invariants preserved under random inputs?

**Property: FIFO Ordering**

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_fifo_ordering(syndromes in vec(syndrome_entry(), 1..256)) {
        let buffer = SyndromeRingBuffer::<256>::new();

        // Push syndromes
        for syndrome in &syndromes {
            buffer.push(*syndrome).unwrap();
        }

        // Pop syndromes (must match push order)
        for expected in &syndromes {
            let actual = buffer.pop().unwrap();
            prop_assert_eq!(actual.generation, expected.generation);
        }
    }
}

fn syndrome_entry() -> impl Strategy<Value = SyndromeEntry> {
    (any::<u32>(), prop::array::uniform8(any::<u64>()), 0..512_u16)
        .prop_map(|(gen, bits, weight)| SyndromeEntry {
            generation: gen,
            syndrome_bits: bits,
            syndrome_weight: weight,
            ..Default::default()
        })
}
```

**Expected Result**: ✅ Pass (1000 random inputs, 0 counterexamples)

---

### Q9: Does the system handle boundary conditions?

**Property: Wraparound Safety**

```rust
proptest! {
    #[test]
    fn prop_wraparound_detection(
        num_cycles in 1000..10000_usize,
    ) {
        let buffer = SyndromeRingBuffer::<256>::new();

        // Simulate many cycles (force wraparound)
        for i in 0..num_cycles {
            let syndrome = SyndromeEntry {
                generation: (i / 256) as u32, // Generation counter
                ..Default::default()
            };

            // Push and immediately pop (keep buffer from filling)
            buffer.push(syndrome).unwrap();
            let popped = buffer.pop().unwrap();

            // Verify generation matches
            prop_assert_eq!(popped.generation, (i / 256) as u32);
        }
    }
}
```

**Expected Result**: ✅ Pass (1000 random inputs, wraparound handled correctly)

---

### Q10: Does the system maintain consistency?

**Property: Exact-Once Processing**

```rust
use std::collections::HashSet;

proptest! {
    #[test]
    fn prop_exact_once_processing(syndromes in vec(syndrome_entry(), 1..256)) {
        let buffer = SyndromeRingBuffer::<256>::new();

        // Push syndromes
        for syndrome in &syndromes {
            buffer.push(*syndrome).unwrap();
        }

        // Pop syndromes (must be unique)
        let mut seen = HashSet::new();
        for _ in 0..syndromes.len() {
            let syndrome = buffer.pop().unwrap();
            let unique = seen.insert(syndrome.generation);
            prop_assert!(unique, "Duplicate syndrome detected");
        }
    }
}
```

**Expected Result**: ✅ Pass (1000 random inputs, no duplicates)

---

### Q11: Does latency stay within bounds?

**Property: Latency Bounds**

```rust
proptest! {
    #[test]
    fn prop_latency_bounds(
        syndrome_weight in 0..50_u16,
        distance in 3..10_u8,
    ) {
        let config = QECConfig {
            code_distance: distance,
            syndrome_weight_threshold: (distance as u16 * distance as u16) / 2,
            ..Default::default()
        };

        let capsule = QECIntegrationCapsule::new_with_config(config);

        let syndrome = SyndromeEntry {
            syndrome_weight,
            ..Default::default()
        };

        // Run QEC cycle
        let start = Instant::now();
        let result = capsule.run_qec_cycle_with_syndrome(syndrome);
        let elapsed_ns = start.elapsed().as_nanos() as u64;

        // Verify latency bounds
        if syndrome_weight == 0 {
            // Empty syndrome: <10μs
            prop_assert!(elapsed_ns < 10_000);
        } else if syndrome_weight < config.syndrome_weight_threshold {
            // Union-Find: <60μs
            prop_assert!(elapsed_ns < 60_000);
        } else {
            // MWPM: <120μs (with margin)
            prop_assert!(elapsed_ns < 120_000);
        }
    }
}
```

**Expected Result**: ✅ Pass (1000 random inputs, latency within bounds)

---

### Q12: Does the system recover from failures?

**Property: Overflow Recovery**

```rust
proptest! {
    #[test]
    fn prop_overflow_recovery(
        num_syndromes in 300..1000_usize, // Exceeds buffer capacity (256)
    ) {
        let buffer = SyndromeRingBuffer::<256>::new();

        let mut push_count = 0;
        let mut overflow_count = 0;

        // Push syndromes (some will overflow)
        for i in 0..num_syndromes {
            let syndrome = SyndromeEntry {
                generation: i as u32,
                ..Default::default()
            };

            match buffer.push(syndrome) {
                Ok(()) => push_count += 1,
                Err(BufferFull) => overflow_count += 1,
            }
        }

        // Verify overflow counter
        let actual_overflow = buffer.overflow_count.load(Ordering::Relaxed);
        prop_assert_eq!(actual_overflow, overflow_count as u64);

        // Verify buffer state consistent
        let head = buffer.head.load(Ordering::Relaxed);
        let tail = buffer.tail.load(Ordering::Relaxed);
        prop_assert!(head >= tail); // Invariant: head always >= tail
    }
}
```

**Expected Result**: ✅ Pass (overflow handled gracefully, no corruption)

---

### Q13: Does decoder selection optimize latency?

**Property: Adaptive Selection Performance**

```rust
proptest! {
    #[test]
    fn prop_adaptive_selection_optimizes_latency(
        syndrome_weight in 0..50_u16,
    ) {
        let config = QECConfig {
            code_distance: 5,
            decoder_mode: DecoderMode::Auto, // Adaptive
            syndrome_weight_threshold: 12,
            ..Default::default()
        };

        let capsule_adaptive = QECIntegrationCapsule::new_with_config(config);

        let config_mwpm = QECConfig {
            decoder_mode: DecoderMode::MWPM, // Force MWPM
            ..config
        };

        let capsule_mwpm = QECIntegrationCapsule::new_with_config(config_mwpm);

        let syndrome = SyndromeEntry {
            syndrome_weight,
            ..Default::default()
        };

        // Measure adaptive latency
        let start = Instant::now();
        capsule_adaptive.run_qec_cycle_with_syndrome(syndrome).ok();
        let adaptive_latency = start.elapsed().as_nanos() as u64;

        // Measure MWPM latency
        let start = Instant::now();
        capsule_mwpm.run_qec_cycle_with_syndrome(syndrome).ok();
        let mwpm_latency = start.elapsed().as_nanos() as u64;

        // Verify adaptive is faster (or equal) for sparse syndromes
        if syndrome_weight < 12 {
            prop_assert!(
                adaptive_latency <= mwpm_latency,
                "Adaptive should be faster for sparse syndromes"
            );
        }
    }
}
```

**Expected Result**: ✅ Pass (adaptive selection reduces latency for sparse syndromes)

---

### Q14: Does concurrency preserve correctness?

**Property: Concurrent Push/Pop**

```rust
use std::sync::Arc;
use std::thread;

proptest! {
    #[test]
    fn prop_concurrent_push_pop(
        num_syndromes in 100..1000_usize,
        num_threads in 2..8_usize,
    ) {
        let buffer = Arc::new(SyndromeRingBuffer::<256>::new());

        // Spawn producer threads
        let producers: Vec<_> = (0..num_threads)
            .map(|tid| {
                let buffer = Arc::clone(&buffer);
                let syndromes_per_thread = num_syndromes / num_threads;

                thread::spawn(move || {
                    for i in 0..syndromes_per_thread {
                        let syndrome = SyndromeEntry {
                            generation: (tid * syndromes_per_thread + i) as u32,
                            ..Default::default()
                        };
                        // Retry until success (buffer may be full temporarily)
                        while buffer.push(syndrome).is_err() {
                            thread::yield_now();
                        }
                    }
                })
            })
            .collect();

        // Wait for producers
        for p in producers {
            p.join().unwrap();
        }

        // Pop all syndromes
        let mut popped = Vec::new();
        while let Some(syndrome) = buffer.pop() {
            popped.push(syndrome.generation);
        }

        // Verify count
        prop_assert_eq!(popped.len(), num_syndromes);

        // Verify uniqueness
        let mut sorted = popped.clone();
        sorted.sort();
        sorted.dedup();
        prop_assert_eq!(sorted.len(), popped.len(), "Duplicate syndromes detected");
    }
}
```

**Expected Result**: ✅ Pass (concurrent access preserves exact-once semantics)

---

## Q15-Q21: Integration Tests

### Q15: Do components integrate correctly?

**Test: Full QEC Cycle**

```rust
#[test]
fn test_full_qec_cycle() {
    // Setup
    let stabilizer_state = StabilizerStateCapsule::new(5); // d=5 surface code
    let union_find_decoder = UnionFindDecoderCapsule::new(5);
    let mwpm_decoder = MWPMDecoderCapsule::new(5);

    let capsule = QECIntegrationCapsule::new(
        &stabilizer_state,
        &union_find_decoder,
        &mwpm_decoder,
    );

    // Inject error (X error on qubit 0)
    stabilizer_state.apply_pauli(0, PauliOp::X).unwrap();

    // Run QEC cycle
    let result = capsule.run_qec_cycle().unwrap();

    // Verify syndrome extracted (non-empty)
    assert!(result.syndrome_latency_ns > 0);

    // Verify decoding completed
    assert!(result.decode_latency_ns > 0);

    // Verify correction applied
    assert!(result.correct_latency_ns > 0);

    // Verify total latency within budget
    assert!(result.total_latency_ns < 100_000); // <100μs

    // Verify error corrected (state back to |0⟩)
    assert!(stabilizer_state.is_consistent());
}
```

**Expected Result**: ✅ Pass (full pipeline works end-to-end)

---

### Q16: Does decoder selection work in practice?

**Test: Decoder Comparison**

```rust
#[test]
fn test_decoder_comparison() {
    let stabilizer_state = StabilizerStateCapsule::new(5);
    let union_find_decoder = UnionFindDecoderCapsule::new(5);
    let mwpm_decoder = MWPMDecoderCapsule::new(5);

    let capsule = QECIntegrationCapsule::new(
        &stabilizer_state,
        &union_find_decoder,
        &mwpm_decoder,
    );

    // Test sparse syndrome (weight = 2)
    let sparse_syndrome = SyndromeEntry {
        syndrome_weight: 2,
        syndrome_bits: [0b11, 0, 0, 0, 0, 0, 0, 0], // 2 errors
        ..Default::default()
    };

    let decoder_type = capsule.select_decoder(&sparse_syndrome);
    assert_eq!(decoder_type, DecoderType::UnionFind);

    // Test dense syndrome (weight = 18)
    let dense_syndrome = SyndromeEntry {
        syndrome_weight: 18,
        syndrome_bits: [0xFFFF, 0xFF, 0, 0, 0, 0, 0, 0], // 18 errors
        ..Default::default()
    };

    let decoder_type = capsule.select_decoder(&dense_syndrome);
    assert_eq!(decoder_type, DecoderType::MWPM);
}
```

**Expected Result**: ✅ Pass (adaptive selection chooses correct decoder)

---

### Q17: Does the system handle realistic workloads?

**Test: 1000 QEC Cycles**

```rust
#[test]
fn test_1000_qec_cycles() {
    let stabilizer_state = StabilizerStateCapsule::new(5);
    let union_find_decoder = UnionFindDecoderCapsule::new(5);
    let mwpm_decoder = MWPMDecoderCapsule::new(5);

    let capsule = QECIntegrationCapsule::new(
        &stabilizer_state,
        &union_find_decoder,
        &mwpm_decoder,
    );

    // Run 1000 QEC cycles
    for i in 0..1000 {
        // Inject random error (10% probability per qubit)
        inject_random_errors(&stabilizer_state, 0.001);

        // Run QEC cycle
        let result = capsule.run_qec_cycle().unwrap();

        // Verify latency within bounds
        assert!(
            result.total_latency_ns < 100_000,
            "Cycle {} exceeded latency budget: {}μs",
            i,
            result.total_latency_ns / 1000
        );
    }

    // Verify telemetry
    let telemetry = capsule.telemetry_snapshot();
    assert_eq!(telemetry.cycle_count, 1000);

    // Verify low overflow rate (<1%)
    let overflow_rate = telemetry.overflow_count as f64 / 1000.0;
    assert!(overflow_rate < 0.01, "Overflow rate too high: {:.2}%", overflow_rate * 100.0);
}
```

**Expected Result**: ✅ Pass (1000 cycles complete successfully)

---

### Q18: Does telemetry track real-world metrics?

**Test: Telemetry Accuracy**

```rust
#[test]
fn test_telemetry_accuracy() {
    let capsule = QECIntegrationCapsule::new(/* ... */);

    // Run 100 QEC cycles with known error injection
    for _ in 0..100 {
        // Inject X error on qubit 0
        capsule.stabilizer_state.apply_pauli(0, PauliOp::X).unwrap();

        capsule.run_qec_cycle().unwrap();
    }

    // Verify telemetry
    let telemetry = capsule.telemetry_snapshot();

    // Verify cycle count
    assert_eq!(telemetry.cycle_count, 100);

    // Verify decoder usage (all sparse syndromes → Union-Find)
    assert!(telemetry.union_find_count > 90); // Most cycles use Union-Find

    // Verify physical error rate (should be ~0.001 for single-qubit errors)
    let physical_rate = (telemetry.physical_error_rate as f64) / 65536.0;
    assert!(
        physical_rate > 0.0005 && physical_rate < 0.0015,
        "Physical error rate out of range: {:.4}",
        physical_rate
    );
}
```

**Expected Result**: ✅ Pass (telemetry matches injected errors)

---

### Q19: Does the system handle decoder timeouts?

**Test: Timeout Handling**

```rust
#[test]
fn test_decoder_timeout_handling() {
    let capsule = QECIntegrationCapsule::new(/* ... */);

    // Create syndrome that forces MWPM (dense)
    let dense_syndrome = SyndromeEntry {
        syndrome_weight: 25, // Dense syndrome
        ..Default::default()
    };

    // Decode with very short timeout (guaranteed to fail)
    let result = capsule.decode_with_timeout(
        &dense_syndrome,
        DecoderType::MWPM,
        1000, // 1μs timeout (too short for MWPM)
    );

    // Verify timeout error
    match result {
        Err(DecoderTimeout { elapsed_ns, timeout_ns, .. }) => {
            assert!(elapsed_ns > timeout_ns);
        },
        _ => panic!("Expected DecoderTimeout error"),
    }

    // Verify state machine reset (back to IDLE)
    assert_eq!(
        capsule.pipeline_state.decoder_state.load(Ordering::Relaxed),
        IDLE
    );

    // Verify telemetry updated
    let telemetry = capsule.telemetry_snapshot();
    assert_eq!(telemetry.decoder_timeouts, 1);
}
```

**Expected Result**: ✅ Pass (timeout handled gracefully, state recovered)

---

### Q20: Does the system maintain stabilizer consistency?

**Test: Stabilizer Consistency After Corrections**

```rust
#[test]
fn test_stabilizer_consistency() {
    let capsule = QECIntegrationCapsule::new(/* ... */);

    // Run 100 QEC cycles with random errors
    for _ in 0..100 {
        // Inject random errors (depolarizing noise, p=0.001)
        inject_random_errors(&capsule.stabilizer_state, 0.001);

        // Run QEC cycle
        capsule.run_qec_cycle().unwrap();

        // Verify stabilizer state consistent (no logical errors)
        assert!(
            capsule.stabilizer_state.is_consistent(),
            "Stabilizer state inconsistent after QEC cycle"
        );
    }
}
```

**Expected Result**: ✅ Pass (stabilizer state consistent after all corrections)

---

### Q21: Does the system handle buffer overflow?

**Test: Buffer Overflow Stress**

```rust
#[test]
fn test_buffer_overflow_stress() {
    let capsule = QECIntegrationCapsule::new(/* ... */);

    // Inject errors faster than decoder can process (force overflow)
    for i in 0..1000 {
        let syndrome = SyndromeEntry {
            generation: i,
            ..Default::default()
        };

        // Push syndrome (may overflow)
        capsule.syndrome_buffer.push_with_eviction(syndrome);
    }

    // Verify overflow counter > 0 (buffer filled)
    let overflow = capsule.syndrome_buffer.overflow_count.load(Ordering::Relaxed);
    assert!(overflow > 0, "Expected buffer overflow");

    // Verify buffer state consistent (no corruption)
    let head = capsule.syndrome_buffer.head.load(Ordering::Relaxed);
    let tail = capsule.syndrome_buffer.tail.load(Ordering::Relaxed);
    assert!(head >= tail, "Buffer invariant violated");

    // Verify oldest syndromes evicted (FIFO)
    let popped = capsule.syndrome_buffer.pop().unwrap();
    assert!(
        popped.generation > 0,
        "Expected oldest syndromes evicted, but got generation 0"
    );
}
```

**Expected Result**: ✅ Pass (overflow handled, FIFO eviction works)

---

## Q22-Q28: Production Tests

### Q22: Does the system sustain production load?

**Test: 10,000 QEC Cycles Sustained**

```rust
#[test]
fn test_10k_cycles_sustained() {
    let capsule = QECIntegrationCapsule::new(/* ... */);

    // Run 10,000 QEC cycles
    let mut latencies = Vec::with_capacity(10_000);

    for i in 0..10_000 {
        // Inject realistic errors (depolarizing noise, p=0.001)
        inject_random_errors(&capsule.stabilizer_state, 0.001);

        // Run QEC cycle
        let start = Instant::now();
        let result = capsule.run_qec_cycle().unwrap();
        let elapsed_ns = start.elapsed().as_nanos() as u64;

        latencies.push(elapsed_ns);

        // Log progress every 1000 cycles
        if i % 1000 == 0 {
            println!("Completed {} cycles", i);
        }
    }

    // Compute statistics
    latencies.sort();
    let p50 = latencies[5_000];
    let p99 = latencies[9_900];
    let max = latencies[9_999];

    println!("Latency P50: {}μs", p50 / 1000);
    println!("Latency P99: {}μs", p99 / 1000);
    println!("Latency Max: {}μs", max / 1000);

    // Verify P99 within target
    assert!(
        p99 < 100_000,
        "P99 latency exceeded target: {}μs",
        p99 / 1000
    );

    // Verify throughput
    let throughput = 10_000.0 / (latencies.iter().sum::<u64>() as f64 / 1e9);
    println!("Throughput: {:.0} cycles/sec", throughput);
    assert!(throughput > 10_000.0, "Throughput below target: {:.0}", throughput);
}
```

**Expected Result**: ✅ Pass (10K cycles sustained, P99 <100μs, throughput >10K cycles/sec)

---

### Q23: Does the system suppress logical errors?

**Test: Logical Error Suppression**

```rust
#[test]
fn test_logical_error_suppression() {
    let capsule_qec = QECIntegrationCapsule::new(/* ... */);
    let capsule_no_qec = StabilizerStateCapsule::new(5); // No QEC (baseline)

    // Run 1000 cycles with QEC
    let mut logical_errors_qec = 0;
    for _ in 0..1000 {
        inject_random_errors(&capsule_qec.stabilizer_state, 0.001);
        capsule_qec.run_qec_cycle().unwrap();

        if !capsule_qec.stabilizer_state.is_consistent() {
            logical_errors_qec += 1;
        }
    }

    // Run 1000 cycles without QEC (baseline)
    let mut logical_errors_no_qec = 0;
    for _ in 0..1000 {
        inject_random_errors(&capsule_no_qec, 0.001);

        if !capsule_no_qec.is_consistent() {
            logical_errors_no_qec += 1;
        }
    }

    // Compute suppression factor
    let suppression_factor = (logical_errors_no_qec as f64) / (logical_errors_qec as f64);

    println!("Logical errors (QEC): {}", logical_errors_qec);
    println!("Logical errors (no QEC): {}", logical_errors_no_qec);
    println!("Suppression factor: {:.2}×", suppression_factor);

    // Verify >90% suppression (10× reduction)
    assert!(
        suppression_factor > 10.0,
        "Logical error suppression below target: {:.2}×",
        suppression_factor
    );
}
```

**Expected Result**: ✅ Pass (>10× logical error suppression)

---

### Q24: Does the system handle real-world error distributions?

**Test: Depolarizing Noise**

```rust
#[test]
fn test_depolarizing_noise() {
    let capsule = QECIntegrationCapsule::new(/* ... */);

    // Run 1000 cycles with depolarizing noise (p=0.001)
    for _ in 0..1000 {
        inject_depolarizing_noise(&capsule.stabilizer_state, 0.001);
        capsule.run_qec_cycle().unwrap();
    }

    // Verify telemetry
    let telemetry = capsule.telemetry_snapshot();

    // Verify physical error rate (should be ~0.001)
    let physical_rate = (telemetry.physical_error_rate as f64) / 65536.0;
    assert!(
        physical_rate > 0.0005 && physical_rate < 0.0015,
        "Physical error rate out of range: {:.4}",
        physical_rate
    );

    // Verify logical error rate (<0.01)
    let logical_rate = (telemetry.logical_error_rate as f64) / 65536.0;
    assert!(
        logical_rate < 0.01,
        "Logical error rate too high: {:.4}",
        logical_rate
    );
}
```

**Expected Result**: ✅ Pass (realistic error model handled correctly)

---

### Q25: Does the system perform under stress?

**Test: High Error Rate Stress**

```rust
#[test]
fn test_high_error_rate_stress() {
    let capsule = QECIntegrationCapsule::new(/* ... */);

    // Run 1000 cycles with high error rate (p=0.01, 10× normal)
    for _ in 0..1000 {
        inject_random_errors(&capsule.stabilizer_state, 0.01);
        capsule.run_qec_cycle().unwrap();
    }

    // Verify telemetry
    let telemetry = capsule.telemetry_snapshot();

    // Verify MWPM usage increased (dense syndromes)
    let mwpm_usage = (telemetry.mwpm_count as f64) / 1000.0;
    assert!(
        mwpm_usage > 0.20,
        "Expected >20% MWPM usage under high error rate, got {:.2}%",
        mwpm_usage * 100.0
    );

    // Verify overflow rate acceptable (<5%)
    let overflow_rate = (telemetry.overflow_count as f64) / 1000.0;
    assert!(
        overflow_rate < 0.05,
        "Overflow rate too high: {:.2}%",
        overflow_rate * 100.0
    );
}
```

**Expected Result**: ✅ Pass (system handles 10× normal error rate)

---

### Q26: Does the system validate decoder accuracy?

**Test: Decoder Accuracy vs Ideal**

```rust
#[test]
fn test_decoder_accuracy_vs_ideal() {
    let capsule = QECIntegrationCapsule::new(/* ... */);
    let ideal_decoder = IdealDecoderOffline::new(5); // Unlimited latency, optimal

    let mut matches = 0;
    let total = 1000;

    for _ in 0..total {
        // Inject random error
        inject_random_errors(&capsule.stabilizer_state, 0.001);

        // Extract syndrome
        let syndrome = capsule.extract_syndrome().unwrap();

        // Decode with adaptive decoder
        let decoder_type = capsule.select_decoder(&syndrome);
        let actual_corrections = capsule.decode_syndrome(&syndrome, decoder_type).unwrap();

        // Decode with ideal decoder
        let ideal_corrections = ideal_decoder.decode(&syndrome.syndrome_bits);

        // Compare corrections (equivalent up to logical operator)
        if corrections_equivalent(&actual_corrections, &ideal_corrections) {
            matches += 1;
        }
    }

    let accuracy = (matches as f64) / (total as f64);

    println!("Decoder accuracy: {:.2}%", accuracy * 100.0);

    // Verify >95% accuracy
    assert!(
        accuracy > 0.95,
        "Decoder accuracy below target: {:.2}%",
        accuracy * 100.0
    );
}
```

**Expected Result**: ✅ Pass (>95% accuracy vs ideal decoder)

---

### Q27: Does the system maintain Q34 audit trail?

**Test: Audit Trail Integrity**

```rust
#[test]
fn test_audit_trail_integrity() {
    let capsule = QECIntegrationCapsule::new(/* ... */);

    // Run 100 QEC cycles
    for _ in 0..100 {
        inject_random_errors(&capsule.stabilizer_state, 0.001);
        capsule.run_qec_cycle().unwrap();
    }

    // Verify hash chain integrity
    let result = capsule.verify_audit_trail();
    assert!(
        result.is_ok(),
        "Audit trail integrity check failed: {:?}",
        result.err()
    );

    // Generate compliance report
    let report = capsule.compliance_report();

    // Verify report fields
    assert_eq!(report.total_cycles, 100);
    assert!(report.audit_trail_valid);
    assert!(report.decoder_accuracy > 0.95);
}
```

**Expected Result**: ✅ Pass (audit trail intact, compliance report valid)

---

### Q28: Does the system simplify deployment?

**Test: Builder Pattern API**

```rust
#[test]
fn test_builder_pattern_api() {
    // Build capsule using builder pattern
    let capsule = QECIntegrationBuilder::new()
        .stabilizer_state(&STABILIZER_STATE)
        .union_find_decoder(&UNION_FIND_DECODER)
        .mwpm_decoder(&MWPM_DECODER)
        .distance(5)
        .decoder_mode(DecoderMode::Auto)
        .buffer_capacity(256)
        .build()
        .unwrap();

    // Verify configuration
    assert_eq!(capsule.config.code_distance, 5);
    assert_eq!(capsule.config.decoder_mode, DecoderMode::Auto);
    assert_eq!(capsule.config.buffer_capacity, 256);

    // Run single QEC cycle (verify API simplicity)
    let result = capsule.run_qec_cycle();
    assert!(result.is_ok());
}
```

**Expected Result**: ✅ Pass (builder pattern simplifies configuration)

---

## Test Infrastructure

### Test Utilities

```rust
/// Inject random errors (depolarizing noise)
fn inject_random_errors(state: &StabilizerStateCapsule, error_rate: f64) {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for qubit_id in 0..state.num_qubits {
        if rng.gen::<f64>() < error_rate {
            // Random Pauli error (X, Y, or Z)
            let pauli_op = match rng.gen_range(0..3) {
                0 => PauliOp::X,
                1 => PauliOp::Y,
                2 => PauliOp::Z,
                _ => unreachable!(),
            };

            state.apply_pauli(qubit_id, pauli_op).unwrap();
        }
    }
}

/// Inject depolarizing noise (X, Y, Z with equal probability)
fn inject_depolarizing_noise(state: &StabilizerStateCapsule, error_rate: f64) {
    inject_random_errors(state, error_rate);
}

/// Compare corrections (equivalent up to logical operator)
fn corrections_equivalent(
    actual: &[Correction],
    ideal: &[Correction],
) -> bool {
    // Simplified: exact match (conservative)
    if actual.len() != ideal.len() {
        return false;
    }

    for (a, i) in actual.iter().zip(ideal.iter()) {
        if a.qubit_id != i.qubit_id || a.pauli_op != i.pauli_op {
            return false;
        }
    }

    true
}
```

---

## Coverage Analysis

### Code Coverage Targets

| Component | Target | Actual (Expected) |
|-----------|--------|-------------------|
| **Ring Buffer** | 100% | 100% (all paths tested) |
| **Decoder Selection** | 100% | 100% (all branches tested) |
| **State Machine** | 100% | 100% (all transitions tested) |
| **Telemetry** | 95% | 95% (histogram internals excluded) |
| **Error Correction** | 90% | 90% (logical error edge cases rare) |
| **Overall** | 95% | 95% (comprehensive coverage) |

### Coverage Measurement

```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Run tests with coverage
cargo llvm-cov --lib --tests --all-features --html

# Open coverage report
open target/llvm-cov/html/index.html
```

---

## CI/CD Integration

### GitHub Actions Workflow

```yaml
name: QEC Integration Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest

    steps:
    - uses: actions/checkout@v3

    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: nightly
        override: true

    - name: Run Unit Tests (Q1-Q7)
      run: cargo test --lib unit_test

    - name: Run Property Tests (Q8-Q14)
      run: cargo test --lib prop_ -- --nocapture

    - name: Run Integration Tests (Q15-Q21)
      run: cargo test --lib integration_test

    - name: Run Production Tests (Q22-Q28)
      run: cargo test --lib production_test -- --nocapture

    - name: Generate Coverage Report
      run: |
        cargo install cargo-llvm-cov
        cargo llvm-cov --lib --tests --all-features --lcov --output-path lcov.info

    - name: Upload Coverage to Codecov
      uses: codecov/codecov-action@v3
      with:
        files: ./lcov.info
```

---

## Summary

**T28 Coverage**: 28 comprehensive tests (7 unit + 7 property + 7 integration + 7 production)

**Expected Results**: 28/28 passing (100% success rate)

**Performance Validation**: P99 <100μs latency, >10K cycles/sec throughput, >90% logical error suppression

**Framework Compliance**: UCE34 (Q1-Q34), Chaos (100% lockfree), B32 (fair baselines), ASSUM (99.99% safe), Q34 (audit trails)

**Status**: Test plan complete, ready for implementation alongside QECIntegrationCapsule

**Next Steps**: Implement tests in parallel with capsule development (TDD approach: write test → implement → verify → iterate)
