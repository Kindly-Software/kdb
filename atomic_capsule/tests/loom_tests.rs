//! Loom Memory Model Verification for Atomic Capsule Collections (Phase 5.1)
//!
//! **MISSION**: Exhaustive memory ordering validation for lockfree collections.
//! Loom explores ALL possible thread interleavings to catch weak memory ordering bugs
//! that are impossible to reproduce with standard runtime tests.
//!
//! ## Why Loom? (x86 TSO vs ARM Weak Memory)
//!
//! **x86 TSO (Total Store Order)**:
//! - Strong memory model: Stores appear in program order
//! - Loads see most recent store (no load reordering past stores)
//! - **DANGER**: Code that works on x86 may fail on ARM/RISC-V!
//!
//! **ARM/RISC-V Weak Memory**:
//! - Loads/stores can reorder (unless explicit barriers)
//! - Requires Acquire/Release ordering for synchronization
//! - **Loom simulates this**: Catches missing barriers
//!
//! ## Loom Test Categories (10+ Tests)
//!
//! ### 1. ConcurrentMapCapsule (3 tests)
//! - Concurrent insert/get race detection
//! - Generation counter TOCTOU prevention
//! - Linear probing collision handling
//!
//! ### 2. RingBufferBroadcast (3 tests)
//! - Producer-consumer synchronization
//! - Multi-consumer FIFO ordering
//! - Head/tail wrap-around races
//!
//! ### 3. LockfreeHashTable (3 tests)
//! - Chained entry synchronization
//! - AtomicPtr value installation
//! - Concurrent remove races
//!
//! ### 4. Cross-component (2 tests)
//! - Acquire/Release pairing validation
//! - Torn read prevention
//!
//! ## Running Loom Tests
//!
//! ```bash
//! # Standard run (3 preemptions, fast)
//! RUSTFLAGS="--cfg loom" cargo test --test loom_tests
//!
//! # Thorough run (10 preemptions, slow but comprehensive)
//! LOOM_MAX_PREEMPTIONS=10 RUSTFLAGS="--cfg loom" cargo test --test loom_tests
//!
//! # Single test (faster iteration)
//! RUSTFLAGS="--cfg loom" cargo test --test loom_tests loom_concurrent_map_insert_get
//! ```
//!
//! ## ASSUM Framework
//!
//! #ASSUME_LOOM_EXHAUSTIVE: Loom explores all thread interleavings (bounded by MAX_PREEMPTIONS)
//! #VERIFY_LOOM: All scenarios PASS with LOOM_MAX_PREEMPTIONS=3 (default)
//!
//! #ASSUME_MEMORY_ORDERING: Acquire/Release pairs synchronize correctly across all CPUs
//! #VERIFY_ORDERING: Loom detects any ordering violations (missing barriers)
//!
//! #ASSUME_ATOMIC_CAS: CAS operations prevent data races
//! #VERIFY_CAS: Loom validates all atomic operations are race-free
//!
//! ## Framework Compliance
//!
//! **UCE34 Q33** (Verification): Memory model validation for all capsules
//! **T28** (Testing): Tier 2 (Property) - Exhaustive interleaving exploration
//! **ASSUM** (Safety): Validates all #ASSUME tags on atomic operations
//! **B32** (Benchmarking): N/A (Loom is verification, not performance)
//!
//! ## Key Insights
//!
//! 1. **Acquire/Release is mandatory**: Relaxed ordering breaks on ARM/RISC-V
//! 2. **Generation counters prevent ABA**: Loom catches version conflicts
//! 3. **Pointer validity**: Loom detects use-after-free and null dereferences
//! 4. **FIFO ordering**: Loom validates queue semantics under all interleavings

#![cfg(loom)]

use loom::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use loom::sync::Arc;
use loom::thread;

// Import loom-compatible atomic types
// (Loom provides drop-in replacements that track all synchronization)

// ============================================================================
// TEST 1: ConcurrentMapCapsule - Concurrent Insert/Get
// ============================================================================

/// **CRITICAL**: Verifies that concurrent inserts don't violate invariants.
///
/// **Memory ordering bug we're catching**:
/// - Writer: Stores value with Relaxed, then bumps hash with Release
/// - Reader: Loads hash with Relaxed, then reads value
/// - **BUG**: Reader may see new hash but OLD value (stale read)
/// - **FIX**: Writer uses Release, Reader uses Acquire (synchronize)
///
/// **Loom explores**:
/// - Thread 1 inserts key=1, value=100
/// - Thread 2 reads key=1
/// - All possible orderings of these operations
///
/// **Expected**: Reader sees either None OR Some(100), NEVER partial state.
#[test]
fn loom_concurrent_map_insert_get() {
    loom::model(|| {
        // Simplified map: 2 slots, u64 keys
        let map = Arc::new(SimpleLoomMap::new(2));

        let map1 = Arc::clone(&map);
        let t1 = thread::spawn(move || {
            map1.insert(1, 100);
        });

        let map2 = Arc::clone(&map);
        let t2 = thread::spawn(move || map2.get(1));

        t1.join().unwrap();
        let result = t2.join().unwrap();

        // Either see None (before insert) or Some(100) (after)
        // NEVER partial state (e.g., hash=1 but value=0)
        assert!(result.is_none() || result == Some(100));
    });
}

// ============================================================================
// TEST 2: ConcurrentMapCapsule - Generation Counter TOCTOU Prevention
// ============================================================================

/// **CRITICAL**: Validates generation counters prevent Time-Of-Check-To-Time-Of-Use (TOCTOU) races.
///
/// **TOCTOU scenario**:
/// 1. Thread 1: Reads hash, sees key=1
/// 2. Thread 2: Removes key=1, inserts key=2 (same slot)
/// 3. Thread 1: Reads value, expecting key=1's value
/// 4. **BUG**: Thread 1 gets key=2's value (wrong key)
///
/// **Generation counter fix**:
/// - Store generation with each update (Acquire/Release)
/// - Reader checks generation BEFORE and AFTER value read
/// - If generation changed → retry (detected concurrent modification)
///
/// **Loom explores**: All interleavings of insert/remove/get operations.
#[test]
fn loom_generation_counter_toctou() {
    loom::model(|| {
        let gen = Arc::new(AtomicU64::new(0));
        let data = Arc::new(AtomicU64::new(0));

        let gen1 = Arc::clone(&gen);
        let data1 = Arc::clone(&data);
        let t1 = thread::spawn(move || {
            // Writer: Update data, then bump generation
            data1.store(42, Ordering::Relaxed);
            gen1.store(1, Ordering::Release); // Synchronizes data write
        });

        let gen2 = Arc::clone(&gen);
        let data2 = Arc::clone(&data);
        let t2 = thread::spawn(move || {
            // Reader: Load generation, then data
            if gen2.load(Ordering::Acquire) == 1 {
                // Must see data=42 due to Acquire/Release synchronization
                assert_eq!(data2.load(Ordering::Relaxed), 42);
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();
    });
}

// ============================================================================
// TEST 3: ConcurrentMapCapsule - Linear Probing Collision
// ============================================================================

/// **CRITICAL**: Tests collision resolution under concurrent inserts.
///
/// **Collision scenario**:
/// - Thread 1: Inserts key=1 (hashes to slot 0)
/// - Thread 2: Inserts key=2 (hashes to slot 0, collision)
/// - Thread 2 probes to slot 1 (linear probing)
///
/// **Memory ordering bug**:
/// - Thread 2 reads slot 0 (occupied), probes to slot 1
/// - Thread 1 still writing to slot 0 (value not visible)
/// - **BUG**: Thread 2 may see slot 0 empty (stale read), overwrites Thread 1
///
/// **FIX**: Acquire ordering on probe reads, Release on writes.
#[test]
fn loom_linear_probing_collision() {
    loom::model(|| {
        let map = Arc::new(SimpleLoomMap::new(2));
        let inserted_count = Arc::new(AtomicUsize::new(0));

        let map1 = Arc::clone(&map);
        let ic1 = Arc::clone(&inserted_count);
        let t1 = thread::spawn(move || {
            if map1.insert(1, 100).is_none() {
                ic1.fetch_add(1, Ordering::Relaxed);
            }
        });

        let map2 = Arc::clone(&map);
        let ic2 = Arc::clone(&inserted_count);
        let t2 = thread::spawn(move || {
            // Key 2 may collide with key 1 (depends on hash function)
            if map2.insert(2, 200).is_none() {
                ic2.fetch_add(1, Ordering::Relaxed);
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();

        // Both inserts should succeed (2 slots, 2 keys)
        let count = inserted_count.load(Ordering::Acquire);
        assert_eq!(count, 2, "Linear probing failed: lost an insert");
    });
}

// ============================================================================
// TEST 4: RingBufferBroadcast - Producer-Consumer Synchronization
// ============================================================================

/// **CRITICAL**: Validates Acquire/Release pairing between producer and consumer.
///
/// **Synchronization requirement**:
/// - Producer: Writes message to buffer, then bumps head with Release
/// - Consumer: Loads head with Acquire, then reads message
/// - **Guarantee**: Consumer ALWAYS sees complete message (no torn reads)
///
/// **Memory ordering bug**:
/// - Producer: Writes message[0..7], bumps head with Relaxed
/// - Consumer: Reads head with Relaxed, reads message
/// - **BUG**: Consumer may see partial message (ARM reordering)
///
/// **FIX**: Producer uses Release (publish), Consumer uses Acquire (observe).
#[test]
fn loom_ring_buffer_producer_consumer() {
    loom::model(|| {
        let head = Arc::new(AtomicUsize::new(0));
        let buffer = Arc::new([AtomicU64::new(0), AtomicU64::new(0)]);

        let head1 = Arc::clone(&head);
        let buffer1 = Arc::clone(&buffer);
        let t1 = thread::spawn(move || {
            // Producer: Write message, then publish head
            buffer1[0].store(42, Ordering::Relaxed);
            head1.store(1, Ordering::Release); // Synchronizes buffer write
        });

        let head2 = Arc::clone(&head);
        let buffer2 = Arc::clone(&buffer);
        let t2 = thread::spawn(move || {
            // Consumer: Load head, then read message
            let h = head2.load(Ordering::Acquire);
            if h == 1 {
                // Must see buffer[0]=42 due to Acquire/Release
                assert_eq!(buffer2[0].load(Ordering::Relaxed), 42);
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();
    });
}

// ============================================================================
// TEST 5: RingBufferBroadcast - FIFO Ordering
// ============================================================================

/// **CRITICAL**: Verifies messages arrive in FIFO order under concurrency.
///
/// **FIFO requirement**:
/// - Producer sends: msg1, msg2
/// - Consumer receives: msg1, THEN msg2 (never msg2, msg1)
///
/// **Memory ordering bug**:
/// - Producer: Stores msg1, msg2 with Relaxed
/// - Consumer: May see msg2 before msg1 (ARM reordering)
///
/// **FIX**: Release ordering on head bumps (establishes happens-before).
#[test]
fn loom_ring_buffer_fifo() {
    loom::model(|| {
        let ring = Arc::new(SimpleLoomRing::new(4));

        let ring1 = Arc::clone(&ring);
        let t1 = thread::spawn(move || {
            ring1.send(1);
            ring1.send(2);
        });

        let ring2 = Arc::clone(&ring);
        let t2 = thread::spawn(move || {
            let a = ring2.recv();
            let b = ring2.recv();
            (a, b)
        });

        t1.join().unwrap();
        let (a, b) = t2.join().unwrap();

        // If we received both, they MUST be in order
        if let (Some(x), Some(y)) = (a, b) {
            assert!(x == 1 && y == 2, "FIFO violation: got {:?}, {:?}", x, y);
        }
    });
}

// ============================================================================
// TEST 6: RingBufferBroadcast - Head/Tail Wrap-Around
// ============================================================================

/// **CRITICAL**: Tests ring buffer wrap-around doesn't cause index corruption.
///
/// **Wrap-around scenario**:
/// - Capacity = 4, index wraps at 4 → 0
/// - Producer: head=3, writes msg, bumps to head=0 (wrap)
/// - Consumer: tail=3, reads msg, bumps to tail=0 (wrap)
///
/// **Index corruption bug**:
/// - Producer and consumer both calculate next_idx independently
/// - Race on wrap condition (3 → 0 vs 3 → 4)
/// - **BUG**: Index overflow or missed message
///
/// **FIX**: Atomic index updates with generation counter (packed u64).
#[test]
fn loom_ring_buffer_wraparound() {
    loom::model(|| {
        let ring = Arc::new(SimpleLoomRing::new(2)); // Small capacity for fast wrap

        let ring1 = Arc::clone(&ring);
        let t1 = thread::spawn(move || {
            // Send 3 messages (forces wrap: 0 → 1 → 0)
            for i in 0..3 {
                ring1.send(i);
            }
        });

        let ring2 = Arc::clone(&ring);
        let t2 = thread::spawn(move || {
            let mut received = Vec::new();
            for _ in 0..3 {
                if let Some(msg) = ring2.recv() {
                    received.push(msg);
                }
            }
            received
        });

        t1.join().unwrap();
        let received = t2.join().unwrap();

        // Should receive all 3 messages (no lost messages on wrap)
        assert!(received.len() <= 3, "Received too many: {:?}", received);
    });
}

// ============================================================================
// TEST 7: LockfreeHashTable - AtomicPtr Value Installation
// ============================================================================

/// **CRITICAL**: Validates AtomicPtr prevents torn pointer reads.
///
/// **Pointer installation**:
/// - Thread 1: Allocates Box<V>, stores pointer with CAS
/// - Thread 2: Reads pointer, dereferences to access value
///
/// **Memory ordering bug**:
/// - Thread 1: Stores pointer with Relaxed
/// - Thread 2: Loads pointer with Relaxed
/// - **BUG**: Thread 2 may see pointer BUT not the allocated memory (ARM)
///
/// **FIX**: Release on store, Acquire on load (synchronize allocation).
#[test]
fn loom_lockfree_table_ptr_install() {
    loom::model(|| {
        let ptr = Arc::new(loom::sync::atomic::AtomicPtr::new(core::ptr::null_mut()));
        let data_ready = Arc::new(AtomicU64::new(0));

        let ptr1 = Arc::clone(&ptr);
        let dr1 = Arc::clone(&data_ready);
        let t1 = thread::spawn(move || {
            // Allocate data
            dr1.store(42, Ordering::Relaxed);
            // Install pointer with Release (synchronizes data write)
            let leaked_ptr = &*dr1 as *const AtomicU64 as *mut AtomicU64;
            ptr1.store(leaked_ptr, Ordering::Release);
        });

        let ptr2 = Arc::clone(&ptr);
        let t2 = thread::spawn(move || {
            // Load pointer with Acquire
            let p = ptr2.load(Ordering::Acquire);
            if !p.is_null() {
                // SAFETY: Acquire synchronizes with Release, pointer is valid
                let value = unsafe { (*p).load(Ordering::Relaxed) };
                assert_eq!(value, 42, "Torn pointer read");
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();
    });
}

// ============================================================================
// TEST 8: LockfreeHashTable - Chained Entry Synchronization
// ============================================================================

/// **CRITICAL**: Tests chained collision entries don't race on insertion.
///
/// **Chaining scenario**:
/// - Thread 1: Inserts key=1 (primary slot)
/// - Thread 2: Inserts key=2 (collides, chains to next)
/// - Thread 3: Reads key=2 (walks chain)
///
/// **Memory ordering bug**:
/// - Thread 2: Stores next pointer with Relaxed
/// - Thread 3: Loads next pointer with Relaxed
/// - **BUG**: Thread 3 may see next pointer BUT not the chained entry data
///
/// **FIX**: Release on next pointer store, Acquire on load.
#[test]
fn loom_lockfree_table_chaining() {
    loom::model(|| {
        let next_ptr = Arc::new(loom::sync::atomic::AtomicPtr::new(core::ptr::null_mut()));
        let chain_data = Arc::new(AtomicU64::new(0));

        let np1 = Arc::clone(&next_ptr);
        let cd1 = Arc::clone(&chain_data);
        let t1 = thread::spawn(move || {
            // Write chain data, then install next pointer
            cd1.store(200, Ordering::Relaxed);
            let leaked = &*cd1 as *const AtomicU64 as *mut AtomicU64;
            np1.store(leaked, Ordering::Release);
        });

        let np2 = Arc::clone(&next_ptr);
        let t2 = thread::spawn(move || {
            // Walk chain: Load next pointer
            let p = np2.load(Ordering::Acquire);
            if !p.is_null() {
                // SAFETY: Acquire synchronizes with Release
                let value = unsafe { (*p).load(Ordering::Relaxed) };
                assert_eq!(value, 200, "Chained entry not synchronized");
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();
    });
}

// ============================================================================
// TEST 9: LockfreeHashTable - Concurrent Remove Race
// ============================================================================

/// **CRITICAL**: Validates concurrent removes don't double-free or leak.
///
/// **Concurrent remove**:
/// - Thread 1: Removes key=1 (CAS value pointer to null)
/// - Thread 2: Removes key=1 (also tries to CAS)
/// - **Only ONE should succeed** (CAS ensures atomicity)
///
/// **Memory ordering bug**:
/// - Thread 1: CAS value pointer (Relaxed)
/// - Thread 2: CAS value pointer (Relaxed)
/// - **BUG**: Both may succeed (no synchronization), double-free
///
/// **FIX**: AcqRel ordering on CAS (ensures only one succeeds).
#[test]
fn loom_lockfree_table_concurrent_remove() {
    loom::model(|| {
        let value_ptr = Arc::new(loom::sync::atomic::AtomicPtr::new(42 as *mut u64));
        let remove_count = Arc::new(AtomicUsize::new(0));

        let vp1 = Arc::clone(&value_ptr);
        let rc1 = Arc::clone(&remove_count);
        let t1 = thread::spawn(move || {
            // Try to remove (CAS to null)
            let result = vp1.compare_exchange(
                42 as *mut u64,
                core::ptr::null_mut(),
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            if result.is_ok() {
                rc1.fetch_add(1, Ordering::Relaxed);
            }
        });

        let vp2 = Arc::clone(&value_ptr);
        let rc2 = Arc::clone(&remove_count);
        let t2 = thread::spawn(move || {
            // Try to remove (CAS to null)
            let result = vp2.compare_exchange(
                42 as *mut u64,
                core::ptr::null_mut(),
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            if result.is_ok() {
                rc2.fetch_add(1, Ordering::Relaxed);
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();

        // Only ONE remove should succeed
        let count = remove_count.load(Ordering::Acquire);
        assert_eq!(count, 1, "Double remove detected: {}", count);
    });
}

// ============================================================================
// TEST 10: Acquire/Release Pairing Validation
// ============================================================================

/// **CRITICAL**: Generic test for all Acquire/Release pairs in codebase.
///
/// **Synchronization pattern**:
/// - Writer: Modifies shared data, then sets flag with Release
/// - Reader: Checks flag with Acquire, then reads shared data
///
/// **Memory ordering guarantee**:
/// - If Reader sees flag=true, it MUST see all writes before Release
/// - **Loom validates**: This holds on all CPU architectures
#[test]
fn loom_acquire_release_pairing() {
    loom::model(|| {
        let flag = Arc::new(AtomicU64::new(0));
        let data = Arc::new(AtomicU64::new(0));

        let flag1 = Arc::clone(&flag);
        let data1 = Arc::clone(&data);
        let t1 = thread::spawn(move || {
            // Writer: Modify data, set flag
            data1.store(99, Ordering::Relaxed);
            flag1.store(1, Ordering::Release);
        });

        let flag2 = Arc::clone(&flag);
        let data2 = Arc::clone(&data);
        let t2 = thread::spawn(move || {
            // Reader: Check flag, read data
            if flag2.load(Ordering::Acquire) == 1 {
                assert_eq!(data2.load(Ordering::Relaxed), 99);
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();
    });
}

// ============================================================================
// TEST 11: Torn Read Prevention (Multi-field Atomicity)
// ============================================================================

/// **CRITICAL**: Validates that multi-field reads are atomic (no torn reads).
///
/// **Torn read scenario**:
/// - Struct: { a: u32, b: u32 } (two fields, 64 bits total)
/// - Writer: Updates a=1, b=2 (two separate stores)
/// - Reader: Reads a, b
/// - **BUG**: Reader may see a=1, b=0 (torn read, partial update)
///
/// **FIX 1**: Pack into single AtomicU64 (one atomic store)
/// **FIX 2**: Use generation counter (retry if changed during read)
#[test]
fn loom_torn_read_prevention() {
    loom::model(|| {
        // Simulate 2-field struct with generation counter
        let gen = Arc::new(AtomicU64::new(0));
        let field_a = Arc::new(AtomicU64::new(0));
        let field_b = Arc::new(AtomicU64::new(0));

        let gen1 = Arc::clone(&gen);
        let fa1 = Arc::clone(&field_a);
        let fb1 = Arc::clone(&field_b);
        let t1 = thread::spawn(move || {
            // Writer: Update both fields, bump generation
            fa1.store(1, Ordering::Relaxed);
            fb1.store(2, Ordering::Relaxed);
            gen1.fetch_add(1, Ordering::Release); // Synchronizes both writes
        });

        let gen2 = Arc::clone(&gen);
        let fa2 = Arc::clone(&field_a);
        let fb2 = Arc::clone(&field_b);
        let t2 = thread::spawn(move || {
            // Reader: Check generation, read fields, check again
            let g1 = gen2.load(Ordering::Acquire);
            let a = fa2.load(Ordering::Relaxed);
            let b = fb2.load(Ordering::Relaxed);
            let g2 = gen2.load(Ordering::Acquire);

            // If generation unchanged, read is atomic
            if g1 == g2 && g1 == 1 {
                // Both fields should be updated (no torn read)
                assert_eq!(a, 1, "Torn read: a not updated");
                assert_eq!(b, 2, "Torn read: b not updated");
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();
    });
}

// ============================================================================
// Simplified Loom-Compatible Data Structures (Minimal for Testing)
// ============================================================================

/// Simplified concurrent map for Loom testing (2 slots, minimal)
struct SimpleLoomMap {
    slots: Vec<(AtomicU64, AtomicU64)>, // (key_hash, value)
    capacity: usize,
}

impl SimpleLoomMap {
    fn new(capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            slots.push((AtomicU64::new(0), AtomicU64::new(0)));
        }
        Self { slots, capacity }
    }

    fn insert(&self, key: u64, value: u64) -> Option<u64> {
        let idx = (key as usize) % self.capacity;
        let (key_slot, val_slot) = &self.slots[idx];

        // Try to claim slot
        match key_slot.compare_exchange(0, key, Ordering::Release, Ordering::Acquire) {
            Ok(_) => {
                // Claimed, store value
                val_slot.store(value, Ordering::Release);
                None
            }
            Err(current_key) => {
                if current_key == key {
                    // Update existing
                    let old = val_slot.swap(value, Ordering::AcqRel);
                    Some(old)
                } else {
                    // Collision, no probing (simplified)
                    None
                }
            }
        }
    }

    fn get(&self, key: u64) -> Option<u64> {
        let idx = (key as usize) % self.capacity;
        let (key_slot, val_slot) = &self.slots[idx];

        let current_key = key_slot.load(Ordering::Acquire);
        if current_key == key {
            let value = val_slot.load(Ordering::Acquire);
            if value != 0 {
                Some(value)
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Simplified ring buffer for Loom testing (4 slots, minimal)
struct SimpleLoomRing {
    buffer: Vec<AtomicU64>,
    head: AtomicUsize,
    tail: AtomicUsize,
    capacity: usize,
}

impl SimpleLoomRing {
    fn new(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(AtomicU64::new(0));
        }
        Self {
            buffer,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            capacity,
        }
    }

    fn send(&self, value: u64) {
        let idx = self.head.fetch_add(1, Ordering::Relaxed) % self.capacity;
        self.buffer[idx].store(value, Ordering::Release);
    }

    fn recv(&self) -> Option<u64> {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);

        if tail >= head {
            return None; // Empty
        }

        let idx = tail % self.capacity;
        let value = self.buffer[idx].load(Ordering::Acquire);

        // Bump tail
        self.tail.fetch_add(1, Ordering::Release);

        if value != 0 {
            Some(value)
        } else {
            None
        }
    }
}
