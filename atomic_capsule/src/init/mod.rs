//! # Init Module - Capsule OS System Initialization Orchestrator
//!
//! **Framework**: UCE34 (Q1-Q34 systematic discovery)
//! **Tiers Used**: T1 (Atomic), T4 (Batch), T6 (Mixed)
//! **Status**: Production Ready
//! **Chaos Compliance**: 100% lockfree, all atomic primitives
//!
//! ## Purpose
//!
//! System initialization orchestrator for Capsule OS providing:
//! - Parallel service startup with dependency resolution (<500ms boot target)
//! - Service lifecycle management (start/stop/restart/status)
//! - Dependency graph with topological sort for boot ordering
//! - Boot phase coordination with generation counters
//!
//! ## Module Structure
//!
//! ```text
//! init/
//! ├── mod.rs                    (This file: module exports and documentation)
//! ├── dependency_graph.rs       (T1 Atomic: Service dependency DAG, 512B)
//! ├── service_manager.rs        (T4 Batch: Service lifecycle coordination, 1KB)
//! └── orchestrator.rs           (T6 Mixed: Boot sequence coordination, 2KB)
//! ```
//!
//! ## Framework Compliance
//!
//! ### UCE34 (Q1-Q34 Systematic Discovery)
//!
//! **Q1-Q9**: Problem Analysis
//! - **Q1 (Problem)**: Parallel service startup with dependency resolution
//! - **Q2 (Value)**: <500ms boot time vs sequential (5-10s with systemd)
//! - **Q3 (Scale)**: 64 services max, 256 dependencies max
//! - **Q4 (Context)**: Capsule OS init replacement for systemd/init.d
//! - **Q5 (Success)**: <500ms boot, correct ordering, graceful failure handling
//! - **Q6 (Data Shape)**: DAG (services + dependencies), service states (8 states)
//! - **Q7 (Core Operation)**: Topological sort (O(V+E)), parallel phase execution
//! - **Q8 (Alternative)**: Sequential startup (5-10s), shell scripts (error-prone)
//! - **Q9 (Transform)**: Sequential → Wave-based parallel (10-20× faster)
//!
//! **Q10-Q12**: Tier Selection
//! - **Q10 (Tier)**: T6 Mixed (T1 dependency graph + T4 batch service starts)
//! - **Q11 (Rust Transform)**: AtomicU64 state machines, lockfree DAG traversal
//! - **Q12 (Nightly)**: Optional portable_simd for dependency bitmap operations
//!
//! **Q30-Q34**: Validation
//! - **Q30 (Validation)**: Compile-time alignment verification
//! - **Q33 (Atomic Capsule)**: All structures use AtomicU64, DualAtomicU64
//! - **Q34 (Auditability)**: Hash-chained boot audit trail
//!
//! ### ASSUM Safety (99.99% Target)
//!
//! - **Generation Counters**: Prevent TOCTOU race conditions in state transitions
//! - **Cache Alignment**: 64B/128B/512B prevent false sharing
//! - **Memory Ordering**: Acquire/Release for state transitions, Relaxed for counters
//! - **No Unsafe**: Zero unsafe code, all safety via type system
//!
//! ### B32 Benchmarking (Fair Baselines)
//!
//! - **Baseline**: systemd sequential startup (~5s for 20 services)
//! - **Capsule**: Wave-based parallel startup
//! - **Expected Speedup**: 10-20× (wave parallelism + lockfree coordination)
//! - **Measurement**: 1000+ iterations, 95% CI
//!
//! ### T28 Testing (4-Tier Pyramid)
//!
//! - **Unit Tests (Q1-Q7)**: 15 tests (dependency graph, service states)
//! - **Property Tests (Q8-Q14)**: 5 tests (DAG acyclicity, topological ordering)
//! - **Integration Tests (Q15-Q21)**: 6 tests (full boot simulation)
//! - **Production Tests (Q22-Q28)**: 4 tests (stress tests, failure recovery)
//! - **Total**: 30 tests minimum
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │              InitOrchestratorCapsule (T6, 2KB)                  │
//! │  ┌─────────────────────────────────────────────────────────┐   │
//! │  │ DualAtomicU64: boot_state (phase | generation)          │   │
//! │  │ Boot Phases: Init → DependencyResolve → ServiceStart →  │   │
//! │  │              Running → Shutdown → Terminated             │   │
//! │  └─────────────────────────────────────────────────────────┘   │
//! │                              │                                  │
//! │  ┌───────────────────────────┼───────────────────────────┐     │
//! │  │                           ▼                           │     │
//! │  │  ┌─────────────────────────────────────────────────┐ │     │
//! │  │  │     DependencyGraphCapsule (T1, 512B)           │ │     │
//! │  │  │  - 64 services max (64-bit bitmaps)             │ │     │
//! │  │  │  - 256 edges max (adjacency list)               │ │     │
//! │  │  │  - O(V+E) topological sort                      │ │     │
//! │  │  │  - Wave-level parallelism extraction            │ │     │
//! │  │  └─────────────────────────────────────────────────┘ │     │
//! │  │                           │                           │     │
//! │  │  ┌─────────────────────────────────────────────────┐ │     │
//! │  │  │     ServiceManagerCapsule (T4, 1KB)             │ │     │
//! │  │  │  - 64 service slots (AtomicU64 each)            │ │     │
//! │  │  │  - Batch start/stop operations                  │ │     │
//! │  │  │  - Health monitoring (heartbeat)                │ │     │
//! │  │  │  - Restart policies (always/on-failure/never)   │ │     │
//! │  │  └─────────────────────────────────────────────────┘ │     │
//! │  └───────────────────────────────────────────────────────┘     │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Performance Targets
//!
//! | Operation | Target | Notes |
//! |-----------|--------|-------|
//! | Boot sequence (20 services) | <500ms | Wave-parallel startup |
//! | Dependency resolution | <1ms | Topological sort + wave extraction |
//! | Service state query | <10ns | Single atomic load |
//! | Service start | <50ms | Process spawn + health check |
//! | Shutdown sequence | <2s | Reverse dependency order |
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use atomic_capsule::init::{
//!     InitOrchestratorCapsule, ServiceDescriptor, RestartPolicy,
//! };
//!
//! // Create orchestrator
//! let orchestrator = InitOrchestratorCapsule::new();
//!
//! // Register services with dependencies
//! orchestrator.register_service(ServiceDescriptor {
//!     name: "database",
//!     command: "/usr/bin/postgres",
//!     depends_on: &[],
//!     restart_policy: RestartPolicy::Always,
//! })?;
//!
//! orchestrator.register_service(ServiceDescriptor {
//!     name: "web-server",
//!     command: "/usr/bin/nginx",
//!     depends_on: &["database"],
//!     restart_policy: RestartPolicy::OnFailure,
//! })?;
//!
//! // Boot system (parallel wave execution)
//! orchestrator.boot()?;
//!
//! // Query service status
//! let status = orchestrator.service_status("web-server");
//! println!("web-server: {:?}", status);
//!
//! // Graceful shutdown
//! orchestrator.shutdown()?;
//! ```

pub mod dependency_graph;
pub mod service_manager;
pub mod orchestrator;

// Re-export public types
pub use dependency_graph::{
    DependencyGraphCapsule, DependencyError, ServiceId, MAX_SERVICES, MAX_EDGES,
};
pub use service_manager::{
    ServiceManagerCapsule, ServiceState, ServiceDescriptor, RestartPolicy,
    ServiceError, ServiceStats, MAX_SERVICE_NAME_LEN,
};
pub use orchestrator::{
    InitOrchestratorCapsule, BootPhase, BootError, BootStats, BootConfig,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all types are exported
        let _service_id: ServiceId = 0;
        let _restart_policy = RestartPolicy::Always;
        let _boot_phase = BootPhase::Init;
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_SERVICES, 64);
        assert_eq!(MAX_EDGES, 256);
        assert!(MAX_SERVICE_NAME_LEN >= 32);
    }
}
