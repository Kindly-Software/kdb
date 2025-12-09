//! Dashboard integration module (feature-gated)
//!
//! Provides MetricsSource implementation for clapi_core to integrate with kindly_dash.
//!
//! # I20 Integration Framework Compliance
//!
//! **Q1-Q5: Scope & Justification**
//! - Component A: kindly_dash::MetricsSource (trait)
//! - Component B: clapi_core::BudgetRegistry + MetricsSnapshot + ProviderCircuitArray
//! - Dependency: One-way (clapi_core → kindly_dash)
//! - Problem: Dashboard needs real-time metrics from clapi_core
//! - Justification: Observable system state without manual queries
//!
//! **Q6-Q10: Compatibility**
//! - Q6: Both 100% lockfree (atomic loads only)
//! - Q7: <100ns metrics reads (within budget)
//! - Q8: Both use Result<T, E> (no errors in read-only operations)
//! - Q9: Both Send+Sync (Arc enables shared access)
//! - Q10: No boundary issues (read-only trait, zero mutation)
//!
//! **Q11-Q15: Safety**
//! - Q11: Assumption = Atomic reads never corrupt
//! - Q12: No cascading failures (read-only operations)
//! - Q13: Invariant = Metrics consistent within 100ms window
//! - Q14: No race conditions (100% lockfree atomic loads)
//! - Q15: Rollback = git revert or disable feature flag
//!
//! **Q16-Q20: Validation**
//! - Q16: Minimal test = Spawn dashboard, verify snapshot
//! - Q17: Property = Metrics monotonic (counters never decrease)
//! - Q18: Budget = <100ns overhead (atomic loads only)
//! - Q19: Strategy = I20-Capsule (100% big bang, feature-gated)
//! - Q20: Rollback = Git revert (<5min) or disable `dashboard` feature
//!
//! # Architecture
//!
//! - **Tier**: Polymorphism layer (no capsule tier, read-only trait)
//! - **Pattern**: Adapter (BudgetRegistry → MetricsSource)
//! - **Performance**: <100ns per metric read (atomic loads)
//! - **Concurrency**: 100% lockfree (Arc + atomic loads)
//! - **Integration**: I20-Capsule (deterministic, deploy at 100%)

mod metrics_adapter;

pub use metrics_adapter::ClapiMetricsAdapter;
