//! T28 Comprehensive Test Suite
//!
//! This module orchestrates all test categories:
//! - Q1-Q7: Unit tests (individual functions)
//! - Q8-Q14: Property tests (invariants)
//! - Q15-Q21: Integration tests (workflows)
//! - Q22-Q28: Production tests (performance, stress)

mod fixtures;
mod unit;
mod property;
mod integration;
mod production;
