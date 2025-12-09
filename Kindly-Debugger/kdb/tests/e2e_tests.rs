//! E2E Test Entry Point
//!
//! This file serves as the entry point for the E2E test module.
//! The actual test infrastructure is in the e2e/ directory.
//!
//! Run E2E tests with:
//!   cargo test --test e2e_tests
//!
//! Or run specific E2E tests:
//!   cargo test --test e2e_tests test_basic_attach_detach

// Include the e2e module from the directory
mod e2e;

// Re-run the e2e tests through this entry point
// The actual tests are in e2e/mod.rs
