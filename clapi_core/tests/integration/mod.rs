//! Integration Tests Module - T28 Q15-Q21
//!
//! **Framework**: T28 Comprehensive Testing (Integration Tier)
//! **Coverage**: All major features integrated and tested end-to-end
//!
//! # Test Organization
//!
//! ## kindlydb_integration_test.rs
//! - OAuth session persistence
//! - KindlyDB CRUD operations
//! - MVCC concurrent reads
//! - Session lifecycle (create, verify, refresh, revoke)
//! - Performance: <10ms p50 query latency
//!
//! ## stripe_webhook_test.rs
//! - Payment workflow integration
//! - Webhook idempotency
//! - State transitions (pending → confirmed → refunded)
//! - Fixed-point arithmetic validation
//! - Performance: <150ns payment creation, <500ms webhook processing
//!
//! ## provider_api_test.rs
//! - Provider routing integration
//! - Circuit breaker failure detection
//! - Multi-provider failover
//! - Budget preservation on failure
//! - Performance: <300ns total proxy overhead
//!
//! ## compliance_e2e_test.rs
//! - SOX/SOC2/GDPR compliance exports
//! - Hash chain integrity
//! - Multi-format exports (JSON/CSV/etc)
//! - Performance: <10s for 100K entries
//!
//! ## cross_feature_test.rs
//! - Budget + OAuth integration
//! - OAuth + Payment integration
//! - Budget + Circuit breaker integration
//! - Full stack integration (all 4 features)
//! - End-to-end user journey
//! - Performance: <1ms full stack latency
//!
//! # T28 Framework Validation
//!
//! - **Q15**: All integration scopes covered (component interactions)
//! - **Q16**: Minimal integration tests for each feature pair
//! - **Q17**: Property invariants validated (data consistency)
//! - **Q18**: Performance budgets met (<10ms p50 target)
//! - **Q19**: Edge cases handled (failures, expiry, exhaustion)
//! - **Q20**: Stress testing (1000+ concurrent operations)
//! - **Q21**: System recovery (graceful degradation, isolation)
//!
//! # Running Integration Tests
//!
//! ```bash
//! # All integration tests
//! cargo test --test integration --all-features
//!
//! # Specific module
//! cargo test --test integration kindlydb
//! cargo test --test integration stripe
//! cargo test --test integration provider
//! cargo test --test integration compliance
//! cargo test --test integration cross_feature
//!
//! # With ignored tests (stress/load)
//! cargo test --test integration -- --ignored
//! ```

#[cfg(feature = "kindlydb")]
pub mod kindlydb_integration_test;

#[cfg(feature = "payments")]
pub mod stripe_webhook_test;

pub mod provider_api_test;

#[cfg(feature = "compliance")]
pub mod compliance_e2e_test;

pub mod cross_feature_test;
