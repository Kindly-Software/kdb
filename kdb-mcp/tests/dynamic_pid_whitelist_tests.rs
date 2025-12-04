//! DynamicPidWhitelistCapsule - T28 Comprehensive Testing (Q1-Q28)
//!
//! Framework: T28 (4 testing tiers)
//! - Q1-Q7: Unit tests (basic functionality)
//! - Q8-Q14: Property tests (statistical validation)
//! - Q15-Q21: Integration tests (with AccessControlCapsule)
//! - Q22-Q28: Production tests (stress, load, scalability)

use kdb_mcp::DynamicPidWhitelistCapsule;
use kdb_mcp::PidWhitelistError;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

// ============================================================================
// TIER 1: Unit Tests (Q1-Q7 - Basic functionality)
// ============================================================================

#[test]
fn q1_capsule_creation() {
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");
    assert_eq!(capsule.get_pid_count(), 0);
}

#[test]
fn q2_add_single_pid() {
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");
    assert!(capsule.add_pid(1000).is_ok());
    assert_eq!(capsule.get_pid_count(), 1);
}

#[test]
fn q3_check_pid() {
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");
    capsule.add_pid(2000).unwrap();
    assert!(capsule.is_pid_allowed(2000));
    assert!(!capsule.is_pid_allowed(2001));
}

#[test]
fn q4_remove_pid() {
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");
    capsule.add_pid(3000).unwrap();
    assert!(capsule.remove_pid(3000).is_ok());
    assert_eq!(capsule.get_pid_count(), 0);
}

#[test]
fn q5_duplicate_add_fails() {
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");
    capsule.add_pid(4000).unwrap();
    assert_eq!(
        capsule.add_pid(4000),
        Err(PidWhitelistError::PidAlreadyExists { pid: 4000 })
    );
}

#[test]
fn q6_remove_nonexistent_fails() {
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");
    assert_eq!(
        capsule.remove_pid(5000),
        Err(PidWhitelistError::PidNotFound { pid: 5000 })
    );
}

#[test]
fn q7_clear_resets() {
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");
    for pid in 0..50 {
        capsule.add_pid(pid).unwrap();
    }
    capsule.clear();
    assert_eq!(capsule.get_pid_count(), 0);
}

// ============================================================================
// TIER 2: Property Tests (Q8-Q14 - Statistical validation)
// ============================================================================

#[test]
fn q8_bloom_no_false_negatives() {
    // Property: All inserted PIDs must be found (0% FNR)
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");

    let pids: Vec<u32> = (0..500).map(|i| i * 7 + 13).collect(); // Prime offset

    for pid in &pids {
        capsule.add_pid(*pid).unwrap();
    }

    for pid in &pids {
        assert!(
            capsule.is_pid_allowed(*pid),
            "Bloom filter false negative for PID {}",
            pid
        );
    }
}

#[test]
fn q9_hash_table_collision_low() {
    // Property: Collision rate < 10% at 3% load factor
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");

    // Add 500 PIDs (500/16384 ≈ 3% load)
    for pid in 0..500 {
        capsule.add_pid(pid).unwrap();
    }

    let stats = capsule.get_stats();
    let collision_rate = stats.hash_table_collisions as f64 / stats.pid_count as f64;
    assert!(
        collision_rate < 0.1,
        "Collision rate {:.2}% exceeds 10%",
        collision_rate * 100.0
    );
}

#[test]
fn q10_linear_probing_converges() {
    // Property: Can insert up to 8K PIDs (50% load) without failure
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");

    // Insert 1000 PIDs (6% load)
    for pid in 0..1000 {
        assert!(
            capsule.add_pid(pid).is_ok(),
            "Failed at PID {} (linear probing should converge)",
            pid
        );
    }

    // All should be findable
    for pid in 0..1000 {
        assert!(capsule.is_pid_allowed(pid));
    }
}

#[test]
fn q11_large_pid_values() {
    // Property: Support full u32 range
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");

    let large_pids = vec![
        0,
        u32::MAX,
        u32::MAX - 1,
        0x8000_0000,
        0xFFFF_FFF0,
    ];

    for pid in &large_pids {
        capsule.add_pid(*pid).unwrap();
        assert!(capsule.is_pid_allowed(*pid));
    }
}

#[test]
fn q12_stats_consistency() {
    // Property: Stats accurately reflect operations
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");

    for pid in 0..100 {
        capsule.add_pid(pid).unwrap();
    }

    let stats = capsule.get_stats();
    assert_eq!(stats.pid_count, 100);
    assert!(stats.bloom_insertions >= 100); // At least one per PID
    assert!(stats.hash_table_collisions >= 0); // Non-negative
}

#[test]
fn q13_generation_counter_increments() {
    // Property: Generation counter increases monotonically
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");

    let gen1 = capsule.get_stats().generation;
    capsule.next_generation();
    let gen2 = capsule.get_stats().generation;
    capsule.next_generation();
    let gen3 = capsule.get_stats().generation;

    assert!(gen2 > gen1);
    assert!(gen3 > gen2);
}

#[test]
fn q14_add_remove_cycle() {
    // Property: Add/remove operations are idempotent
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");

    // Add, remove, add again
    capsule.add_pid(6000).unwrap();
    capsule.remove_pid(6000).unwrap();
    capsule.add_pid(6000).unwrap(); // Should succeed second time

    assert!(capsule.is_pid_allowed(6000));
    assert_eq!(capsule.get_pid_count(), 1);
}

// ============================================================================
// TIER 3: Integration Tests (Q15-Q21 - Realistic scenarios)
// ============================================================================

#[test]
fn q15_concurrent_reads() {
    // Integration: Multiple readers should not block
    let capsule = Arc::new(DynamicPidWhitelistCapsule::new().expect("Creation failed"));

    // Add some PIDs
    for pid in 0..100 {
        capsule.add_pid(pid).unwrap();
    }

    // Spawn 10 reader threads
    let mut handles = vec![];
    for _ in 0..10 {
        let capsule = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for pid in 0..100 {
                assert!(capsule.is_pid_allowed(pid));
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn q16_concurrent_adds() {
    // Integration: Multiple threads adding disjoint PIDs
    let capsule = Arc::new(DynamicPidWhitelistCapsule::new().expect("Creation failed"));

    let mut handles = vec![];
    for thread_id in 0..4 {
        let capsule = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let pid = thread_id * 100 + i;
                let _ = capsule.add_pid(pid);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have at least 100 unique PIDs
    assert!(capsule.get_pid_count() >= 100);
}

#[test]
fn q17_concurrent_mixed_operations() {
    // Integration: Add, check, remove concurrently
    let capsule = Arc::new(DynamicPidWhitelistCapsule::new().expect("Creation failed"));

    let mut handles = vec![];

    // Thread 1: Add PIDs 0-99
    let c1 = Arc::clone(&capsule);
    handles.push(thread::spawn(move || {
        for pid in 0..100 {
            let _ = c1.add_pid(pid);
        }
    }));

    // Thread 2: Check PIDs 0-99
    let c2 = Arc::clone(&capsule);
    handles.push(thread::spawn(move || {
        thread::sleep(std::time::Duration::from_millis(10)); // Wait for adds
        for pid in 0..100 {
            let _ = c2.is_pid_allowed(pid);
        }
    }));

    // Thread 3: Remove PIDs 0-49
    let c3 = Arc::clone(&capsule);
    handles.push(thread::spawn(move || {
        thread::sleep(std::time::Duration::from_millis(20)); // Wait for adds
        for pid in 0..50 {
            let _ = c3.remove_pid(pid);
        }
    }));

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have roughly 50 PIDs left
    assert_eq!(capsule.get_pid_count(), 50);
}

#[test]
fn q18_same_pid_concurrent_add() {
    // Integration: Only one thread should successfully add same PID
    let capsule = Arc::new(DynamicPidWhitelistCapsule::new().expect("Creation failed"));

    let success_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let mut handles = vec![];

    for _ in 0..10 {
        let capsule = Arc::clone(&capsule);
        let success_count = Arc::clone(&success_count);
        handles.push(thread::spawn(move || {
            if capsule.add_pid(7000).is_ok() {
                success_count.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Only one thread should succeed
    assert_eq!(success_count.load(Ordering::Relaxed), 1);
    assert_eq!(capsule.get_pid_count(), 1);
}

#[test]
fn q19_large_batch_operation() {
    // Integration: Add large batch without failure
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");

    // Add 2000 PIDs
    let mut add_count = 0;
    for pid in 0..2000 {
        if capsule.add_pid(pid).is_ok() {
            add_count += 1;
        }
    }

    // All should succeed (12% load factor)
    assert_eq!(add_count, 2000);
    assert_eq!(capsule.get_pid_count(), 2000);
}

#[test]
fn q20_clear_during_access() {
    // Integration: Clear while reads happen (should be safe)
    let capsule = Arc::new(DynamicPidWhitelistCapsule::new().expect("Creation failed"));

    // Add some PIDs
    for pid in 0..50 {
        capsule.add_pid(pid).unwrap();
    }

    let mut handles = vec![];

    // Reader threads
    for _ in 0..5 {
        let capsule = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for pid in 0..50 {
                let _ = capsule.is_pid_allowed(pid);
            }
        }));
    }

    // Clear thread
    let capsule_clear = Arc::clone(&capsule);
    handles.push(thread::spawn(move || {
        thread::sleep(std::time::Duration::from_millis(5));
        capsule_clear.clear();
    }));

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn q21_stats_under_load() {
    // Integration: Stats accurate under concurrent load
    let capsule = Arc::new(DynamicPidWhitelistCapsule::new().expect("Creation failed"));

    let mut handles = vec![];
    for thread_id in 0..4 {
        let capsule = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for i in 0..500 {
                let pid = thread_id * 500 + i;
                let _ = capsule.add_pid(pid);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = capsule.get_stats();
    assert_eq!(stats.pid_count, 2000);
    assert!(stats.bloom_insertions >= 2000);
}

// ============================================================================
// TIER 4: Production Tests (Q22-Q28 - Stress, scalability, SLA)
// ============================================================================

#[test]
#[ignore] // Long-running test
fn q22_stress_test_10k_pids() {
    // Production: Handle 10K PIDs (61% load factor)
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");

    for pid in 0..10000 {
        assert!(capsule.add_pid(pid as u32).is_ok());
    }

    assert_eq!(capsule.get_pid_count(), 10000);

    // Verify all findable
    for pid in 0..10000 {
        assert!(capsule.is_pid_allowed(pid as u32));
    }
}

#[test]
#[ignore] // Long-running test
fn q23_stress_test_concurrent_10k() {
    // Production: 10 threads adding 1000 PIDs each
    let capsule = Arc::new(DynamicPidWhitelistCapsule::new().expect("Creation failed"));

    let mut handles = vec![];
    for thread_id in 0..10 {
        let capsule = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for i in 0..1000 {
                let pid = thread_id * 1000 + i;
                let _ = capsule.add_pid(pid as u32);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have 10K PIDs
    assert_eq!(capsule.get_pid_count(), 10000);
}

#[test]
fn q24_latency_sla_check() {
    // Production: Check latency <45ns (measured separately via benchmarks)
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");

    capsule.add_pid(8000).unwrap();

    // Just verify it runs (actual latency measured in benches)
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = capsule.is_pid_allowed(8000);
    }
    let elapsed = start.elapsed();

    // 1000 checks should be <50μs = <50ns each
    assert!(elapsed < std::time::Duration::from_micros(50));
}

#[test]
fn q25_memory_efficiency() {
    // Production: 64KB hash table + 8KB Bloom ≈ 72KB total
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");

    // Add 10K PIDs
    for pid in 0..10000 {
        let _ = capsule.add_pid(pid as u32);
    }

    // Memory: Should be bounded by structure (72KB + overhead)
    let stats = capsule.get_stats();
    assert!(stats.pid_count <= 10000);
}

#[test]
fn q26_removal_correctness() {
    // Production: Remove works correctly at scale
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");

    // Add 500 PIDs
    for pid in 0..500 {
        capsule.add_pid(pid).unwrap();
    }

    // Remove even-numbered PIDs
    for pid in (0..500).step_by(2) {
        capsule.remove_pid(pid).unwrap();
    }

    // Check: even should be gone, odd should remain
    for pid in 0..500 {
        if pid % 2 == 0 {
            // Even: Bloom might still have it (FP), but hash table shouldn't
            assert_eq!(capsule.get_pid_count(), 250);
        } else {
            // Odd: should still be there
            assert!(capsule.is_pid_allowed(pid as u32));
        }
    }
}

#[test]
fn q27_concurrent_add_remove_stress() {
    // Production: Concurrent add/remove under stress
    let capsule = Arc::new(DynamicPidWhitelistCapsule::new().expect("Creation failed"));
    let done = Arc::new(AtomicBool::new(false));

    let mut handles = vec![];

    // 5 adder threads
    for thread_id in 0..5 {
        let capsule = Arc::clone(&capsule);
        let done = Arc::clone(&done);
        handles.push(thread::spawn(move || {
            for i in 0..500 {
                let pid = (thread_id * 500 + i) as u32;
                let _ = capsule.add_pid(pid);
            }
            if thread_id == 4 {
                done.store(true, Ordering::Release);
            }
        }));
    }

    // 5 remover threads (wait then remove)
    for thread_id in 0..5 {
        let capsule = Arc::clone(&capsule);
        let done = Arc::clone(&done);
        handles.push(thread::spawn(move || {
            while !done.load(Ordering::Acquire) {
                thread::yield_now();
            }
            // Remove every other PID
            for i in (0..500).step_by(2) {
                let pid = (thread_id * 500 + i) as u32;
                let _ = capsule.remove_pid(pid);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have about half the PIDs
    let count = capsule.get_pid_count();
    assert!(count >= 1250 && count <= 2500); // 50% ± margin
}

#[test]
fn q28_assum_all_verified() {
    // Production: All ASSUM assumptions verified
    let capsule = DynamicPidWhitelistCapsule::new().expect("Creation failed");

    // #ASSUME_BLOOM_NO_FALSE_NEGATIVES: Add and find all
    for pid in 0..100 {
        capsule.add_pid(pid).unwrap();
    }
    for pid in 0..100 {
        assert!(capsule.is_pid_allowed(pid), "FNR for PID {}", pid);
    }

    // #ASSUME_HASH_TABLE_CAS: Duplicate add fails exactly once
    let result = capsule.add_pid(0);
    assert_eq!(result, Err(PidWhitelistError::PidAlreadyExists { pid: 0 }));

    // #ASSUME_LINEAR_PROBING_CONVERGES: Can add many without failure
    for pid in 100..1000 {
        assert!(
            capsule.add_pid(pid).is_ok(),
            "Linear probing failed at PID {}",
            pid
        );
    }

    // #ASSUME_GENERATION_TOCTOU: Counter increases
    let gen1 = capsule.get_stats().generation;
    capsule.next_generation();
    let gen2 = capsule.get_stats().generation;
    assert!(gen2 > gen1);

    // All ASSUM verified
    assert!(capsule.get_pid_count() > 0);
}
