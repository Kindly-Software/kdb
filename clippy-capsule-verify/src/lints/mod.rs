//! # Clippy Capsule Verification Lints
//!
//! Collection of custom clippy lints for enforcing Computational Capsule Architecture (Chaos)
//! compliance at compile-time.
//!
//! ## Lint Priority Levels
//!
//! ### P0 Critical (DENY - Compile Error)
//!
//! These lints catch violations that cause data races, undefined behavior, or breaking
//! the core lockfree mandate:
//!
//! - **CAPSULE_MUTEX_VIOLATION** - Mutex/RwLock in capsules (violates 100% lockfree mandate)
//! - **CAPSULE_UNALIGNED_VIOLATION** - Size not multiple of alignment (false sharing, SIMD crashes)
//! - **CAPSULE_NON_ATOMIC_FIELD** - Non-atomic fields in T1 (Atomic) tier (data races)
//!
//! ### P1 High (WARN - Warnings)
//!
//! These lints enforce best practices that improve performance and prevent subtle bugs:
//!
//! - **CAPSULE_MISSING_GENERATION** - T1 (Atomic) capsule without generation counter (TOCTOU races)
//! - **CAPSULE_MISSING_REPR_ALIGN** - Capsule without #[repr(align)] (alignment unreliable)
//! - **CAPSULE_SCATTERED_ATOMICS** - Multiple atomic fields instead of DualAtomicU64 (complexity)
//! - **CAPSULE_MISSING_PADDING** - Size not aligned to declared alignment (false sharing risk)
//!
//! ### P2 Medium (ALLOW by default - Opt-in)
//!
//! These lints provide advanced diagnostics for safety analysis:
//!
//! - **CAPSULE_MEMORY_ORDERING** - Memory ordering violations (Relaxed where Acquire/Release needed)
//! - **CAPSULE_MISSING_ASSUM** - Unsafe blocks without #ASSUME tags (documentation)
//! - **CAPSULE_TOCTOU_PATTERN** - Load-check-load race patterns (generation counter prevention)
//!
//! ## Usage
//!
//! ```bash
//! # Enable all P0 critical lints (deny violations)
//! cargo clippy --all-features -- \
//!   -D clippy::capsule_mutex_violation \
//!   -D clippy::capsule_unaligned_violation \
//!   -D clippy::capsule_non_atomic_field
//!
//! # Enable P1 high-priority lints (warn)
//! cargo clippy --all-features -- \
//!   -W clippy::capsule_missing_generation \
//!   -W clippy::capsule_missing_repr_align
//!
//! # Enable P2 medium-priority lints (opt-in)
//! cargo clippy --all-features -- \
//!   -W clippy::capsule_memory_ordering
//! ```
//!
//! ## UCE34 Framework Alignment
//!
//! - **Q10 (Tier Selection)**: Lints validate tier constraints (size limits, alignment requirements)
//! - **Q33 (Atomic Capsule)**: Lints enforce verification for 100% lockfree compliance
//! - **Q34 (Auditability)**: Lints ensure capsules have audit trail support (generation counters)

pub mod atomic_field_violation;
pub mod generation_violation;
pub mod mutex_violation;

pub use atomic_field_violation::CAPSULE_NON_ATOMIC_FIELD;
pub use generation_violation::CAPSULE_MISSING_GENERATION;
pub use mutex_violation::CAPSULE_MUTEX_VIOLATION;
