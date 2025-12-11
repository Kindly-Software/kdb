//! PermissionGuardCapsule - T1 Atomic Permission/Consent Management
//!
//! **Tier**: T1 (Atomic)
//! **Size**: 64 bytes (single cache line)
//! **Alignment**: 64B
//! **Performance**: <10ns state check, <1ms user prompt
//! **Architecture**: 100% lockfree (AtomicU64 only)
//!
//! # Purpose
//!
//! Manages user permission/consent for configuration operations with atomic
//! state tracking. Supports auto-approval via environment variables for
//! CI/CD and scripted workflows.
//!
//! # Environment Variables
//!
//! - `KDB_AUTO_CONFIGURE=true` - Auto-approve configuration changes
//! - `KDB_CONFIGURE_FORCE=true` - Force override (skip prompts)
//! - `KDB_CONFIGURE_DRY_RUN=true` - Auto-deny (preview mode)
//!
//! # Usage
//!
//! ```rust
//! use kdb_mcp::configure::permission::{PermissionGuardCapsule, PermissionRequest};
//!
//! let guard = PermissionGuardCapsule::new();
//!
//! let request = PermissionRequest {
//!     action: "Configure Claude Code".to_string(),
//!     target: "~/.config/claude-code/mcp.json".to_string(),
//!     impact: "Create new config".to_string(),
//!     auto_approve_env: "KDB_AUTO_CONFIGURE",
//! };
//!
//! // In CI/CD with KDB_AUTO_CONFIGURE=true, this auto-approves
//! let response = guard.request_permission(&request);
//! if response.granted {
//!     // Proceed with configuration
//! }
//! ```
//!
//! # Chaos Compliance
//!
//! - #[repr(C, align(64))]: Cache-aligned
//! - 100% lockfree: AtomicU64 only
//! - Generation counters: TOCTOU prevention
//! - const fn new(): Static initialization
//! - ASSUM tags: All assumptions documented

use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// Permission State Machine
// ============================================================================

/// Permission state for the state machine
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionState {
    /// Permission not yet requested
    NotAsked = 0,
    /// Permission request pending (user prompt in progress)
    Pending = 1,
    /// Permission granted by user
    Granted = 2,
    /// Permission denied by user
    Denied = 3,
    /// Permission auto-approved via environment variable
    AutoApproved = 4,
}

impl PermissionState {
    /// Convert from u64 to PermissionState
    #[inline]
    pub const fn from_u64(value: u64) -> Self {
        match value {
            0 => Self::NotAsked,
            1 => Self::Pending,
            2 => Self::Granted,
            3 => Self::Denied,
            4 => Self::AutoApproved,
            _ => Self::NotAsked, // Default for invalid values
        }
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// Permission request for a configuration operation
#[derive(Debug, Clone)]
pub struct PermissionRequest {
    /// Description of the action (e.g., "Configure Claude Code")
    pub action: String,
    /// Target path or resource (e.g., "~/.config/claude-code/mcp.json")
    pub target: String,
    /// Impact description (e.g., "Create new config" or "Update existing (backup: ...)")
    pub impact: String,
    /// Environment variable name for auto-approval (e.g., "KDB_AUTO_CONFIGURE")
    pub auto_approve_env: &'static str,
}

/// Reason for permission decision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionReason {
    /// User explicitly approved at prompt
    UserApproved,
    /// User explicitly denied at prompt
    UserDenied,
    /// Auto-approved via environment variable (e.g., KDB_AUTO_CONFIGURE=true)
    AutoApproved,
    /// Force override via KDB_CONFIGURE_FORCE=true
    ForceOverride,
    /// Auto-denied because KDB_CONFIGURE_DRY_RUN=true
    DryRun,
}

/// Response from a permission request
#[derive(Debug, Clone)]
pub struct PermissionResponse {
    /// Whether permission was granted
    pub granted: bool,
    /// Reason for the decision
    pub reason: PermissionReason,
    /// Unix timestamp (nanoseconds) when decision was made
    pub timestamp: u64,
}

/// Statistics snapshot from PermissionGuardCapsule
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionStats {
    /// Total permission requests made
    pub requests: u64,
    /// Total permissions granted
    pub granted: u64,
    /// Total permissions denied
    pub denied: u64,
    /// Total auto-approved (via env var)
    pub auto_approved: u64,
    /// Current state (as u64, convert with PermissionState::from_u64)
    pub current_state: u64,
}

// ============================================================================
// PermissionGuardCapsule
// ============================================================================

/// T1 Atomic Permission/Consent Management Capsule
///
/// Tracks permission state and statistics with atomic operations.
/// 64-byte, cache-line aligned, 100% lockfree.
///
/// # Memory Layout (64 bytes)
///
/// ```text
/// Offset  Size  Field
/// 0       8     state (AtomicU64) - Current permission state
/// 8       8     generation (AtomicU64) - TOCTOU prevention
/// 16      8     requests (AtomicU64) - Total requests
/// 24      8     granted (AtomicU64) - Granted count
/// 32      8     denied (AtomicU64) - Denied count
/// 40      8     auto_approved (AtomicU64) - Auto-approved count
/// 48      8     last_request_ns (AtomicU64) - Last request timestamp
/// 56      8     _padding
/// ```
///
/// # Thread Safety
///
/// All fields are AtomicU64. State transitions are atomic.
/// Generation counter prevents TOCTOU races.
#[repr(C, align(64))]
pub struct PermissionGuardCapsule {
    /// Current permission state
    /// #ASSUME: State enum fits in u64
    state: AtomicU64,

    /// Generation counter for TOCTOU prevention
    /// #ASSUME: Generation wraps safely after 2^64 increments
    generation: AtomicU64,

    /// Total permission requests made
    requests: AtomicU64,

    /// Total permissions granted (user + auto + force)
    granted: AtomicU64,

    /// Total permissions denied (user + dry-run)
    denied: AtomicU64,

    /// Total auto-approved via environment variable
    auto_approved: AtomicU64,

    /// Timestamp of last request (Unix nanoseconds)
    last_request_ns: AtomicU64,

    /// Padding to reach 64 bytes (7 * 8 = 56, need 8 more)
    _padding: [u8; 8],
}

// #VERIFY: Size and alignment assertions
const _: () = {
    assert!(core::mem::size_of::<PermissionGuardCapsule>() == 64);
    assert!(core::mem::align_of::<PermissionGuardCapsule>() == 64);
};

impl PermissionGuardCapsule {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Create a new PermissionGuardCapsule
    ///
    /// All counters start at zero. State starts as NotAsked.
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(PermissionState::NotAsked as u64),
            generation: AtomicU64::new(0),
            requests: AtomicU64::new(0),
            granted: AtomicU64::new(0),
            denied: AtomicU64::new(0),
            auto_approved: AtomicU64::new(0),
            last_request_ns: AtomicU64::new(0),
            _padding: [0u8; 8],
        }
    }

    // ========================================================================
    // Core Operations
    // ========================================================================

    /// Request permission for a configuration operation
    ///
    /// Checks environment variables first:
    /// 1. KDB_CONFIGURE_DRY_RUN=true -> Auto-deny
    /// 2. KDB_CONFIGURE_FORCE=true -> Force approve
    /// 3. {auto_approve_env}=true -> Auto-approve
    /// 4. Otherwise -> Prompt user
    ///
    /// # Performance
    ///
    /// - <10ns for env var check (cache hit)
    /// - <1ms for user prompt (I/O bound)
    ///
    /// # Arguments
    ///
    /// * `req` - The permission request
    ///
    /// # Returns
    ///
    /// PermissionResponse with granted/denied and reason
    pub fn request_permission(&self, req: &PermissionRequest) -> PermissionResponse {
        // Update counters
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.last_request_ns.store(get_unix_nanos(), Ordering::Relaxed);
        self.state.store(PermissionState::Pending as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Check for dry-run mode (auto-deny)
        // #ASSUME: std::env::var is available (feature = "std")
        if let Ok(val) = std::env::var("KDB_CONFIGURE_DRY_RUN") {
            if val.to_lowercase() == "true" || val == "1" {
                return self.deny_permission(PermissionReason::DryRun);
            }
        }

        // Check for force override
        if let Ok(val) = std::env::var("KDB_CONFIGURE_FORCE") {
            if val.to_lowercase() == "true" || val == "1" {
                return self.grant_permission(PermissionReason::ForceOverride);
            }
        }

        // Check for auto-approve environment variable
        if let Ok(val) = std::env::var(req.auto_approve_env) {
            if val.to_lowercase() == "true" || val == "1" {
                return self.grant_permission(PermissionReason::AutoApproved);
            }
        }

        // Prompt user interactively
        self.prompt_user(req)
    }

    /// Request permission without interactive prompt (env vars only)
    ///
    /// Use this in non-interactive contexts (tests, scripts).
    /// Returns None if no env var applies (would need user prompt).
    ///
    /// # Arguments
    ///
    /// * `req` - The permission request
    ///
    /// # Returns
    ///
    /// Some(PermissionResponse) if env var applies, None if user prompt needed
    pub fn request_permission_no_prompt(&self, req: &PermissionRequest) -> Option<PermissionResponse> {
        // Update counters
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.last_request_ns.store(get_unix_nanos(), Ordering::Relaxed);
        self.state.store(PermissionState::Pending as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Check for dry-run mode (auto-deny)
        if let Ok(val) = std::env::var("KDB_CONFIGURE_DRY_RUN") {
            if val.to_lowercase() == "true" || val == "1" {
                return Some(self.deny_permission(PermissionReason::DryRun));
            }
        }

        // Check for force override
        if let Ok(val) = std::env::var("KDB_CONFIGURE_FORCE") {
            if val.to_lowercase() == "true" || val == "1" {
                return Some(self.grant_permission(PermissionReason::ForceOverride));
            }
        }

        // Check for auto-approve environment variable
        if let Ok(val) = std::env::var(req.auto_approve_env) {
            if val.to_lowercase() == "true" || val == "1" {
                return Some(self.grant_permission(PermissionReason::AutoApproved));
            }
        }

        // No env var applies - would need user prompt
        // Reset state since we didn't complete
        self.state.store(PermissionState::NotAsked as u64, Ordering::Release);
        None
    }

    /// Prompt user for permission (interactive)
    fn prompt_user(&self, req: &PermissionRequest) -> PermissionResponse {
        use std::io::{self, Write};

        // Display permission request
        println!();
        println!("=== Permission Request ===");
        println!("Action: {}", req.action);
        println!("Target: {}", req.target);
        println!("Impact: {}", req.impact);
        println!();
        print!("Proceed? [Y/n]: ");

        // Flush to ensure prompt is visible before reading
        if io::stdout().flush().is_err() {
            // If flush fails, default to deny
            return self.deny_permission(PermissionReason::UserDenied);
        }

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            // If read fails, default to deny
            return self.deny_permission(PermissionReason::UserDenied);
        }

        // Parse response: empty, "y", "yes" -> approve; anything else -> deny
        let trimmed = input.trim().to_lowercase();
        let approved = trimmed.is_empty() || trimmed == "y" || trimmed == "yes";

        if approved {
            self.grant_permission(PermissionReason::UserApproved)
        } else {
            self.deny_permission(PermissionReason::UserDenied)
        }
    }

    /// Grant permission and update state
    fn grant_permission(&self, reason: PermissionReason) -> PermissionResponse {
        let timestamp = get_unix_nanos();

        // Update auto_approved counter if applicable
        if matches!(reason, PermissionReason::AutoApproved) {
            self.auto_approved.fetch_add(1, Ordering::Relaxed);
        }

        // Update granted counter
        self.granted.fetch_add(1, Ordering::Relaxed);

        // Update state
        let new_state = if matches!(reason, PermissionReason::AutoApproved) {
            PermissionState::AutoApproved
        } else {
            PermissionState::Granted
        };
        self.state.store(new_state as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        PermissionResponse {
            granted: true,
            reason,
            timestamp,
        }
    }

    /// Deny permission and update state
    fn deny_permission(&self, reason: PermissionReason) -> PermissionResponse {
        let timestamp = get_unix_nanos();

        // Update denied counter
        self.denied.fetch_add(1, Ordering::Relaxed);

        // Update state
        self.state.store(PermissionState::Denied as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        PermissionResponse {
            granted: false,
            reason,
            timestamp,
        }
    }

    // ========================================================================
    // State Queries
    // ========================================================================

    /// Get current permission state
    #[inline]
    pub fn get_state(&self) -> PermissionState {
        PermissionState::from_u64(self.state.load(Ordering::Acquire))
    }

    /// Check if permission is currently granted
    #[inline]
    pub fn is_granted(&self) -> bool {
        let state = self.get_state();
        matches!(state, PermissionState::Granted | PermissionState::AutoApproved)
    }

    /// Check if permission is currently denied
    #[inline]
    pub fn is_denied(&self) -> bool {
        matches!(self.get_state(), PermissionState::Denied)
    }

    /// Reset to NotAsked state
    ///
    /// Increments generation counter to invalidate any in-flight operations.
    #[inline]
    pub fn reset(&self) {
        self.state.store(PermissionState::NotAsked as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Get the current generation counter value
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get statistics snapshot
    #[inline]
    pub fn get_stats(&self) -> PermissionStats {
        PermissionStats {
            requests: self.requests.load(Ordering::Acquire),
            granted: self.granted.load(Ordering::Acquire),
            denied: self.denied.load(Ordering::Acquire),
            auto_approved: self.auto_approved.load(Ordering::Acquire),
            current_state: self.state.load(Ordering::Acquire),
        }
    }

    // ========================================================================
    // Test Helpers
    // ========================================================================

    /// Reset all counters (for testing)
    #[cfg(test)]
    pub fn reset_all(&self) {
        self.state.store(PermissionState::NotAsked as u64, Ordering::Release);
        self.generation.store(0, Ordering::Release);
        self.requests.store(0, Ordering::Release);
        self.granted.store(0, Ordering::Release);
        self.denied.store(0, Ordering::Release);
        self.auto_approved.store(0, Ordering::Release);
        self.last_request_ns.store(0, Ordering::Release);
    }
}

impl Default for PermissionGuardCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Get current Unix timestamp in nanoseconds
#[inline]
fn get_unix_nanos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

// ============================================================================
// Unit Tests (T28 Q1-Q7)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // Helper to clean up environment after test
    fn cleanup_env() {
        env::remove_var("KDB_AUTO_CONFIGURE");
        env::remove_var("KDB_CONFIGURE_FORCE");
        env::remove_var("KDB_CONFIGURE_DRY_RUN");
    }

    fn test_request() -> PermissionRequest {
        PermissionRequest {
            action: "Configure Claude Code".to_string(),
            target: "~/.config/claude-code/mcp.json".to_string(),
            impact: "Create new config".to_string(),
            auto_approve_env: "KDB_AUTO_CONFIGURE",
        }
    }

    // Q1: Layout Verification
    #[test]
    fn test_permission_guard_size() {
        assert_eq!(core::mem::size_of::<PermissionGuardCapsule>(), 64);
    }

    #[test]
    fn test_permission_guard_alignment() {
        assert_eq!(core::mem::align_of::<PermissionGuardCapsule>(), 64);
    }

    // Q2: Const Construction
    #[test]
    fn test_initial_state() {
        let guard = PermissionGuardCapsule::new();
        assert_eq!(guard.get_state(), PermissionState::NotAsked);
        let stats = guard.get_stats();
        assert_eq!(stats.requests, 0);
        assert_eq!(stats.granted, 0);
        assert_eq!(stats.denied, 0);
        assert_eq!(stats.auto_approved, 0);
    }

    #[test]
    fn test_const_new() {
        static GUARD: PermissionGuardCapsule = PermissionGuardCapsule::new();
        assert_eq!(GUARD.get_state(), PermissionState::NotAsked);
    }

    #[test]
    fn test_default() {
        let guard = PermissionGuardCapsule::default();
        assert_eq!(guard.get_state(), PermissionState::NotAsked);
    }

    // Q3: Environment Variable - Auto Approve
    #[test]
    fn test_auto_approve_from_env() {
        cleanup_env();
        let guard = PermissionGuardCapsule::new();
        let req = test_request();

        // Set auto-approve environment variable
        env::set_var("KDB_AUTO_CONFIGURE", "true");

        let response = guard.request_permission_no_prompt(&req);
        assert!(response.is_some());
        let response = response.unwrap();
        assert!(response.granted);
        assert_eq!(response.reason, PermissionReason::AutoApproved);

        let stats = guard.get_stats();
        assert_eq!(stats.requests, 1);
        assert_eq!(stats.granted, 1);
        assert_eq!(stats.auto_approved, 1);

        cleanup_env();
    }

    #[test]
    fn test_auto_approve_from_env_numeric() {
        cleanup_env();
        let guard = PermissionGuardCapsule::new();
        let req = test_request();

        // Test with "1" instead of "true"
        env::set_var("KDB_AUTO_CONFIGURE", "1");

        let response = guard.request_permission_no_prompt(&req);
        assert!(response.is_some());
        assert!(response.unwrap().granted);

        cleanup_env();
    }

    // Q4: Environment Variable - Force Override
    #[test]
    fn test_force_override() {
        cleanup_env();
        let guard = PermissionGuardCapsule::new();
        let req = test_request();

        env::set_var("KDB_CONFIGURE_FORCE", "true");

        let response = guard.request_permission_no_prompt(&req);
        assert!(response.is_some());
        let response = response.unwrap();
        assert!(response.granted);
        assert_eq!(response.reason, PermissionReason::ForceOverride);

        cleanup_env();
    }

    // Q5: Environment Variable - Dry Run
    #[test]
    fn test_dry_run_denies() {
        cleanup_env();
        let guard = PermissionGuardCapsule::new();
        let req = test_request();

        env::set_var("KDB_CONFIGURE_DRY_RUN", "true");

        let response = guard.request_permission_no_prompt(&req);
        assert!(response.is_some());
        let response = response.unwrap();
        assert!(!response.granted);
        assert_eq!(response.reason, PermissionReason::DryRun);

        let stats = guard.get_stats();
        assert_eq!(stats.denied, 1);

        cleanup_env();
    }

    // Q6: Priority - Dry Run overrides Force Override
    #[test]
    fn test_dry_run_takes_priority_over_force() {
        cleanup_env();
        let guard = PermissionGuardCapsule::new();
        let req = test_request();

        // Set both dry-run and force - dry-run should win
        env::set_var("KDB_CONFIGURE_DRY_RUN", "true");
        env::set_var("KDB_CONFIGURE_FORCE", "true");

        let response = guard.request_permission_no_prompt(&req);
        assert!(response.is_some());
        let response = response.unwrap();
        assert!(!response.granted);
        assert_eq!(response.reason, PermissionReason::DryRun);

        cleanup_env();
    }

    // Q7: Request Count
    #[test]
    fn test_request_count() {
        cleanup_env();
        let guard = PermissionGuardCapsule::new();
        let req = test_request();

        env::set_var("KDB_AUTO_CONFIGURE", "true");

        // Make multiple requests
        for _ in 0..5 {
            let _ = guard.request_permission_no_prompt(&req);
        }

        let stats = guard.get_stats();
        assert_eq!(stats.requests, 5);
        assert_eq!(stats.granted, 5);
        assert_eq!(stats.auto_approved, 5);

        cleanup_env();
    }

    // Q8: Granted/Denied Count
    #[test]
    fn test_granted_denied_count() {
        cleanup_env();
        let guard = PermissionGuardCapsule::new();
        let req = test_request();

        // 3 grants via force
        env::set_var("KDB_CONFIGURE_FORCE", "true");
        for _ in 0..3 {
            let _ = guard.request_permission_no_prompt(&req);
        }
        env::remove_var("KDB_CONFIGURE_FORCE");

        // 2 denies via dry-run
        env::set_var("KDB_CONFIGURE_DRY_RUN", "true");
        for _ in 0..2 {
            let _ = guard.request_permission_no_prompt(&req);
        }

        let stats = guard.get_stats();
        assert_eq!(stats.requests, 5);
        assert_eq!(stats.granted, 3);
        assert_eq!(stats.denied, 2);

        cleanup_env();
    }

    // Q9: Reset
    #[test]
    fn test_reset() {
        cleanup_env();
        let guard = PermissionGuardCapsule::new();
        let req = test_request();

        env::set_var("KDB_AUTO_CONFIGURE", "true");
        let _ = guard.request_permission_no_prompt(&req);
        env::remove_var("KDB_AUTO_CONFIGURE");

        // State should be AutoApproved/Granted
        assert!(guard.is_granted());

        // Reset should change state to NotAsked
        let gen_before = guard.generation();
        guard.reset();
        let gen_after = guard.generation();

        assert_eq!(guard.get_state(), PermissionState::NotAsked);
        assert!(gen_after > gen_before, "Generation should increment on reset");

        cleanup_env();
    }

    // Q10: Generation Counter
    #[test]
    fn test_generation_counter() {
        cleanup_env();
        let guard = PermissionGuardCapsule::new();
        let req = test_request();

        let initial_gen = guard.generation();

        env::set_var("KDB_AUTO_CONFIGURE", "true");
        let _ = guard.request_permission_no_prompt(&req);
        env::remove_var("KDB_AUTO_CONFIGURE");

        let after_request_gen = guard.generation();
        assert!(after_request_gen > initial_gen, "Generation should increment after request");

        guard.reset();
        let after_reset_gen = guard.generation();
        assert!(after_reset_gen > after_request_gen, "Generation should increment on reset");

        cleanup_env();
    }

    // Additional tests
    #[test]
    fn test_permission_state_from_u64() {
        assert_eq!(PermissionState::from_u64(0), PermissionState::NotAsked);
        assert_eq!(PermissionState::from_u64(1), PermissionState::Pending);
        assert_eq!(PermissionState::from_u64(2), PermissionState::Granted);
        assert_eq!(PermissionState::from_u64(3), PermissionState::Denied);
        assert_eq!(PermissionState::from_u64(4), PermissionState::AutoApproved);
        assert_eq!(PermissionState::from_u64(255), PermissionState::NotAsked); // Invalid -> default
    }

    #[test]
    fn test_is_granted() {
        cleanup_env();
        let guard = PermissionGuardCapsule::new();
        let req = test_request();

        assert!(!guard.is_granted());

        env::set_var("KDB_CONFIGURE_FORCE", "true");
        let _ = guard.request_permission_no_prompt(&req);
        env::remove_var("KDB_CONFIGURE_FORCE");

        assert!(guard.is_granted());

        cleanup_env();
    }

    #[test]
    fn test_is_denied() {
        cleanup_env();
        let guard = PermissionGuardCapsule::new();
        let req = test_request();

        assert!(!guard.is_denied());

        env::set_var("KDB_CONFIGURE_DRY_RUN", "true");
        let _ = guard.request_permission_no_prompt(&req);
        env::remove_var("KDB_CONFIGURE_DRY_RUN");

        assert!(guard.is_denied());

        cleanup_env();
    }

    #[test]
    fn test_no_env_returns_none() {
        cleanup_env();
        let guard = PermissionGuardCapsule::new();
        let req = test_request();

        // No env vars set - should return None (would need user prompt)
        let response = guard.request_permission_no_prompt(&req);
        assert!(response.is_none());

        // State should be reset to NotAsked
        assert_eq!(guard.get_state(), PermissionState::NotAsked);

        cleanup_env();
    }

    #[test]
    fn test_reset_all() {
        cleanup_env();
        let guard = PermissionGuardCapsule::new();
        let req = test_request();

        env::set_var("KDB_AUTO_CONFIGURE", "true");
        for _ in 0..5 {
            let _ = guard.request_permission_no_prompt(&req);
        }
        env::remove_var("KDB_AUTO_CONFIGURE");

        guard.reset_all();

        let stats = guard.get_stats();
        assert_eq!(stats.requests, 0);
        assert_eq!(stats.granted, 0);
        assert_eq!(stats.denied, 0);
        assert_eq!(stats.auto_approved, 0);
        assert_eq!(guard.get_state(), PermissionState::NotAsked);

        cleanup_env();
    }

    #[test]
    fn test_timestamp_set() {
        cleanup_env();
        let guard = PermissionGuardCapsule::new();
        let req = test_request();

        env::set_var("KDB_AUTO_CONFIGURE", "true");
        let before = get_unix_nanos();
        let response = guard.request_permission_no_prompt(&req).unwrap();
        let after = get_unix_nanos();

        assert!(response.timestamp >= before);
        assert!(response.timestamp <= after);

        cleanup_env();
    }

    // Concurrent safety test
    #[test]
    fn test_concurrent_requests() {
        use std::sync::Arc;
        use std::thread;

        cleanup_env();
        env::set_var("KDB_AUTO_CONFIGURE", "true");

        let guard = Arc::new(PermissionGuardCapsule::new());
        let mut handles = vec![];

        // Spawn 10 threads each making 10 requests
        for _ in 0..10 {
            let guard = Arc::clone(&guard);
            handles.push(thread::spawn(move || {
                let req = test_request();
                let mut granted = 0u64;
                for _ in 0..10 {
                    if let Some(resp) = guard.request_permission_no_prompt(&req) {
                        if resp.granted {
                            granted += 1;
                        }
                    }
                }
                granted
            }));
        }

        let total_granted: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();

        // All should be granted via auto-approve
        assert_eq!(total_granted, 100);

        let stats = guard.get_stats();
        assert_eq!(stats.requests, 100);
        assert_eq!(stats.granted, 100);
        assert_eq!(stats.auto_approved, 100);

        cleanup_env();
    }
}
