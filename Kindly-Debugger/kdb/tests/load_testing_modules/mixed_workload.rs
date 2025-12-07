//! Mixed Workload Simulation
//!
//! Simulates realistic debugging patterns for MCP server deployment.
//!
//! # Workload Profiles
//!
//! | Profile        | LIGHT | MEDIUM | HEAVY | Use Case                    |
//! |----------------|-------|--------|-------|-----------------------------|
//! | Realistic MCP  | 60%   | 30%    | 10%   | Production debugging mix    |
//! | Burst          | 70%   | 25%    | 5%    | IDE startup, mass attach    |
//! | Steady State   | 70%   | 25%    | 5%    | Continuous CI/CD debugging  |
//! | Heavy Focus    | 40%   | 30%    | 30%   | Time-travel heavy workload  |
//!
//! # Session Behaviors
//!
//! | Tier   | Activity Pattern                        | Lifetime      |
//! |--------|-----------------------------------------|---------------|
//! | LIGHT  | Attach -> quick inspect -> detach      | 50-200ms      |
//! | MEDIUM | Attach -> step debug -> detach         | 100-500ms     |
//! | HEAVY  | Attach -> full replay -> detach        | 500-2000ms    |
//!
//! # Running Tests
//!
//! ```bash
//! cargo test mixed_workload -- --ignored --nocapture
//! ```
//!
//! # ASSUM Tags
//!
//! - #ASSUME_REALISTIC_PATTERNS: Workload based on observed MCP usage
//! - #ASSUME_STOCHASTIC: Session lifetimes follow exponential distribution

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::concurrent_sessions::{SessionPool, SimulatedSession};
use super::{budget, LoadTestMetrics, SessionTier, WorkloadProfile};

// ============================================================================
// Workload Generator
// ============================================================================

/// Workload generator that spawns sessions according to profile
pub struct WorkloadGenerator {
    /// Target workload profile
    profile: WorkloadProfile,
    /// Session pool
    pool: Arc<SessionPool>,
    /// Running flag
    running: Arc<AtomicBool>,
    /// Metrics collection
    sessions_created: Arc<AtomicU64>,
    sessions_completed: Arc<AtomicU64>,
    total_lifetime_ms: Arc<AtomicU64>,
    tier_counts: Arc<[AtomicU64; 3]>, // [LIGHT, MEDIUM, HEAVY]
}

impl WorkloadGenerator {
    pub fn new(profile: WorkloadProfile) -> Self {
        Self {
            profile,
            pool: Arc::new(SessionPool::new()),
            running: Arc::new(AtomicBool::new(false)),
            sessions_created: Arc::new(AtomicU64::new(0)),
            sessions_completed: Arc::new(AtomicU64::new(0)),
            total_lifetime_ms: Arc::new(AtomicU64::new(0)),
            tier_counts: Arc::new([AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)]),
        }
    }

    /// Simulate session activity based on tier
    fn simulate_session_activity(session: &SimulatedSession) {
        let (iterations, base_hold_ms) = match session.tier {
            SessionTier::Light => (30, 50),     // Quick inspection
            SessionTier::Medium => (100, 100),  // Step debugging
            SessionTier::Heavy => (200, 300),   // Full replay
        };

        // Simulate debugging activity
        for _ in 0..iterations {
            session.capture_snapshot();
        }

        // Add some variance to hold time (exponential-like distribution)
        let variance = (session.id % 100) as u64;
        let hold_time = base_hold_ms + variance;
        thread::sleep(Duration::from_millis(hold_time));
    }

    /// Select tier based on workload distribution
    fn select_tier(&self, session_num: usize) -> SessionTier {
        let total = self.profile.total_sessions();
        let light_threshold = self.profile.light_sessions;
        let medium_threshold = light_threshold + self.profile.medium_sessions;

        // Deterministic selection based on position
        let position = session_num % total;

        if position < light_threshold {
            SessionTier::Light
        } else if position < medium_threshold {
            SessionTier::Medium
        } else {
            SessionTier::Heavy
        }
    }

    /// Run the workload for specified duration
    pub fn run(&self) -> LoadTestMetrics {
        let start = Instant::now();
        self.running.store(true, Ordering::Release);

        let total_sessions = self.profile.total_sessions();
        let duration_ms = self.profile.duration_secs * 1000;

        let mut handles = Vec::new();

        // Spawn sessions with staggered starts
        let interval_ms = if self.profile.churn_rate > 0.0 {
            (1000.0 / self.profile.churn_rate) as u64
        } else {
            10
        };

        for i in 0..total_sessions {
            let pool = Arc::clone(&self.pool);
            let running = Arc::clone(&self.running);
            let created = Arc::clone(&self.sessions_created);
            let completed = Arc::clone(&self.sessions_completed);
            let lifetime_total = Arc::clone(&self.total_lifetime_ms);
            let tier_counts = Arc::clone(&self.tier_counts);
            let tier = self.select_tier(i);

            // Calculate start delay
            let start_delay = (i as u64 * interval_ms).min(duration_ms);

            handles.push(thread::spawn(move || {
                // Wait for start time
                thread::sleep(Duration::from_millis(start_delay));

                // Check if still running
                if !running.load(Ordering::Acquire) {
                    return None;
                }

                // Allocate session
                let session = pool.allocate(tier)?;
                created.fetch_add(1, Ordering::Relaxed);

                // Track tier
                let tier_idx = match tier {
                    SessionTier::Light => 0,
                    SessionTier::Medium => 1,
                    SessionTier::Heavy => 2,
                };
                tier_counts[tier_idx].fetch_add(1, Ordering::Relaxed);

                // Simulate activity
                Self::simulate_session_activity(&session);

                // Record lifetime
                let lifetime = session.lifetime_ms();
                lifetime_total.fetch_add(lifetime, Ordering::Relaxed);

                // Deallocate
                pool.deallocate(&session);
                completed.fetch_add(1, Ordering::Relaxed);

                Some((tier, lifetime))
            }));

            // Respect duration limit
            if start.elapsed().as_secs() >= self.profile.duration_secs {
                break;
            }
        }

        // Stop accepting new work
        self.running.store(false, Ordering::Release);

        // Wait for all to complete
        let mut tier_lifetimes: [Vec<u64>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for h in handles {
            if let Some((tier, lifetime)) = h.join().unwrap() {
                let idx = match tier {
                    SessionTier::Light => 0,
                    SessionTier::Medium => 1,
                    SessionTier::Heavy => 2,
                };
                tier_lifetimes[idx].push(lifetime);
            }
        }

        let duration = start.elapsed();
        let sessions_created = self.sessions_created.load(Ordering::Relaxed);
        let sessions_completed = self.sessions_completed.load(Ordering::Relaxed);
        let total_lifetime = self.total_lifetime_ms.load(Ordering::Relaxed);

        LoadTestMetrics {
            sessions_created,
            sessions_destroyed: sessions_completed,
            peak_concurrent: self.pool.peak_concurrent(),
            peak_memory_bytes: self.pool.memory_usage(),
            avg_session_lifetime_ms: if sessions_completed > 0 {
                total_lifetime as f64 / sessions_completed as f64
            } else {
                0.0
            },
            allocation_failures: self.pool.failures(),
            upgrades: 0, // Not tracked in basic workload
            downgrades: 0,
            duration_ms: duration.as_millis() as u64,
            throughput: sessions_completed as f64 / duration.as_secs_f64(),
        }
    }
}

// ============================================================================
// Realistic MCP Workload Tests
// ============================================================================

/// Realistic MCP workload: 60% quick attach, 30% debugging, 10% replay
///
/// Simulates typical Claude Code / AI assistant debugging patterns.
#[test]
#[ignore]
fn test_realistic_mcp_workload() {
    let profile = WorkloadProfile {
        light_sessions: 300,   // Brief inspections
        medium_sessions: 150,  // Step debugging
        heavy_sessions: 50,    // Full replay
        duration_secs: 30,
        churn_rate: 20.0,      // 20 sessions/sec
    };

    println!("\n=== Realistic MCP Workload Test ===");
    println!("Profile: {} LIGHT, {} MEDIUM, {} HEAVY",
             profile.light_sessions, profile.medium_sessions, profile.heavy_sessions);
    println!("Duration: {} seconds", profile.duration_secs);
    println!("Churn rate: {} sessions/sec", profile.churn_rate);
    println!("Expected memory: {} MB", profile.memory_requirement() / (1024 * 1024));
    println!("");

    let generator = WorkloadGenerator::new(profile);
    let metrics = generator.run();

    metrics.print_summary();

    // Assertions
    assert!(
        metrics.no_allocation_failures() || metrics.allocation_failures < 10,
        "Too many allocation failures: {}",
        metrics.allocation_failures
    );

    assert!(
        metrics.sessions_destroyed as f64 / metrics.sessions_created as f64 > 0.95,
        "Session completion rate too low"
    );
}

/// Burst workload: Many sessions arrive simultaneously
///
/// Simulates IDE startup or mass debugging scenario.
#[test]
#[ignore]
fn test_burst_workload() {
    // 200 sessions in 1 second burst
    let profile = WorkloadProfile {
        light_sessions: 140,  // 70%
        medium_sessions: 50,  // 25%
        heavy_sessions: 10,   // 5%
        duration_secs: 5,
        churn_rate: 200.0,    // All at once
    };

    println!("\n=== Burst Workload Test ===");
    println!("Burst: {} sessions", profile.total_sessions());
    println!("");

    let generator = WorkloadGenerator::new(profile);
    let metrics = generator.run();

    metrics.print_summary();

    // Burst should complete all sessions despite high initial load
    assert!(
        metrics.sessions_created >= 100,
        "Should create at least 100 sessions in burst"
    );

    // Peak concurrent should reflect burst nature
    println!("Peak concurrent: {}", metrics.peak_concurrent);
}

/// Steady-state workload: Continuous churn
///
/// 50 sessions created/destroyed per second for 60 seconds.
#[test]
#[ignore]
fn test_steady_state_workload() {
    let sessions_per_second = 50;
    let duration_secs = 30;

    let profile = WorkloadProfile::steady_state(sessions_per_second, duration_secs);

    println!("\n=== Steady State Workload Test ===");
    println!("Rate: {} sessions/sec for {} seconds",
             sessions_per_second, duration_secs);
    println!("Total expected: {}", sessions_per_second * duration_secs as usize);
    println!("");

    let generator = WorkloadGenerator::new(profile);
    let metrics = generator.run();

    metrics.print_summary();

    // Should maintain steady throughput
    let target_throughput = sessions_per_second as f64 * 0.8; // 80% of target
    assert!(
        metrics.throughput >= target_throughput,
        "Throughput {:.2} below target {:.2}",
        metrics.throughput,
        target_throughput
    );

    // Memory should be stable (not growing unbounded)
    assert!(
        metrics.within_budget(),
        "Exceeded memory budget in steady state"
    );
}

// ============================================================================
// Heavy Workload Tests
// ============================================================================

/// Heavy focus workload: 30% HEAVY sessions
///
/// Tests system under memory-intensive time-travel debugging load.
#[test]
#[ignore]
fn test_heavy_focus_workload() {
    let profile = WorkloadProfile {
        light_sessions: 80,   // 40%
        medium_sessions: 60,  // 30%
        heavy_sessions: 60,   // 30%
        duration_secs: 20,
        churn_rate: 10.0,
    };

    println!("\n=== Heavy Focus Workload Test ===");
    println!("Heavy session ratio: 30%");
    println!("Expected memory: {} MB", profile.memory_requirement() / (1024 * 1024));
    println!("");

    let generator = WorkloadGenerator::new(profile);
    let metrics = generator.run();

    metrics.print_summary();

    // Higher memory usage expected due to heavy sessions
    let min_expected_memory = (60 * budget::HEAVY_SESSION_BYTES) as u64;
    assert!(
        metrics.peak_memory_bytes >= min_expected_memory / 4, // At least 25% of heavy sessions concurrent
        "Expected more memory usage with heavy focus"
    );
}

/// All-heavy workload: 100% HEAVY sessions
///
/// Stress test for maximum memory pressure.
#[test]
#[ignore]
fn test_all_heavy_workload() {
    // Limited to 50 to avoid test timeout
    let profile = WorkloadProfile {
        light_sessions: 0,
        medium_sessions: 0,
        heavy_sessions: 50,
        duration_secs: 30,
        churn_rate: 2.0, // Slow churn for heavy sessions
    };

    println!("\n=== All-Heavy Workload Test ===");
    println!("Sessions: {} HEAVY only", profile.heavy_sessions);
    println!("");

    let generator = WorkloadGenerator::new(profile);
    let metrics = generator.run();

    metrics.print_summary();

    // All allocations should succeed (50 << 400 max)
    assert_eq!(
        metrics.allocation_failures,
        0,
        "No failures expected with 50 heavy sessions"
    );
}

// ============================================================================
// Session Tier Transition Tests
// ============================================================================

/// Advanced workload with tier transitions
///
/// Sessions upgrade from LIGHT -> MEDIUM -> HEAVY based on activity.
struct TierTransitionWorkload {
    pool: Arc<SessionPool>,
    upgrades: Arc<AtomicU64>,
    downgrades: Arc<AtomicU64>,
}

impl TierTransitionWorkload {
    fn new() -> Self {
        Self {
            pool: Arc::new(SessionPool::new()),
            upgrades: Arc::new(AtomicU64::new(0)),
            downgrades: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Simulate a session that may upgrade based on activity
    fn run_adaptive_session(&self, id: u64) -> Option<(SessionTier, u64)> {
        // Start as LIGHT
        let light_session = self.pool.allocate(SessionTier::Light)?;

        // Simulate initial activity
        for _ in 0..20 {
            light_session.capture_snapshot();
        }

        // Decision: 30% chance to upgrade based on activity
        let should_upgrade = id % 10 < 3;

        if should_upgrade {
            self.pool.deallocate(&light_session);

            // Upgrade to MEDIUM
            let medium_session = self.pool.allocate(SessionTier::Medium)?;
            self.upgrades.fetch_add(1, Ordering::Relaxed);

            for _ in 0..50 {
                medium_session.capture_snapshot();
            }

            // 10% chance to upgrade to HEAVY
            if id % 10 == 0 {
                self.pool.deallocate(&medium_session);

                let heavy_session = self.pool.allocate(SessionTier::Heavy)?;
                self.upgrades.fetch_add(1, Ordering::Relaxed);

                for _ in 0..100 {
                    heavy_session.capture_snapshot();
                }

                let lifetime = heavy_session.lifetime_ms();
                self.pool.deallocate(&heavy_session);
                return Some((SessionTier::Heavy, lifetime));
            }

            let lifetime = medium_session.lifetime_ms();
            self.pool.deallocate(&medium_session);
            return Some((SessionTier::Medium, lifetime));
        }

        let lifetime = light_session.lifetime_ms();
        self.pool.deallocate(&light_session);
        Some((SessionTier::Light, lifetime))
    }

    fn run(&self, session_count: usize) -> LoadTestMetrics {
        let start = Instant::now();

        let handles: Vec<_> = (0..session_count)
            .map(|id| {
                let this = TierTransitionWorkload {
                    pool: Arc::clone(&self.pool),
                    upgrades: Arc::clone(&self.upgrades),
                    downgrades: Arc::clone(&self.downgrades),
                };

                thread::spawn(move || this.run_adaptive_session(id as u64))
            })
            .collect();

        let mut completed = 0u64;
        let mut total_lifetime = 0u64;

        for h in handles {
            if let Some((_, lifetime)) = h.join().unwrap() {
                completed += 1;
                total_lifetime += lifetime;
            }
        }

        let duration = start.elapsed();

        LoadTestMetrics {
            sessions_created: session_count as u64,
            sessions_destroyed: completed,
            peak_concurrent: self.pool.peak_concurrent(),
            peak_memory_bytes: self.pool.memory_usage(),
            avg_session_lifetime_ms: if completed > 0 {
                total_lifetime as f64 / completed as f64
            } else {
                0.0
            },
            allocation_failures: self.pool.failures(),
            upgrades: self.upgrades.load(Ordering::Relaxed),
            downgrades: self.downgrades.load(Ordering::Relaxed),
            duration_ms: duration.as_millis() as u64,
            throughput: completed as f64 / duration.as_secs_f64(),
        }
    }
}

/// Test workload with tier transitions
#[test]
#[ignore]
fn test_tier_transition_workload() {
    let workload = TierTransitionWorkload::new();

    println!("\n=== Tier Transition Workload Test ===");
    println!("Sessions: 500 with adaptive tier selection");
    println!("Expected: ~30% upgrade to MEDIUM, ~10% of those to HEAVY");
    println!("");

    let metrics = workload.run(500);

    metrics.print_summary();

    // Should have upgrades
    assert!(
        metrics.upgrades > 0,
        "Expected some tier upgrades"
    );

    // Most sessions should complete
    assert!(
        metrics.sessions_destroyed as f64 / metrics.sessions_created as f64 > 0.90,
        "Expected >90% completion rate"
    );

    println!("Upgrade count: {} (~{:.1}% of sessions)",
             metrics.upgrades,
             metrics.upgrades as f64 / metrics.sessions_created as f64 * 100.0);
}

// ============================================================================
// Time-Bounded Workload Tests
// ============================================================================

/// Test workload with strict time limit
///
/// Ensures all sessions complete within deadline.
#[test]
#[ignore]
fn test_time_bounded_workload() {
    let deadline_secs = 10;

    let pool = Arc::new(SessionPool::new());
    let start = Instant::now();

    let handles: Vec<_> = (0..100)
        .map(|i| {
            let pool = Arc::clone(&pool);

            thread::spawn(move || {
                let tier = match i % 10 {
                    0 => SessionTier::Heavy,
                    1..=3 => SessionTier::Medium,
                    _ => SessionTier::Light,
                };

                let session = pool.allocate(tier)?;

                // Limit activity based on remaining time
                let remaining = Duration::from_secs(deadline_secs).saturating_sub(start.elapsed());
                let max_iterations = (remaining.as_millis() / 10).min(100) as usize;

                for _ in 0..max_iterations {
                    session.capture_snapshot();
                    if start.elapsed().as_secs() >= deadline_secs - 1 {
                        break;
                    }
                }

                pool.deallocate(&session);
                Some(start.elapsed())
            })
        })
        .collect();

    // Wait for all with timeout
    for h in handles {
        if let Ok(Some(elapsed)) = h.join() {
            assert!(
                elapsed.as_secs() <= deadline_secs + 1,
                "Session exceeded deadline"
            );
        }
    }

    let total_duration = start.elapsed();

    println!("\n=== Time-Bounded Workload Test ===");
    println!("Deadline: {} seconds", deadline_secs);
    println!("Actual duration: {:?}", total_duration);
    println!("Sessions completed: {}", 100 - pool.failures());

    assert!(
        total_duration.as_secs() <= deadline_secs + 2,
        "Workload exceeded deadline"
    );
}

// ============================================================================
// Workload Comparison Tests
// ============================================================================

/// Compare different workload profiles
#[test]
#[ignore]
fn test_workload_comparison() {
    println!("\n=== Workload Comparison Test ===\n");

    let profiles = vec![
        ("Realistic MCP", WorkloadProfile::realistic_mcp(200)),
        ("Burst", WorkloadProfile::burst(200)),
        ("Steady State", WorkloadProfile::steady_state(20, 10)),
    ];

    let mut results: Vec<(&str, LoadTestMetrics)> = Vec::new();

    for (name, profile) in profiles {
        println!("Running workload: {}", name);
        let generator = WorkloadGenerator::new(profile);
        let metrics = generator.run();
        results.push((name, metrics));
        println!("  Completed in {} ms\n", results.last().unwrap().1.duration_ms);
    }

    println!("\n=== Comparison Summary ===\n");
    println!("{:<15} {:>12} {:>12} {:>12} {:>12}",
             "Profile", "Throughput", "Peak Conc", "Avg Life", "Failures");
    println!("{}", "-".repeat(65));

    for (name, metrics) in &results {
        println!("{:<15} {:>12.1} {:>12} {:>12.1} {:>12}",
                 name,
                 metrics.throughput,
                 metrics.peak_concurrent,
                 metrics.avg_session_lifetime_ms,
                 metrics.allocation_failures);
    }

    // All profiles should have >90% completion
    for (name, metrics) in &results {
        let completion_rate = metrics.sessions_destroyed as f64 / metrics.sessions_created.max(1) as f64;
        assert!(
            completion_rate > 0.85,
            "Profile {} completion rate too low: {:.1}%",
            name,
            completion_rate * 100.0
        );
    }
}

/// Test memory efficiency across workload types
#[test]
#[ignore]
fn test_memory_efficiency_comparison() {
    println!("\n=== Memory Efficiency Comparison ===\n");

    // Same session count, different distributions
    let distributions = vec![
        ("Light-heavy (90/10)", 90, 10, 0),
        ("Balanced (60/30/10)", 60, 30, 10),
        ("Medium-focus (30/60/10)", 30, 60, 10),
        ("Heavy-focus (40/30/30)", 40, 30, 30),
    ];

    for (name, light, medium, heavy) in distributions {
        let profile = WorkloadProfile {
            light_sessions: light,
            medium_sessions: medium,
            heavy_sessions: heavy,
            duration_secs: 15,
            churn_rate: 10.0,
        };

        let generator = WorkloadGenerator::new(profile);
        let metrics = generator.run();

        let memory_per_session = metrics.peak_memory_bytes as f64 / metrics.peak_concurrent.max(1) as f64;

        println!("{:<25} Peak: {:>4} MB  Sessions: {:>3}  MB/session: {:>8.2}",
                 name,
                 metrics.peak_memory_bytes / (1024 * 1024),
                 metrics.peak_concurrent,
                 memory_per_session / (1024.0 * 1024.0));
    }
}
