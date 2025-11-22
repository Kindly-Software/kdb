//! # Atomic Capsule Foundation Primitives
//!
//! **Foundation crate for atomic capsule architecture.**
//!
//! This crate provides the fundamental primitives needed for atomic capsule-based systems:
//! - **Alignment tiers**: Type-safe cache alignment for hot/warm/cold data paths
//! - **Retry policies**: Exponential backoff for compare-exchange loops
//! - **Architecture detection**: Runtime CPU cache line size detection
//! - **SIMD capsules**: Vectorized computation primitives (nightly feature)
//! - **Fixed-point capsules**: Deterministic arithmetic primitives
//!
//! ## Design Principles (UCE33 Framework Applied)
//!
//! - **Q28 (Simplicity)**: Minimal foundation - alignment + retry + arch + SIMD primitives
//! - **Q29 (Constraints)**: Hardware limits - 64/128/256-byte cache lines, SIMD alignment
//! - **Q30 (Validation)**: Compile-time alignment verification via const generics
//! - **Q31 (Rust Transform)**: Zero-cost abstractions via traits + const generics
//! - **Q32 (Nightly Enhancement)**: Cutting-edge features for advanced capabilities
//!   - `portable_simd`: Cross-platform SIMD acceleration (std::simd)
//!   - `const_fn_floating`: Compile-time floating-point math
//!   - `const_trait_impl`: Const trait implementations
//! - **Q33 (Atomic Capsule)**: Foundation patterns for all capsule implementations
//!
//! ## IMPL-2 V3.0 Compliance
//!
//! This foundation crate ships **exactly** what's needed:
//! - Core: lib.rs, alignment.rs, arch.rs, retry.rs
//! - SIMD primitives: simd_f32.rs, simd_f64.rs, fixed_q16_16.rs (optional, nightly)
//! - Zero dependencies (uses only `core`)
//! - No premature abstraction - traits justified by 3+ implementations
//!
//! ## Usage
//!
//! ```rust
//! use atomic_capsule::{AlignmentTier, HotTier, RetryPolicy};
//!
//! // Define hot-path structure with 64-byte alignment
//! #[repr(C, align(64))]
//! struct HotCapsule {
//!     data: [u8; 64],
//! }
//!
//! impl AlignmentTier for HotCapsule {
//!     const TIER: &'static str = "hot";
//!     const ALIGNMENT: usize = 64;
//! }
//!
//! // Use retry policy for CAS loops
//! let mut policy = RetryPolicy::default();
//! loop {
//!     // attempt compare_exchange...
//!     if policy.should_yield() {
//!         policy.backoff();
//!     }
//! }
//! ```
//!
//! ## SIMD Primitives (Nightly Feature)
//!
//! ```rust,ignore
//! use atomic_capsule::SimdF32x8Capsule;
//!
//! let a = SimdF32x8Capsule::from_array([1.0; 8]);
//! let b = SimdF32x8Capsule::from_array([2.0; 8]);
//! let result = a.add(&b);
//! assert_eq!(result.load(), [3.0; 8]);
//! ```
//!
//! ## Safety Model (ASSUM Framework)
//!
//! All alignment assumptions are verified at compile-time:
//! - `#[repr(C, align(N))]` guarantees alignment
//! - Const generic bounds enforce power-of-2 alignments
//! - No runtime alignment checks needed (zero-cost)
//!
//! ## Tier 0: Auditable Capsule Foundation (NEW)
//!
//! Hash-chain integrity for compliance audit trails:
//! - **Fast Hash** (xxHash64): <5ns, development use (feature: `fast-hash`)
//! - **Crypto Hash** (BLAKE3): <100ns, production audit trails (feature: `audit-trail`)
//! - **FIPS Hash** (SHA-256): <200ns, government compliance (feature: `fips-compliant`)
//!
//! ## Tier 9: Capsule-Native Mmap (NEW - v0.3.4)
//!
//! Zero-dependency memory-mapped file management:
//! - **Zero dependencies**: Uses only std + libc FFI (no memmap2)
//! - **100% capsule-based**: Atomic coordination, lockfree allocation
//! - **Cross-platform**: Linux/macOS/Windows mmap syscalls
//! - **Performance**: <10ms init for 1GB, <50ns allocation, <5ns access
//!
//! ```rust,ignore
//! use atomic_capsule::mmap::CapsuleMmap;
//!
//! // Create memory-mapped file
//! let mmap = CapsuleMmap::create("data.bin", 1024 * 1024)?;
//!
//! // Zero-copy atomic access
//! let region = mmap.allocate_region(64)?;
//! region.write_u64(0, 12345);
//! assert_eq!(region.read_u64(0), 12345);
//!
//! // Persist to disk
//! mmap.flush()?;
//! ```
//!
//! **Migration Timeline**: capsule-mmap (v0.3.4) → mmap-persistence deprecated (v0.4.0) → removed (v0.5.0)
//!
//! ## Performance Targets
//!
//! Based on The Atomic Capsule principles:
//! - Alignment check: Compile-time only (0ns runtime)
//! - Retry backoff: <5ns per iteration
//! - Arch detection: One-time cost <100ns, then cached
//! - SIMD operations: 2-6ns (8 parallel operations)
//! - Fixed-point arithmetic: 5-15ns (deterministic)
//! - **Hash computation: <5ns (fast), <100ns (crypto), <200ns (FIPS)**
//!
//! ## No-std Compatible
//!
//! This crate works in `no_std` environments (default).
//! Enable `std` feature for additional functionality.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs, rust_2018_idioms)]
// Q32 Nightly Enhancement - Cutting-edge features
#![cfg_attr(feature = "portable_simd", feature(portable_simd))]
#![cfg_attr(
    any(feature = "portable_simd", feature = "const-serialize"),
    feature(const_trait_impl)
)]
#![cfg_attr(feature = "nightly", feature(generic_const_exprs))]
#![cfg_attr(feature = "nightly-atomic", feature(atomic_from_mut))]
#![cfg_attr(
    any(feature = "portable_simd", feature = "nightly"),
    allow(incomplete_features)
)] // generic_const_exprs and const_trait_impl are incomplete

#[cfg(feature = "std")]
extern crate std;

// Panic handler for no_std mode (required by Rust)
// #ASSUME_NO_STD_PANIC: In no_std mode, panics abort execution
// #VERIFY_NO_STD_PANIC: Embedded targets must handle panics via linker
#[cfg(all(not(feature = "std"), not(test)))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    // In no_std mode, panics are unrecoverable
    // Embedded targets should override this via linker script
    loop {
        // Infinite loop to satisfy diverging function requirement
        // Hardware watchdog or debugger should catch this
        core::hint::spin_loop();
    }
}

// Public API modules - Core foundation
pub mod alignment;
pub mod arch;
pub mod retry;

// Compile-time verification macros (Q30 Empirical Validation)
// Note: Macros are exported at crate root due to #[macro_export]
pub mod verification;

// Tier 0: Auditable primitives (hex encoding, etc. - zero dependencies)
pub mod auditable;

// Q33 Computational Capsule Trait Hierarchy
pub mod traits;

// Integration helpers (Phase 2 Priorities #7-8)
pub mod macros;
#[cfg(feature = "nightly")] // Requires generic_const_exprs
pub mod packed_state;

// Field-level optimization patterns (Phase 4: Field Optimization, requires nightly)
// DEPRECATED: Use pub mod patterns unconditionally for DualAtomicU64 access
pub mod patterns;

// Re-export computational capsule traits
#[allow(unused_imports)] // Public API re-exports
pub use traits::{
    AtomicCapsule, BatchCapsule, ComputationalCapsule, FixedPointCapsule, MixedCapsule,
    StreamingCapsule,
};

// Tier 0: Auditable Capsule Foundation (requires std)
#[cfg(feature = "std")]
pub use traits::{AuditableCapsule, CapsuleAuditTrail, CapsuleSnapshot};

#[cfg(feature = "portable_simd")]
pub use traits::SimdCapsule;

// Q33 SIMD Computational Primitives (requires portable_simd OR nightly-atomic OR complex-fixed OR cpu-capabilities)
// primitives module exposed when either feature enabled
// Primitives module (always available - includes progress_tracker)
pub mod primitives;

// Composite Capsules Module (Tier 6 Mixed Compound) - Phase 10
#[cfg(feature = "portable_simd")]
pub mod composite;

// Tier 7: Heterogeneous GPU Acceleration (Phase 5: GPU Foundation)
#[cfg(any(feature = "gpu-cuda", feature = "gpu-rocm", feature = "gpu-all"))]
pub mod gpu;

#[cfg(feature = "portable_simd")]
pub use primitives::{SimdF32x8Capsule, SimdF64x8Capsule};

// Phase 2.1: SIMD + Fixed-Point Vectorization Layer (portable_simd feature)
#[cfg(feature = "portable_simd")]
pub use primitives::{
    BatchSimdFixedPoint,
    // Aliased to avoid conflicts with existing types
    SimdF32x8CapsuleNew,
    SimdFixedPointQ16x8Capsule,
    SimdI32x8CapsuleNew,
};

// Re-export core types for convenience
pub use alignment::{AlignmentMarker, AlignmentTier, ColdTier, HotTier, WarmTier};
pub use arch::{
    detect_cache_line_size, recommended_hot_alignment, recommended_warm_alignment, CacheLineSize,
};
pub use retry::{BackoffStrategy, RetryPolicy};

// Re-export integration helpers
pub use macros::define_capsule;
#[cfg(feature = "nightly")] // Requires generic_const_exprs
pub use packed_state::{PackedStateBuilder, PackedStateUnpacker, UnpackState};

// Re-export field optimization patterns (nightly only)
#[cfg(feature = "nightly")]
pub use patterns::{CacheLineAligned, DualAtomicU64};

// Phase 4.2: Complex Number Primitives (T2 SIMD + T3 Fixed-Point)
#[cfg(feature = "complex-simd")]
pub use primitives::complex::ComplexF32x4;

#[cfg(feature = "complex-fixed")]
pub use primitives::complex::ComplexCell;

// Phase 4.2: CNLS Pattern (T6 Mixed Composite)
// Note: Re-exports will be enabled when CNLSRuleCapsule is implemented
// #[cfg(feature = "cnls")]
// pub use patterns::cnls::{CNLSRuleCapsule, evolve_cnls_4d};

// Phase 1: CPU Capability Detection (T1 Atomic)
#[cfg(feature = "cpu-capabilities")]
pub use crate::primitives::cpu_capabilities::CpuCapabilityCapsule;

// Error types for audit and protection operations
#[cfg(feature = "std")]
pub mod error;

// Tier 0: Auditable Capsule Foundation - Hash module
pub mod hash;

// Tier 1: Installation State Tracking - Progress and phase management (requires std)
#[cfg(feature = "std")]
pub mod install;

// Tier 1: Parallel Computing - Lockfree work-stealing (requires std)
#[cfg(feature = "std")]
pub mod parallel;

// Tier 0: Forensics & Compliance Module (SOX/GDPR/SOC2, requires std)
#[cfg(feature = "std")]
pub mod forensics;

// Tier 1/4: Collections - Lockfree concurrent data structures (requires std)
#[cfg(feature = "std")]
pub mod collections;

// Tier 0: Deterministic Serialization (Phase 4 - CapsuleSerialize, optional feature)
#[cfg(feature = "capsule-serialize")]
pub mod serialize;

// Tier 1: CBOR Writer/Reader Capsule (RFC 8949, lockfree binary serialization)
#[cfg(feature = "cbor")]
pub mod cbor_capsule;
#[cfg(feature = "cbor")]
pub use cbor_capsule::{CborWriterCapsule, CborReaderCapsule, CborValue, CborError};

// Tier 9: Persistent Capsules - Memory-mapped file manager (Phase 9, optional feature)
#[cfg(feature = "mmap-persistence")]
pub mod persistence;

// Phase 1: Capsule-Native Mmap (T9 tier) - NEW v0.3.4 (parallel to mmap-persistence)
// #ASSUME_CAPSULE_MMAP: Zero dependencies, 100% capsule-based implementation
// #VERIFY_CAPSULE_MMAP: All 20 I20 integration questions validated
//
// I20 Integration Strategy:
// - Q19 (Strategy): I20-Progressive (parallel deployment, deprecate memmap2 in v0.4.0)
// - Q20 (Rollback): Git revert (<5 minutes, likelihood <5%)
//
// Migration Timeline:
// - v0.3.4: capsule-mmap introduced (parallel to mmap-persistence)
// - v0.4.0: mmap-persistence marked deprecated
// - v0.5.0: mmap-persistence removed (breaking change with migration guide)
//
// Usage: cargo build --features capsule-mmap
#[cfg(feature = "capsule-mmap")]
pub mod mmap;

// Phase 2.7: MCP (Model Context Protocol) - T1 Atomic tool registry (<120ns lookup)
// Tool registration and routing for MCP-compatible systems
#[cfg(feature = "std")]
pub mod mcp;

// Phase 2: Inference Primitives - T2+T3+T4+T5 compound inference capsules (nightly-first)
#[cfg(feature = "inference-primitives")]
pub mod inference;

// Tier 10: Probabilistic Computational Capsules (LSH + MinHash)
#[cfg(feature = "probabilistic")]
pub mod probabilistic;

// T8 Network: HTTP Server Capsules
#[cfg(feature = "http")]
pub mod http;

// T3 Fixed-Point: TUI Configuration Capsules (T3 deterministic thresholds)
#[cfg(feature = "std")]
pub mod tui;

// Re-export TUI capsules for convenience (requires std)
#[cfg(feature = "std")]
pub use tui::{RenderBufferCapsule, AuditLogCapsule, FileNavigatorCapsule, ScreenStateCapsule, TerminalCapabilityCapsule};

// T8: Network Capsules - Distributed coordination (requires std + tokio)
#[cfg(all(feature = "std", feature = "network"))]
pub mod network;

// T1+T8+T10: Load Balancing Capsules - Health checking, session affinity, consistent hashing
#[cfg(feature = "std")]
pub mod load_balancing;

// T1+T5 Runtime Module - Lockfree async runtime primitives (requires std)
#[cfg(feature = "std")]
pub mod runtime;

// T5 Streaming Module - O(1) incremental computation (window, aggregation, filters)
#[cfg(feature = "streaming")]
pub mod streaming;

// WebSocket Module - T5 Frame parsing + T8 Network (RFC 6455)
#[cfg(feature = "std")]
pub mod websocket;

// T8+T1+T4 Network: TLS Module (TLS 1.3 encryption, session caching, ALPN)
#[cfg(feature = "tls")]
pub mod tls;

// Phase 3: Data Protection Capsules - T6 Mixed (T0+T1+T9) (requires std)
#[cfg(feature = "std")]
pub mod protection;

// Phase 2.6: Daemon Module - T1 Atomic Inter-Process Synchronization (requires std)
#[cfg(feature = "std")]
pub mod daemon;

// CLI Capsule - Universal command-line parser (zero deps, T0 Auditable)
#[cfg(feature = "std")]
pub mod cli;

// Shell Module - Universal shell alias management (T6 Mixed: T0+T1+T9)
#[cfg(all(feature = "std", feature = "queue-bounded"))]
pub mod shell;

// Phase 3 Compression Primitives (T2/T3/T4/T6 multi-tier capsules)
#[cfg(any(
    feature = "compression-lz4",
    feature = "compression-q4-4",
    feature = "compression-simd-parse",
    feature = "compression-streaming"
))]
pub mod compression;

// T11 QuantumHybrid: Quantum State Simulation + CNLS Wave Dynamics
// - quantum-simulation: Full quantum algorithms (Shor's, Grover's, QAOA)
// - quantum-cnls: CNLS wave simulation (T6 Mixed: T2 SIMD + T3 Fixed-Point)
// - quantum-multi-qubit: Multi-qubit gates (CNOT, CZ, SWAP, Toffoli) - Phase Q3.3
// - quantum-stabilizer: Gottesman-Knill stabilizer formalism - Phase Q3.6
#[cfg(any(feature = "quantum-simulation", feature = "quantum-stabilizer", feature = "quantum-cnls", feature = "quantum-multi-qubit", feature = "quantum-fusion", feature = "quantum-syndrome", feature = "quantum-union-find", feature = "qec-decoders"))]
pub mod quantum;

// T11 QuantumHybrid: Pure-Capsule Quantum Simulator (Phase Q3)
// - quantum-pure: Zero-dependency quantum simulator (SIMD gates, lockfree coordination)
// - Phase Q3.3: Multi-qubit gates (CNOT, CZ, SWAP, Toffoli)
#[cfg(feature = "quantum-pure")]
pub mod quantum_pure;

// Observability Capsules (T1 Atomic + T10 Probabilistic)
#[cfg(feature = "std")]
// pub mod observability; // TODO: Merge from atomic-capsule-http-server branch

// T2 SIMD + T4 Batch: Text Processing Capsules
// - TokenizationBatchCapsule (T4): Available with tokenization-batch feature
// - SIMD text hashing (T2): Available with portable_simd feature
#[cfg(any(feature = "portable_simd", feature = "tokenization-batch"))]
pub mod text;

// Platform-Specific Capsules (T5/T8/T9 Native OS Features)
// NEW: Day 2 - Platform module for OS-specific features
//
// This module provides platform-specific implementations:
// - native::persistence: T9 mmap capsules (replaces old mmap module)
// - native::async_log: T5 async logging (replaces collections::async_log)
// - native::network: T8 network capsules (replaces old network module)
//
// Feature flags:
// - preset-native: Enable all native platform features
// - capsule-mmap: Enable T9 persistence only
// - async-log: Enable T5 async logging only
// - network: Enable T8 network only
//
// WASM exclusion: Platform module automatically excluded for wasm32 target
// Enabled when capsule-mmap, async-log, or network features are active
#[cfg(all(
    not(target_arch = "wasm32"),
    any(feature = "capsule-mmap", feature = "async-log", feature = "network")
))]
pub mod platform;



// Re-export hash types for convenience
pub use hash::{AtomicHash256, AtomicHash64};

// Re-export installation state tracking and signature verification (requires std)
#[cfg(feature = "std")]
pub use install::{InstallerStateCapsule, SignatureVerifierCapsule, SignatureVerifierError, VerificationResult};

// Re-export collection types for convenience (requires std)
#[cfg(feature = "std")]
pub use collections::{
    channel, BroadcastError, BroadcastReceiver, BroadcastResult, BroadcastSender,
    ConcurrentMapCapsule, LockfreeHashTable, StatsCapsule64, StatsSnapshot,
};

// Re-export protection capsules for convenience (requires std)
#[cfg(feature = "std")]
pub use protection::{
    AuditEntry, AuditTrailCapsule, BackupCoordinatorCapsule, BackupResult, BackupStatus,
    DataProtectionCapsule, PrecommitGuardCapsule, PrecommitResult,
};

// Re-export daemon capsules for convenience (requires std)
#[cfg(feature = "std")]
pub use daemon::{DaemonError, DaemonLockCapsule, DaemonResult, LockGuard};

#[cfg(all(feature = "std", feature = "queue-bounded"))]
pub use daemon::{DaemonQueueCapsule, WaitEntry};

// Re-export load balancing capsules for convenience (requires std)
#[cfg(feature = "std")]
pub use load_balancing::{
    BackendHealthState, HealthCheckCapsule, HealthCheckError, HealthCheckResult, HealthCheckType,
    HealthStatus, ErrorType, PassiveHealthMonitor, CircuitBreakerIntegration,
    SessionAffinityCapsule, AffinityMode, SessionEntry, SessionStatistics,
};

// Re-export TLS capsules for convenience (requires std)
// Phase 1 (current): Metrics + Certificate only
// Phase 2+ (planned): Add AlpnMetrics, CacheMetrics, etc.
#[cfg(feature = "std")]
pub use runtime::tls::{
    TlsHandshakeMetricsCapsule, TlsHandshakeError, HandshakeMetrics, ComplianceReport,
};

// Re-export observability capsules for convenience (requires std)
#[cfg(feature = "std")]
// pub use observability // TODO: Merge from atomic-capsule-http-server branch::{WorkloadDetectorCapsule, WorkloadMode};

// Re-export composite capsules for convenience (requires portable_simd) - Phase 2.4.1
#[cfg(feature = "portable_simd")]
pub use composite::{
    // T1+T2 composites (Atomic + SIMD)
    AtomicSimdAccumulator,
    AtomicSimdCounter,
    AtomicSimdF32x8,
    // T1+T2+T3+T4 full compound capsules
    BatchAtomicSimdFixedQ16Capsule,
    FinancialBatchProcessor,
    MLBatchInference,
    // T2+T3 composites (SIMD + Fixed-Point)
    OverflowError,
    SimdDeterministicML,
    SimdFinancialCalc,
    SimdFixedQ16x8,
};

// Re-export Phase 11 composite capsules (tier-specific feature flags)
#[cfg(feature = "tier1-tier2")]
pub use composite::AtomicSimdCapsule;

#[cfg(feature = "tier2-tier3")]
pub use composite::SimdFixedPointCapsule;

#[cfg(feature = "tier1-tier2-tier3")]
pub use composite::FullCompositeCapsule;

// Re-export SIMD primitives when nightly feature is enabled

// Re-export atomic_from_mut for nightly-atomic feature (Phase 2.3 - T0 tier)
#[cfg(feature = "nightly-atomic")]
pub use primitives::atomic_from_mut::{
    from_mut_pair, AtomicFromMut, AtomicFromMutError, CACHE_LINE_SIZE,
};

/// Foundation crate version for compatibility tracking
pub const VERSION: &str = core::env!("CARGO_PKG_VERSION");

/// Maximum supported alignment (256 bytes for multi-cache-line structures)
///
/// # ASSUM Framework
/// - `#ASSUME_ALIGNMENT_MAX`: 256 bytes sufficient for all atomic capsule patterns
/// - `#VERIFY_ALIGNMENT_MAX`: Documented in The Atomic Capsule architecture
pub const MAX_ALIGNMENT: usize = 256;

/// Minimum supported alignment (64 bytes = typical cache line)
///
/// # ASSUM Framework
/// - `#ASSUME_ALIGNMENT_MIN`: 64 bytes is minimum for false sharing prevention
/// - `#VERIFY_ALIGNMENT_MIN`: x86/ARM/RISC-V all have 64-byte cache lines
pub const MIN_ALIGNMENT: usize = 64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_exists() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_alignment_constants() {
        assert_eq!(MIN_ALIGNMENT, 64);
        assert_eq!(MAX_ALIGNMENT, 256);
        assert!(MAX_ALIGNMENT >= MIN_ALIGNMENT);

        // Verify power of 2
        assert_eq!(MIN_ALIGNMENT.count_ones(), 1);
        assert_eq!(MAX_ALIGNMENT.count_ones(), 1);
    }
}
