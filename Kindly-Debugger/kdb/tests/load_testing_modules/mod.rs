//! Load Testing Module for KDB
//!
//! Validates concurrent session capacity and memory budget compliance for
//! MCP server deployment on kindly-hub (64GB RAM).
//!
//! # Test Categories
//!
//! ## Concurrent Sessions
//! - 500-2000 concurrent debugging sessions
//! - Session pool allocation/deallocation
//! - Tier-based session management (LIGHT/MEDIUM/HEAVY)
//!
//! ## Memory Budget Validation
//! - LIGHT Pool: 96 MB (1,500 x 64KB slots)
//! - MEDIUM Pool: 150 MB (600 x 256KB slots)
//! - HEAVY Pool: 436 MB (400 x 1.09MB slots)
//! - Memory Replay: ~26 GB (400 x 64MB max)
//!
//! ## Mixed Workload Simulation
//! - Realistic MCP usage patterns (60% LIGHT, 30% MEDIUM, 10% HEAVY)
//! - Burst workloads and steady-state churn
//! - Session tier upgrades/downgrades
//!
//! ## Stress Scenarios
//! - Memory pressure and eviction
//! - Rapid tier transitions
//! - Concurrent reconstruction
//! - Recovery from near-OOM conditions
//!
//! # Running Tests
//!
//! These tests are expensive and ignored by default:
//!
//! ```bash
//! # Run all load tests
//! cargo test --test load_testing -- --ignored --nocapture
//!
//! # Run specific category
//! cargo test concurrent_sessions -- --ignored --nocapture
//! cargo test memory_budget -- --ignored --nocapture
//! cargo test mixed_workload -- --ignored --nocapture
//! cargo test stress_scenarios -- --ignored --nocapture
//! ```
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T6 Mixed tier load validation
//! - **B32**: Memory budget validation with 95% CI
//! - **T28**: Production stress testing (Q22-Q28)
//! - **ASSUM**: Resource limits documented and verified
//!
//! # ASSUM Tags
//!
//! - #ASSUME_64GB_TARGET: Tests designed for kindly-hub (64GB RAM)
//! - #ASSUME_LINUX_ONLY: ptrace-based operations require Linux
//! - #ASSUME_MULTI_CORE: Concurrent tests assume 8+ cores

pub mod concurrent_sessions;
pub mod memory_budget;
pub mod mixed_workload;
pub mod stress_scenarios;

// Re-export common utilities
pub use concurrent_sessions::*;
pub use memory_budget::*;
pub use mixed_workload::*;
pub use stress_scenarios::*;

/// Memory budget constants from deployment plan
pub mod budget {
    /// Light pool total allocation (96 MB)
    pub const LIGHT_POOL_BYTES: usize = 96 * 1024 * 1024;

    /// Light session size (64 KB)
    pub const LIGHT_SESSION_BYTES: usize = 64 * 1024;

    /// Maximum light sessions
    pub const MAX_LIGHT_SESSIONS: usize = 1500;

    /// Medium pool total allocation (150 MB)
    pub const MEDIUM_POOL_BYTES: usize = 150 * 1024 * 1024;

    /// Medium session size (256 KB)
    pub const MEDIUM_SESSION_BYTES: usize = 256 * 1024;

    /// Maximum medium sessions
    pub const MAX_MEDIUM_SESSIONS: usize = 600;

    /// Heavy pool total allocation (436 MB)
    pub const HEAVY_POOL_BYTES: usize = 436 * 1024 * 1024;

    /// Heavy session size (1.09 MB)
    pub const HEAVY_SESSION_BYTES: usize = 1_147_392;

    /// Maximum heavy sessions
    pub const MAX_HEAVY_SESSIONS: usize = 400;

    /// Memory replay per heavy session (64 MB max)
    pub const HEAVY_REPLAY_BYTES: usize = 64 * 1024 * 1024;

    /// Total memory budget for 64GB server
    pub const TOTAL_BUDGET_BYTES: usize = 26 * 1024 * 1024 * 1024; // ~26 GB for replay

    /// Safety margin (10% of total budget)
    pub const SAFETY_MARGIN_BYTES: usize = TOTAL_BUDGET_BYTES / 10;
}

/// Session tier enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTier {
    /// Quick attach/detach (64 KB)
    Light,
    /// Step debugging with registers (256 KB)
    Medium,
    /// Full replay with memory tracking (1.09 MB + 64 MB replay)
    Heavy,
}

impl SessionTier {
    /// Get memory footprint for this tier (capsule only, not replay buffer)
    pub fn capsule_bytes(&self) -> usize {
        match self {
            SessionTier::Light => budget::LIGHT_SESSION_BYTES,
            SessionTier::Medium => budget::MEDIUM_SESSION_BYTES,
            SessionTier::Heavy => budget::HEAVY_SESSION_BYTES,
        }
    }

    /// Get total memory footprint including replay buffer (for Heavy tier)
    pub fn total_bytes(&self) -> usize {
        match self {
            SessionTier::Light => budget::LIGHT_SESSION_BYTES,
            SessionTier::Medium => budget::MEDIUM_SESSION_BYTES,
            SessionTier::Heavy => budget::HEAVY_SESSION_BYTES + budget::HEAVY_REPLAY_BYTES,
        }
    }

    /// Get maximum sessions for this tier
    pub fn max_sessions(&self) -> usize {
        match self {
            SessionTier::Light => budget::MAX_LIGHT_SESSIONS,
            SessionTier::Medium => budget::MAX_MEDIUM_SESSIONS,
            SessionTier::Heavy => budget::MAX_HEAVY_SESSIONS,
        }
    }
}

/// Workload distribution profile
#[derive(Debug, Clone, Copy)]
pub struct WorkloadProfile {
    /// Number of light sessions
    pub light_sessions: usize,
    /// Number of medium sessions
    pub medium_sessions: usize,
    /// Number of heavy sessions
    pub heavy_sessions: usize,
    /// Test duration in seconds
    pub duration_secs: u64,
    /// Session churn rate (sessions/second)
    pub churn_rate: f64,
}

impl WorkloadProfile {
    /// Create realistic MCP workload (60% LIGHT, 30% MEDIUM, 10% HEAVY)
    pub fn realistic_mcp(total_sessions: usize) -> Self {
        Self {
            light_sessions: (total_sessions as f64 * 0.60) as usize,
            medium_sessions: (total_sessions as f64 * 0.30) as usize,
            heavy_sessions: (total_sessions as f64 * 0.10) as usize,
            duration_secs: 60,
            churn_rate: 10.0,
        }
    }

    /// Create burst workload (all sessions at once)
    pub fn burst(total_sessions: usize) -> Self {
        Self {
            light_sessions: (total_sessions as f64 * 0.60) as usize,
            medium_sessions: (total_sessions as f64 * 0.30) as usize,
            heavy_sessions: (total_sessions as f64 * 0.10) as usize,
            duration_secs: 10,
            churn_rate: 1000.0, // High churn = burst
        }
    }

    /// Create steady-state workload (continuous churn)
    pub fn steady_state(sessions_per_second: usize, duration_secs: u64) -> Self {
        let total = sessions_per_second * duration_secs as usize;
        Self {
            light_sessions: (total as f64 * 0.70) as usize, // More light for steady state
            medium_sessions: (total as f64 * 0.25) as usize,
            heavy_sessions: (total as f64 * 0.05) as usize,
            duration_secs,
            churn_rate: sessions_per_second as f64,
        }
    }

    /// Calculate total memory requirement
    pub fn memory_requirement(&self) -> usize {
        self.light_sessions * budget::LIGHT_SESSION_BYTES
            + self.medium_sessions * budget::MEDIUM_SESSION_BYTES
            + self.heavy_sessions * (budget::HEAVY_SESSION_BYTES + budget::HEAVY_REPLAY_BYTES)
    }

    /// Calculate total session count
    pub fn total_sessions(&self) -> usize {
        self.light_sessions + self.medium_sessions + self.heavy_sessions
    }
}

/// Test result metrics
#[derive(Debug, Clone, Default)]
pub struct LoadTestMetrics {
    /// Total sessions created
    pub sessions_created: u64,
    /// Total sessions destroyed
    pub sessions_destroyed: u64,
    /// Peak concurrent sessions
    pub peak_concurrent: u64,
    /// Peak memory usage in bytes
    pub peak_memory_bytes: u64,
    /// Average session lifetime in milliseconds
    pub avg_session_lifetime_ms: f64,
    /// Session allocation failures
    pub allocation_failures: u64,
    /// Session upgrade count
    pub upgrades: u64,
    /// Session downgrade count
    pub downgrades: u64,
    /// Test duration in milliseconds
    pub duration_ms: u64,
    /// Throughput (sessions/second)
    pub throughput: f64,
}

impl LoadTestMetrics {
    /// Check if test passed memory budget
    pub fn within_budget(&self) -> bool {
        self.peak_memory_bytes <= budget::TOTAL_BUDGET_BYTES as u64
    }

    /// Check if all allocations succeeded
    pub fn no_allocation_failures(&self) -> bool {
        self.allocation_failures == 0
    }

    /// Print summary report
    pub fn print_summary(&self) {
        println!("\n========== Load Test Results ==========");
        println!("Sessions Created:    {:>10}", self.sessions_created);
        println!("Sessions Destroyed:  {:>10}", self.sessions_destroyed);
        println!("Peak Concurrent:     {:>10}", self.peak_concurrent);
        println!("Peak Memory:         {:>10} MB", self.peak_memory_bytes / (1024 * 1024));
        println!("Avg Session Lifetime:{:>10.2} ms", self.avg_session_lifetime_ms);
        println!("Allocation Failures: {:>10}", self.allocation_failures);
        println!("Upgrades:            {:>10}", self.upgrades);
        println!("Downgrades:          {:>10}", self.downgrades);
        println!("Duration:            {:>10} ms", self.duration_ms);
        println!("Throughput:          {:>10.2} sessions/sec", self.throughput);
        println!("Budget Status:       {}", if self.within_budget() { "PASS" } else { "FAIL" });
        println!("========================================\n");
    }
}
