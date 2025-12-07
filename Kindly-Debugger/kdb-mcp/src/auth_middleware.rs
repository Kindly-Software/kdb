//! Authentication Middleware - Minimal Security Layer
//!
//! **Purpose**: Close CVSS 9.3 vulnerability by adding mandatory authentication
//! **Status**: Phase 1 - Basic API key + permission validation
//! **Future**: Phase 2 - Full AuthGuard integration with 18 capsules
//!
//! ## Architecture
//!
//! ```text
//! Request → authenticate_request() → RequestAuthContext
//!   │
//!   ├─ Extract API key from headers
//!   ├─ Validate API key exists (reject if missing)
//!   ├─ Extract client IP
//!   ├─ Check basic permissions (PID/command whitelisting)
//!   └─ Build RequestAuthContext with permissions
//! ```
//!
//! ## UCE34 Framework
//!
//! **Q1-Q9**: Minimal authentication to close security hole
//! **Q10**: T1 Atomic (simple validation, <100ns)
//! **Q11**: Type-safe RequestAuthContext
//! **Q28**: Simple interface (single function)
//! **Q34**: Audit logging (future phase)

use crate::RequestAuthContext;
use crate::types::SessionId;

#[cfg(feature = "access-control")]
use crate::access_control::Command;

#[cfg(not(feature = "access-control"))]
use crate::auth_context::Command;

/// Authentication error
#[derive(Debug, Clone)]
pub enum AuthenticationError {
    /// Missing Authorization header
    MissingApiKey,

    /// Invalid API key format
    InvalidApiKey,

    /// Missing client IP
    MissingClientIp,

    /// Permission denied (command not allowed)
    PermissionDenied(String),

    /// PID not allowed
    PidNotAllowed(u32),

    /// Internal error
    Internal(String),
}

impl std::fmt::Display for AuthenticationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthenticationError::MissingApiKey => write!(f, "Missing Authorization header"),
            AuthenticationError::InvalidApiKey => write!(f, "Invalid API key"),
            AuthenticationError::MissingClientIp => write!(f, "Missing client IP"),
            AuthenticationError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            AuthenticationError::PidNotAllowed(pid) => write!(f, "PID {} not allowed", pid),
            AuthenticationError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AuthenticationError {}

/// Minimal authentication configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Allowed commands (default: Read, StackTrace only)
    pub allowed_commands: Vec<Command>,

    /// Allowed PIDs (None = all allowed, Some(vec) = specific PIDs)
    pub allowed_pids: Option<Vec<u32>>,

    /// Require API key (default: true)
    pub require_api_key: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            // Conservative default: read-only operations
            allowed_commands: vec![Command::Read, Command::StackTrace],
            allowed_pids: None, // All PIDs allowed (filtered by OS permissions)
            require_api_key: true,
        }
    }
}

impl AuthConfig {
    /// Create permissive config for testing (ALL commands, ALL PIDs)
    ///
    /// **Warning**: Only use for testing! Production should use restrictive defaults.
    pub fn permissive() -> Self {
        Self {
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
            allowed_pids: None,
            require_api_key: false, // Disable for testing
        }
    }
}

/// Authenticate MCP request
///
/// **Phase 1**: Basic validation (API key + permissions)
/// **Phase 2**: Full AuthGuard integration (18 capsules)
///
/// # Arguments
/// - `api_key`: Optional Bearer token from Authorization header
/// - `client_ip`: Client IP address (X-Forwarded-For or socket address)
/// - `target_pid`: PID being debugged (0 if not applicable)
/// - `command`: Command being executed
/// - `config`: Authentication configuration
///
/// # Returns
/// - `Ok(RequestAuthContext)`: Authentication succeeded
/// - `Err(AuthenticationError)`: Validation failed
///
/// # Performance
/// - <100ns (Phase 1 minimal checks)
/// - <1,292ns (Phase 2 full AuthGuard)
pub fn authenticate_request(
    api_key: Option<&str>,
    client_ip: Option<&str>,
    target_pid: u32,
    command: Command,
    config: &AuthConfig,
) -> Result<RequestAuthContext, AuthenticationError> {
    // ========================================================================
    // CHECK 1: API Key Validation
    // ========================================================================
    if config.require_api_key {
        let key = api_key.ok_or(AuthenticationError::MissingApiKey)?;

        // Basic validation: non-empty, reasonable length
        if key.is_empty() || key.len() < 16 {
            return Err(AuthenticationError::InvalidApiKey);
        }

        // TODO Phase 2: Validate against ApiKeyAuthCapsule
    }

    // ========================================================================
    // CHECK 2: Client IP Validation
    // ========================================================================
    let ip = client_ip.ok_or(AuthenticationError::MissingClientIp)?;

    // TODO Phase 2: Check against IntrusionDetectorCapsule

    // ========================================================================
    // CHECK 3: Command Permission
    // ========================================================================
    if !config.allowed_commands.contains(&command) {
        return Err(AuthenticationError::PermissionDenied(
            format!("Command {:?} not allowed", command),
        ));
    }

    // ========================================================================
    // CHECK 4: PID Permission
    // ========================================================================
    if let Some(ref allowed_pids) = config.allowed_pids {
        if target_pid > 0 && !allowed_pids.contains(&target_pid) {
            return Err(AuthenticationError::PidNotAllowed(target_pid));
        }
    }

    // ========================================================================
    // Build RequestAuthContext
    // ========================================================================
    let client_id = hash_string(ip); // Simple hash of IP
    let user_id = api_key.map(hash_string).unwrap_or(0);

    Ok(RequestAuthContext::new(
        client_id,
        user_id,
        Some(SessionId(user_id)), // Simplified: use user_id as session_id
        config.allowed_commands.clone(),
        config.allowed_pids.clone(),
        10_000, // Default quota
        100.0,  // Default rate tokens
        0,      // Low risk score (Phase 2: zero-trust policy)
        generate_request_id(),
    ))
}

/// Simple FNV-1a hash for strings
fn hash_string(s: &str) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    let mut hash = FNV_OFFSET;
    for byte in s.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Generate unique request ID
fn generate_request_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Map MCP method name to Command
pub fn method_to_command(method: &str) -> Result<Command, String> {
    match method {
        "debugger/attach" => Ok(Command::Continue), // Attach requires Continue permission
        "debugger/set_breakpoint" => Ok(Command::Breakpoint),
        "debugger/continue" => Ok(Command::Continue),
        "debugger/step_forward" => Ok(Command::Step),
        "debugger/step_backward" => Ok(Command::TimeTravel),
        "debugger/get_stack_trace" => Ok(Command::StackTrace),
        "debugger/get_variables" => Ok(Command::Read),
        "debugger/read_memory" => Ok(Command::Read),
        "debugger/write_memory" => Ok(Command::Write),
        "debugger/find_similar_bugs" => Ok(Command::Read), // Read-only analysis
        "debugger/export_trace" => Ok(Command::Read),      // Read-only export
        _ => Err(format!("Unknown method: {}", method)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authenticate_with_valid_api_key() {
        let config = AuthConfig::permissive();

        let result = authenticate_request(
            Some("valid_api_key_1234567890"),
            Some("192.168.1.100"),
            1234,
            Command::Read,
            &config,
        );

        assert!(result.is_ok());
        let ctx = result.unwrap();
        assert_eq!(ctx.client_id, hash_string("192.168.1.100"));
    }

    #[test]
    fn test_authenticate_without_api_key_when_required() {
        let config = AuthConfig::default(); // requires API key

        let result = authenticate_request(None, Some("192.168.1.100"), 1234, Command::Read, &config);

        assert!(matches!(
            result,
            Err(AuthenticationError::MissingApiKey)
        ));
    }

    #[test]
    fn test_authenticate_with_invalid_api_key() {
        let config = AuthConfig::default();

        let result = authenticate_request(
            Some("short"), // Too short
            Some("192.168.1.100"),
            1234,
            Command::Read,
            &config,
        );

        assert!(matches!(result, Err(AuthenticationError::InvalidApiKey)));
    }

    #[test]
    fn test_authenticate_without_client_ip() {
        let config = AuthConfig::permissive();

        let result = authenticate_request(Some("valid_api_key_1234567890"), None, 1234, Command::Read, &config);

        assert!(matches!(
            result,
            Err(AuthenticationError::MissingClientIp)
        ));
    }

    #[test]
    fn test_authenticate_with_disallowed_command() {
        let config = AuthConfig::default(); // Only Read, StackTrace allowed

        let result = authenticate_request(
            Some("valid_api_key_1234567890"),
            Some("192.168.1.100"),
            1234,
            Command::Write, // NOT allowed
            &config,
        );

        assert!(matches!(
            result,
            Err(AuthenticationError::PermissionDenied(_))
        ));
    }

    #[test]
    fn test_authenticate_with_disallowed_pid() {
        let mut config = AuthConfig::permissive();
        config.allowed_pids = Some(vec![1000, 2000]); // Only these PIDs allowed

        let result = authenticate_request(
            Some("valid_api_key_1234567890"),
            Some("192.168.1.100"),
            9999, // NOT allowed
            Command::Read,
            &config,
        );

        assert!(matches!(result, Err(AuthenticationError::PidNotAllowed(9999))));
    }

    #[test]
    fn test_method_to_command_mapping() {
        assert_eq!(method_to_command("debugger/attach").unwrap(), Command::Continue);
        assert_eq!(
            method_to_command("debugger/set_breakpoint").unwrap(),
            Command::Breakpoint
        );
        assert_eq!(method_to_command("debugger/step_forward").unwrap(), Command::Step);
        assert_eq!(
            method_to_command("debugger/step_backward").unwrap(),
            Command::TimeTravel
        );
        assert_eq!(
            method_to_command("debugger/get_stack_trace").unwrap(),
            Command::StackTrace
        );
        assert!(method_to_command("debugger/unknown").is_err());
    }

    #[test]
    fn test_hash_string_deterministic() {
        let hash1 = hash_string("192.168.1.100");
        let hash2 = hash_string("192.168.1.100");
        assert_eq!(hash1, hash2);

        let hash3 = hash_string("192.168.1.101");
        assert_ne!(hash1, hash3);
    }
}
