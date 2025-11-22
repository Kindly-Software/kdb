//! HTTP Parser Tests
//!
//! **T28 Testing Framework Coverage**:
//! - Tier 1 (Unit): See individual module tests + hybrid_tests.rs
//! - Tier 2 (Property): See property_tests.rs
//! - Tier 3 (Integration): See integration_tests.rs
//! - Tier 4 (Production): See production_tests.rs + assum_validation.rs
//!
//! **ASSUM Safety Validation**:
//! - Concurrent stress tests: assum_validation.rs

mod assum_validation;
mod chunked_encoding_tests;
mod hpack_tests;
mod http2_connection_tests;
mod hybrid_tests;
mod integration_tests;
mod keep_alive_tests;
mod production_tests;
