//! MCP Tools Modules
//!
//! All 27 tools are implemented across server.rs and this module:
//!
//! ## Debugging Tools (1-9)
//! 1. `debugger/attach` - Attach to process via ptrace
//! 2. `debugger/set_breakpoint` - Add breakpoint at address
//! 3. `debugger/continue` - Resume execution
//! 4. `debugger/step_forward` - Single step forward
//! 5. `debugger/step_backward` - Time-travel step backward!
//! 6. `debugger/get_stack_trace` - SIMD-accelerated stack unwind (<20μs)
//! 7. `debugger/get_variables` - Read memory at address
//! 8. `debugger/find_similar_bugs` - T10 probabilistic LSH similarity
//! 9. `debugger/export_trace` - T5 streaming trace export
//!
//! ## Admin Tools (10-12)
//! 10. `debugger/quota_status` - Quota tier/limits/usage (T1 Atomic, <70ns)
//! 11. `debugger/license_info` - License tier/validation/expiry (T1 Atomic, <10ns)
//! 12. `debugger/get_comprehensive_audit` - Q34 compliance audit (<10μs)
//!
//! ## Session Pool Tools (13-17) - T6 Mixed, <100ns lockfree
//! 13. `debugger/allocate_session` - Allocate tiered session (Light/Medium/Heavy)
//! 14. `debugger/release_session` - Release session back to pool
//! 15. `debugger/get_session_tier` - Query session tier (<10ns)
//! 16. `debugger/upgrade_session` - Upgrade to higher tier (<1μs)
//! 17. `debugger/get_pool_stats` - Pool statistics snapshot (<50ns)
//!
//! ## Memory Replay Tools (18-23) - T6 Mixed, COW tracking
//! 18. `debugger/enable_memory_replay` - Enable COW memory tracking (<10ms)
//! 19. `debugger/capture_memory_snapshot` - Capture snapshot (<50ms)
//! 20. `debugger/read_memory_at_snapshot` - Read at historical snapshot (<2ms)
//! 21. `debugger/navigate_to_snapshot` - Navigate snapshots (<100ns)
//! 22. `debugger/get_memory_replay_stats` - Replay statistics (<50ns)
//! 23. `debugger/verify_memory_integrity` - Q34 hash-chain integrity (O(n))
//!
//! ## Access Control Tools (24-27) - T1 Atomic, Ed25519 authentication
//! 24. `debugger/get_access_mode` - Query current access mode (Observer/Operator)
//! 25. `debugger/request_operator_challenge` - Request Ed25519 challenge nonce
//! 26. `debugger/elevate_to_operator` - Submit signature to elevate to Operator
//! 27. `debugger/revoke_operator` - Voluntarily drop to Observer mode
//!
//! ## Session Tiers
//!
//! | Tier   | Size    | Description                |
//! |--------|---------|----------------------------|
//! | Light  | 64KB    | Basic debugging            |
//! | Medium | 256KB   | Extended state tracking    |
//! | Heavy  | 1.09MB  | Full memory replay support |
//!
//! ## Access Control Model
//!
//! | Mode      | Permission | Description                        |
//! |-----------|------------|-------------------------------------|
//! | Observer  | Read-only  | Can view state, cannot modify      |
//! | Operator  | Full       | Can modify breakpoints, step, etc. |
//!
//! ## Performance Targets
//!
//! - Session allocation/release: <100ns (lockfree pool)
//! - Session tier query: <10ns (atomic load)
//! - Session upgrade: <1μs (includes data migration)
//! - Memory replay enable: <10ms (page table setup)
//! - Snapshot capture: <50ms (depends on dirty pages)
//! - Memory read at snapshot: <2ms (delta reconstruction)
//! - Snapshot navigation: <100ns (atomic state update)
//! - Integrity verification: O(n) (hash-chain traversal)
//! - Access mode query: <10ns (atomic load)
//! - Challenge generation: <1μs (OsRng + timestamp)
//! - Signature verification: <100μs (Ed25519 verify_strict)
//! - Mode transition: <100ns (atomic CAS)
//!
//! ## Q34 Compliance
//!
//! Memory replay with `enable_merkle_verification: true` provides:
//! - Tamper-evident snapshots via CRC64 hash-chain
//! - Merkle tree integrity for audit compliance
//! - SOX/SOC2/GDPR/HIPAA framework support
//!
//! ## Security (Access Control)
//!
//! - Ed25519 signatures with `verify_strict()` (rejects weak keys)
//! - Single-use challenges (generation counter + atomic CAS)
//! - Configurable session timeouts (5min/30min/1hr/never)
//! - Q34 audit trail via rolling hash-chain

// Note: Document tools (xpath_query, validate_schema, cache_stats, preload_documents)
// have been removed to reduce bloat and focus on core debugging functionality.

pub mod access_control;

pub use access_control::{
    // Tool IDs
    TOOL_ID_GET_ACCESS_MODE,
    TOOL_ID_REQUEST_OPERATOR_CHALLENGE,
    TOOL_ID_ELEVATE_TO_OPERATOR,
    TOOL_ID_REVOKE_OPERATOR,
    // Error type
    AccessControlError,
    // Tool handlers
    handle_get_access_mode,
    handle_revoke_operator,
};

#[cfg(feature = "operator-challenge")]
pub use access_control::handle_request_operator_challenge;

pub use access_control::handle_elevate_to_operator;
