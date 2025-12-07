//! Load Testing Suite for KDB
//!
//! Validates concurrent session capacity and memory budget compliance for
//! MCP server deployment on kindly-hub (64GB RAM).
//!
//! # Running Load Tests
//!
//! These tests are expensive and marked `#[ignore]` by default.
//! Run them explicitly with:
//!
//! ```bash
//! # Run all load tests
//! cargo test --test load_testing -- --ignored --nocapture
//!
//! # Run specific category
//! cargo test --test load_testing concurrent -- --ignored --nocapture
//! cargo test --test load_testing memory -- --ignored --nocapture
//! cargo test --test load_testing workload -- --ignored --nocapture
//! cargo test --test load_testing stress -- --ignored --nocapture
//!
//! # Run single test
//! cargo test --test load_testing test_500_light -- --ignored --nocapture
//! ```
//!
//! # Test Categories
//!
//! | Category              | Tests | Description                            |
//! |-----------------------|-------|----------------------------------------|
//! | concurrent_sessions   | 12    | 500-2000 concurrent session tests      |
//! | memory_budget         | 11    | Per-session and system memory limits   |
//! | mixed_workload        | 10    | Realistic MCP workload simulation      |
//! | stress_scenarios      | 13    | Edge cases and failure recovery        |
//!
//! # Memory Budget (64GB Server - kindly-hub)
//!
//! | Pool   | Size   | Sessions | Per Session |
//! |--------|--------|----------|-------------|
//! | Light  | 96 MB  | 1,500    | 64 KB       |
//! | Medium | 150 MB | 600      | 256 KB      |
//! | Heavy  | 436 MB | 400      | 1.09 MB     |
//! | Replay | ~26 GB | 400      | 64 MB max   |
//!
//! # Framework Compliance
//!
//! - **T28**: Production stress testing (Q22-Q28)
//! - **B32**: Memory budget validation (95% CI)
//! - **ASSUM**: Resource limits documented
//!
//! # ASSUM Tags
//!
//! - #ASSUME_64GB_TARGET: Tests designed for kindly-hub (64GB RAM)
//! - #ASSUME_LINUX_ONLY: ptrace-based operations require Linux
//! - #ASSUME_MULTI_CORE: Concurrent tests assume 8+ cores

// Include the load testing modules
#[path = "load_testing_modules/mod.rs"]
mod load_testing_modules;

// Re-export for test discovery
pub use load_testing_modules::*;
