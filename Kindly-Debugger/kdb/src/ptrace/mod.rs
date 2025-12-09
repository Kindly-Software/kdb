//! Ptrace Integration Module
//!
//! Real debugging via ptrace syscalls (Linux x86-64/aarch64).
//! Per MCP_PTRACE_CAPSULE_ARCHITECTURE.md: 10 core + 3 utility capsules.
//!
//! # Implemented Modules
//! - ✅ `process_state`: ProcessStateCapsule - T1 Atomic state tracking (128B, <10ns)
//! - ✅ `registers`: RegisterReaderCapsule - T2 SIMD register copy
//! - ✅ `stack`: StackUnwinderCapsule - T5 Streaming stack frame traversal (6.9KB, <2μs/frame)
//! - ✅ `symbols`: SymbolResolverCapsule - T5+T9 DWARF symbol resolution (744KB, <50μs)
//! - ✅ `quota`: QuotaTrackerCapsule - T1 Atomic free tier quota management (128B, <50ns)
//! - ✅ `license`: LicenseValidatorCapsule - T0+T1 Ed25519 license validation (256B, <100μs)
//! - ✅ `memory`: MemoryReaderCapsule + BatchMemoryReader - T4 Batch memory reads (4KB + 1MB)
//!
//! # Future Modules (Phases 2-3)
//! - `wrapper`: PtraceWrapperCapsule - T1 Atomic syscall wrapper
//! - `breakpoint`: BreakpointManagerCapsule - T1+T5 breakpoint CRUD
//! - `variable`: VariableInspectorCapsule - T4 Batch local inspection
//! - `signal`: SignalHandlerCapsule - T1 Atomic SIGTRAP routing
//! - `process_map`: ProcessMapCapsule - T5 Streaming /proc/pid/maps
//!
//! See: /home/samuel/Primitives/kdb/MCP_PTRACE_CAPSULE_ARCHITECTURE.md

// ✅ PHASE 1 IMPLEMENTED: ProcessStateCapsule
pub mod process_state;
pub use process_state::{ProcessState, ProcessStateCapsule, ProcessStateError};

pub mod registers;
pub use registers::{RegisterError, RegisterReaderCapsule};

// ✅ PHASE 1 IMPLEMENTED: StackUnwinderCapsule
pub mod stack;
pub use stack::{MemoryReader, StackFrame, StackUnwindError, StackUnwinderCapsule, UserRegs};

// ✅ PHASE 3 IMPLEMENTED: SymbolResolverCapsule
pub mod symbols;
pub use symbols::{SymbolError, SymbolInfo, SymbolResolverCapsule};

// ✅ PHASE 4 IMPLEMENTED: VariableInspectorCapsule
pub mod variables;
pub use variables::{InspectError, Value, Variable, VariableInspectorCapsule};

// ✅ PHASE 5 IMPLEMENTED: QuotaTrackerCapsule
pub mod quota;
pub use quota::{QuotaComplianceInfo, QuotaError, QuotaStatus, QuotaTrackerCapsule, UserTier};

// ✅ PHASE 6 IMPLEMENTED: LicenseValidatorCapsule
pub mod license;
pub use license::{LicenseError, LicenseStatus, LicenseTier, LicenseValidatorCapsule, VerificationState};

// ✅ PHASE 7 IMPLEMENTED: SessionTrackerCapsule
pub mod session_tracker;
pub use session_tracker::{CurrentSessionAuditInfo, SessionError, SessionStatus, SessionTier, SessionTrackerCapsule};

// ✅ PHASE 8 IMPLEMENTED: DeletionProofCapsule (GDPR Article 17 compliance)
pub mod deletion_proof;
pub use deletion_proof::{
    AuditEventCompact, DeletionCertificate, DeletionError, DeletionProofCapsule,
    LifecycleState, RetentionPolicy, RetentionStatus, SubscriptionTier,
    TierRetentionConfig, TierRetentionManager, VerificationError,
    retention_durations,
};

// ✅ PHASE 3.3 IMPLEMENTED: MemoryReaderCapsule + BatchMemoryReader
pub mod memory;
pub use memory::{
    // Constants
    PAGE_SIZE, MAX_BATCH_PAGES,
    // Page alignment helpers
    is_page_aligned, page_floor, page_ceil, pages_in_range,
    // BatchMemoryReader (optimized for COW snapshot capture)
    BatchMemoryReader, BatchReadStats, DirtyPageIterator, MemoryError,
    // MemoryReaderCapsule (existing T4 Batch capsule)
    MemoryReaderCapsule, MemoryReaderStats, MemoryReadError,
};

// ✅ PHASE 9 IMPLEMENTED: IsolationCapsule (Multi-tenant security)
pub mod isolation;
pub use isolation::{validate_attach_permission, IsolationError};

// ✅ PHASE 10 IMPLEMENTED: HeapSnapshotCapsule (T9 Persistent mmap-backed)
pub mod heap_snapshot;
pub use heap_snapshot::{HeapSnapshotCapsule, HeapMetadata, SnapshotError};

// Future capsules (Phases 2-3)
// pub mod wrapper;
// pub mod breakpoint;
// pub mod signal;
// pub mod process_map;
