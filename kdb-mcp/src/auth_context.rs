//! AuthContext - Authenticated Request Context
//!
//! Carries authentication and authorization state through the MCP request pipeline.
//! Created by AuthGuard.authenticate() and consumed by tool routing/execution.
//!
//! **Purpose**: Type-safe authentication state preventing unauthenticated request execution
//! **Framework**: UCE34 Q31 (Rust type safety), COCA (zero runtime overhead)

use crate::types::SessionId;

#[cfg(feature = "access-control")]
use crate::access_control::Command;

#[cfg(not(feature = "access-control"))]
/// Stub Command type when access-control feature is disabled
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Read,
    Write,
    Step,
    Continue,
    Breakpoint,
    StackTrace,
    Registers,
    TimeTravel,
    Attach,
    Unknown,
}

/// Authenticated request context for MCP request pipeline
///
/// **Lifecycle**:
/// 1. Created from AuthGuard::AuthContext after all security checks pass
/// 2. Passed to tool routing and execution
/// 3. Consumed by audit logging
///
/// **Security**: Presence of RequestAuthContext guarantees request was authenticated
/// and contains all permissions/quotas needed for authorization.
///
/// **Note**: This is different from auth_guard::AuthContext which is the minimal
/// return value from AuthGuard.authenticate(). RequestAuthContext is enriched
/// with additional information needed for the request pipeline.
#[derive(Debug, Clone)]
pub struct RequestAuthContext {
    /// Client identifier (IP hash or API key hash)
    pub client_id: u64,

    /// User identifier (extracted from JWT or API key metadata)
    pub user_id: u64,

    /// Session ID (if session-based authentication)
    pub session_id: Option<SessionId>,

    /// Allowed commands for this user/client (from AccessControlCapsule)
    pub allowed_commands: Vec<Command>,

    /// Allowed PIDs for this user/client (from AccessControlCapsule)
    /// None = all PIDs allowed, Some(vec) = specific PIDs only
    pub allowed_pids: Option<Vec<u32>>,

    /// Remaining quota for this client (requests)
    pub quota_remaining: u64,

    /// Rate limit tokens remaining
    pub rate_tokens_remaining: f32,

    /// Authentication timestamp (Unix nanoseconds)
    pub auth_timestamp_ns: u64,

    /// Risk score from ZeroTrustPolicy (Q8.8 fixed-point, 0-255)
    pub risk_score: u32,

    /// Request metadata (for audit logging)
    pub request_id: u64,
}

impl RequestAuthContext {
    /// Create new request authentication context
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        client_id: u64,
        user_id: u64,
        session_id: Option<SessionId>,
        allowed_commands: Vec<Command>,
        allowed_pids: Option<Vec<u32>>,
        quota_remaining: u64,
        rate_tokens_remaining: f32,
        risk_score: u32,
        request_id: u64,
    ) -> Self {
        Self {
            client_id,
            user_id,
            session_id,
            allowed_commands,
            allowed_pids,
            quota_remaining,
            rate_tokens_remaining,
            auth_timestamp_ns: Self::current_timestamp_ns(),
            risk_score,
            request_id,
        }
    }

    /// Check if command is allowed for this context
    pub fn has_command_permission(&self, command: Command) -> bool {
        self.allowed_commands.contains(&command)
    }

    /// Check if PID is allowed for this context
    pub fn has_pid_permission(&self, pid: u32) -> bool {
        match &self.allowed_pids {
            None => true, // All PIDs allowed
            Some(allowed) => allowed.contains(&pid),
        }
    }

    /// Get current Unix timestamp in nanoseconds
    fn current_timestamp_ns() -> u64 {
        #[cfg(feature = "std")]
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0)
        }
        #[cfg(not(feature = "std"))]
        {
            0
        }
    }

    /// Create admin context for testing
    #[cfg(test)]
    pub fn mock_admin() -> Self {
        Self {
            client_id: 1,
            user_id: 1,
            session_id: None,
            allowed_commands: vec![
                Command::Read,
                Command::Write,
                Command::Step,
                Command::Continue,
                Command::Breakpoint,
                Command::StackTrace,
                Command::Registers,
                Command::TimeTravel,
            ],
            allowed_pids: None, // All PIDs allowed
            quota_remaining: 10_000,
            rate_tokens_remaining: 100.0,
            auth_timestamp_ns: Self::current_timestamp_ns(),
            risk_score: 0, // Low risk
            request_id: 1,
        }
    }

    /// Create a restricted context for testing
    #[cfg(test)]
    pub fn mock_restricted() -> Self {
        Self {
            client_id: 2,
            user_id: 2,
            session_id: None,
            allowed_commands: vec![Command::Read, Command::StackTrace], // Read-only
            allowed_pids: Some(vec![1234, 5678]), // Specific PIDs only
            quota_remaining: 100,
            rate_tokens_remaining: 10.0,
            auth_timestamp_ns: Self::current_timestamp_ns(),
            risk_score: 128, // Medium risk (Q8.8 50%)
            request_id: 2,
        }
    }
}

/// Permission check error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionError {
    /// Command not allowed for this user
    CommandNotAllowed(Command),

    /// PID not allowed for this user
    PidNotAllowed(u32),

    /// High risk score rejected by policy
    HighRiskRejected { risk_score: u32 },
}

impl std::fmt::Display for PermissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionError::CommandNotAllowed(cmd) => {
                write!(f, "Command {:?} not allowed for this user", cmd)
            }
            PermissionError::PidNotAllowed(pid) => {
                write!(f, "PID {} not allowed for this user", pid)
            }
            PermissionError::HighRiskRejected { risk_score } => {
                write!(
                    f,
                    "Request rejected due to high risk score: {}",
                    risk_score
                )
            }
        }
    }
}

impl std::error::Error for PermissionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_admin_permissions() {
        let ctx = RequestAuthContext::mock_admin();

        // Admin has all commands
        assert!(ctx.has_command_permission(Command::Read));
        assert!(ctx.has_command_permission(Command::Write));
        assert!(ctx.has_command_permission(Command::Breakpoint));

        // Admin has all PIDs
        assert!(ctx.has_pid_permission(1));
        assert!(ctx.has_pid_permission(9999));
    }

    #[test]
    fn test_mock_restricted_permissions() {
        let ctx = RequestAuthContext::mock_restricted();

        // Restricted has only Read and StackTrace
        assert!(ctx.has_command_permission(Command::Read));
        assert!(ctx.has_command_permission(Command::StackTrace));
        assert!(!ctx.has_command_permission(Command::Write));
        assert!(!ctx.has_command_permission(Command::Breakpoint));

        // Restricted has only specific PIDs
        assert!(ctx.has_pid_permission(1234));
        assert!(ctx.has_pid_permission(5678));
        assert!(!ctx.has_pid_permission(9999));
    }

    #[test]
    fn test_timestamp_generation() {
        let ctx = RequestAuthContext::mock_admin();
        assert!(ctx.auth_timestamp_ns > 0, "Timestamp should be non-zero");
    }

    #[test]
    fn test_auth_context_creation() {
        let ctx = RequestAuthContext::new(
            100,
            200,
            Some(SessionId(42)),
            vec![Command::Read],
            Some(vec![1000]),
            5000,
            50.0,
            64, // Q8.8 25% risk
            99,
        );

        assert_eq!(ctx.client_id, 100);
        assert_eq!(ctx.user_id, 200);
        assert_eq!(ctx.session_id, Some(SessionId(42)));
        assert_eq!(ctx.quota_remaining, 5000);
        assert_eq!(ctx.risk_score, 64);
        assert_eq!(ctx.request_id, 99);
    }
}
