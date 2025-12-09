//! Integration Test Suite for kindly_dedup v3.1.0
//!
//! **Status**: v3.1.0 Commercial Release - Integration Testing
//! **Framework**: T28 (Integration Tier Q15-Q21)
//! **Purpose**: End-to-end workflow validation and protection system integration
//!
//! ## Test Modules
//!
//! ### end_to_end_test
//! Full deduplication workflow integration:
//! - Load synthetic corpus (100 docs with known duplicates)
//! - Run UniversalDedupPipeline
//! - Verify duplicate detection accuracy
//! - Export results to JSON format
//! - Verify output integrity
//!
//! ### protection_test
//! Protection system integration (requires `binary-protection` feature):
//! - License validation flow
//! - Demo tier limits (CommercialLimiterCapsule)
//! - Protection status checks
//! - Tier enforcement
//!
//! ## Execution
//!
//! ```bash
//! # All integration tests
//! cargo test --test integration
//!
//! # End-to-end only
//! cargo test --test integration end_to_end
//!
//! # Protection tests (requires feature)
//! cargo test --test integration protection --features binary-protection
//! ```
//!
//! ## Framework Compliance
//!
//! - **T28**: Integration tier (Q15-Q21)
//! - **ASSUM**: All assumptions documented and verified
//! - **Chaos**: Tests use capsule APIs only (no internal state access)
//! - **B32**: Performance assertions based on validated benchmarks

pub mod end_to_end_test;
pub mod protection_test;
