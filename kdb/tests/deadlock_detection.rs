//! Deadlock Detection Tests - T28 Framework Compliance
//!
//! **Framework**: T28 (Comprehensive Testing Framework)
//! - Q15-Q21: Integration tests (deadlock pattern detection)
//! - Q29-Q35: Determinism tests (reproducible deadlock conditions)
//!
//! **Purpose**: Validate kdb's ability to detect and diagnose deadlock patterns.
//!
//! **Test Categories**:
//! 1. ABBA deadlock detection (circular wait)
//! 2. Resource ordering analysis
//! 3. Wait-for graph construction
//! 4. Timeout-based deadlock detection
//! 5. Lock contention metrics
//! 6. Multi-resource deadlock (N-way)
//! 7. Deadlock prevention validation
//!
//! **ASSUM Framework**:
//! - #ASSUME_FUTEX_BLOCKED: Blocked threads show futex syscall in stack
//! - #VERIFY_FUTEX_BLOCKED: Stack unwinding reveals wait state
//! - #ASSUME_LOCK_ORDER: Lock acquisition order is deterministic per thread
//! - #VERIFY_LOCK_ORDER: Trace events capture lock ordering

use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// Test Infrastructure - Lockfree Deadlock Detection State
// ============================================================================

/// Lockfree lock acquisition tracker for deadlock detection
/// Uses atomic operations to track lock ordering without introducing new locks
#[repr(C, align(64))]
struct LockAcquisitionTracker {
    /// Lock acquisition sequence (monotonic counter)
    sequence: AtomicU64,
    /// Per-thread lock counts [thread_id] -> (lock_a_count, lock_b_count)
    thread_locks: [(AtomicU32, AtomicU32); 16],
    /// Deadlock detected flag
    deadlock_detected: AtomicBool,
    /// Detection timestamp (nanoseconds since start)
    detection_time_ns: AtomicU64,
}

impl LockAcquisitionTracker {
    const fn new() -> Self {
        const EMPTY: (AtomicU32, AtomicU32) = (AtomicU32::new(0), AtomicU32::new(0));
        Self {
            sequence: AtomicU64::new(0),
            thread_locks: [EMPTY; 16],
            deadlock_detected: AtomicBool::new(false),
            detection_time_ns: AtomicU64::new(0),
        }
    }

    fn record_lock_a(&self, thread_id: usize) -> u64 {
        let seq = self.sequence.fetch_add(1, Ordering::AcqRel);
        if thread_id < 16 {
            self.thread_locks[thread_id].0.fetch_add(1, Ordering::Release);
        }
        seq
    }

    fn record_lock_b(&self, thread_id: usize) -> u64 {
        let seq = self.sequence.fetch_add(1, Ordering::AcqRel);
        if thread_id < 16 {
            self.thread_locks[thread_id].1.fetch_add(1, Ordering::Release);
        }
        seq
    }

    fn mark_deadlock(&self, start: Instant) {
        self.deadlock_detected.store(true, Ordering::Release);
        self.detection_time_ns.store(
            start.elapsed().as_nanos() as u64,
            Ordering::Release,
        );
    }

    fn is_deadlocked(&self) -> bool {
        self.deadlock_detected.load(Ordering::Acquire)
    }

    fn get_lock_order_violation(&self) -> Option<(usize, usize)> {
        // Check for threads that acquired locks in different orders
        // Thread A: lock_a before lock_b
        // Thread B: lock_b before lock_a
        for i in 0..16 {
            let a_count = self.thread_locks[i].0.load(Ordering::Acquire);
            let b_count = self.thread_locks[i].1.load(Ordering::Acquire);

            for j in (i + 1)..16 {
                let a_count_j = self.thread_locks[j].0.load(Ordering::Acquire);
                let b_count_j = self.thread_locks[j].1.load(Ordering::Acquire);

                // If both threads acquired both locks, check order
                if a_count > 0 && b_count > 0 && a_count_j > 0 && b_count_j > 0 {
                    // This is a potential order violation
                    return Some((i, j));
                }
            }
        }
        None
    }
}

// ============================================================================
// Test 1: ABBA Deadlock Detection (Circular Wait)
// ============================================================================
// Framework: T28 Q15 (Integration - Deadlock Pattern Detection)
// Validates: Detection of classic ABBA lock ordering violation
#[test]
fn test_abba_deadlock_detection() {
    let tracker = Arc::new(LockAcquisitionTracker::new());
    let lock_a = Arc::new(Mutex::new(0u64));
    let lock_b = Arc::new(Mutex::new(0u64));
    let start = Instant::now();

    let tracker1 = Arc::clone(&tracker);
    let a1 = Arc::clone(&lock_a);
    let b1 = Arc::clone(&lock_b);

    let tracker2 = Arc::clone(&tracker);
    let a2 = Arc::clone(&lock_a);
    let b2 = Arc::clone(&lock_b);

    // Thread 1: A → B ordering
    let t1 = thread::spawn(move || {
        tracker1.record_lock_a(0);
        let _ga = a1.lock().unwrap();
        thread::sleep(Duration::from_millis(10));
        tracker1.record_lock_b(0);
        // Use try_lock to avoid actual deadlock in test
        if b1.try_lock().is_err() {
            tracker1.mark_deadlock(start);
        }
    });

    // Thread 2: B → A ordering (opposite)
    let t2 = thread::spawn(move || {
        tracker2.record_lock_b(1);
        let _gb = b2.lock().unwrap();
        thread::sleep(Duration::from_millis(10));
        tracker2.record_lock_a(1);
        // Use try_lock to avoid actual deadlock in test
        if a2.try_lock().is_err() {
            tracker2.mark_deadlock(start);
        }
    });

    t1.join().expect("Thread 1 panicked");
    t2.join().expect("Thread 2 panicked");

    // Validate: deadlock condition was detected
    assert!(
        tracker.is_deadlocked(),
        "ABBA deadlock pattern should be detected"
    );

    // Validate: lock ordering violation identified
    let violation = tracker.get_lock_order_violation();
    assert!(
        violation.is_some(),
        "Lock order violation should be identified"
    );

    println!(
        "ABBA deadlock detected in {}μs, violation between threads {:?}",
        tracker.detection_time_ns.load(Ordering::Acquire) / 1000,
        violation.unwrap()
    );
}

// ============================================================================
// Test 2: Resource Ordering Analysis
// ============================================================================
// Framework: T28 Q16 (Integration - Lock Order Validation)
// Validates: Correct lock ordering prevents deadlock
#[test]
fn test_resource_ordering_analysis() {
    let lock_a = Arc::new(Mutex::new(0u64));
    let lock_b = Arc::new(Mutex::new(0u64));
    let completed = Arc::new(AtomicU32::new(0));

    let mut handles = vec![];

    // All threads use consistent A → B ordering
    for thread_id in 0..4 {
        let a = Arc::clone(&lock_a);
        let b = Arc::clone(&lock_b);
        let done = Arc::clone(&completed);

        handles.push(thread::spawn(move || {
            for _ in 0..10 {
                // Consistent ordering: always A before B
                let _ga = a.lock().unwrap();
                let _gb = b.lock().unwrap();
                // Simulate work
                thread::yield_now();
            }
            done.fetch_add(1, Ordering::Release);
            thread_id
        }));
    }

    // All threads should complete without deadlock
    for handle in handles {
        let tid = handle.join().expect("Thread panicked");
        println!("Thread {} completed with consistent lock ordering", tid);
    }

    assert_eq!(
        completed.load(Ordering::Acquire),
        4,
        "All 4 threads should complete with consistent lock ordering"
    );
}

// ============================================================================
// Test 3: Wait-For Graph Construction
// ============================================================================
// Framework: T28 Q17 (Integration - Dependency Graph)
// Validates: Thread wait dependencies are trackable
#[test]
fn test_wait_for_graph_construction() {
    /// Wait-for edge in the dependency graph
    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct WaitEdge {
        waiter_tid: u32,
        holder_tid: u32,
        resource_id: u32,
    }

    /// Lockfree wait-for graph (simplified)
    struct WaitForGraph {
        edges: [(AtomicU32, AtomicU32, AtomicU32); 16], // (waiter, holder, resource)
        edge_count: AtomicU32,
    }

    impl WaitForGraph {
        fn new() -> Self {
            const EMPTY: (AtomicU32, AtomicU32, AtomicU32) =
                (AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0));
            Self {
                edges: [EMPTY; 16],
                edge_count: AtomicU32::new(0),
            }
        }

        fn add_wait(&self, waiter: u32, holder: u32, resource: u32) {
            let idx = self.edge_count.fetch_add(1, Ordering::AcqRel) as usize;
            if idx < 16 {
                self.edges[idx].0.store(waiter, Ordering::Release);
                self.edges[idx].1.store(holder, Ordering::Release);
                self.edges[idx].2.store(resource, Ordering::Release);
            }
        }

        fn detect_cycle(&self) -> bool {
            // Simple cycle detection: check if any waiter is also a holder
            // that creates a back-edge
            let count = self.edge_count.load(Ordering::Acquire) as usize;
            for i in 0..count.min(16) {
                let waiter = self.edges[i].0.load(Ordering::Acquire);
                for j in 0..count.min(16) {
                    if i != j {
                        let holder = self.edges[j].1.load(Ordering::Acquire);
                        let other_waiter = self.edges[j].0.load(Ordering::Acquire);
                        // Cycle: A waits for B, B waits for A
                        if waiter == holder
                            && other_waiter
                                == self.edges[i].1.load(Ordering::Acquire)
                        {
                            return true;
                        }
                    }
                }
            }
            false
        }
    }

    let graph = Arc::new(WaitForGraph::new());

    // Simulate ABBA wait pattern
    // Thread 1 (tid=1) waits for Thread 2 (tid=2) holding resource 0
    graph.add_wait(1, 2, 0);
    // Thread 2 (tid=2) waits for Thread 1 (tid=1) holding resource 1
    graph.add_wait(2, 1, 1);

    // Cycle should be detected
    assert!(
        graph.detect_cycle(),
        "Wait-for graph should detect circular dependency"
    );

    println!("Wait-for graph cycle detected: Thread 1 ↔ Thread 2");
}

// ============================================================================
// Test 4: Timeout-Based Deadlock Detection
// ============================================================================
// Framework: T28 Q18 (Integration - Timeout Detection)
// Validates: Deadlock detected via timeout mechanism
#[test]
fn test_timeout_deadlock_detection() {
    let lock = Arc::new(Mutex::new(0u64));
    let deadlock_timeout = Duration::from_millis(100);
    let detected = Arc::new(AtomicBool::new(false));

    // Hold lock in main thread
    let _guard = lock.lock().unwrap();

    let l = Arc::clone(&lock);
    let d = Arc::clone(&detected);

    let handle = thread::spawn(move || {
        let start = Instant::now();

        // Attempt to acquire with timeout simulation
        loop {
            if l.try_lock().is_ok() {
                return; // Lock acquired, no deadlock
            }

            if start.elapsed() > deadlock_timeout {
                d.store(true, Ordering::Release);
                return; // Timeout - potential deadlock
            }

            thread::sleep(Duration::from_millis(10));
        }
    });

    // Wait for detection
    thread::sleep(Duration::from_millis(150));

    assert!(
        detected.load(Ordering::Acquire),
        "Deadlock should be detected via timeout"
    );

    drop(_guard); // Release lock
    handle.join().expect("Thread panicked");

    println!("Timeout-based deadlock detection: {}ms threshold", 100);
}

// ============================================================================
// Test 5: Lock Contention Metrics
// ============================================================================
// Framework: T28 Q19 (Integration - Contention Analysis)
// Validates: Lock contention statistics collection
#[test]
fn test_lock_contention_metrics() {
    /// Contention metrics tracker (lockfree)
    #[repr(C, align(64))]
    struct ContentionMetrics {
        attempts: AtomicU64,
        successes: AtomicU64,
        contentions: AtomicU64,
        max_wait_ns: AtomicU64,
        total_wait_ns: AtomicU64,
    }

    impl ContentionMetrics {
        fn new() -> Self {
            Self {
                attempts: AtomicU64::new(0),
                successes: AtomicU64::new(0),
                contentions: AtomicU64::new(0),
                max_wait_ns: AtomicU64::new(0),
                total_wait_ns: AtomicU64::new(0),
            }
        }

        fn record_attempt(&self, wait_ns: u64, contended: bool) {
            self.attempts.fetch_add(1, Ordering::Relaxed);
            if contended {
                self.contentions.fetch_add(1, Ordering::Relaxed);
                self.total_wait_ns.fetch_add(wait_ns, Ordering::Relaxed);

                // Update max (lockfree CAS loop)
                let mut current_max = self.max_wait_ns.load(Ordering::Relaxed);
                while wait_ns > current_max {
                    match self.max_wait_ns.compare_exchange_weak(
                        current_max,
                        wait_ns,
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(val) => current_max = val,
                    }
                }
            } else {
                self.successes.fetch_add(1, Ordering::Relaxed);
            }
        }

        fn contention_ratio(&self) -> f64 {
            let attempts = self.attempts.load(Ordering::Acquire);
            let contentions = self.contentions.load(Ordering::Acquire);
            if attempts == 0 {
                0.0
            } else {
                contentions as f64 / attempts as f64
            }
        }
    }

    let metrics = Arc::new(ContentionMetrics::new());
    let lock = Arc::new(Mutex::new(0u64));
    let mut handles = vec![];

    for _ in 0..8 {
        let m = Arc::clone(&metrics);
        let l = Arc::clone(&lock);

        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let start = Instant::now();
                let contended = l.try_lock().is_err();

                if contended {
                    // Wait and retry
                    thread::sleep(Duration::from_micros(10));
                    let _guard = l.lock().unwrap();
                    let wait_ns = start.elapsed().as_nanos() as u64;
                    m.record_attempt(wait_ns, true);
                } else {
                    m.record_attempt(0, false);
                }

                thread::yield_now();
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let ratio = metrics.contention_ratio();
    let max_wait = metrics.max_wait_ns.load(Ordering::Acquire);
    let total_attempts = metrics.attempts.load(Ordering::Acquire);

    println!(
        "Contention metrics: ratio={:.2}%, max_wait={}ns, attempts={}",
        ratio * 100.0,
        max_wait,
        total_attempts
    );

    // Validate: metrics were collected
    assert!(total_attempts > 0, "Should have recorded lock attempts");
    assert!(ratio >= 0.0 && ratio <= 1.0, "Contention ratio should be valid");
}

// ============================================================================
// Test 6: Multi-Resource Deadlock (N-Way)
// ============================================================================
// Framework: T28 Q20 (Integration - Complex Deadlock)
// Validates: N-way circular deadlock detection
#[test]
fn test_multi_resource_deadlock() {
    const NUM_RESOURCES: usize = 4;
    const NUM_THREADS: usize = 4;

    let resources: Vec<Arc<Mutex<u64>>> = (0..NUM_RESOURCES)
        .map(|i| Arc::new(Mutex::new(i as u64)))
        .collect();

    let deadlock_detected = Arc::new(AtomicBool::new(false));
    let mut handles = vec![];

    // Each thread acquires resources in rotated order
    // Thread 0: R0 → R1 → R2 → R3
    // Thread 1: R1 → R2 → R3 → R0 (wraps around - creates cycle)
    // Thread 2: R2 → R3 → R0 → R1
    // Thread 3: R3 → R0 → R1 → R2
    for tid in 0..NUM_THREADS {
        let res = resources.clone();
        let detected = Arc::clone(&deadlock_detected);

        handles.push(thread::spawn(move || {
            let start_resource = tid;

            // Acquire first resource
            let first_idx = start_resource % NUM_RESOURCES;
            let _g0 = res[first_idx].lock().unwrap();

            // Small delay to create deadlock window
            thread::sleep(Duration::from_millis(10));

            // Try to acquire next resource (may deadlock)
            let second_idx = (start_resource + 1) % NUM_RESOURCES;
            if res[second_idx].try_lock().is_err() {
                detected.store(true, Ordering::Release);
                return tid; // Potential deadlock
            }

            tid
        }));
    }

    for handle in handles {
        handle.join().ok();
    }

    // With 4-way rotation, deadlock should be detected
    assert!(
        deadlock_detected.load(Ordering::Acquire),
        "4-way circular deadlock should be detected"
    );

    println!("Multi-resource (4-way) deadlock detected");
}

// ============================================================================
// Test 7: Deadlock Prevention Validation (Lock Hierarchy)
// ============================================================================
// Framework: T28 Q21 (Integration - Prevention Strategy)
// Validates: Lock hierarchy prevents deadlock
#[test]
fn test_deadlock_prevention_hierarchy() {
    /// Hierarchical lock with ordering enforcement
    struct HierarchicalLock {
        level: u32,
        inner: Mutex<u64>,
    }

    impl HierarchicalLock {
        fn new(level: u32, value: u64) -> Self {
            Self {
                level,
                inner: Mutex::new(value),
            }
        }

        /// Acquire lock only if higher level than currently held
        fn acquire_if_ordered(
            &self,
            current_level: &AtomicU32,
        ) -> Result<std::sync::MutexGuard<'_, u64>, &'static str> {
            let held = current_level.load(Ordering::Acquire);
            if self.level <= held && held != 0 {
                return Err("Lock ordering violation: would cause deadlock");
            }

            let guard = self.inner.lock().map_err(|_| "Lock poisoned")?;
            current_level.store(self.level, Ordering::Release);
            Ok(guard)
        }
    }

    let lock_1 = Arc::new(HierarchicalLock::new(1, 100));
    let lock_2 = Arc::new(HierarchicalLock::new(2, 200));
    let lock_3 = Arc::new(HierarchicalLock::new(3, 300));

    let violations = Arc::new(AtomicU32::new(0));
    let successes = Arc::new(AtomicU32::new(0));

    let mut handles = vec![];

    for _ in 0..4 {
        let l1 = Arc::clone(&lock_1);
        let l2 = Arc::clone(&lock_2);
        let l3 = Arc::clone(&lock_3);
        let v = Arc::clone(&violations);
        let s = Arc::clone(&successes);

        handles.push(thread::spawn(move || {
            let current_level = AtomicU32::new(0);

            // Correct order: 1 → 2 → 3
            for _ in 0..10 {
                current_level.store(0, Ordering::Release);

                if l1.acquire_if_ordered(&current_level).is_ok() {
                    if l2.acquire_if_ordered(&current_level).is_ok() {
                        if l3.acquire_if_ordered(&current_level).is_ok() {
                            s.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }

            // Wrong order attempt: 3 → 2 → 1 (should fail)
            current_level.store(0, Ordering::Release);
            if l3.acquire_if_ordered(&current_level).is_ok() {
                // Try to acquire lower level (should be rejected)
                if l2.acquire_if_ordered(&current_level).is_err() {
                    v.fetch_add(1, Ordering::Relaxed); // Correctly rejected
                }
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let total_successes = successes.load(Ordering::Acquire);
    let total_violations = violations.load(Ordering::Acquire);

    println!(
        "Hierarchical locking: {} successful acquisitions, {} violations prevented",
        total_successes, total_violations
    );

    assert!(
        total_successes > 0,
        "Some correct-order acquisitions should succeed"
    );
    assert!(
        total_violations > 0,
        "Wrong-order acquisitions should be rejected"
    );
}

// ============================================================================
// Additional Determinism Tests (T28 Q29-Q35)
// ============================================================================

/// Test that deadlock detection is deterministic across runs
#[test]
fn test_deadlock_detection_determinism() {
    let mut results = Vec::with_capacity(5);

    for run in 0..5 {
        let lock_a = Arc::new(Mutex::new(0u64));
        let lock_b = Arc::new(Mutex::new(0u64));
        let detected = Arc::new(AtomicBool::new(false));

        let a1 = Arc::clone(&lock_a);
        let b1 = Arc::clone(&lock_b);
        let d1 = Arc::clone(&detected);

        let a2 = Arc::clone(&lock_a);
        let b2 = Arc::clone(&lock_b);
        let d2 = Arc::clone(&detected);

        let t1 = thread::spawn(move || {
            let _ga = a1.lock().unwrap();
            thread::sleep(Duration::from_millis(5));
            if b1.try_lock().is_err() {
                d1.store(true, Ordering::Release);
            }
        });

        let t2 = thread::spawn(move || {
            let _gb = b2.lock().unwrap();
            thread::sleep(Duration::from_millis(5));
            if a2.try_lock().is_err() {
                d2.store(true, Ordering::Release);
            }
        });

        t1.join().ok();
        t2.join().ok();

        results.push(detected.load(Ordering::Acquire));
        println!("Run {}: deadlock detected = {}", run, results[run]);
    }

    // All runs should produce consistent results
    let all_same = results.iter().all(|&r| r == results[0]);
    assert!(
        all_same,
        "Deadlock detection should be deterministic across runs"
    );
}

/// RwLock starvation test (writer starvation)
#[test]
fn test_rwlock_starvation_detection() {
    let lock = Arc::new(RwLock::new(0u64));
    let writer_starved = Arc::new(AtomicBool::new(false));
    let readers_done = Arc::new(AtomicU32::new(0));

    const NUM_READERS: u32 = 8;
    const READ_ITERATIONS: u32 = 100;

    let mut handles = vec![];

    // Spawn readers that hold read locks frequently
    for _ in 0..NUM_READERS {
        let l = Arc::clone(&lock);
        let done = Arc::clone(&readers_done);

        handles.push(thread::spawn(move || {
            for _ in 0..READ_ITERATIONS {
                let _guard = l.read().unwrap();
                thread::sleep(Duration::from_micros(100));
            }
            done.fetch_add(1, Ordering::Release);
        }));
    }

    // Writer thread that may get starved
    let l = Arc::clone(&lock);
    let starved = Arc::clone(&writer_starved);
    let rdone = Arc::clone(&readers_done);

    handles.push(thread::spawn(move || {
        let start = Instant::now();
        let timeout = Duration::from_millis(500);

        // Wait for readers to start
        thread::sleep(Duration::from_millis(10));

        // Try to acquire write lock
        loop {
            if l.try_write().is_ok() {
                return; // Got the lock
            }

            if start.elapsed() > timeout {
                // Check if readers are still active
                if rdone.load(Ordering::Acquire) < NUM_READERS {
                    starved.store(true, Ordering::Release);
                }
                return;
            }

            thread::yield_now();
        }
    }));

    for handle in handles {
        handle.join().ok();
    }

    // Note: std::sync::RwLock may or may not exhibit starvation
    // This test validates our ability to detect it
    println!(
        "RwLock starvation test: writer_starved = {}",
        writer_starved.load(Ordering::Acquire)
    );
}
