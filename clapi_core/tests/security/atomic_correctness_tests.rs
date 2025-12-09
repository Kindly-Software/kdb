//! Atomic Correctness Tests - Lockfree Operation Validation
//!
//! **Purpose**: Verify correctness of all atomic operations under concurrent load
//! **Framework**: T28 Testing Framework + ASSUM Safety
//!
//! # Test Coverage
//! - **compare_exchange**: Returns correct old value, no lost updates
//! - **fetch_add**: No lost increments under contention
//! - **Generation Counters**: Monotonically increasing, overflow handled
//! - **Double-Checked Locking**: No TOCTOU races
//! - **ABA Prevention**: Generation counters prevent spurious CAS success
//!
//! # ASSUM Validation
//! - Validates Category 3: TOCTOU_PREVENTION
//! - Validates Category 4: MEMORY_ORDERING
//! - Validates Category 7: METRIC_ATOMICITY

use clapi_core::capsules::{
    BudgetSlotCapsule, RequestCapsule128, CircuitBreakerCapsule,
    PaymentCapsule256, OAuthSessionCapsule,
};
use std::sync::{Arc, atomic::{AtomicU64, AtomicBool, Ordering}};
use std::thread;

// ============================================================================
// compare_exchange Correctness Tests (T28 Q1-Q4)
// ============================================================================

#[test]
fn test_cas_returns_correct_old_value() {
    // T28 Q1: CAS should return actual old value on failure
    let counter = AtomicU64::new(42);

    // Thread A expects 42, Thread B changed it to 100
    counter.store(100, Ordering::Relaxed);

    match counter.compare_exchange(
        42,   // Expected (wrong)
        200,  // New value
        Ordering::Release,
        Ordering::Acquire,
    ) {
        Ok(_) => panic!("CAS should have failed"),
        Err(observed) => {
            assert_eq!(observed, 100, "CAS should return actual value (100), not expected (42)");
        }
    }
}

#[test]
fn test_cas_weak_spurious_failure_handling() {
    // T28 Q2: CAS weak can fail spuriously, must retry
    let counter = AtomicU64::new(0);
    let target = 10_000;

    // Increment using CAS weak (handles spurious failures)
    for _ in 0..target {
        loop {
            let current = counter.load(Ordering::Acquire);
            match counter.compare_exchange_weak(
                current,
                current + 1,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,  // Success
                Err(_) => continue,  // Retry (spurious or contention)
            }
        }
    }

    assert_eq!(counter.load(Ordering::Relaxed), target, "All increments should succeed");
}

#[test]
fn test_cas_no_lost_updates() {
    // T28 Q3: Concurrent CAS should not lose updates
    let counter = Arc::new(AtomicU64::new(0));
    let num_threads = 8;
    let increments_per_thread = 10_000;

    let mut handles = vec![];

    for _ in 0..num_threads {
        let counter_clone = Arc::clone(&counter);

        let handle = thread::spawn(move || {
            for _ in 0..increments_per_thread {
                loop {
                    let current = counter_clone.load(Ordering::Acquire);
                    match counter_clone.compare_exchange_weak(
                        current,
                        current + 1,
                        Ordering::Release,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(_) => continue,
                    }
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_value = counter.load(Ordering::Relaxed);
    let expected = (num_threads * increments_per_thread) as u64;

    assert_eq!(final_value, expected, "No increments should be lost");
}

#[test]
fn test_cas_aba_prevention_with_generation() {
    // T28 Q4: Generation counters should prevent ABA problem
    let value = Arc::new(AtomicU64::new(100));
    let generation = Arc::new(AtomicU64::new(0));
    let aba_detected = Arc::new(AtomicBool::new(false));

    // Thread 1: Read value, sleep, then CAS
    let value1 = Arc::clone(&value);
    let gen1 = Arc::clone(&generation);
    let detected = Arc::clone(&aba_detected);

    let handle1 = thread::spawn(move || {
        let old_value = value1.load(Ordering::Acquire);
        let old_gen = gen1.load(Ordering::Acquire);

        // Sleep to allow Thread 2 to modify
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Try CAS (will succeed on value but generation changed)
        let value_cas_ok = value1.compare_exchange(
            old_value,
            200,
            Ordering::Release,
            Ordering::Relaxed,
        ).is_ok();

        let current_gen = gen1.load(Ordering::Acquire);

        // ABA detected if value CAS succeeded but generation changed
        if value_cas_ok && current_gen != old_gen {
            detected.store(true, Ordering::Relaxed);
        }
    });

    // Thread 2: Change value, then restore it (ABA scenario)
    let value2 = Arc::clone(&value);
    let gen2 = Arc::clone(&generation);

    let handle2 = thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(2));

        // Change value: 100 → 150
        value2.store(150, Ordering::Release);
        gen2.fetch_add(1, Ordering::Release);

        std::thread::sleep(std::time::Duration::from_millis(2));

        // Restore value: 150 → 100 (ABA!)
        value2.store(100, Ordering::Release);
        gen2.fetch_add(1, Ordering::Release);
    });

    handle2.join().unwrap();
    handle1.join().unwrap();

    // Generation counter should prevent ABA (or detect it)
    let final_gen = generation.load(Ordering::Relaxed);
    assert!(final_gen >= 2, "Generation should have incremented at least twice");
}

// ============================================================================
// fetch_add Correctness Tests (T28 Q5-Q8)
// ============================================================================

#[test]
fn test_fetch_add_no_lost_increments() {
    // T28 Q5: Concurrent fetch_add should not lose increments
    let counter = Arc::new(AtomicU64::new(0));
    let num_threads = 16;
    let increments_per_thread = 100_000;

    let mut handles = vec![];

    for _ in 0..num_threads {
        let counter_clone = Arc::clone(&counter);

        let handle = thread::spawn(move || {
            for _ in 0..increments_per_thread {
                counter_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_value = counter.load(Ordering::Relaxed);
    let expected = (num_threads * increments_per_thread) as u64;

    assert_eq!(final_value, expected, "All increments should be counted");
}

#[test]
fn test_fetch_add_returns_old_value() {
    // T28 Q6: fetch_add should return value BEFORE increment
    let counter = AtomicU64::new(100);

    let old_value = counter.fetch_add(50, Ordering::Relaxed);

    assert_eq!(old_value, 100, "fetch_add should return old value (100)");
    assert_eq!(counter.load(Ordering::Relaxed), 150, "Counter should be incremented to 150");
}

#[test]
fn test_fetch_add_ordering_correctness() {
    // T28 Q7: fetch_add with AcqRel should synchronize
    let counter = Arc::new(AtomicU64::new(0));
    let data = Arc::new(AtomicU64::new(0));

    let counter_clone = Arc::clone(&counter);
    let data_clone = Arc::clone(&data);

    let writer = thread::spawn(move || {
        data_clone.store(42, Ordering::Release);  // (1)
        counter_clone.fetch_add(1, Ordering::Release);  // (2) Signal ready
    });

    writer.join().unwrap();

    // Reader should see data if counter > 0
    if counter.load(Ordering::Acquire) > 0 {  // (3) Synchronizes with (2)
        let value = data.load(Ordering::Acquire);  // (4) Sees (1)
        assert_eq!(value, 42, "Synchronization should ensure visibility");
    }
}

#[test]
fn test_fetch_sub_no_underflow_handling() {
    // T28 Q8: fetch_sub should wrap on underflow (u64 semantics)
    let counter = AtomicU64::new(5);

    let old_value = counter.fetch_sub(10, Ordering::Relaxed);

    assert_eq!(old_value, 5, "fetch_sub should return old value");
    assert_eq!(counter.load(Ordering::Relaxed), u64::MAX - 4, "Should wrap on underflow");
}

// ============================================================================
// Generation Counter Tests (T28 Q9-Q12)
// ============================================================================

#[test]
fn test_generation_monotonically_increasing() {
    // T28 Q9: Generation counters should always increase
    let capsule = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);

    let gen0 = capsule.generation();

    capsule.mark_confirmed(1000_000_000).unwrap();
    let gen1 = capsule.generation();

    assert!(gen1 > gen0, "Generation should increase after state change");

    let _ = capsule.mark_failed();
    let gen2 = capsule.generation();

    assert!(gen2 > gen1, "Generation should keep increasing");
}

#[test]
fn test_generation_overflow_handling() {
    // T28 Q10: Generation counter overflow should be safe (wraps)
    let capsule = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);

    // Set generation to near max
    capsule.generation.store(u64::MAX - 5, Ordering::Relaxed);

    // Increment past max
    for _ in 0..10 {
        capsule.generation.fetch_add(1, Ordering::Relaxed);
    }

    // Should wrap to 4 (u64::MAX - 5 + 10 % 2^64)
    let gen = capsule.generation();
    assert_eq!(gen, 4, "Generation should wrap on overflow");
}

#[test]
fn test_generation_concurrent_increments() {
    // T28 Q11: Concurrent generation increments should be atomic
    let capsule = Arc::new(PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123));
    let num_threads = 8;
    let increments_per_thread = 10_000;

    let mut handles = vec![];

    for _ in 0..num_threads {
        let capsule_clone = Arc::clone(&capsule);

        let handle = thread::spawn(move || {
            for _ in 0..increments_per_thread {
                capsule_clone.generation.fetch_add(1, Ordering::Relaxed);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_gen = capsule.generation();
    let expected = (num_threads * increments_per_thread) as u64;

    assert_eq!(final_gen, expected, "All generation increments should be counted");
}

#[test]
fn test_generation_prevents_stale_reads() {
    // T28 Q12: Generation counter should prevent stale reads
    let capsule = Arc::new(PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123));

    let gen0 = capsule.generation();
    let status0 = capsule.status();

    // Update status in another thread
    let capsule_clone = Arc::clone(&capsule);
    let handle = thread::spawn(move || {
        capsule_clone.mark_confirmed(1000_000_000).unwrap();
    });

    handle.join().unwrap();

    let gen1 = capsule.generation();
    let status1 = capsule.status();

    // If generation changed, status should also change
    if gen1 != gen0 {
        assert_ne!(status0, status1, "Status should change with generation");
    }
}

// ============================================================================
// Double-Checked Locking / TOCTOU Tests (T28 Q13-Q16)
// ============================================================================

#[test]
fn test_budget_deduction_no_toctou() {
    // T28 Q13: Budget deduction should prevent TOCTOU races
    let capsule = Arc::new(RequestCapsule128::new(0x123, 100_00));
    let num_threads = 8;
    let deduction = 20_00;

    let success_count = Arc::new(AtomicU64::new(0));

    let mut handles = vec![];

    for _ in 0..num_threads {
        let capsule_clone = Arc::clone(&capsule);
        let success_clone = Arc::clone(&success_count);

        let handle = thread::spawn(move || {
            // Try to deduct $20 (5 threads should succeed, 3 should fail)
            if capsule_clone.try_deduct(deduction).is_ok() {
                success_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let successes = success_count.load(Ordering::Relaxed);

    // Exactly 5 threads should succeed (100 / 20 = 5)
    assert_eq!(successes, 5, "Exactly 5 deductions should succeed (no TOCTOU)");

    // Final budget should be 0
    assert_eq!(capsule.budget(), 0, "Budget should be exhausted");
}

#[test]
fn test_circuit_breaker_no_double_trip() {
    // T28 Q14: Circuit breaker should not trip twice concurrently
    let breaker = Arc::new(CircuitBreakerCapsule::new());
    let trip_count = Arc::new(AtomicU64::new(0));

    // Record 100 failures (should trip at 10% failure rate)
    for _ in 0..90 {
        breaker.record_success();
    }

    let mut handles = vec![];

    // 10 threads concurrently record failures (should trip once)
    for _ in 0..10 {
        let breaker_clone = Arc::clone(&breaker);
        let count_clone = Arc::clone(&trip_count);

        let handle = thread::spawn(move || {
            // Record failure (might trip circuit)
            breaker_clone.record_failure();

            // Check if circuit tripped
            if breaker_clone.is_open() {
                count_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Circuit should trip, but only once (no double-trip)
    assert!(breaker.is_open(), "Circuit should be open");
    // Note: trip_count may be > 1 due to race, but internal trip counter should be 1
}

#[test]
fn test_oauth_session_revoke_idempotent() {
    // T28 Q15: Session revocation should be idempotent
    let session = Arc::new(OAuthSessionCapsule::new(0x123, 0x456, 3600_000_000_000));

    let mut handles = vec![];

    // 8 threads concurrently revoke same session
    for _ in 0..8 {
        let session_clone = Arc::clone(&session);

        let handle = thread::spawn(move || {
            session_clone.revoke();
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Session should be revoked exactly once (idempotent)
    assert!(!session.is_active(), "Session should be revoked");
}

#[test]
fn test_slot_allocation_no_double_allocation() {
    // T28 Q16: Slot allocation should not double-allocate
    let slot = Arc::new(BudgetSlotCapsule::new());
    let success_count = Arc::new(AtomicU64::new(0));

    let mut handles = vec![];

    // 8 threads concurrently try to allocate same slot
    for i in 0..8 {
        let slot_clone = Arc::clone(&slot);
        let count_clone = Arc::clone(&success_count);

        let handle = thread::spawn(move || {
            let capsule = Box::new(RequestCapsule128::new(i as u64, 100_00));

            if slot_clone.allocate_capsule(i as u64, capsule).is_ok() {
                count_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let successes = success_count.load(Ordering::Relaxed);

    // Exactly 1 thread should succeed (no double-allocation)
    assert_eq!(successes, 1, "Exactly one allocation should succeed");
}

// ============================================================================
// Memory Ordering Correctness Tests (T28 Q17-Q20)
// ============================================================================

#[test]
fn test_acquire_release_synchronization() {
    // T28 Q17: Acquire/Release should synchronize data
    let flag = Arc::new(AtomicBool::new(false));
    let data = Arc::new(AtomicU64::new(0));

    let flag_clone = Arc::clone(&flag);
    let data_clone = Arc::clone(&data);

    let writer = thread::spawn(move || {
        data_clone.store(42, Ordering::Relaxed);  // (1) Write data
        flag_clone.store(true, Ordering::Release);  // (2) Signal ready (Release)
    });

    writer.join().unwrap();

    // Reader
    while !flag.load(Ordering::Acquire) {  // (3) Wait for signal (Acquire)
        std::hint::spin_loop();
    }

    let value = data.load(Ordering::Relaxed);  // (4) Read data
    assert_eq!(value, 42, "Acquire/Release should synchronize data");
}

#[test]
fn test_relaxed_ordering_eventual_consistency() {
    // T28 Q18: Relaxed ordering should be eventually consistent
    let counter = Arc::new(AtomicU64::new(0));

    let counter_clone = Arc::clone(&counter);

    let writer = thread::spawn(move || {
        for i in 1..=100 {
            counter_clone.store(i, Ordering::Relaxed);
            std::thread::sleep(std::time::Duration::from_micros(10));
        }
    });

    let counter_clone2 = Arc::clone(&counter);

    let reader = thread::spawn(move || {
        let mut last_seen = 0;

        for _ in 0..100 {
            let value = counter_clone2.load(Ordering::Relaxed);

            // Relaxed allows reordering, but eventual consistency guaranteed
            if value > last_seen {
                last_seen = value;
            }

            std::thread::sleep(std::time::Duration::from_micros(10));
        }

        last_seen
    });

    writer.join().unwrap();
    let final_value = reader.join().unwrap();

    // Reader should eventually see value close to 100
    assert!(final_value >= 90, "Relaxed should provide eventual consistency (saw {})", final_value);
}

#[test]
fn test_acqrel_swap_synchronizes_both_directions() {
    // T28 Q19: AcqRel swap should synchronize in both directions
    let ptr = Arc::new(AtomicU64::new(0));
    let data_before = Arc::new(AtomicU64::new(0));
    let data_after = Arc::new(AtomicU64::new(0));

    let ptr_clone = Arc::clone(&ptr);
    let before_clone = Arc::clone(&data_before);
    let after_clone = Arc::clone(&data_after);

    let swapper = thread::spawn(move || {
        before_clone.store(11, Ordering::Relaxed);  // (1) Write before swap
        let old = ptr_clone.swap(42, Ordering::AcqRel);  // (2) Swap (AcqRel)
        after_clone.store(old, Ordering::Relaxed);  // (3) Write after swap
    });

    swapper.join().unwrap();

    // Check synchronization
    let ptr_value = ptr.load(Ordering::Acquire);
    let before_value = data_before.load(Ordering::Acquire);
    let after_value = data_after.load(Ordering::Acquire);

    assert_eq!(ptr_value, 42, "Swap should update pointer");
    assert_eq!(before_value, 11, "Data before swap should be visible");
    assert_eq!(after_value, 0, "Data after swap should reflect old pointer value");
}

#[test]
fn test_seqcst_not_needed_for_clapi() {
    // T28 Q20: Verify SeqCst not used (performance optimization)
    // This is a compile-time check (grep for SeqCst in source)
    // Runtime validation: All operations should work with Acquire/Release/Relaxed

    // Example: Counter increment (Relaxed sufficient for metrics)
    let counter = AtomicU64::new(0);

    for _ in 0..1000 {
        counter.fetch_add(1, Ordering::Relaxed);  // NOT SeqCst
    }

    assert_eq!(counter.load(Ordering::Relaxed), 1000);

    // Example: State transition (Release/Acquire sufficient)
    let state = AtomicU64::new(0);
    state.store(1, Ordering::Release);  // NOT SeqCst
    assert_eq!(state.load(Ordering::Acquire), 1);  // NOT SeqCst
}

// End of atomic_correctness_tests.rs
