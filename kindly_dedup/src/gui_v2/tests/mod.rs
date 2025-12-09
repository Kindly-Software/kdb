//! GUI v2 Test Suite
//!
//! **Purpose**: Comprehensive testing for KGPU-based GUI rendering
//!
//! # Test Modules
//!
//! - `kgpu_validation`: End-to-end KGPU integration tests
//!
//! # Running Tests
//!
//! ```bash
//! # Unit tests (fast)
//! cargo test --lib gui_v2::tests
//!
//! # Include ignored tests (stress/benchmark)
//! cargo test --lib gui_v2::tests -- --ignored
//!
//! # All tests
//! cargo test --lib gui_v2::tests -- --include-ignored
//! ```

#[cfg(test)]
mod kgpu_validation;
