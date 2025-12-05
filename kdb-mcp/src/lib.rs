//! Atomic MCP Server - T6 Mixed Computational Capsule
//!
//! A 256 KB MCP (Model Context Protocol) debugging server orchestrating 8 capsules:
//! - JsonRpcCapsule (T1 Atomic): <1μs parse/format
//! - RateLimiterCapsule (T1 Atomic): <150ns rate check
//! - QuotaTrackerCapsule (T1 Atomic): <70ns quota check
//! - McpToolRegistryCapsule (T1 Atomic): <120ns routing
//! - LicenseValidatorCapsule (T1 Atomic): <10ns cached validation
//! - DebuggerCapsule (1 MB): Variable latency debugging
//! - HistogramCapsule (T1): <10ns latency recording
//! - AuditLogCapsule (T0): <50ns audit
//!
//! **Total latency target**: <10μs end-to-end (10-100× faster than kindly_mcp)
//!
//! ## Architecture
//!
//! ```text
//! McpServerCapsule (256 KB)
//!   ├── JsonRpcCapsule (4 KB) - Parse JSON-RPC requests
//!   ├── LicenseValidatorCapsule (4 KB) - Auth validation
//!   ├── RateLimiterCapsule (4 KB) - Token bucket rate limiting
//!   ├── QuotaTrackerCapsule (4 KB) - Usage tracking
//!   ├── McpToolRegistryCapsule (16 KB) - Tool routing
//!   ├── DebuggerCapsule (1 MB) - Debugging operations
//!   ├── HistogramCapsule (16 KB) - Latency monitoring
//!   └── AuditLogCapsule (32 KB) - Request audit trail
//! ```
//!
//! ## MCP Tools
//!
//! - `debugger/attach` - Attach to process
//! - `debugger/set_breakpoint` - Add breakpoint
//! - `debugger/continue` - Resume execution
//! - `debugger/step_forward` - Single step
//! - `debugger/step_backward` - Time-travel!
//! - `debugger/get_stack_trace` - SIMD stack unwind
//! - `debugger/get_variables` - Read memory
//! - `debugger/find_similar_bugs` - T10 probabilistic
//! - `debugger/export_trace` - T5 streaming export
//! - `debugger/quota_status` - Quota tier/limits/usage (T1 Atomic, <70ns)
//! - `debugger/license_info` - License tier/validation/expiry (T1 Atomic, <10ns)

#![cfg_attr(not(feature = "std"), no_std)]

// ============================================================================
// Core MCP Server Modules
// ============================================================================

pub mod types;  // Common types and stubs
pub mod json_rpc;
pub mod rate_limiter;
pub mod quota_tracker;
pub mod config_loader;  // PID allowlist config file reader

// Document processing (T2+T3 Mixed)
pub mod document;
pub mod tool_registry;
pub mod license_validator;
pub mod server;
pub mod tools;
pub mod auth_context;  // Authentication context for authenticated requests
pub mod auth_middleware;  // Phase 1 authentication middleware (basic validation)

// ============================================================================
// T28 Deterministic Testing Framework (Q8-Q14 Property Tests)
// ============================================================================

pub mod deterministic_mcp;  // Deterministic context for reproducible testing

#[cfg(target_os = "linux")]
pub mod security;  // PID validation and privilege escalation prevention (Linux only)

// ============================================================================
// Phase 2A: Security & Authentication Modules (17 modules)
// ============================================================================

#[cfg(feature = "api-key-auth")]
pub mod api_key_auth;

#[cfg(feature = "auth-guard")]
pub mod auth_guard;

#[cfg(feature = "auth-token")]
pub mod auth_token;

#[cfg(feature = "totp")]
pub mod totp_validator;

#[cfg(feature = "access-control")]
pub mod access_control;

#[cfg(feature = "zero-trust")]
pub mod zero_trust_policy;

#[cfg(feature = "capability-checker")]
pub mod capability_checker;

#[cfg(feature = "dynamic-pid")]
pub mod dynamic_pid_whitelist;

#[cfg(feature = "secrets")]
pub mod secrets_manager;

#[cfg(feature = "memory-encryption")]
pub mod memory_encryption;

#[cfg(feature = "hsm")]
pub mod hsm_integration;

#[cfg(feature = "key-rotation")]
pub mod key_rotation;

#[cfg(feature = "tls")]
pub mod tls_capsule;

#[cfg(feature = "acme")]
pub mod acme_cert_manager;

#[cfg(feature = "intrusion-detection")]
pub mod intrusion_detector;

#[cfg(feature = "anomaly-detection")]
pub mod anomaly_detector;

#[cfg(feature = "session")]
pub mod session;

// ============================================================================
// Phase 2B: Observability & Monitoring Modules (5 modules)
// ============================================================================

#[cfg(feature = "metrics")]
pub mod metrics;

#[cfg(feature = "tracing")]
pub mod tracing_setup;

#[cfg(feature = "audit")]
pub mod audit_enhancement;

#[cfg(feature = "audit-rotation")]
pub mod audit_log_rotation;

#[cfg(feature = "rate-limiting")]
pub mod per_client_rate_limiter;

// ============================================================================
// Phase 2C: Infrastructure & Transport Modules (8 modules)
// ============================================================================

#[cfg(feature = "runtime")]
pub mod runtime;

#[cfg(feature = "http-transport")]
pub mod http_transport;

#[cfg(feature = "stdio-transport")]
pub mod stdio_transport;

#[cfg(feature = "connection-pool")]
pub mod connection_pool;

#[cfg(feature = "shared-state")]
pub mod shared_state;

#[cfg(feature = "tool-executor")]
pub mod tool_executor;

#[cfg(feature = "feature-flags")]
pub mod feature_flags;

#[cfg(feature = "ab-testing")]
pub mod ab_testing;

// ============================================================================
// Public Re-exports (Core)
// ============================================================================

pub use server::McpServerCapsule;
pub use json_rpc::JsonRpcCapsule;
pub use rate_limiter::RateLimiterCapsule;
pub use quota_tracker::QuotaTrackerCapsule;
pub use tool_registry::McpToolRegistryCapsule;
pub use license_validator::LicenseValidatorCapsule;
pub use config_loader::{PidAllowlistConfig, ConfigError, DEFAULT_CONFIG_PATH, is_pid_allowed_by_config};

// Common types (feature-gated types re-exported directly from types module)
pub use types::SessionId;

// Authentication types
pub use auth_context::{RequestAuthContext, PermissionError};
pub use auth_context::RequestAuthContext as AuthContext;  // Alias for tests
pub use auth_middleware::{authenticate_request, method_to_command, AuthConfig, AuthenticationError};

// Re-export Command from access_control if available
#[cfg(feature = "access-control")]
pub use access_control::Command;

// Re-export Operation from audit_enhancement if available
#[cfg(feature = "audit")]
pub use audit_enhancement::Operation;

// Re-export PolicyAction from zero_trust_policy if available
#[cfg(feature = "zero-trust")]
pub use zero_trust_policy::PolicyAction;

// Capsule re-exports (conditional stubs from types module)
#[cfg(feature = "audit")]
pub use types::AuditEnhancementCapsule;
#[cfg(not(feature = "audit"))]
pub use types::AuditEnhancementCapsule;

#[cfg(feature = "dynamic-pid")]
pub use types::DynamicPidWhitelistCapsule;
#[cfg(not(feature = "dynamic-pid"))]
pub use types::DynamicPidWhitelistCapsule;

#[cfg(feature = "key-rotation")]
pub use types::KeyRotationCapsule;
#[cfg(not(feature = "key-rotation"))]
pub use types::KeyRotationCapsule;

#[cfg(feature = "memory-encryption")]
pub use types::MemoryEncryptionCapsule;
#[cfg(not(feature = "memory-encryption"))]
pub use types::MemoryEncryptionCapsule;

#[cfg(feature = "acme")]
pub use types::AcmeCertManagerCapsule;
#[cfg(not(feature = "acme"))]
pub use types::AcmeCertManagerCapsule;

// ============================================================================
// Public Re-exports (Security & Authentication)
// ============================================================================

#[cfg(feature = "api-key-auth")]
pub use api_key_auth::ApiKeyAuthCapsule;

#[cfg(feature = "auth-guard")]
pub use auth_guard::{AuthGuard, AuthGuardConfig, AuthGuardError, AuthGuardStats};

#[cfg(feature = "auth-token")]
pub use auth_token::{AuthTokenCapsule, AuthError};

#[cfg(feature = "totp")]
pub use totp_validator::TotpValidatorCapsule;

#[cfg(feature = "access-control")]
pub use access_control::AccessControlCapsule;

#[cfg(feature = "zero-trust")]
pub use zero_trust_policy::{
    ZeroTrustPolicyCapsule,
    PolicyDecision,
    PolicyRules,
    PolicyStats,
    PolicyError,
    RiskScore,
    RiskComponents,
};

#[cfg(feature = "secrets")]
pub use secrets_manager::SecretsManagerCapsule;

#[cfg(feature = "tls")]
pub use tls_capsule::{TlsCapsule, TlsError};

#[cfg(feature = "intrusion-detection")]
pub use intrusion_detector::IntrusionDetectorCapsule;

#[cfg(feature = "anomaly-detection")]
pub use anomaly_detector::{
    AnomalyDetectorCapsule,
    AnomalyError,
    RequestFeatures,
    AnomalyPrediction,
    AnomalyDetectorStats,
};

#[cfg(feature = "session")]
pub use session::SessionCapsule;

#[cfg(feature = "dynamic-pid")]
pub use dynamic_pid_whitelist::PidWhitelistError;

#[cfg(feature = "hsm")]
pub use hsm_integration::{
    HsmIntegrationCapsule,
    HsmError,
    HsmStatus,
    HsmKeyPair,
    ED25519_PUBLIC_KEY_SIZE,
};

// ============================================================================
// Public Re-exports (Observability & Monitoring)
// ============================================================================

#[cfg(feature = "metrics")]
pub use metrics::MetricsCapsule;

#[cfg(feature = "rate-limiting")]
pub use per_client_rate_limiter::{
    PerClientRateLimiterCapsule,
    ClientId,
    ClientTokenBucket,
    RateLimitDecision,
    RateLimitError,
    ClientBucketStats,
};

#[cfg(feature = "audit")]
pub use audit_enhancement::AuditEnhancementCapsule as RealAuditEnhancementCapsule;

// ============================================================================
// Public Re-exports (Infrastructure & Transport)
// ============================================================================

#[cfg(feature = "runtime")]
pub use runtime::McpRuntimeCapsule;

#[cfg(feature = "stdio-transport")]
pub use stdio_transport::StdioTransportCapsule;

#[cfg(feature = "http-transport")]
pub use http_transport::HttpTransport;

#[cfg(feature = "tool-executor")]
pub use tool_executor::{ToolExecutorCapsule, ExecutionState};

#[cfg(feature = "feature-flags")]
pub use feature_flags::FeatureFlagsCapsule;

#[cfg(feature = "connection-pool")]
pub use connection_pool::ConnectionPoolCapsule;

// Note: ab_testing module doesn't define AbTestingCapsule yet
// #[cfg(feature = "ab-testing")]
// pub use ab_testing::AbTestingCapsule;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{size_of, align_of};

    #[test]
    fn test_server_size() {
        // Protocol state field (AtomicU8) + alignment padding = 256 bytes growth
        assert_eq!(size_of::<McpServerCapsule>(), 262_400, "McpServerCapsule must be 256 KB + protocol_state (8B) + alignment");
    }

    #[test]
    fn test_server_alignment() {
        assert_eq!(align_of::<McpServerCapsule>(), 256, "McpServerCapsule must be 256-byte aligned");
    }
}
