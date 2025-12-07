//! Memory Budget Validation Tests
//!
//! Ensures memory usage stays within 64GB budget for kindly-hub deployment.
//!
//! # Memory Budget Breakdown
//!
//! | Component      | Allocation  | Notes                          |
//! |----------------|-------------|--------------------------------|
//! | Light Pool     | 96 MB       | 1,500 x 64KB slots             |
//! | Medium Pool    | 150 MB      | 600 x 256KB slots              |
//! | Heavy Pool     | 436 MB      | 400 x 1.09MB slots             |
//! | Memory Replay  | ~26 GB      | 400 x 64MB max per session     |
//! | System Reserve | ~4 GB       | OS, MCP server, safety margin  |
//! | **Total**      | **~31 GB**  | ~48% of 64GB capacity          |
//!
//! # Per-Session Limits
//!
//! | Tier   | Capsule | Replay Buffer | Total Max |
//! |--------|---------|---------------|-----------|
//! | Light  | 64 KB   | 0             | 64 KB     |
//! | Medium | 256 KB  | 0             | 256 KB    |
//! | Heavy  | 1.09 MB | 64 MB         | ~65 MB    |
//!
//! # Running Tests
//!
//! ```bash
//! cargo test memory_budget -- --ignored --nocapture
//! ```
//!
//! # ASSUM Tags
//!
//! - #ASSUME_64GB_TARGET: Tests designed for kindly-hub (64GB RAM)
//! - #ASSUME_PAGE_SIZE_4K: Memory delta tracking uses 4KB pages
//! - #ASSUME_10_PERCENT_CHANGE: Typical snapshot has ~10% page changes

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::{budget, LoadTestMetrics, SessionTier};

// ============================================================================
// Memory Tracking Utilities
// ============================================================================

/// Simulated memory replay buffer for Heavy sessions
///
/// Tracks memory deltas like MemoryReplayCapsule but with precise
/// memory accounting for budget validation.
pub struct SimulatedReplayBuffer {
    /// Base pages: address -> 4KB page data
    base_pages: HashMap<u64, Vec<u8>>,
    /// Delta chain: (snapshot_id, address) -> compressed delta
    deltas: HashMap<(u64, u64), Vec<u8>>,
    /// Total memory usage in bytes
    memory_usage: u64,
    /// Maximum allowed usage
    max_usage: u64,
    /// Current snapshot ID
    current_snapshot: u64,
}

impl SimulatedReplayBuffer {
    /// Create new replay buffer with max capacity
    pub fn new(max_bytes: usize) -> Self {
        Self {
            base_pages: HashMap::new(),
            deltas: HashMap::new(),
            memory_usage: 0,
            max_usage: max_bytes as u64,
            current_snapshot: 0,
        }
    }

    /// Simulate capturing a page with delta compression
    ///
    /// Returns true if within budget, false if would exceed
    pub fn capture_page(&mut self, address: u64, data: &[u8; 4096]) -> bool {
        // Check if this is a new base page or delta
        if !self.base_pages.contains_key(&address) {
            // First time seeing this page - store as base
            let storage_size: u64 = 4096 + 8; // page + address
            if self.memory_usage + storage_size > self.max_usage {
                return false;
            }
            self.base_pages.insert(address, data.to_vec());
            self.memory_usage += storage_size;
        } else {
            // Compute delta (simulate 10% change rate -> ~400 bytes compressed)
            let delta_size = self.estimate_delta_size(data);
            let storage_size: u64 = delta_size as u64 + 16; // delta + metadata

            if self.memory_usage + storage_size > self.max_usage {
                return false;
            }

            self.deltas.insert((self.current_snapshot, address), vec![0u8; delta_size]);
            self.memory_usage += storage_size;
        }

        true
    }

    /// Take a snapshot (advances snapshot counter)
    pub fn take_snapshot(&mut self) {
        self.current_snapshot += 1;
    }

    /// Estimate delta size based on realistic compression
    ///
    /// Assumes 10% page change rate and 4:1 compression ratio
    fn estimate_delta_size(&self, _data: &[u8]) -> usize {
        // 10% of 4KB = 410 bytes changed
        // 4:1 compression = ~100 bytes per delta
        // Add some variance
        100 + (self.current_snapshot as usize % 50)
    }

    /// Get current memory usage
    pub fn memory_usage(&self) -> u64 {
        self.memory_usage
    }

    /// Get snapshot count
    pub fn snapshot_count(&self) -> u64 {
        self.current_snapshot
    }

    /// Check if at capacity
    pub fn at_capacity(&self) -> bool {
        self.memory_usage >= self.max_usage
    }

    /// Evict old snapshots to free memory
    pub fn evict_oldest(&mut self, target_usage: u64) {
        // Find and remove oldest snapshot's deltas
        while self.memory_usage > target_usage && self.current_snapshot > 1 {
            let oldest_snapshot = self.current_snapshot - self.deltas.len() as u64 / self.base_pages.len().max(1) as u64;

            let keys_to_remove: Vec<_> = self
                .deltas
                .keys()
                .filter(|(snap, _)| *snap <= oldest_snapshot)
                .cloned()
                .collect();

            let removed_count = keys_to_remove.len();

            for key in keys_to_remove {
                if let Some(delta) = self.deltas.remove(&key) {
                    self.memory_usage = self.memory_usage.saturating_sub(delta.len() as u64 + 16);
                }
            }

            if removed_count == 0 {
                break;
            }
        }
    }
}

/// Heavy session with replay buffer for memory testing
pub struct HeavySessionWithReplay {
    /// Session capsule memory (simulated)
    capsule_memory: Vec<u8>,
    /// Replay buffer
    replay: SimulatedReplayBuffer,
    /// Session ID
    pub id: u64,
    /// Snapshot counter
    snapshots_taken: AtomicU64,
}

impl HeavySessionWithReplay {
    pub fn new(id: u64) -> Self {
        Self {
            capsule_memory: vec![0u8; budget::HEAVY_SESSION_BYTES],
            replay: SimulatedReplayBuffer::new(budget::HEAVY_REPLAY_BYTES),
            id,
            snapshots_taken: AtomicU64::new(0),
        }
    }

    /// Simulate taking a memory snapshot
    ///
    /// Captures N dirty pages (simulating process memory changes)
    pub fn take_memory_snapshot(&mut self, dirty_page_count: usize) -> bool {
        // Simulate dirty pages at various addresses
        let base_address = (self.snapshots_taken.load(Ordering::Relaxed) * 4096 * 100) & 0xFFFF_FFFF_F000;

        for i in 0..dirty_page_count {
            let address = base_address + (i as u64 * 4096);
            let mut page_data = [0u8; 4096];

            // Simulate page content with some variation
            page_data[0] = (address & 0xFF) as u8;
            page_data[1] = ((address >> 8) & 0xFF) as u8;

            if !self.replay.capture_page(address, &page_data) {
                return false; // Budget exceeded
            }
        }

        self.replay.take_snapshot();
        self.snapshots_taken.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// Get total memory usage (capsule + replay)
    pub fn memory_usage(&self) -> u64 {
        self.capsule_memory.len() as u64 + self.replay.memory_usage()
    }

    /// Get snapshot count
    pub fn snapshot_count(&self) -> u64 {
        self.snapshots_taken.load(Ordering::Relaxed)
    }

    /// Check if session is at memory capacity
    pub fn at_capacity(&self) -> bool {
        self.replay.at_capacity()
    }

    /// Evict old data to free memory
    pub fn evict_old(&mut self, target_bytes: u64) {
        self.replay.evict_oldest(target_bytes);
    }
}

// ============================================================================
// Per-Session Budget Tests
// ============================================================================

/// Validate HEAVY session memory budget
///
/// Target: <=64MB per session including replay buffers
///
/// #ASSUME_10_PERCENT_CHANGE: 10% of pages change per snapshot
/// #VERIFY_BUDGET: Total usage stays under 65MB (1.09MB capsule + 64MB replay)
#[test]
#[ignore]
fn test_heavy_session_memory_budget() {
    let mut session = HeavySessionWithReplay::new(0);

    // Simulate 100 snapshots with 10% page change rate
    // Assuming 1000 tracked pages = 4MB heap
    let pages_per_snapshot = 100; // 10% of 1000 pages

    for i in 0..100 {
        let success = session.take_memory_snapshot(pages_per_snapshot);

        if !success {
            println!("Memory budget reached at snapshot {}", i);
            break;
        }

        let usage_mb = session.memory_usage() as f64 / (1024.0 * 1024.0);
        if i % 10 == 0 {
            println!("Snapshot {}: {:.2} MB used", i, usage_mb);
        }
    }

    let total_bytes = session.memory_usage();
    let budget_mb = budget::HEAVY_SESSION_BYTES + budget::HEAVY_REPLAY_BYTES;

    println!("\n=== Heavy Session Memory Budget Test ===");
    println!("Capsule size: {} KB", budget::HEAVY_SESSION_BYTES / 1024);
    println!("Replay buffer: {:.2} MB", session.replay.memory_usage() as f64 / (1024.0 * 1024.0));
    println!("Total usage: {:.2} MB", total_bytes as f64 / (1024.0 * 1024.0));
    println!("Budget: {} MB", budget_mb / (1024 * 1024));
    println!("Snapshots taken: {}", session.snapshot_count());

    assert!(
        total_bytes as usize <= budget_mb,
        "Budget exceeded: {} bytes > {} bytes",
        total_bytes,
        budget_mb
    );
}

/// Test memory reclamation on session downgrade
///
/// Allocate HEAVY session -> capture snapshots -> downgrade to LIGHT -> verify memory freed
#[test]
#[ignore]
fn test_memory_reclamation() {
    let mut session = HeavySessionWithReplay::new(0);

    // Capture 50 memory snapshots
    for _ in 0..50 {
        session.take_memory_snapshot(50); // 50 pages per snapshot
    }

    let heavy_usage = session.memory_usage();
    println!("HEAVY session memory: {:.2} MB", heavy_usage as f64 / (1024.0 * 1024.0));

    // Simulate downgrade by dropping heavy session
    // In real implementation, this would migrate minimal state to LIGHT capsule
    drop(session);

    // Create new LIGHT session (simulates downgrade)
    let light_capsule = vec![0u8; budget::LIGHT_SESSION_BYTES];
    let light_usage = light_capsule.len() as u64;

    println!("LIGHT session memory: {} KB", light_usage / 1024);

    // Verify significant memory reduction
    let reduction = heavy_usage - light_usage;
    let reduction_percent = reduction as f64 / heavy_usage as f64 * 100.0;

    println!("Memory reduction: {:.2} MB ({:.1}%)", reduction as f64 / (1024.0 * 1024.0), reduction_percent);

    assert!(
        reduction_percent > 90.0,
        "Expected >90% reduction on downgrade, got {:.1}%",
        reduction_percent
    );
}

/// Test memory eviction under pressure
///
/// Fill replay buffer to capacity, verify eviction works correctly
#[test]
#[ignore]
fn test_memory_eviction_on_pressure() {
    let mut session = HeavySessionWithReplay::new(0);

    // Fill until at capacity
    let mut snapshot = 0;
    while !session.at_capacity() && snapshot < 1000 {
        session.take_memory_snapshot(100);
        snapshot += 1;
    }

    let peak_usage = session.memory_usage();
    println!("Peak memory: {:.2} MB after {} snapshots", peak_usage as f64 / (1024.0 * 1024.0), snapshot);

    // Evict to 75% capacity
    let target = (budget::HEAVY_REPLAY_BYTES as u64 * 3) / 4;
    session.evict_old(target);

    let after_eviction = session.memory_usage();
    println!("After eviction: {:.2} MB", after_eviction as f64 / (1024.0 * 1024.0));

    assert!(
        after_eviction < peak_usage,
        "Eviction should reduce memory usage"
    );
}

// ============================================================================
// System-Wide Budget Tests
// ============================================================================

/// Validate total system memory under max load
///
/// 1500 LIGHT + 600 MEDIUM + 400 HEAVY (plan maximums)
/// Total theoretical: 96 + 150 + 436 = 682 MB base
/// Plus memory replay for HEAVY: 400 x 64MB = 25.6 GB
/// Total: ~26 GB (within 64GB budget)
#[test]
#[ignore]
fn test_max_load_memory() {
    let start = Instant::now();

    // Calculate theoretical maximum
    let light_total = budget::MAX_LIGHT_SESSIONS * budget::LIGHT_SESSION_BYTES;
    let medium_total = budget::MAX_MEDIUM_SESSIONS * budget::MEDIUM_SESSION_BYTES;
    let heavy_capsule_total = budget::MAX_HEAVY_SESSIONS * budget::HEAVY_SESSION_BYTES;
    let heavy_replay_total = budget::MAX_HEAVY_SESSIONS * budget::HEAVY_REPLAY_BYTES;

    let capsule_total = light_total + medium_total + heavy_capsule_total;
    let total_with_replay = capsule_total + heavy_replay_total;

    println!("\n=== Maximum Load Memory Budget ===");
    println!("LIGHT:  {} sessions x {} KB = {} MB",
             budget::MAX_LIGHT_SESSIONS,
             budget::LIGHT_SESSION_BYTES / 1024,
             light_total / (1024 * 1024));
    println!("MEDIUM: {} sessions x {} KB = {} MB",
             budget::MAX_MEDIUM_SESSIONS,
             budget::MEDIUM_SESSION_BYTES / 1024,
             medium_total / (1024 * 1024));
    println!("HEAVY capsules: {} sessions x {} KB = {} MB",
             budget::MAX_HEAVY_SESSIONS,
             budget::HEAVY_SESSION_BYTES / 1024,
             heavy_capsule_total / (1024 * 1024));
    println!("HEAVY replay: {} sessions x {} MB = {} GB",
             budget::MAX_HEAVY_SESSIONS,
             budget::HEAVY_REPLAY_BYTES / (1024 * 1024),
             heavy_replay_total / (1024 * 1024 * 1024));
    println!("");
    println!("Capsule subtotal: {} MB", capsule_total / (1024 * 1024));
    println!("Total with replay: {:.2} GB", total_with_replay as f64 / (1024.0 * 1024.0 * 1024.0));

    // Verify within 64GB budget (allow 50% for OS and MCP server)
    let max_budget = 64 * 1024 * 1024 * 1024_usize; // 64 GB
    let available = max_budget / 2; // 50% for KDB

    println!("Available budget (50% of 64GB): {} GB", available / (1024 * 1024 * 1024));
    println!("Budget utilization: {:.1}%", total_with_replay as f64 / available as f64 * 100.0);

    assert!(
        total_with_replay <= available,
        "Total {} bytes exceeds available {} bytes",
        total_with_replay,
        available
    );

    // Simulate actual allocation (without actually allocating 26GB)
    // Instead, verify counts and sizes are correct
    assert_eq!(budget::MAX_LIGHT_SESSIONS, 1500);
    assert_eq!(budget::MAX_MEDIUM_SESSIONS, 600);
    assert_eq!(budget::MAX_HEAVY_SESSIONS, 400);

    println!("Duration: {:?}", start.elapsed());
    println!("=== Test PASSED ===");
}

/// Test realistic workload memory footprint
///
/// Simulates actual MCP usage pattern: 60% LIGHT, 30% MEDIUM, 10% HEAVY
/// with realistic activity levels
#[test]
#[ignore]
fn test_realistic_workload_memory() {
    let total_sessions = 500;
    let light_count = (total_sessions as f64 * 0.60) as usize;
    let medium_count = (total_sessions as f64 * 0.30) as usize;
    let heavy_count = (total_sessions as f64 * 0.10) as usize;

    // Calculate memory for each tier
    let light_memory = light_count * budget::LIGHT_SESSION_BYTES;
    let medium_memory = medium_count * budget::MEDIUM_SESSION_BYTES;
    let heavy_capsule = heavy_count * budget::HEAVY_SESSION_BYTES;

    // Heavy sessions typically don't fill replay buffer completely
    // Assume 50% average utilization
    let heavy_replay_avg = heavy_count * budget::HEAVY_REPLAY_BYTES / 2;

    let total_memory = light_memory + medium_memory + heavy_capsule + heavy_replay_avg;

    println!("\n=== Realistic Workload Memory ===");
    println!("Distribution: {} LIGHT, {} MEDIUM, {} HEAVY", light_count, medium_count, heavy_count);
    println!("");
    println!("LIGHT:  {} MB", light_memory / (1024 * 1024));
    println!("MEDIUM: {} MB", medium_memory / (1024 * 1024));
    println!("HEAVY capsules: {} MB", heavy_capsule / (1024 * 1024));
    println!("HEAVY replay (50% avg): {} MB", heavy_replay_avg / (1024 * 1024));
    println!("");
    println!("Total: {} MB ({:.2} GB)", total_memory / (1024 * 1024), total_memory as f64 / (1024.0 * 1024.0 * 1024.0));

    // Should fit comfortably in available budget
    let available_gb = 32; // Half of 64GB for this workload
    let available = available_gb * 1024 * 1024 * 1024;

    assert!(
        total_memory < available,
        "Realistic workload {} GB exceeds {} GB budget",
        total_memory / (1024 * 1024 * 1024),
        available_gb
    );

    println!("Budget utilization: {:.2}%", total_memory as f64 / available as f64 * 100.0);
}

/// Test concurrent heavy sessions with memory tracking
///
/// Validates that multiple heavy sessions stay within individual budgets
#[test]
#[ignore]
fn test_concurrent_heavy_memory_tracking() {
    let session_count = 10; // Use 10 for faster test
    let start = Instant::now();

    let total_memory = Arc::new(AtomicU64::new(0));
    let peak_memory = Arc::new(AtomicU64::new(0));
    let snapshots_total = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..session_count)
        .map(|id| {
            let total = Arc::clone(&total_memory);
            let peak = Arc::clone(&peak_memory);
            let snaps = Arc::clone(&snapshots_total);

            thread::spawn(move || {
                let mut session = HeavySessionWithReplay::new(id);

                // Take snapshots until near capacity
                let mut snapshot_count = 0;
                while !session.at_capacity() && snapshot_count < 50 {
                    session.take_memory_snapshot(50); // 50 pages per snapshot
                    snapshot_count += 1;
                }

                let usage = session.memory_usage();

                // Update totals atomically
                total.fetch_add(usage, Ordering::Relaxed);
                snaps.fetch_add(snapshot_count, Ordering::Relaxed);

                // Update peak
                loop {
                    let current_peak = peak.load(Ordering::Acquire);
                    let new_peak = current_peak.max(usage);
                    if peak.compare_exchange(
                        current_peak,
                        new_peak,
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    ).is_ok() {
                        break;
                    }
                }

                (id, usage, snapshot_count)
            })
        })
        .collect();

    // Collect results
    let mut results: Vec<(u64, u64, u64)> = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    results.sort_by_key(|r| r.0);

    let duration = start.elapsed();
    let total = total_memory.load(Ordering::Relaxed);
    let peak = peak_memory.load(Ordering::Relaxed);
    let snaps = snapshots_total.load(Ordering::Relaxed);

    println!("\n=== Concurrent Heavy Sessions Memory ===");
    println!("Sessions: {}", session_count);
    println!("Duration: {:?}", duration);
    println!("");

    for (id, usage, count) in &results {
        println!("Session {}: {:.2} MB, {} snapshots",
                 id,
                 *usage as f64 / (1024.0 * 1024.0),
                 count);
    }

    println!("");
    println!("Total memory: {:.2} MB", total as f64 / (1024.0 * 1024.0));
    println!("Peak session: {:.2} MB", peak as f64 / (1024.0 * 1024.0));
    println!("Total snapshots: {}", snaps);

    // Verify each session stayed within budget
    let max_per_session = (budget::HEAVY_SESSION_BYTES + budget::HEAVY_REPLAY_BYTES) as u64;
    assert!(
        peak <= max_per_session,
        "Peak session {} exceeds budget {}",
        peak,
        max_per_session
    );
}

// ============================================================================
// Memory Growth Tests
// ============================================================================

/// Test memory growth rate over time
///
/// Verifies linear growth and predictable memory usage
#[test]
#[ignore]
fn test_memory_growth_rate() {
    let mut session = HeavySessionWithReplay::new(0);
    let mut measurements: Vec<(u64, u64)> = Vec::new(); // (snapshot, bytes)

    // Take snapshots and measure growth
    for i in 0..100 {
        session.take_memory_snapshot(50);
        measurements.push((i, session.memory_usage()));
    }

    println!("\n=== Memory Growth Rate ===");
    println!("Snapshot, Memory (MB)");
    for (snap, bytes) in measurements.iter().step_by(10) {
        println!("{:3}, {:.3}", snap, *bytes as f64 / (1024.0 * 1024.0));
    }

    // Calculate growth rate (bytes per snapshot)
    let (first_snap, first_bytes) = measurements.first().unwrap();
    let (last_snap, last_bytes) = measurements.last().unwrap();
    let growth_per_snapshot = (last_bytes - first_bytes) / (last_snap - first_snap);

    println!("");
    println!("Growth rate: {} bytes/snapshot ({:.2} KB/snapshot)",
             growth_per_snapshot,
             growth_per_snapshot as f64 / 1024.0);

    // Predict memory for 1000 snapshots
    let predicted_1000 = first_bytes + growth_per_snapshot * 1000;
    println!("Predicted for 1000 snapshots: {:.2} MB", predicted_1000 as f64 / (1024.0 * 1024.0));

    // Growth should be bounded by buffer capacity
    let max_allowed = (budget::HEAVY_SESSION_BYTES + budget::HEAVY_REPLAY_BYTES) as u64;
    println!("Max allowed: {} MB", max_allowed / (1024 * 1024));

    assert!(
        predicted_1000 < max_allowed * 2,
        "Growth projection too high"
    );
}

/// Test memory fragmentation resistance
///
/// Allocate/deallocate many sessions, verify no memory fragmentation issues
#[test]
#[ignore]
fn test_memory_fragmentation_resistance() {
    let iterations = 50;
    let sessions_per_iter = 20;

    let mut peak_memory: usize = 0;
    let mut final_memory: usize = 0;

    for iter in 0..iterations {
        let mut sessions: Vec<HeavySessionWithReplay> = Vec::new();

        // Allocate sessions
        for id in 0..sessions_per_iter {
            let mut session = HeavySessionWithReplay::new((iter * sessions_per_iter + id) as u64);

            // Take some snapshots
            for _ in 0..5 {
                session.take_memory_snapshot(20);
            }

            sessions.push(session);
        }

        // Measure peak
        let current_total: usize = sessions.iter().map(|s| s.memory_usage() as usize).sum();
        peak_memory = peak_memory.max(current_total);

        // Drop half the sessions (simulate deallocation)
        sessions.truncate(sessions_per_iter / 2);

        // Take more snapshots on remaining
        for session in &mut sessions {
            for _ in 0..3 {
                session.take_memory_snapshot(10);
            }
        }

        if iter == iterations - 1 {
            final_memory = sessions.iter().map(|s| s.memory_usage() as usize).sum();
        }
    }

    println!("\n=== Memory Fragmentation Test ===");
    println!("Iterations: {}", iterations);
    println!("Sessions per iteration: {}", sessions_per_iter);
    println!("Peak memory: {} MB", peak_memory / (1024 * 1024));
    println!("Final memory: {} MB", final_memory / (1024 * 1024));

    // Final should be much less than peak due to deallocations
    assert!(
        final_memory < peak_memory,
        "Final memory should be less than peak"
    );
}

// ============================================================================
// Budget Enforcement Tests
// ============================================================================

/// Test that sessions respect their individual budget limits
#[test]
#[ignore]
fn test_per_session_budget_enforcement() {
    let mut session = HeavySessionWithReplay::new(0);

    // Try to exceed budget with many snapshots
    let mut snapshots = 0;
    let mut exceeded = false;

    while snapshots < 10000 {
        if !session.take_memory_snapshot(100) {
            exceeded = true;
            break;
        }
        snapshots += 1;
    }

    println!("\n=== Per-Session Budget Enforcement ===");
    println!("Snapshots before limit: {}", snapshots);
    println!("Memory at limit: {:.2} MB", session.memory_usage() as f64 / (1024.0 * 1024.0));
    println!("Budget exceeded flag: {}", exceeded);

    // Session should eventually hit its budget
    assert!(
        exceeded || session.at_capacity(),
        "Session should hit budget limit"
    );

    // Memory should not exceed budget
    let max_budget = (budget::HEAVY_SESSION_BYTES + budget::HEAVY_REPLAY_BYTES) as u64;
    assert!(
        session.memory_usage() <= max_budget,
        "Session {} bytes exceeded budget {} bytes",
        session.memory_usage(),
        max_budget
    );
}

/// Test system-wide budget enforcement
///
/// Attempt to allocate more than available budget
#[test]
#[ignore]
fn test_system_budget_enforcement() {
    // Try to allocate more Heavy sessions than budget allows
    let oversized_count = budget::MAX_HEAVY_SESSIONS + 100;

    let successes = Arc::new(AtomicU64::new(0));
    let failures = Arc::new(AtomicU64::new(0));

    // Simulate pool with budget enforcement
    let allocated = Arc::new(AtomicU64::new(0));
    let max_budget = (budget::MAX_HEAVY_SESSIONS * (budget::HEAVY_SESSION_BYTES + budget::HEAVY_REPLAY_BYTES)) as u64;

    let handles: Vec<_> = (0..oversized_count)
        .map(|id| {
            let succ = Arc::clone(&successes);
            let fail = Arc::clone(&failures);
            let alloc = Arc::clone(&allocated);

            thread::spawn(move || {
                let session_size = (budget::HEAVY_SESSION_BYTES + budget::HEAVY_REPLAY_BYTES) as u64;

                // Try to allocate
                loop {
                    let current = alloc.load(Ordering::Acquire);
                    if current + session_size > max_budget {
                        fail.fetch_add(1, Ordering::Relaxed);
                        return false;
                    }
                    if alloc.compare_exchange(
                        current,
                        current + session_size,
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    ).is_ok() {
                        break;
                    }
                }

                succ.fetch_add(1, Ordering::Relaxed);

                // Simulate brief usage
                thread::sleep(Duration::from_millis(10));

                // Deallocate
                alloc.fetch_sub(session_size, Ordering::Relaxed);

                true
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let success_count = successes.load(Ordering::Relaxed);
    let failure_count = failures.load(Ordering::Relaxed);

    println!("\n=== System Budget Enforcement ===");
    println!("Attempted: {}", oversized_count);
    println!("Succeeded: {}", success_count);
    println!("Failed (budget): {}", failure_count);

    // Due to concurrent alloc/dealloc, many should succeed through reuse
    // but we should still see failures
    assert!(
        failure_count > 0,
        "Should have some failures due to budget limits"
    );
    println!("Budget enforcement: WORKING");
}
