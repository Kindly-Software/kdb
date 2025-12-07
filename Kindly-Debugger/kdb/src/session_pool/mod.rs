//! Session Pool Module - Tiered Session Management for MCP Server
//!
//! Provides three-tier session architecture for scalable debugging:
//! - **LIGHT (64KB)**: Quick attach, inspect (1,500 sessions capacity)
//! - **MEDIUM (256KB)**: Step debugging (600 sessions capacity)
//! - **HEAVY (1.09MB)**: Full replay with COW memory (400 sessions capacity)
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                   SessionPoolCapsule (T6 Orchestrator)          │
//! │                        512 bytes orchestrator                    │
//! ├─────────────────────┬─────────────────────┬─────────────────────┤
//! │   LIGHT Pool        │   MEDIUM Pool       │   HEAVY Pool        │
//! │   1,500 × 64KB      │   600 × 256KB       │   400 × 1.09MB      │
//! │   = 96MB            │   = 150MB           │   = 436MB           │
//! └─────────────────────┴─────────────────────┴─────────────────────┘
//!                       Total: ~682MB session pools
//! ```
//!
//! # Session Lifecycle
//!
//! ```text
//! LIGHT ──(48+ snapshots)──► MEDIUM ──(384+ snapshots)──► HEAVY
//!       ◄──(idle >30min)───        ◄──(idle >30min)────
//! ```
//!
//! # Foundation Capsules
//! - SlotMetadata: 64-byte cache-line aligned slot state tracking
//! - SessionLookup: 32KB lockfree session ID to slot index hash table
//! - SessionPoolCapsule: T6 orchestrator for tiered session management
//!
//! # Performance Targets (B32 Validated)
//! - `allocate_session()`: <100ns lockfree
//! - `release_session()`: <100ns lockfree
//! - `upgrade_session()`: <1μs (data migration)
//! - `get_pool_stats()`: <50ns (atomic snapshot)
//!
//! # Framework Compliance
//! - **UCE34**: T6 Mixed tier (orchestrating T1 pools)
//! - **COCA**: 100% lockfree, zero mutex/RwLock
//! - **ASSUM**: All unsafe blocks documented
//! - **T28**: 12+ unit tests per capsule

// Foundation capsules (Phase 1 - from other agents)
pub mod session_lookup;
pub mod slot_metadata;

// T6 Orchestrator (Phase 1 Agent 4)
pub mod session_pool_capsule;

// Tiered debugger capsules
pub mod light_debugger_capsule;
pub mod medium_debugger_capsule;

// Q34 Audit Trail Integration (Phase 4)
pub mod audit_integration;

// Re-export foundation capsules
pub use session_lookup::{
    LookupEntry, SessionLookup, SessionLookupError, LOOKUP_CAPACITY, MAX_ENTRIES,
};
pub use slot_metadata::{PackedMetadata, SessionTier, SlotMetadata, SlotState};

// Re-export light debugger capsule types
pub use light_debugger_capsule::{
    LightDebugError, LightDebuggerCapsule, LightDebuggerStats, LightExecutionStateSnapshot,
    UpgradeReason, UPGRADE_BREAKPOINT_THRESHOLD as LIGHT_UPGRADE_BREAKPOINT_THRESHOLD,
    UPGRADE_SNAPSHOT_THRESHOLD as LIGHT_UPGRADE_SNAPSHOT_THRESHOLD,
    UPGRADE_TRACE_THRESHOLD as LIGHT_UPGRADE_TRACE_THRESHOLD,
};

// Re-export medium debugger capsule types
pub use medium_debugger_capsule::{
    MediumBreakpoint, MediumBreakpointTable, MediumDebuggerCapsule, MediumExecutionState,
    MediumMetadata, MediumReplayEngine, MediumSnapshot, MediumThreadState, MediumThreadTable,
    MediumTraceBuffer, MediumWatchpoint, MediumWatchpointTable, StackCapture, StackWindow,
    TraceEntry, WatchKind, DOWNGRADE_SNAPSHOT_THRESHOLD, IDLE_DOWNGRADE_THRESHOLD_NS,
    MAX_STACK_WINDOWS, MEDIUM_CAPSULE_SIZE, MEDIUM_MAX_BREAKPOINTS, MEDIUM_MAX_SNAPSHOTS,
    MEDIUM_MAX_THREADS, MEDIUM_MAX_WATCHPOINTS, MEDIUM_TRACE_ENTRIES, STACK_WINDOW_SIZE,
    UPGRADE_BREAKPOINT_THRESHOLD, UPGRADE_SNAPSHOT_THRESHOLD,
};

// Re-export T6 orchestrator types
pub use session_pool_capsule::{
    PoolConfig, PoolError, PoolStats, SessionId, SessionPoolCapsule, SessionTierType,
    SlotState as PoolSlotState, // Alias to avoid conflict with slot_metadata::SlotState
    DEFAULT_DOWNGRADE_IDLE_SECONDS, DEFAULT_HEAVY_CAPACITY, DEFAULT_LIGHT_CAPACITY,
    DEFAULT_MEDIUM_CAPACITY, DEFAULT_UPGRADE_LIGHT_TO_MEDIUM, DEFAULT_UPGRADE_MEDIUM_TO_HEAVY,
};

// Re-export Q34 audit trail types (Phase 4)
pub use audit_integration::{
    SessionAuditEvent, SessionAuditEntry, SessionAuditTrailCapsule, SessionAuditStats,
    record_allocate, record_release, record_upgrade, record_downgrade, record_snapshot,
    AUDIT_ENTRY_COUNT, AUDIT_ENTRY_SIZE, AUDIT_TRAIL_SIZE,
};
