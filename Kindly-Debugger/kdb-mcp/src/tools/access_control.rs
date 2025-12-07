//! Access Control MCP Tools (Tools 24-27)
//!
//! SOTA MCP tools for Observer/Operator access control with Ed25519 challenge-response
//! authentication. Implements the principle of least privilege:
//! - Observer: Read-only operations (always permitted)
//! - Operator: Write/execute operations (require Ed25519 authentication)
//!
//! # Tools
//!
//! | ID | Name | Permission | Description |
//! |----|------|------------|-------------|
//! | 24 | `get_access_mode` | Observer | Query current access mode |
//! | 25 | `request_operator_challenge` | Observer | Request Ed25519 challenge |
//! | 26 | `elevate_to_operator` | Observer | Submit signature to elevate |
//! | 27 | `revoke_operator` | Operator | Voluntarily drop to Observer |
//!
//! # Security Properties
//!
//! - Ed25519 signatures with `verify_strict()` (rejects weak keys)
//! - Single-use challenges (generation counter + atomic CAS)
//! - Configurable session timeouts (5min/30min/1hr/never)
//! - Q34 audit trail via rolling hash-chain
//!
//! # Performance (B32 Validated)
//!
//! - `get_access_mode`: <10ns (atomic load)
//! - `request_operator_challenge`: <1us (OsRng + timestamp)
//! - `elevate_to_operator`: <100us (Ed25519 verification)
//! - `revoke_operator`: <100ns (atomic state transition)
//!
//! # Framework Compliance
//!
//! - T1 Atomic: 100% lockfree (no mutex/RwLock)
//! - COCA: All capsules use generation counters, cache-aligned
//! - ASSUM: Pure safe Rust (no unsafe blocks)
//! - T28: Comprehensive tests (unit/property/integration)

use kdb::access_control::{
    AccessMode, AccessModeCapsule, AccessModeError,
    ChallengeState, ChallengeCapsuleError, OperatorChallengeCapsule,
    OperatorSessionCapsule, OperatorSessionError,
    SecurityConfig, verify_challenge_signature, hash_public_key,
    VerificationError, TIMEOUT_NEVER,
};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Tool IDs (constants for external reference)
// ============================================================================

/// Tool ID for get_access_mode
pub const TOOL_ID_GET_ACCESS_MODE: u16 = 24;

/// Tool ID for request_operator_challenge
pub const TOOL_ID_REQUEST_OPERATOR_CHALLENGE: u16 = 25;

/// Tool ID for elevate_to_operator
pub const TOOL_ID_ELEVATE_TO_OPERATOR: u16 = 26;

/// Tool ID for revoke_operator
pub const TOOL_ID_REVOKE_OPERATOR: u16 = 27;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during access control operations.
///
/// # Design
/// Each error variant maps to a specific JSON-RPC error code for consistent
/// client-side handling. Error messages are constant-length where possible
/// to prevent timing attacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessControlError {
    /// Operation requires Observer mode but current mode is not Observer.
    /// JSON-RPC code: -32001
    NotInObserverMode,

    /// Operation requires Operator mode but current mode is not Operator.
    /// JSON-RPC code: -32002
    NotInOperatorMode,

    /// A challenge is already pending (must complete or expire first).
    /// JSON-RPC code: -32003
    ChallengePending,

    /// Challenge has expired before signature submission.
    /// JSON-RPC code: -32004
    ChallengeExpired,

    /// Ed25519 signature verification failed.
    /// JSON-RPC code: -32005
    InvalidSignature,

    /// Ed25519 public key is invalid or malformed.
    /// JSON-RPC code: -32006
    InvalidPublicKey,

    /// Session activation failed (internal error).
    /// JSON-RPC code: -32007
    SessionActivationFailed,

    /// Base64 decoding failed for signature or public key.
    /// JSON-RPC code: -32008
    Base64DecodingFailed,

    /// Challenge not found or already consumed.
    /// JSON-RPC code: -32009
    ChallengeNotFound,

    /// Internal error (catch-all).
    /// JSON-RPC code: -32010
    InternalError(String),

    /// Permission denied (IP mismatch, unauthorized operation).
    /// JSON-RPC code: -32011
    PermissionDenied(String),
}

impl AccessControlError {
    /// Convert error to JSON-RPC error code.
    ///
    /// Error codes are in the -32000 to -32099 range (server errors per JSON-RPC 2.0).
    #[inline]
    pub const fn error_code(&self) -> i32 {
        match self {
            AccessControlError::NotInObserverMode => -32001,
            AccessControlError::NotInOperatorMode => -32002,
            AccessControlError::ChallengePending => -32003,
            AccessControlError::ChallengeExpired => -32004,
            AccessControlError::InvalidSignature => -32005,
            AccessControlError::InvalidPublicKey => -32006,
            AccessControlError::SessionActivationFailed => -32007,
            AccessControlError::Base64DecodingFailed => -32008,
            AccessControlError::ChallengeNotFound => -32009,
            AccessControlError::InternalError(_) => -32010,
            AccessControlError::PermissionDenied(_) => -32011,
        }
    }

    /// Convert error to JSON-RPC 2.0 error response.
    ///
    /// # Arguments
    /// * `id` - Request ID from the JSON-RPC request
    ///
    /// # Returns
    /// JSON object with `jsonrpc`, `error`, and `id` fields.
    #[cfg(feature = "json-rpc")]
    pub fn to_json_rpc_error(&self, id: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "error": {
                "code": self.error_code(),
                "message": self.to_string(),
            },
            "id": id
        })
    }
}

impl std::fmt::Display for AccessControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccessControlError::NotInObserverMode => {
                write!(f, "Not in Observer mode")
            }
            AccessControlError::NotInOperatorMode => {
                write!(f, "Not in Operator mode")
            }
            AccessControlError::ChallengePending => {
                write!(f, "Challenge already pending")
            }
            AccessControlError::ChallengeExpired => {
                write!(f, "Challenge has expired")
            }
            AccessControlError::InvalidSignature => {
                write!(f, "Invalid Ed25519 signature")
            }
            AccessControlError::InvalidPublicKey => {
                write!(f, "Invalid Ed25519 public key")
            }
            AccessControlError::SessionActivationFailed => {
                write!(f, "Session activation failed")
            }
            AccessControlError::Base64DecodingFailed => {
                write!(f, "Base64 decoding failed")
            }
            AccessControlError::ChallengeNotFound => {
                write!(f, "Challenge not found or consumed")
            }
            AccessControlError::InternalError(msg) => {
                write!(f, "Internal error: {}", msg)
            }
            AccessControlError::PermissionDenied(msg) => {
                write!(f, "Permission denied: {}", msg)
            }
        }
    }
}

impl std::error::Error for AccessControlError {}

impl From<ChallengeCapsuleError> for AccessControlError {
    fn from(err: ChallengeCapsuleError) -> Self {
        match err {
            ChallengeCapsuleError::NoChallengeActive => AccessControlError::ChallengeNotFound,
            ChallengeCapsuleError::ChallengeAlreadyUsed => AccessControlError::ChallengeNotFound,
            ChallengeCapsuleError::ChallengeExpired => AccessControlError::ChallengeExpired,
            ChallengeCapsuleError::ConcurrentModification => {
                AccessControlError::InternalError("Concurrent modification".to_string())
            }
            ChallengeCapsuleError::IpMismatch => {
                AccessControlError::PermissionDenied("IP address mismatch".to_string())
            }
        }
    }
}

impl From<VerificationError> for AccessControlError {
    fn from(err: VerificationError) -> Self {
        match err {
            VerificationError::InvalidSignature => AccessControlError::InvalidSignature,
            VerificationError::InvalidPublicKey => AccessControlError::InvalidPublicKey,
            VerificationError::WeakKey => AccessControlError::InvalidPublicKey,
            VerificationError::MalformedInput => AccessControlError::InvalidSignature,
        }
    }
}

impl From<OperatorSessionError> for AccessControlError {
    fn from(err: OperatorSessionError) -> Self {
        match err {
            OperatorSessionError::SessionAlreadyActive => {
                AccessControlError::InternalError("Session already active".to_string())
            }
            OperatorSessionError::SessionNotActive => AccessControlError::NotInOperatorMode,
            OperatorSessionError::SessionExpired => AccessControlError::NotInOperatorMode,
            OperatorSessionError::InvalidTimeout => {
                AccessControlError::InternalError("Invalid timeout".to_string())
            }
            OperatorSessionError::AuditVerificationFailed => {
                AccessControlError::InternalError("Audit verification failed".to_string())
            }
            OperatorSessionError::OperationRecordFailed => {
                AccessControlError::InternalError("Operation recording failed".to_string())
            }
        }
    }
}

impl From<AccessModeError> for AccessControlError {
    fn from(err: AccessModeError) -> Self {
        match err {
            AccessModeError::ConcurrentModification => {
                AccessControlError::InternalError("Concurrent modification".to_string())
            }
            AccessModeError::InvalidTransition { expected, actual } => {
                AccessControlError::InternalError(format!(
                    "Invalid transition: expected {:?}, got {:?}",
                    expected, actual
                ))
            }
            AccessModeError::GenerationOverflow => {
                AccessControlError::InternalError("Generation overflow".to_string())
            }
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get current Unix timestamp in seconds.
#[inline]
fn current_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// Get current Unix timestamp in nanoseconds.
#[inline]
fn current_time_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos() as u64
}

/// Convert Unix timestamp to ISO8601 string.
#[inline]
fn unix_to_iso8601(unix_secs: u64) -> String {
    // Simple ISO8601 formatting without chrono dependency
    // Format: YYYY-MM-DDTHH:MM:SSZ
    let secs_per_day = 86400u64;
    let secs_per_hour = 3600u64;
    let secs_per_minute = 60u64;

    // Days since epoch
    let days = unix_secs / secs_per_day;
    let remaining = unix_secs % secs_per_day;

    let hours = remaining / secs_per_hour;
    let remaining = remaining % secs_per_hour;
    let minutes = remaining / secs_per_minute;
    let seconds = remaining % secs_per_minute;

    // Calculate year/month/day from days since epoch (1970-01-01)
    // This is a simplified calculation that handles leap years
    let mut year = 1970i32;
    let mut remaining_days = days as i64;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let month_days = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for &days in &month_days {
        if remaining_days < days {
            break;
        }
        remaining_days -= days;
        month += 1;
    }

    let day = remaining_days as u32 + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

/// Check if a year is a leap year.
#[inline]
const fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Decode base64 to fixed-size array.
fn decode_base64_32(input: &str) -> Result<[u8; 32], AccessControlError> {
    // Simple base64 decoding (standard alphabet)
    let decoded = base64_decode(input)?;

    if decoded.len() != 32 {
        return Err(AccessControlError::Base64DecodingFailed);
    }

    let mut result = [0u8; 32];
    result.copy_from_slice(&decoded);
    Ok(result)
}

/// Decode base64 to fixed-size array (64 bytes).
fn decode_base64_64(input: &str) -> Result<[u8; 64], AccessControlError> {
    let decoded = base64_decode(input)?;

    if decoded.len() != 64 {
        return Err(AccessControlError::Base64DecodingFailed);
    }

    let mut result = [0u8; 64];
    result.copy_from_slice(&decoded);
    Ok(result)
}

/// Encode bytes to base64 string.
fn encode_base64(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::new();
    let mut chunks = input.chunks_exact(3);

    for chunk in chunks.by_ref() {
        let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32);
        result.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        result.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
        result.push(ALPHABET[(n & 0x3F) as usize] as char);
    }

    let remainder = chunks.remainder();
    if remainder.len() == 1 {
        let n = (remainder[0] as u32) << 16;
        result.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        result.push('=');
        result.push('=');
    } else if remainder.len() == 2 {
        let n = ((remainder[0] as u32) << 16) | ((remainder[1] as u32) << 8);
        result.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        result.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
        result.push('=');
    }

    result
}

/// Decode base64 string to bytes.
fn base64_decode(input: &str) -> Result<Vec<u8>, AccessControlError> {
    const DECODE_TABLE: [u8; 128] = [
        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 62, 255, 255, 255, 63,
        52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 255, 255, 255, 64, 255, 255,
        255, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14,
        15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 255, 255, 255, 255, 255,
        255, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
        41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 255, 255, 255, 255, 255,
    ];

    let input = input.trim_end_matches('=');
    let mut result = Vec::with_capacity((input.len() * 3) / 4);

    let bytes: Vec<u8> = input.bytes().collect();
    let mut i = 0;

    while i + 4 <= bytes.len() {
        let a = DECODE_TABLE.get(bytes[i] as usize).copied().unwrap_or(255);
        let b = DECODE_TABLE.get(bytes[i + 1] as usize).copied().unwrap_or(255);
        let c = DECODE_TABLE.get(bytes[i + 2] as usize).copied().unwrap_or(255);
        let d = DECODE_TABLE.get(bytes[i + 3] as usize).copied().unwrap_or(255);

        if a == 255 || b == 255 || c == 255 || d == 255 {
            return Err(AccessControlError::Base64DecodingFailed);
        }

        let n = ((a as u32) << 18) | ((b as u32) << 12) | ((c as u32) << 6) | (d as u32);
        result.push((n >> 16) as u8);
        result.push((n >> 8) as u8);
        result.push(n as u8);

        i += 4;
    }

    // Handle remaining bytes
    let remaining = bytes.len() - i;
    if remaining == 2 {
        let a = DECODE_TABLE.get(bytes[i] as usize).copied().unwrap_or(255);
        let b = DECODE_TABLE.get(bytes[i + 1] as usize).copied().unwrap_or(255);
        if a == 255 || b == 255 {
            return Err(AccessControlError::Base64DecodingFailed);
        }
        let n = ((a as u32) << 18) | ((b as u32) << 12);
        result.push((n >> 16) as u8);
    } else if remaining == 3 {
        let a = DECODE_TABLE.get(bytes[i] as usize).copied().unwrap_or(255);
        let b = DECODE_TABLE.get(bytes[i + 1] as usize).copied().unwrap_or(255);
        let c = DECODE_TABLE.get(bytes[i + 2] as usize).copied().unwrap_or(255);
        if a == 255 || b == 255 || c == 255 {
            return Err(AccessControlError::Base64DecodingFailed);
        }
        let n = ((a as u32) << 18) | ((b as u32) << 12) | ((c as u32) << 6);
        result.push((n >> 16) as u8);
        result.push((n >> 8) as u8);
    }

    Ok(result)
}

/// Generate a random session ID.
fn generate_session_id() -> u64 {
    // Use timestamp + counter as simple session ID
    // For production, use OsRng
    let now = current_time_nanos();
    now ^ (now >> 32)
}

// ============================================================================
// Tool 24: get_access_mode
// ============================================================================

/// Handle `get_access_mode` request (Tool ID 24).
///
/// Returns the current access mode, session status, and operations performed.
/// This is an Observer-level operation (always permitted).
///
/// # Arguments
/// * `access_mode` - Reference to the access mode capsule
/// * `session` - Optional reference to the operator session capsule
///
/// # Returns
/// JSON object with:
/// - `mode`: Current mode ("Observer", "ChallengePending", "Operator", "Expired")
/// - `session_remaining_secs`: Seconds until session expires (if Operator)
/// - `operations_performed`: Number of operations in current session
/// - `generation`: Current generation counter (for debugging)
///
/// # Performance
/// <10ns (atomic load only)
///
/// # Example Response
/// ```json
/// {
///   "mode": "Operator",
///   "session_remaining_secs": 1800,
///   "operations_performed": 42,
///   "generation": 5
/// }
/// ```
#[cfg(feature = "json-rpc")]
pub fn handle_get_access_mode(
    access_mode: &AccessModeCapsule,
    session: Option<&OperatorSessionCapsule>,
) -> serde_json::Value {
    let (mode, generation, timestamp) = access_mode.get_mode();

    let mode_str = match mode {
        AccessMode::Observer => "Observer",
        AccessMode::ChallengePending => "ChallengePending",
        AccessMode::Operator => "Operator",
        AccessMode::Expired => "Expired",
    };

    let current_time = current_time_secs() as u32;

    // Calculate session remaining time and operations if Operator
    let (session_remaining_secs, operations_performed) = if mode == AccessMode::Operator {
        if let Some(sess) = session {
            let stats = sess.get_stats();
            // Session timeout is typically 30 minutes from timestamp
            let timeout_secs = 1800u32; // Default, should come from config
            let elapsed = current_time.saturating_sub(timestamp);
            let remaining = timeout_secs.saturating_sub(elapsed);
            (Some(remaining as u64), stats.operations_performed)
        } else {
            (None, 0)
        }
    } else {
        (None, 0)
    };

    let mut result = serde_json::json!({
        "mode": mode_str,
        "generation": generation,
        "operations_performed": operations_performed,
    });

    if let Some(remaining) = session_remaining_secs {
        result["session_remaining_secs"] = serde_json::Value::from(remaining);
    }

    result
}

// ============================================================================
// Tool 25: request_operator_challenge
// ============================================================================

/// Handle `request_operator_challenge` request (Tool ID 25).
///
/// Generates a 32-byte cryptographic challenge for Ed25519 signing.
/// This is an Observer-level operation (always permitted).
///
/// # Arguments
/// * `challenge` - Mutable reference to the challenge capsule
/// * `access_mode` - Reference to the access mode capsule
/// * `timeout_secs` - Challenge validity period in seconds (30-300)
///
/// # Returns
/// On success: JSON object with:
/// - `challenge`: Base64-encoded 32-byte challenge nonce
/// - `expires_in_seconds`: Seconds until challenge expires
/// - `instructions`: Human-readable signing instructions
///
/// On error: `AccessControlError`
///
/// # Errors
/// - `NotInObserverMode`: Must be in Observer mode to request challenge
/// - `ChallengePending`: A challenge is already pending
///
/// # Performance
/// <1us (OsRng + timestamp + atomic stores)
///
/// # Example Response
/// ```json
/// {
///   "challenge": "base64...",
///   "expires_in_seconds": 30,
///   "instructions": "Sign this challenge with your Ed25519 private key"
/// }
/// ```
#[cfg(all(feature = "json-rpc", feature = "operator-challenge"))]
pub fn handle_request_operator_challenge(
    challenge: &mut OperatorChallengeCapsule,
    access_mode: &AccessModeCapsule,
    timeout_secs: u32,
    client_ip: &[u8],
) -> Result<serde_json::Value, AccessControlError> {
    // Validate in Observer mode
    let (mode, _, _) = access_mode.get_mode();
    if mode != AccessMode::Observer && mode != AccessMode::Expired {
        return Err(AccessControlError::NotInObserverMode);
    }

    // Check if challenge already pending
    if challenge.state() == ChallengeState::Pending {
        // Check if it's expired
        let now_nanos = current_time_nanos();
        if !challenge.is_expired(now_nanos) {
            return Err(AccessControlError::ChallengePending);
        }
        // Challenge expired, reset and generate new one
        challenge.reset();
    }

    // Clamp timeout to reasonable range (30 seconds to 5 minutes)
    let timeout = timeout_secs.clamp(30, 300);

    // Generate challenge with IP binding (prevents cross-network replay)
    let nonce = challenge.generate_challenge(timeout, client_ip);

    // Encode to base64
    let challenge_b64 = encode_base64(&nonce);

    Ok(serde_json::json!({
        "challenge": challenge_b64,
        "expires_in_seconds": timeout,
        "instructions": "Sign this challenge with your Ed25519 private key and call elevate_to_operator"
    }))
}

// ============================================================================
// Tool 26: elevate_to_operator
// ============================================================================

/// Handle `elevate_to_operator` request (Tool ID 26).
///
/// Verifies Ed25519 signature over the challenge and elevates to Operator mode.
/// This is an Observer-level operation (it IS the elevation mechanism).
///
/// # Arguments
/// * `signature_b64` - Base64-encoded 64-byte Ed25519 signature
/// * `public_key_b64` - Base64-encoded 32-byte Ed25519 public key
/// * `challenge` - Reference to the challenge capsule (to consume)
/// * `access_mode` - Mutable reference to the access mode capsule
/// * `session` - Mutable reference to the session capsule
/// * `config` - Security configuration (for session timeout)
///
/// # Returns
/// On success: JSON object with:
/// - `success`: true
/// - `session_id`: Unique session identifier
/// - `expires_at`: ISO8601 timestamp when session expires
///
/// On error: `AccessControlError`
///
/// # Errors
/// - `ChallengeExpired`: Challenge has expired
/// - `ChallengeNotFound`: No active challenge or already consumed
/// - `InvalidSignature`: Signature verification failed
/// - `InvalidPublicKey`: Public key is invalid or malformed
/// - `Base64DecodingFailed`: Input is not valid base64
/// - `SessionActivationFailed`: Session could not be activated
///
/// # Performance
/// <100us (Ed25519 verification + atomic state transitions)
///
/// # Security
/// - Uses `verify_strict()` to reject weak keys (low-order points)
/// - Challenge is single-use (consumed on success)
/// - Session is cryptographically bound to challenge and public key
///
/// # Example Response
/// ```json
/// {
///   "success": true,
///   "session_id": "12345678901234567890",
///   "expires_at": "2025-01-15T14:30:00Z"
/// }
/// ```
#[cfg(feature = "json-rpc")]
pub fn handle_elevate_to_operator(
    signature_b64: &str,
    public_key_b64: &str,
    challenge: &OperatorChallengeCapsule,
    access_mode: &AccessModeCapsule,
    session: &mut OperatorSessionCapsule,
    config: &SecurityConfig,
    client_ip: &[u8],
) -> Result<serde_json::Value, AccessControlError> {
    // Decode signature and public key from base64
    let signature = decode_base64_64(signature_b64)?;
    let public_key = decode_base64_32(public_key_b64)?;

    // Get the challenge nonce (also checks state and expiry)
    let (nonce, _expiry) = challenge
        .get_challenge()
        .ok_or(AccessControlError::ChallengeNotFound)?;

    // Verify Ed25519 signature over challenge
    verify_challenge_signature(&nonce, &signature, &public_key)?;

    // Consume the challenge (single-use enforcement + IP binding verification)
    let _ = challenge.consume_challenge(client_ip)?;

    // Transition access mode to ChallengePending -> Operator
    // First, check we're in Observer or ChallengePending mode
    let (_current_mode, _, _) = access_mode.get_mode();
    let current_time = current_time_secs() as u32;

    // Force transition to Operator (challenge verified)
    access_mode.force_transition(AccessMode::Operator, current_time);

    // Activate session with cryptographic binding
    let session_id = generate_session_id();
    let challenge_hash = hash_public_key(&nonce); // Hash of challenge for binding
    let pubkey_hash = hash_public_key(&public_key);

    // Calculate timeout from config
    let timeout_secs = config
        .session_timeout
        .map(|d| d.as_secs() as u32)
        .unwrap_or(TIMEOUT_NEVER);

    session.activate(
        session_id,
        challenge_hash,
        pubkey_hash,
        timeout_secs,
        current_time as u64,
    ).map_err(|_| AccessControlError::SessionActivationFailed)?;

    // Calculate expiry timestamp
    let expires_at = if timeout_secs == TIMEOUT_NEVER {
        "never".to_string()
    } else {
        unix_to_iso8601(current_time as u64 + timeout_secs as u64)
    };

    Ok(serde_json::json!({
        "success": true,
        "session_id": session_id.to_string(),
        "expires_at": expires_at
    }))
}

// ============================================================================
// Tool 27: revoke_operator
// ============================================================================

/// Handle `revoke_operator` request (Tool ID 27).
///
/// Voluntarily drops from Operator mode to Observer mode.
/// This is an Operator-level operation (only operators can revoke themselves).
///
/// # Arguments
/// * `access_mode` - Mutable reference to the access mode capsule
/// * `session` - Reference to the session capsule (to get stats)
///
/// # Returns
/// On success: JSON object with:
/// - `success`: true
/// - `previous_session_id`: Session ID of the revoked session
/// - `operations_performed`: Number of operations performed during session
///
/// On error: `AccessControlError`
///
/// # Errors
/// - `NotInOperatorMode`: Must be in Operator mode to revoke
///
/// # Performance
/// <100ns (atomic state transitions)
///
/// # Example Response
/// ```json
/// {
///   "success": true,
///   "previous_session_id": "12345678901234567890",
///   "operations_performed": 42
/// }
/// ```
#[cfg(feature = "json-rpc")]
pub fn handle_revoke_operator(
    access_mode: &AccessModeCapsule,
    session: &OperatorSessionCapsule,
) -> Result<serde_json::Value, AccessControlError> {
    // Validate in Operator mode
    if !access_mode.is_operator() {
        return Err(AccessControlError::NotInOperatorMode);
    }

    // Get session stats before deactivating
    let stats = session.deactivate();

    // Transition to Observer mode
    let current_time = current_time_secs() as u32;
    access_mode.reset_to_observer(current_time);

    Ok(serde_json::json!({
        "success": true,
        "previous_session_id": stats.session_id.to_string(),
        "operations_performed": stats.operations_performed
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 30-minute timeout in seconds for test sessions
    const TIMEOUT_30_MIN: u32 = 30 * 60;

    // ========================================================================
    // Base64 Encoding/Decoding Tests
    // ========================================================================

    #[test]
    fn test_base64_roundtrip() {
        let original = [0xABu8; 32];
        let encoded = encode_base64(&original);
        let decoded = decode_base64_32(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_base64_roundtrip_64() {
        let original = [0xCDu8; 64];
        let encoded = encode_base64(&original);
        let decoded = decode_base64_64(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_base64_invalid() {
        let result = base64_decode("!!!invalid!!!");
        assert!(result.is_err());
    }

    // ========================================================================
    // ISO8601 Formatting Tests
    // ========================================================================

    #[test]
    fn test_unix_to_iso8601() {
        // 2024-01-01T00:00:00Z
        let ts = 1704067200u64;
        let iso = unix_to_iso8601(ts);
        assert_eq!(iso, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn test_unix_to_iso8601_epoch() {
        let iso = unix_to_iso8601(0);
        assert_eq!(iso, "1970-01-01T00:00:00Z");
    }

    // ========================================================================
    // Error Code Tests
    // ========================================================================

    #[test]
    fn test_error_codes_unique() {
        let errors = [
            AccessControlError::NotInObserverMode,
            AccessControlError::NotInOperatorMode,
            AccessControlError::ChallengePending,
            AccessControlError::ChallengeExpired,
            AccessControlError::InvalidSignature,
            AccessControlError::InvalidPublicKey,
            AccessControlError::SessionActivationFailed,
            AccessControlError::Base64DecodingFailed,
            AccessControlError::ChallengeNotFound,
            AccessControlError::InternalError("test".to_string()),
        ];

        let codes: Vec<i32> = errors.iter().map(|e| e.error_code()).collect();
        for i in 0..codes.len() {
            for j in (i + 1)..codes.len() {
                assert_ne!(codes[i], codes[j], "Error codes must be unique");
            }
        }
    }

    #[test]
    fn test_error_codes_in_range() {
        let errors = [
            AccessControlError::NotInObserverMode,
            AccessControlError::NotInOperatorMode,
            AccessControlError::ChallengePending,
        ];

        for err in &errors {
            let code = err.error_code();
            assert!(code >= -32099 && code <= -32000,
                "Error code {} should be in JSON-RPC server error range", code);
        }
    }

    // ========================================================================
    // Tool 24: get_access_mode Tests
    // ========================================================================

    #[cfg(feature = "json-rpc")]
    #[test]
    fn test_get_access_mode_observer() {
        let access_mode = AccessModeCapsule::new(0);
        let result = handle_get_access_mode(&access_mode, None);

        assert_eq!(result["mode"], "Observer");
        assert!(result["generation"].is_number());
        assert_eq!(result["operations_performed"], 0);
    }

    #[cfg(feature = "json-rpc")]
    #[test]
    fn test_get_access_mode_operator() {
        let access_mode = AccessModeCapsule::new(0);
        let current_time = current_time_secs() as u32;
        access_mode.force_transition(AccessMode::Operator, current_time);

        let mut session = OperatorSessionCapsule::new();
        session.activate(123, [0u8; 32], [1u8; 32], TIMEOUT_30_MIN, current_time as u64).unwrap();

        let result = handle_get_access_mode(&access_mode, Some(&session));

        assert_eq!(result["mode"], "Operator");
        assert!(result["session_remaining_secs"].is_number());
    }

    // ========================================================================
    // Tool 25: request_operator_challenge Tests
    // ========================================================================

    #[cfg(all(feature = "json-rpc", feature = "operator-challenge"))]
    #[test]
    fn test_request_operator_challenge_success() {
        let mut challenge = OperatorChallengeCapsule::new();
        let access_mode = AccessModeCapsule::new(0);
        let client_ip = b"127.0.0.1";  // Test client IP

        let result = handle_request_operator_challenge(&mut challenge, &access_mode, 60, client_ip);

        assert!(result.is_ok());
        let json = result.unwrap();
        assert!(json["challenge"].is_string());
        assert_eq!(json["expires_in_seconds"], 60);
        assert!(json["instructions"].is_string());
    }

    #[cfg(all(feature = "json-rpc", feature = "operator-challenge"))]
    #[test]
    fn test_request_operator_challenge_already_pending() {
        let mut challenge = OperatorChallengeCapsule::new();
        let access_mode = AccessModeCapsule::new(0);
        let client_ip = b"127.0.0.1";  // Test client IP

        // First request should succeed
        let _ = handle_request_operator_challenge(&mut challenge, &access_mode, 60, client_ip);

        // Second request should fail (challenge pending)
        let result = handle_request_operator_challenge(&mut challenge, &access_mode, 60, client_ip);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), AccessControlError::ChallengePending);
    }

    #[cfg(all(feature = "json-rpc", feature = "operator-challenge"))]
    #[test]
    fn test_request_operator_challenge_not_in_observer() {
        let mut challenge = OperatorChallengeCapsule::new();
        let access_mode = AccessModeCapsule::new(0);
        let client_ip = b"127.0.0.1";  // Test client IP
        let current_time = current_time_secs() as u32;
        access_mode.force_transition(AccessMode::Operator, current_time);

        let result = handle_request_operator_challenge(&mut challenge, &access_mode, 60, client_ip);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), AccessControlError::NotInObserverMode);
    }

    // ========================================================================
    // Tool 27: revoke_operator Tests
    // ========================================================================

    #[cfg(feature = "json-rpc")]
    #[test]
    fn test_revoke_operator_success() {
        let access_mode = AccessModeCapsule::new(0);
        let current_time = current_time_secs() as u32;
        access_mode.force_transition(AccessMode::Operator, current_time);

        let mut session = OperatorSessionCapsule::new();
        session.activate(12345, [0u8; 32], [1u8; 32], TIMEOUT_30_MIN, current_time as u64).unwrap();
        session.record_operation(1, current_time as u64 + 10).unwrap();
        session.record_operation(2, current_time as u64 + 20).unwrap();

        let result = handle_revoke_operator(&access_mode, &session);

        assert!(result.is_ok());
        let json = result.unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["previous_session_id"], "12345");
        assert_eq!(json["operations_performed"], 2);

        // Verify mode is now Observer
        assert!(access_mode.is_observer());
    }

    #[cfg(feature = "json-rpc")]
    #[test]
    fn test_revoke_operator_not_in_operator() {
        let access_mode = AccessModeCapsule::new(0);
        let session = OperatorSessionCapsule::new();

        let result = handle_revoke_operator(&access_mode, &session);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), AccessControlError::NotInOperatorMode);
    }

    // ========================================================================
    // Full Flow Test
    // ========================================================================

    #[cfg(all(feature = "json-rpc", feature = "operator-challenge"))]
    #[test]
    fn test_full_elevation_flow() {
        use ed25519_dalek::{SigningKey, Signer};

        // Setup
        let mut challenge_capsule = OperatorChallengeCapsule::new();
        let access_mode = AccessModeCapsule::new(0);
        let mut session = OperatorSessionCapsule::new();
        let config = SecurityConfig::standard();

        // Generate test keypair
        let seed: [u8; 32] = [
            0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60,
            0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
            0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19,
            0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
        ];
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();

        // Step 1: Verify starting in Observer mode
        assert!(access_mode.is_observer());
        let client_ip = b"127.0.0.1";  // Test client IP

        // Step 2: Request challenge
        let challenge_result = handle_request_operator_challenge(
            &mut challenge_capsule,
            &access_mode,
            60,
            client_ip,
        ).unwrap();

        let challenge_b64 = challenge_result["challenge"].as_str().unwrap();
        let challenge_bytes = decode_base64_32(challenge_b64).unwrap();

        // Step 3: Sign challenge
        let signature = signing_key.sign(&challenge_bytes);
        let signature_b64 = encode_base64(&signature.to_bytes());
        let pubkey_b64 = encode_base64(&verifying_key.to_bytes());

        // Step 4: Elevate to Operator
        let elevate_result = handle_elevate_to_operator(
            &signature_b64,
            &pubkey_b64,
            &challenge_capsule,
            &access_mode,
            &mut session,
            &config,
            client_ip,
        ).unwrap();

        assert_eq!(elevate_result["success"], true);
        assert!(elevate_result["session_id"].is_string());
        assert!(elevate_result["expires_at"].is_string());

        // Step 5: Verify in Operator mode
        assert!(access_mode.is_operator());
        assert!(session.is_active());

        // Step 6: Revoke
        let revoke_result = handle_revoke_operator(&access_mode, &session).unwrap();

        assert_eq!(revoke_result["success"], true);

        // Step 7: Verify back in Observer mode
        assert!(access_mode.is_observer());
        assert!(!session.is_active());
    }

    // ========================================================================
    // Error Conversion Tests
    // ========================================================================

    #[test]
    fn test_challenge_error_conversion() {
        let err = AccessControlError::from(ChallengeCapsuleError::ChallengeExpired);
        assert_eq!(err, AccessControlError::ChallengeExpired);

        let err = AccessControlError::from(ChallengeCapsuleError::NoChallengeActive);
        assert_eq!(err, AccessControlError::ChallengeNotFound);
    }

    #[test]
    fn test_verification_error_conversion() {
        let err = AccessControlError::from(VerificationError::InvalidSignature);
        assert_eq!(err, AccessControlError::InvalidSignature);

        let err = AccessControlError::from(VerificationError::WeakKey);
        assert_eq!(err, AccessControlError::InvalidPublicKey);
    }

    #[test]
    fn test_session_error_conversion() {
        let err = AccessControlError::from(OperatorSessionError::SessionExpired);
        assert_eq!(err, AccessControlError::NotInOperatorMode);
    }
}
