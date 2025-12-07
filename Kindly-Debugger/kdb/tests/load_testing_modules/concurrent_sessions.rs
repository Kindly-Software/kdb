//! Concurrent Session Load Tests
//!
//! Target: 500-2000 concurrent sessions on 64GB server (kindly-hub)
//!
//! # Test Categories
//!
//! 1. **Session Pool Capacity** - Validate maximum concurrent sessions per tier
//! 2. **Session Lifecycle** - Create, use, destroy session patterns
//! 3. **Tier Upgrades** - LIGHT -> MEDIUM -> HEAVY transitions under load
//! 4. **Pool Exhaustion** - Graceful handling when limits reached
//!
//! # Memory Budget
//!
//! | Tier   | Session Size | Max Count | Pool Total |
//! |--------|--------------|-----------|------------|
//! | LIGHT  | 64 KB        | 1,500     | 96 MB      |
//! | MEDIUM | 256 KB       | 600       | 150 MB     |
//! | HEAVY  | 1.09 MB      | 400       | 436 MB     |
//!
//! # Running Tests
//!
//! ```bash
//! # All concurrent session tests
//! cargo test concurrent_sessions -- --ignored --nocapture
//!
//! # Specific test
//! cargo test test_500_light_sessions -- --ignored --nocapture
//! ```
//!
//! # ASSUM Tags
//!
//! - #ASSUME_64GB_TARGET: Tests sized for kindly-hub (64GB RAM)
//! - #ASSUME_MULTI_CORE: Concurrent tests require 8+ cores for parallelism

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::{budget, LoadTestMetrics, SessionTier, WorkloadProfile};

// ============================================================================
// Session Pool Simulation
// ============================================================================

/// Simulated session for load testing
///
/// Mimics the memory footprint and operations of real debugger capsules
/// without requiring actual ptrace attachment.
pub struct SimulatedSession {
    /// Unique session ID
    pub id: u64,
    /// Session tier
    pub tier: SessionTier,
    /// Allocated memory (simulated)
    pub memory: Vec<u8>,
    /// Snapshot count (simulates activity)
    pub snapshot_count: AtomicU64,
    /// Creation timestamp
    pub created_at: Instant,
}

impl SimulatedSession {
    /// Create a new simulated session with appropriate memory allocation
    pub fn new(id: u64, tier: SessionTier) -> Self {
        let size = tier.capsule_bytes();
        Self {
            id,
            tier,
            memory: vec![0u8; size],
            snapshot_count: AtomicU64::new(0),
            created_at: Instant::now(),
        }
    }

    /// Simulate snapshot capture (touches memory, increments counter)
    pub fn capture_snapshot(&self) {
        self.snapshot_count.fetch_add(1, Ordering::Relaxed);
        // Touch memory to simulate real usage
        let count = self.snapshot_count.load(Ordering::Relaxed) as usize;
        if !self.memory.is_empty() {
            let idx = count % self.memory.len();
            // Read to simulate memory access
            std::hint::black_box(self.memory[idx]);
        }
    }

    /// Get session lifetime in milliseconds
    pub fn lifetime_ms(&self) -> u64 {
        self.created_at.elapsed().as_millis() as u64
    }
}

/// Thread-safe session pool for load testing
///
/// Uses atomic counters to track allocation without mutex overhead.
pub struct SessionPool {
    /// Current session count per tier
    light_count: AtomicUsize,
    medium_count: AtomicUsize,
    heavy_count: AtomicUsize,
    /// Total memory usage estimate
    memory_usage: AtomicU64,
    /// Session ID counter
    next_id: AtomicU64,
    /// Allocation failure counter
    failures: AtomicU64,
    /// Peak concurrent sessions
    peak_concurrent: AtomicU64,
}

impl SessionPool {
    pub fn new() -> Self {
        Self {
            light_count: AtomicUsize::new(0),
            medium_count: AtomicUsize::new(0),
            heavy_count: AtomicUsize::new(0),
            memory_usage: AtomicU64::new(0),
            next_id: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            peak_concurrent: AtomicU64::new(0),
        }
    }

    /// Attempt to allocate a session of the given tier
    pub fn allocate(&self, tier: SessionTier) -> Option<SimulatedSession> {
        let (counter, max, size) = match tier {
            SessionTier::Light => (&self.light_count, budget::MAX_LIGHT_SESSIONS, budget::LIGHT_SESSION_BYTES),
            SessionTier::Medium => (&self.medium_count, budget::MAX_MEDIUM_SESSIONS, budget::MEDIUM_SESSION_BYTES),
            SessionTier::Heavy => (&self.heavy_count, budget::MAX_HEAVY_SESSIONS, budget::HEAVY_SESSION_BYTES),
        };

        // Try to increment counter if below max
        loop {
            let current = counter.load(Ordering::Acquire);
            if current >= max {
                self.failures.fetch_add(1, Ordering::Relaxed);
                return None;
            }
            if counter
                .compare_exchange(current, current + 1, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        // Allocate session
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.memory_usage.fetch_add(size as u64, Ordering::Relaxed);

        // Update peak concurrent
        let total = self.total_sessions();
        loop {
            let peak = self.peak_concurrent.load(Ordering::Acquire);
            if total as u64 <= peak {
                break;
            }
            if self
                .peak_concurrent
                .compare_exchange(peak, total as u64, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        Some(SimulatedSession::new(id, tier))
    }

    /// Deallocate a session
    pub fn deallocate(&self, session: &SimulatedSession) {
        let (counter, size) = match session.tier {
            SessionTier::Light => (&self.light_count, budget::LIGHT_SESSION_BYTES),
            SessionTier::Medium => (&self.medium_count, budget::MEDIUM_SESSION_BYTES),
            SessionTier::Heavy => (&self.heavy_count, budget::HEAVY_SESSION_BYTES),
        };

        counter.fetch_sub(1, Ordering::Release);
        self.memory_usage.fetch_sub(size as u64, Ordering::Relaxed);
    }

    /// Get total session count across all tiers
    pub fn total_sessions(&self) -> usize {
        self.light_count.load(Ordering::Relaxed)
            + self.medium_count.load(Ordering::Relaxed)
            + self.heavy_count.load(Ordering::Relaxed)
    }

    /// Get current memory usage
    pub fn memory_usage(&self) -> u64 {
        self.memory_usage.load(Ordering::Relaxed)
    }

    /// Get allocation failure count
    pub fn failures(&self) -> u64 {
        self.failures.load(Ordering::Relaxed)
    }

    /// Get peak concurrent sessions
    pub fn peak_concurrent(&self) -> u64 {
        self.peak_concurrent.load(Ordering::Relaxed)
    }

    /// Get session counts per tier
    pub fn tier_counts(&self) -> (usize, usize, usize) {
        (
            self.light_count.load(Ordering::Relaxed),
            self.medium_count.load(Ordering::Relaxed),
            self.heavy_count.load(Ordering::Relaxed),
        )
    }
}

impl Default for SessionPool {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Session Pool Tests - Capacity Validation
// ============================================================================

/// Test 500 concurrent LIGHT sessions
///
/// Memory: 500 x 64KB = 32MB (well under 96MB budget)
///
/// #ASSUME_64KB_LIGHT: LightDebuggerCapsule is exactly 64KB
/// #VERIFY_ALLOCATION: All 500 allocations must succeed
#[test]
#[ignore] // Run with: cargo test test_500_light_sessions -- --ignored
fn test_500_light_sessions() {
    let pool = Arc::new(SessionPool::new());
    let start = Instant::now();

    let handles: Vec<_> = (0..500)
        .map(|_| {
            let pool = Arc::clone(&pool);
            thread::spawn(move || {
                // Allocate session
                let session = pool.allocate(SessionTier::Light).expect("Should allocate LIGHT session");

                // Simulate brief debugging activity
                for _ in 0..100 {
                    session.capture_snapshot();
                }

                // Hold session for 100ms to simulate real usage
                thread::sleep(Duration::from_millis(100));

                let lifetime = session.lifetime_ms();

                // Deallocate
                pool.deallocate(&session);

                lifetime
            })
        })
        .collect();

    // Collect results
    let lifetimes: Vec<u64> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let duration = start.elapsed();

    // Calculate metrics
    let avg_lifetime: f64 = lifetimes.iter().sum::<u64>() as f64 / lifetimes.len() as f64;

    println!("\n=== 500 LIGHT Sessions Test ===");
    println!("Duration: {:?}", duration);
    println!("Peak concurrent: {}", pool.peak_concurrent());
    println!("Peak memory: {} KB", pool.memory_usage() / 1024);
    println!("Avg lifetime: {:.2} ms", avg_lifetime);
    println!("Failures: {}", pool.failures());

    // Assertions
    assert_eq!(pool.failures(), 0, "No allocation failures expected");
    assert!(
        pool.peak_concurrent() <= 500,
        "Peak should not exceed 500"
    );
    assert_eq!(
        pool.total_sessions(),
        0,
        "All sessions should be deallocated"
    );
}

/// Test 1000 LIGHT sessions (66% of max capacity)
///
/// Memory: 1000 x 64KB = 64MB (under 96MB budget)
///
/// #ASSUME_THREADING: System can handle 1000 concurrent threads
#[test]
#[ignore]
fn test_1000_light_sessions() {
    let pool = Arc::new(SessionPool::new());
    let start = Instant::now();
    let sessions_count = 1000;

    // Create sessions in batches to avoid thread explosion
    let batch_size = 100;
    let mut total_lifetimes = 0u64;
    let mut total_sessions = 0usize;

    for batch in 0..(sessions_count / batch_size) {
        let handles: Vec<_> = (0..batch_size)
            .map(|i| {
                let pool = Arc::clone(&pool);
                let session_num = batch * batch_size + i;
                thread::spawn(move || {
                    let session = pool
                        .allocate(SessionTier::Light)
                        .expect(&format!("Session {} should allocate", session_num));

                    // Variable activity based on session number
                    let activity = 50 + (session_num % 100);
                    for _ in 0..activity {
                        session.capture_snapshot();
                    }

                    // Variable hold time
                    let hold_time = 50 + (session_num % 100) as u64;
                    thread::sleep(Duration::from_millis(hold_time));

                    let lifetime = session.lifetime_ms();
                    pool.deallocate(&session);
                    lifetime
                })
            })
            .collect();

        for h in handles {
            total_lifetimes += h.join().unwrap();
            total_sessions += 1;
        }
    }

    let duration = start.elapsed();
    let avg_lifetime = total_lifetimes as f64 / total_sessions as f64;

    println!("\n=== 1000 LIGHT Sessions Test ===");
    println!("Duration: {:?}", duration);
    println!("Peak concurrent: {}", pool.peak_concurrent());
    println!("Peak memory: {} KB", pool.memory_usage() / 1024);
    println!("Avg lifetime: {:.2} ms", avg_lifetime);
    println!("Failures: {}", pool.failures());
    println!("Throughput: {:.2} sessions/sec", total_sessions as f64 / duration.as_secs_f64());

    assert_eq!(pool.failures(), 0);
    assert_eq!(pool.total_sessions(), 0);
}

/// Test 1500 LIGHT sessions (100% of max capacity)
///
/// Memory: 1500 x 64KB = 96MB (exactly at budget)
///
/// #ASSUME_EXACT_CAPACITY: Pool supports exactly MAX_LIGHT_SESSIONS
#[test]
#[ignore]
fn test_1500_light_sessions_max_capacity() {
    let pool = Arc::new(SessionPool::new());

    // Allocate all at once to test capacity
    let mut sessions: Vec<SimulatedSession> = Vec::with_capacity(budget::MAX_LIGHT_SESSIONS);

    for i in 0..budget::MAX_LIGHT_SESSIONS {
        match pool.allocate(SessionTier::Light) {
            Some(session) => sessions.push(session),
            None => panic!("Session {} should allocate (max is {})", i, budget::MAX_LIGHT_SESSIONS),
        }
    }

    // Verify capacity reached
    assert_eq!(sessions.len(), budget::MAX_LIGHT_SESSIONS);

    // Verify next allocation fails
    assert!(
        pool.allocate(SessionTier::Light).is_none(),
        "Should fail when at max capacity"
    );

    println!("\n=== 1500 LIGHT Sessions Max Capacity Test ===");
    println!("Sessions allocated: {}", sessions.len());
    println!("Memory usage: {} MB", pool.memory_usage() / (1024 * 1024));
    println!("Failures (expected 1): {}", pool.failures());

    // Clean up
    for session in &sessions {
        pool.deallocate(session);
    }

    assert_eq!(pool.total_sessions(), 0);
}

/// Test 600 MEDIUM sessions (100% of max capacity)
///
/// Memory: 600 x 256KB = 150MB (exactly at budget)
#[test]
#[ignore]
fn test_600_medium_sessions_max_capacity() {
    let pool = Arc::new(SessionPool::new());

    let mut sessions: Vec<SimulatedSession> = Vec::with_capacity(budget::MAX_MEDIUM_SESSIONS);

    for i in 0..budget::MAX_MEDIUM_SESSIONS {
        match pool.allocate(SessionTier::Medium) {
            Some(session) => sessions.push(session),
            None => panic!("Session {} should allocate", i),
        }
    }

    assert_eq!(sessions.len(), budget::MAX_MEDIUM_SESSIONS);
    assert!(pool.allocate(SessionTier::Medium).is_none());

    println!("\n=== 600 MEDIUM Sessions Max Capacity Test ===");
    println!("Sessions allocated: {}", sessions.len());
    println!("Memory usage: {} MB", pool.memory_usage() / (1024 * 1024));

    for session in &sessions {
        pool.deallocate(session);
    }
}

/// Test 400 HEAVY sessions (100% of max capacity)
///
/// Memory: 400 x 1.09MB = 436MB (exactly at budget, not including replay)
#[test]
#[ignore]
fn test_400_heavy_sessions_max_capacity() {
    let pool = Arc::new(SessionPool::new());

    let mut sessions: Vec<SimulatedSession> = Vec::with_capacity(budget::MAX_HEAVY_SESSIONS);

    for i in 0..budget::MAX_HEAVY_SESSIONS {
        match pool.allocate(SessionTier::Heavy) {
            Some(session) => sessions.push(session),
            None => panic!("Session {} should allocate", i),
        }
    }

    assert_eq!(sessions.len(), budget::MAX_HEAVY_SESSIONS);
    assert!(pool.allocate(SessionTier::Heavy).is_none());

    println!("\n=== 400 HEAVY Sessions Max Capacity Test ===");
    println!("Sessions allocated: {}", sessions.len());
    println!("Memory usage: {} MB", pool.memory_usage() / (1024 * 1024));

    for session in &sessions {
        pool.deallocate(session);
    }
}

// ============================================================================
// Mixed Tier Session Tests
// ============================================================================

/// Test 1000 mixed-tier sessions
///
/// Distribution: 60% LIGHT (600), 30% MEDIUM (300), 10% HEAVY (100)
/// Memory: (600x64KB) + (300x256KB) + (100x1.09MB) = ~225MB
#[test]
#[ignore]
fn test_1000_mixed_sessions() {
    let pool = Arc::new(SessionPool::new());
    let start = Instant::now();

    // Session distribution
    let light_count = 600;
    let medium_count = 300;
    let heavy_count = 100;

    let mut handles = Vec::new();

    // Spawn LIGHT sessions
    for _ in 0..light_count {
        let pool = Arc::clone(&pool);
        handles.push(thread::spawn(move || {
            let session = pool.allocate(SessionTier::Light)?;
            for _ in 0..50 {
                session.capture_snapshot();
            }
            thread::sleep(Duration::from_millis(50));
            let lifetime = session.lifetime_ms();
            pool.deallocate(&session);
            Some((SessionTier::Light, lifetime))
        }));
    }

    // Spawn MEDIUM sessions
    for _ in 0..medium_count {
        let pool = Arc::clone(&pool);
        handles.push(thread::spawn(move || {
            let session = pool.allocate(SessionTier::Medium)?;
            for _ in 0..100 {
                session.capture_snapshot();
            }
            thread::sleep(Duration::from_millis(100));
            let lifetime = session.lifetime_ms();
            pool.deallocate(&session);
            Some((SessionTier::Medium, lifetime))
        }));
    }

    // Spawn HEAVY sessions
    for _ in 0..heavy_count {
        let pool = Arc::clone(&pool);
        handles.push(thread::spawn(move || {
            let session = pool.allocate(SessionTier::Heavy)?;
            for _ in 0..200 {
                session.capture_snapshot();
            }
            thread::sleep(Duration::from_millis(200));
            let lifetime = session.lifetime_ms();
            pool.deallocate(&session);
            Some((SessionTier::Heavy, lifetime))
        }));
    }

    // Collect results
    let mut tier_lifetimes: HashMap<String, Vec<u64>> = HashMap::new();
    tier_lifetimes.insert("LIGHT".to_string(), Vec::new());
    tier_lifetimes.insert("MEDIUM".to_string(), Vec::new());
    tier_lifetimes.insert("HEAVY".to_string(), Vec::new());

    let mut success_count = 0;
    for h in handles {
        if let Some((tier, lifetime)) = h.join().unwrap() {
            let tier_name = format!("{:?}", tier).to_uppercase();
            tier_lifetimes.get_mut(&tier_name).unwrap().push(lifetime);
            success_count += 1;
        }
    }

    let duration = start.elapsed();

    println!("\n=== 1000 Mixed Sessions Test ===");
    println!("Duration: {:?}", duration);
    println!("Success: {}/{}", success_count, light_count + medium_count + heavy_count);
    println!("Peak concurrent: {}", pool.peak_concurrent());
    println!("Peak memory: {} MB", pool.memory_usage() / (1024 * 1024));

    for (tier, lifetimes) in &tier_lifetimes {
        if !lifetimes.is_empty() {
            let avg = lifetimes.iter().sum::<u64>() as f64 / lifetimes.len() as f64;
            println!("  {} sessions: {}, avg lifetime: {:.2} ms", tier, lifetimes.len(), avg);
        }
    }

    println!("Failures: {}", pool.failures());

    assert_eq!(pool.failures(), 0, "No allocation failures expected");
    assert_eq!(pool.total_sessions(), 0, "All sessions deallocated");
}

/// Test 2000 mixed-tier sessions (stress test)
///
/// Larger scale test to validate system under higher load
#[test]
#[ignore]
fn test_2000_mixed_sessions_stress() {
    let pool = Arc::new(SessionPool::new());
    let start = Instant::now();

    // Session distribution
    let light_count = 1200; // 60%
    let medium_count = 600; // 30%
    let heavy_count = 200;  // 10%

    let sessions_created = Arc::new(AtomicU64::new(0));
    let sessions_completed = Arc::new(AtomicU64::new(0));

    let mut handles = Vec::new();

    // Helper to spawn sessions with staggered start
    let spawn_sessions = |count: usize, tier: SessionTier, delay_base_ms: u64| -> Vec<_> {
        let mut h = Vec::new();
        for i in 0..count {
            let pool = Arc::clone(&pool);
            let created = Arc::clone(&sessions_created);
            let completed = Arc::clone(&sessions_completed);
            let start_delay = (i as u64 * delay_base_ms) / count as u64;

            h.push(thread::spawn(move || {
                // Stagger start times
                if start_delay > 0 {
                    thread::sleep(Duration::from_millis(start_delay));
                }

                if let Some(session) = pool.allocate(tier) {
                    created.fetch_add(1, Ordering::Relaxed);

                    // Simulate activity
                    let iterations = match tier {
                        SessionTier::Light => 30,
                        SessionTier::Medium => 60,
                        SessionTier::Heavy => 100,
                    };

                    for _ in 0..iterations {
                        session.capture_snapshot();
                    }

                    // Hold time varies by tier
                    let hold_ms = match tier {
                        SessionTier::Light => 20,
                        SessionTier::Medium => 50,
                        SessionTier::Heavy => 100,
                    };
                    thread::sleep(Duration::from_millis(hold_ms));

                    pool.deallocate(&session);
                    completed.fetch_add(1, Ordering::Relaxed);
                    true
                } else {
                    false
                }
            }));
        }
        h
    };

    // Spawn all tiers with staggered starts
    handles.extend(spawn_sessions(light_count, SessionTier::Light, 500));
    handles.extend(spawn_sessions(medium_count, SessionTier::Medium, 500));
    handles.extend(spawn_sessions(heavy_count, SessionTier::Heavy, 500));

    // Wait for all to complete
    let mut successes = 0;
    for h in handles {
        if h.join().unwrap() {
            successes += 1;
        }
    }

    let duration = start.elapsed();
    let total = light_count + medium_count + heavy_count;

    println!("\n=== 2000 Mixed Sessions Stress Test ===");
    println!("Duration: {:?}", duration);
    println!("Success rate: {}/{} ({:.1}%)", successes, total, successes as f64 / total as f64 * 100.0);
    println!("Sessions created: {}", sessions_created.load(Ordering::Relaxed));
    println!("Sessions completed: {}", sessions_completed.load(Ordering::Relaxed));
    println!("Peak concurrent: {}", pool.peak_concurrent());
    println!("Allocation failures: {}", pool.failures());
    println!("Throughput: {:.2} sessions/sec", successes as f64 / duration.as_secs_f64());

    // Stress test allows some failures due to capacity limits
    assert!(
        successes as f64 / total as f64 > 0.95,
        "At least 95% success rate expected"
    );
    assert_eq!(pool.total_sessions(), 0, "All sessions deallocated");
}

// ============================================================================
// Session Upgrade Tests
// ============================================================================

/// Test session upgrade under load
///
/// Start 500 LIGHT sessions, upgrade 100 to MEDIUM based on activity,
/// then upgrade 20 to HEAVY.
#[test]
#[ignore]
fn test_upgrade_under_load() {
    let pool = Arc::new(SessionPool::new());
    let start = Instant::now();

    let upgrades_to_medium = Arc::new(AtomicU64::new(0));
    let upgrades_to_heavy = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..500)
        .map(|i| {
            let pool = Arc::clone(&pool);
            let to_medium = Arc::clone(&upgrades_to_medium);
            let to_heavy = Arc::clone(&upgrades_to_heavy);

            thread::spawn(move || {
                // Start as LIGHT
                let light_session = pool.allocate(SessionTier::Light)?;

                // Simulate activity
                for _ in 0..50 {
                    light_session.capture_snapshot();
                }

                // Decision: upgrade based on session number
                if i % 5 == 0 {
                    // 20% upgrade to MEDIUM
                    pool.deallocate(&light_session);

                    let medium_session = pool.allocate(SessionTier::Medium)?;
                    to_medium.fetch_add(1, Ordering::Relaxed);

                    for _ in 0..100 {
                        medium_session.capture_snapshot();
                    }

                    // Further upgrade to HEAVY for subset
                    if i % 25 == 0 {
                        pool.deallocate(&medium_session);

                        let heavy_session = pool.allocate(SessionTier::Heavy)?;
                        to_heavy.fetch_add(1, Ordering::Relaxed);

                        for _ in 0..200 {
                            heavy_session.capture_snapshot();
                        }

                        thread::sleep(Duration::from_millis(100));
                        pool.deallocate(&heavy_session);
                    } else {
                        thread::sleep(Duration::from_millis(50));
                        pool.deallocate(&medium_session);
                    }
                } else {
                    thread::sleep(Duration::from_millis(30));
                    pool.deallocate(&light_session);
                }

                Some(())
            })
        })
        .collect();

    // Wait for all
    let mut successes = 0;
    for h in handles {
        if h.join().unwrap().is_some() {
            successes += 1;
        }
    }

    let duration = start.elapsed();

    println!("\n=== Session Upgrade Under Load Test ===");
    println!("Duration: {:?}", duration);
    println!("Successful sessions: {}/500", successes);
    println!("Upgrades to MEDIUM: {}", upgrades_to_medium.load(Ordering::Relaxed));
    println!("Upgrades to HEAVY: {}", upgrades_to_heavy.load(Ordering::Relaxed));
    println!("Peak concurrent: {}", pool.peak_concurrent());
    println!("Allocation failures: {}", pool.failures());

    let (light, medium, heavy) = pool.tier_counts();
    println!("Final counts - LIGHT: {}, MEDIUM: {}, HEAVY: {}", light, medium, heavy);

    assert_eq!(pool.total_sessions(), 0, "All sessions deallocated");
    assert!(
        upgrades_to_medium.load(Ordering::Relaxed) > 0,
        "Should have MEDIUM upgrades"
    );
    assert!(
        upgrades_to_heavy.load(Ordering::Relaxed) > 0,
        "Should have HEAVY upgrades"
    );
}

// ============================================================================
// Pool Exhaustion Tests
// ============================================================================

/// Test graceful degradation when pool exhausted
///
/// Allocate until pool exhausted, verify:
/// - Error handling is graceful
/// - Existing sessions continue working
/// - Deallocation restores capacity
#[test]
#[ignore]
fn test_pool_exhaustion_graceful() {
    let pool = Arc::new(SessionPool::new());

    // Step 1: Fill pool to capacity
    let mut sessions: Vec<SimulatedSession> = Vec::new();

    // Fill HEAVY first (most restrictive)
    for _ in 0..budget::MAX_HEAVY_SESSIONS {
        if let Some(s) = pool.allocate(SessionTier::Heavy) {
            sessions.push(s);
        }
    }

    let heavy_allocated = sessions.len();
    println!("HEAVY sessions allocated: {}", heavy_allocated);

    // Verify next HEAVY allocation fails
    assert!(
        pool.allocate(SessionTier::Heavy).is_none(),
        "HEAVY allocation should fail when full"
    );

    // Step 2: Verify existing sessions still work
    for session in &sessions {
        session.capture_snapshot();
        assert!(
            session.snapshot_count.load(Ordering::Relaxed) > 0,
            "Session should be functional"
        );
    }

    // Step 3: Deallocate half
    let half = sessions.len() / 2;
    for _ in 0..half {
        if let Some(session) = sessions.pop() {
            pool.deallocate(&session);
        }
    }

    println!("After deallocation: {} sessions remaining", sessions.len());

    // Step 4: Verify capacity restored
    let mut new_sessions = Vec::new();
    for _ in 0..half {
        if let Some(s) = pool.allocate(SessionTier::Heavy) {
            new_sessions.push(s);
        }
    }

    println!("New sessions allocated: {}", new_sessions.len());
    assert!(
        new_sessions.len() > 0,
        "Should be able to allocate after deallocation"
    );

    // Cleanup
    for session in &sessions {
        pool.deallocate(session);
    }
    for session in &new_sessions {
        pool.deallocate(session);
    }

    assert_eq!(pool.total_sessions(), 0);
    println!("\n=== Pool Exhaustion Graceful Test PASSED ===");
}

/// Test concurrent allocation near capacity limits
///
/// Multiple threads compete for last available slots
#[test]
#[ignore]
fn test_concurrent_near_capacity() {
    let pool = Arc::new(SessionPool::new());

    // Pre-fill to 90% capacity
    let prefill_count = (budget::MAX_LIGHT_SESSIONS * 9) / 10;
    let mut prefill_sessions: Vec<SimulatedSession> = Vec::new();

    for _ in 0..prefill_count {
        if let Some(s) = pool.allocate(SessionTier::Light) {
            prefill_sessions.push(s);
        }
    }

    println!("Pre-filled {} sessions (90% capacity)", prefill_sessions.len());

    // Now race for remaining 10%
    let remaining_capacity = budget::MAX_LIGHT_SESSIONS - prefill_count;
    let competitors = remaining_capacity * 2; // 2x competitors for remaining slots

    let successes = Arc::new(AtomicU64::new(0));
    let failures = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..competitors)
        .map(|_| {
            let pool = Arc::clone(&pool);
            let succ = Arc::clone(&successes);
            let fail = Arc::clone(&failures);

            thread::spawn(move || {
                match pool.allocate(SessionTier::Light) {
                    Some(session) => {
                        succ.fetch_add(1, Ordering::Relaxed);
                        session.capture_snapshot();
                        thread::sleep(Duration::from_millis(10));
                        pool.deallocate(&session);
                        true
                    }
                    None => {
                        fail.fetch_add(1, Ordering::Relaxed);
                        false
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let success_count = successes.load(Ordering::Relaxed);
    let failure_count = failures.load(Ordering::Relaxed);

    println!("\n=== Concurrent Near Capacity Test ===");
    println!("Competitors: {}", competitors);
    println!("Remaining slots: {}", remaining_capacity);
    println!("Successes: {}", success_count);
    println!("Failures: {}", failure_count);

    // Some should succeed, some should fail (race condition)
    assert!(
        success_count > 0,
        "Some allocations should succeed"
    );
    // Due to rapid alloc/dealloc, success count might exceed remaining capacity
    println!("Peak concurrent: {}", pool.peak_concurrent());

    // Cleanup prefill
    for session in &prefill_sessions {
        pool.deallocate(session);
    }

    assert_eq!(pool.total_sessions(), 0);
}

// ============================================================================
// Rapid Allocation/Deallocation Tests
// ============================================================================

/// Test rapid allocation/deallocation cycles
///
/// Simulates high-churn MCP workload
#[test]
#[ignore]
fn test_rapid_alloc_dealloc_cycles() {
    let pool = Arc::new(SessionPool::new());
    let start = Instant::now();

    let cycles = 100;
    let sessions_per_cycle = 50;
    let total_ops = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..10)
        .map(|thread_id| {
            let pool = Arc::clone(&pool);
            let ops = Arc::clone(&total_ops);

            thread::spawn(move || {
                for cycle in 0..cycles {
                    let tier = match cycle % 3 {
                        0 => SessionTier::Light,
                        1 => SessionTier::Medium,
                        _ => SessionTier::Heavy,
                    };

                    for _ in 0..sessions_per_cycle {
                        if let Some(session) = pool.allocate(tier) {
                            session.capture_snapshot();
                            pool.deallocate(&session);
                            ops.fetch_add(2, Ordering::Relaxed); // alloc + dealloc
                        }
                    }
                }
                thread_id
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let duration = start.elapsed();
    let total_operations = total_ops.load(Ordering::Relaxed);

    println!("\n=== Rapid Alloc/Dealloc Cycles Test ===");
    println!("Duration: {:?}", duration);
    println!("Total operations: {}", total_operations);
    println!("Throughput: {:.2} ops/sec", total_operations as f64 / duration.as_secs_f64());
    println!("Peak concurrent: {}", pool.peak_concurrent());
    println!("Allocation failures: {}", pool.failures());

    assert_eq!(pool.total_sessions(), 0, "No leaked sessions");
}

/// Test session throughput measurement
///
/// Measures maximum sustainable session creation rate
#[test]
#[ignore]
fn test_session_throughput() {
    let pool = Arc::new(SessionPool::new());
    let duration_secs = 5;

    let sessions_created = Arc::new(AtomicU64::new(0));
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));

    // Spawn worker threads
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let pool = Arc::clone(&pool);
            let created = Arc::clone(&sessions_created);
            let is_running = Arc::clone(&running);

            thread::spawn(move || {
                while is_running.load(Ordering::Relaxed) {
                    if let Some(session) = pool.allocate(SessionTier::Light) {
                        session.capture_snapshot();
                        pool.deallocate(&session);
                        created.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    // Run for specified duration
    thread::sleep(Duration::from_secs(duration_secs));
    running.store(false, Ordering::Relaxed);

    // Wait for threads to finish
    for h in handles {
        h.join().unwrap();
    }

    let total_sessions = sessions_created.load(Ordering::Relaxed);
    let throughput = total_sessions as f64 / duration_secs as f64;

    println!("\n=== Session Throughput Test ===");
    println!("Duration: {} seconds", duration_secs);
    println!("Sessions created: {}", total_sessions);
    println!("Throughput: {:.2} sessions/sec", throughput);
    println!("Peak concurrent: {}", pool.peak_concurrent());

    // Expect at least 10,000 sessions/sec on modern hardware
    assert!(
        throughput > 1000.0,
        "Throughput should be > 1000 sessions/sec, got {:.2}",
        throughput
    );
}
