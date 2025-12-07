// Production Validation Test Suite (T28 Q22-Q28)
// Comprehensive production testing for kdb_mcp
//
// Framework Compliance:
// - UCE34: Q10 T6 Mixed tier validation
// - T28: Complete Q22-Q28 production testing
// - COCA: 100% lockfree validation under production load
// - ASSUM: Stress tests verify assumptions hold under load
// - B32: Performance baselines for regression detection
// - I20: Production integration validation
//
// Test Categories:
// - Q22: Stress Tests (10 tests) - High throughput, concurrency, resource limits
// - Q23: Soak Tests (6 tests) - Long-running stability, memory leaks
// - Q24: Chaos Tests (existing framework) - Resilience under failure injection
// - Q25: Real-World Scenarios (10 tests) - End-to-end workflows
// - Q26: Performance Regression (10 tests) - Baseline establishment
// - Q27: Compliance Validation (9 tests) - SOX/SOC2/GDPR/HIPAA
// - Q28: Monitoring Tests (10 tests) - Prometheus metrics, alerting
// - Load Framework (5 tests) - Configurable load testing
//
// Total: 60+ production tests

mod production;

// Re-export test modules for easier access
pub use production::stress_tests;
pub use production::soak_tests;
pub use production::real_world_scenarios;
pub use production::performance_regression;
pub use production::compliance_tests;
pub use production::monitoring_tests;
pub use production::load_framework;
