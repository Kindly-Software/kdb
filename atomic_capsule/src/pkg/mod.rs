//! # Package Manager Module - Capsule OS Package Management
//!
//! **Framework**: UCE34 (Q1-Q34 systematic discovery)
//! **Tiers Used**: T0 (Auditable), T1 (Atomic), T4 (Batch), T9 (Persistent)
//! **Status**: Phase 1 - PackageDbCapsule + DependencyResolverCapsule
//! **Chaos Compliance**: 100% lockfree, all atomic primitives
//!
//! ## Purpose
//!
//! Package management primitives for Capsule OS inspired by dpkg/apt architecture:
//! - **PackageDbCapsule** (T9): Persistent package database with lockfree queries
//! - **DependencyResolverCapsule** (T4): Parallel SAT-solver for dependency resolution
//! - **PackageManagerMetacapsule** (T6): Orchestrates install/upgrade/remove operations
//! - **RepositoryCacheCapsule** (T1+T9): Repository metadata caching with refresh
//! - **PackageVerifierCapsule** (T0+T1): Cryptographic verification (Ed25519 + SHA256)
//! - **TransactionCapsule** (T1): Atomic multi-package transaction support
//!
//! ## Architecture (dpkg/apt-inspired)
//!
//! ```text
//! pkg/
//! ├── mod.rs                    (This file: module exports and documentation)
//! ├── error.rs                  (Package manager error types)
//! ├── types.rs                  (Package metadata, version, dependency types)
//! ├── package_db.rs             (T9 Persistent: Package database)
//! ├── dependency_resolver.rs    (T4 Batch: Parallel dependency resolution)
//! ├── repository_cache.rs       (T1+T9: Repository metadata caching)
//! ├── package_verifier.rs       (T0+T1: Cryptographic verification)
//! ├── transaction.rs            (T1: Atomic transaction support)
//! ├── download_queue.rs         (T4+T8: Parallel download manager)
//! └── metacapsule.rs            (T6 Mixed: PackageManagerMetacapsule)
//! ```
//!
//! ## Design Principles (dpkg/apt Analysis)
//!
//! ### dpkg Concepts (Low-Level)
//! - **Status database**: /var/lib/dpkg/status (package state)
//! - **Control files**: Package metadata (control, conffiles, scripts)
//! - **File lists**: /var/lib/dpkg/info/*.list
//! - **Atomic operations**: dpkg uses fsync barriers
//! - **Triggers**: Deferred processing for efficiency
//!
//! ### apt Concepts (High-Level)
//! - **Repository sources**: /etc/apt/sources.list
//! - **Package cache**: /var/cache/apt/archives
//! - **Dependency resolution**: SAT-solver (apt 1.0+)
//! - **Transaction planning**: Simulate before execute
//!
//! ### Capsule OS Improvements
//! - **100% Lockfree**: No mutex in hot paths (vs dpkg file locks)
//! - **Persistent atomics**: Generation counters prevent TOCTOU
//! - **Parallel resolution**: T4 Batch for dependency SAT (vs sequential apt)
//! - **SIMD verification**: T2 for hash computation
//! - **Audit trails**: Q34 compliance for all operations
//!
//! ## Framework Compliance
//!
//! ### UCE34 (Q1-Q34 Systematic Discovery)
//!
//! **Q1-Q9**: Problem understanding
//! - **Q1 (State Space)**: Package states (NotInstalled, Unpacked, HalfConfigured, Installed, HalfInstalled)
//! - **Q2 (Constraints)**: Atomic transactions, dependency satisfaction, version compatibility
//! - **Q3 (Atomicity)**: Each operation is atomic state transition (generation counter prevents TOCTOU)
//! - **Q9 (Type Safety)**: PackageState enum makes invalid states unrepresentable
//!
//! **Q10-Q12**: Capsule tier selection
//! - **Q10 (Tier Selection)**: T9 (persistence) + T4 (batch resolution) + T1 (atomic state) + T0 (audit)
//! - **Q11 (Rust Transform)**: Lockfree CAS loops, atomic snapshots, zero-copy queries
//! - **Q12 (Nightly)**: Optional SIMD for hash verification, const generics for version comparison
//!
//! **Q30-Q34**: Validation
//! - **Q30 (Validation)**: Compile-time alignment verification, #[derive(ComputationalCapsule)]
//! - **Q33 (Atomic Capsule)**: All structures use AtomicU64, DualAtomicU64, atomic coordination
//! - **Q34 (Auditability)**: Hash-chained audit trail for all package operations
//!
//! ### ASSUM Safety (99.5%+ Target)
//!
//! - **Generation Counters**: Prevent TOCTOU race conditions in package state transitions
//! - **Cache Alignment**: 64B/128B/256B prevent false sharing
//! - **Memory Ordering**: AcqRel for state changes, Relaxed for reads
//! - **Atomic Snapshots**: Consistent view during dependency resolution
//!
//! ### B32 Benchmarking (Fair Baselines)
//!
//! - **Baseline**: dpkg status query (~100-500us with file locks)
//! - **Capsule**: PackageDbCapsule query (<1us lockfree)
//! - **Expected Speedup**: 100-500x for hot-path queries
//! - **Resolution**: Parallel SAT vs sequential (linear scaling with cores)
//!
//! ### T28 Testing (5-Tier Pyramid)
//!
//! - **Unit Tests (Q1-Q7)**: Package state transitions, version comparison
//! - **Property Tests (Q8-Q14)**: Dependency satisfaction invariants
//! - **Integration Tests (Q15-Q21)**: Full install/upgrade/remove cycles
//! - **Production Tests (Q22-Q28)**: Concurrent operations, crash recovery
//! - **Determinism Tests (Q29-Q35)**: Reproducible builds, bit-exact resolution
//!
//! ## Performance Targets
//!
//! | Operation | dpkg/apt | Capsule OS | Speedup |
//! |-----------|----------|------------|---------|
//! | Status query | 100-500us | <1us | 100-500x |
//! | Dependency resolve (1000 pkgs) | 500ms | <50ms | 10x |
//! | Package install (cached) | 100ms | <50ms | 2x |
//! | Database commit | 10ms | <1ms | 10x |
//! | Repository refresh | 5s | <1s | 5x |
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use atomic_capsule::pkg::{
//!     PackageManagerMetacapsule, PackageSpec, PackageDbCapsule,
//!     DependencyResolverCapsule, VersionConstraint,
//! };
//!
//! // Initialize package manager
//! let pkg_mgr = PackageManagerMetacapsule::new("/var/lib/capsule/pkg")?;
//!
//! // Query package status (<1us)
//! let status = pkg_mgr.status("openssl")?;
//! println!("OpenSSL: {:?}", status);
//!
//! // Resolve dependencies (parallel, <50ms for 1000 packages)
//! let plan = pkg_mgr.resolve(&[
//!     PackageSpec::new("nginx", VersionConstraint::GreaterEqual("1.24.0")),
//!     PackageSpec::new("openssl", VersionConstraint::GreaterEqual("3.0")),
//! ])?;
//!
//! // Execute transaction (atomic)
//! pkg_mgr.execute(plan)?;
//!
//! // Audit trail (Q34 compliance)
//! for event in pkg_mgr.audit_log() {
//!     println!("{}: {}", event.timestamp, event.description);
//! }
//! ```
//!
//! ## Trade Secret Notice
//!
//! This module contains proprietary package management infrastructure for Capsule OS.
//! All implementations are trade secrets. Never commit to public repositories.

// Module declarations
pub mod error;
pub mod types;
pub mod version;
pub mod package_db;
pub mod dependency_resolver;
pub mod repository_cache;
pub mod package_verifier;
pub mod transaction;
pub mod download_queue;
pub mod metacapsule;

// Re-export public types
pub use error::{PkgError, PkgResult};
pub use types::{
    Package, PackageId, PackageMetadata, PackageSpec, PackageState,
    Dependency, DependencyKind, FileEntry, Script, ScriptKind,
    Repository, RepositoryEntry, Priority,
};
pub use version::{Version, VersionConstraint, VersionComparison};
pub use package_db::PackageDbCapsule;
pub use dependency_resolver::{DependencyResolverCapsule, ResolutionPlan, ResolutionAction};
pub use repository_cache::RepositoryCacheCapsule;
pub use package_verifier::{PackageVerifierCapsule, VerificationStatus};
pub use transaction::{TransactionCapsule, TransactionState, TransactionOp};
pub use download_queue::{DownloadQueueCapsule, DownloadTask, DownloadStatus};
pub use metacapsule::PackageManagerMetacapsule;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_structure() {
        // Verify module compiles and exports are available
        let _version = Version::new(1, 2, 3);
        assert!(true);
    }
}
