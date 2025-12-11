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
//! - SessionPoolCapsule (T6): <100ns session allocation/release
//! - MemoryReplayCapsule (T6): <50ms memory snapshot capture
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
//!   ├── SessionPoolCapsule (512B) - Tiered session management
//!   ├── MemoryReplayCapsule (512B+) - COW memory tracking
//!   ├── HistogramCapsule (16 KB) - Latency monitoring
//!   └── AuditLogCapsule (32 KB) - Request audit trail
//! ```
//!
//! ## MCP Tools
//!
//! ### Debugging Tools (1-9)
//! - `debugger/attach` - Attach to process
//! - `debugger/set_breakpoint` - Add breakpoint
//! - `debugger/continue` - Resume execution
//! - `debugger/step_forward` - Single step
//! - `debugger/step_backward` - Time-travel!
//! - `debugger/get_stack_trace` - SIMD stack unwind
//! - `debugger/get_variables` - Read memory
//! - `debugger/find_similar_bugs` - T10 probabilistic
//! - `debugger/export_trace` - T5 streaming export
//!
//! ### Admin Tools (10-12)
//! - `debugger/quota_status` - Quota tier/limits/usage (T1 Atomic, <70ns)
//! - `debugger/license_info` - License tier/validation/expiry (T1 Atomic, <10ns)
//! - `debugger/get_comprehensive_audit` - Q34 compliance audit (<10us)
//!
//! ### Session Pool Tools (13-17)
//! - `debugger/allocate_session` - Allocate tiered session (<100ns)
//! - `debugger/release_session` - Release session (<100ns)
//! - `debugger/get_session_tier` - Get session tier (<10ns)
//! - `debugger/upgrade_session` - Upgrade to higher tier (<1μs)
//! - `debugger/get_pool_stats` - Pool statistics (<50ns)
//!
//! ### Memory Replay Tools (18-23)
//! - `debugger/enable_memory_replay` - Enable COW tracking (<10ms)
//! - `debugger/capture_memory_snapshot` - Capture snapshot (<50ms)
//! - `debugger/read_memory_at_snapshot` - Read at historical snapshot (<2ms)
//! - `debugger/navigate_to_snapshot` - Navigate snapshots (<100ns)
//! - `debugger/get_memory_replay_stats` - Replay statistics (<50ns)
//! - `debugger/verify_memory_integrity` - Q34 integrity check (O(n))

#![cfg_attr(not(feature = "std"), no_std)]

// ============================================================================
// Core MCP Server Modules
// ============================================================================

pub mod types;  // Common types and stubs
pub mod json_rpc;
pub mod rate_limiter;
pub mod tier_rate_limiter;  // Per-tier token bucket rate limiter (T1 Atomic, 512B)
pub mod quota_tracker;
pub mod config_loader;  // PID allowlist config file reader

pub mod tool_registry;
pub mod license_validator;
pub mod server;
pub mod tools;
pub mod auth_context;  // Authentication context for authenticated requests
pub mod auth_middleware;  // Phase 1 authentication middleware (basic validation)
pub mod account_lockout;  // Phase 3: Progressive account lockout (T1 Atomic, NIST 800-63B)
pub mod subscription_tier;  // Subscription tier enum (T0, #[repr(u8)] for atomic storage)
pub mod tier_enforcement;  // T1 Atomic tier-based feature/quota enforcement (64B)
pub mod snapshot_quota;  // T1 Atomic snapshot quota enforcement (256B)
pub mod session_tier_map;  // T1 Atomic session to tier mapping (64KB)
pub mod idempotency_cache;  // T1 Atomic request deduplication cache (16KB)
pub mod sse_session;  // T1 Atomic SSE session state management (256B)
pub mod session_channel_registry;  // T1 Atomic session channel registry (T6 Mixed orchestration)
pub mod daily_limit;  // T1 Atomic daily usage tracking (64B, Hobby tier step_backward limit)
pub mod monthly_quota;  // T1 Atomic monthly session tracking (128B, auto-reset at month boundary)
pub mod trial_state;  // T1 Atomic trial period tracking (128B, 7-day trial with all features)

#[cfg(feature = "configure")]
pub mod configure;  // T0 Auditable multi-source configuration resolution (4KB EnvResolutionCapsule)

#[cfg(feature = "session-persistence")]
pub mod session_persistence;  // T1 Atomic + T9 Persistent session persistence (capsule_cache integration)

// ============================================================================
// Client-Side Features (stdio->HTTP bridge, resilience)
// ============================================================================

#[cfg(feature = "client")]
pub mod client;  // T1+T3 client resilience capsules (metrics, retry, circuit breaker)

// ============================================================================
// OAuth 2.0 Authentication (CSRF + PKCE)
// ============================================================================

pub mod oauth;  // T1 Atomic OAuth state storage (4KB, CSRF/PKCE support)

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

#[cfg(feature = "hsm")]
pub mod hsm_capsule;

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

#[cfg(feature = "sse-transport")]
pub mod sse_connection_pool;

#[cfg(feature = "sse-transport")]
pub mod sse_transport;

#[cfg(feature = "streamable-http")]
pub mod streamable_http;

#[cfg(feature = "shared-state")]
pub mod shared_state;

#[cfg(feature = "tool-executor")]
pub mod tool_executor;

#[cfg(feature = "feature-flags")]
pub mod feature_flags;

#[cfg(feature = "ab-testing")]
pub mod ab_testing;

pub mod response_sanitizer;  // T1 Atomic response filtering (removes Chaos implementation details)

// ============================================================================
// Public Re-exports (Core)
// ============================================================================

pub use server::McpServerCapsule;
pub use json_rpc::JsonRpcCapsule;
pub use rate_limiter::RateLimiterCapsule;
pub use tier_rate_limiter::{
    TierRateLimiterCapsule, SubscriptionTier, RateLimitInfo,
    TierStats, RateLimiterStats as TierRateLimiterStats,
    NUM_TIERS,
};
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

// Account lockout types (Phase 3)
pub use account_lockout::{AccountLockoutCapsule, LockoutStats};

// Tier enforcement types (subscription_tier + tier_enforcement)
pub use subscription_tier::SubscriptionTier as TierLevel;  // Alias to avoid conflict with tier_rate_limiter::SubscriptionTier
pub use tier_enforcement::{
    TierEnforcementCapsule,
    TierEnforcementError,
    TierEnforcementStats,
    FeatureFlags,
};

// Snapshot quota types (snapshot_quota)
pub use snapshot_quota::{
    SnapshotQuotaCapsule,
    SubscriptionTierQuotaExt,
    EnforcementStage,
    QuotaError,
    PruneStats,
    SnapshotQuotaStatus,
};

// Session tier map types (session_tier_map)
pub use session_tier_map::{
    SessionTierMapCapsule,
    SESSION_TABLE_SLOTS,
};

// Idempotency cache types (idempotency_cache)
pub use idempotency_cache::{
    IdempotencyCacheCapsule,
    IdempotencyCacheStats,
    fnv1a_hash,
};

// SSE session types (sse_session)
pub use sse_session::{
    SseSessionCapsule,
    SessionState as SseSessionState,
    SessionError as SseSessionError,
    SessionSnapshot as SseSessionSnapshot,
    SOCKET_NOT_CONNECTED,
    DEFAULT_RATE_LIMIT_TOKENS,
};

// Session channel registry types (session_channel_registry)
pub use session_channel_registry::{
    SessionChannelRegistryCapsule,
    ChannelState,
    SseMessage,
    RegistryError,
    RegistryStats,
    MAX_CHANNELS as MAX_CHANNEL_SLOTS,
};

// Daily limit types (daily_limit) - Hobby tier step_backward enforcement
pub use daily_limit::{
    DailyLimitCapsule,
    DailyLimitResult,
    DailyLimitError,
    DailyLimitStats,
};

// Monthly quota types (monthly_quota) - Monthly session tracking with auto-reset
pub use monthly_quota::{
    MonthlyQuotaCapsule,
    SessionStartResult,
    MonthlyQuotaError,
    MonthlyQuotaStats,
    unix_to_month,
    next_month_start_unix,
};

// Session persistence types (session_persistence) - uses simple TCP client to capsule_cache
#[cfg(feature = "session-persistence")]
pub use session_persistence::{
    SessionPersistenceCapsule,
    ConnectionState,
    SessionMetadata,
    PersistenceError,
    PersistenceStats,
};

// OAuth types (oauth) - CSRF state and PKCE storage
pub use oauth::{
    OAuthStateCapsule,
    OAuthStateSlot,
    OAuthStateError,
    OAuthStateStats,
    StoredStateData,
    CodeChallengeMethod,
    fnv1a_hash as oauth_fnv1a_hash,
};

// Google OAuth client types (feature-gated)
#[cfg(feature = "google-oauth")]
pub use oauth::{
    GoogleOAuthClientCapsule,
    GoogleTokenResponse,
    GoogleUserInfo,
    IdTokenClaims,
    GoogleTokenError,
    GoogleOAuthError,
    OAuthMetrics,
    GOOGLE_AUTH_URL,
    GOOGLE_TOKEN_URL,
    GOOGLE_USERINFO_URL,
    GOOGLE_JWKS_URL,
    GOOGLE_SCOPES,
};

// OAuth user mapping types (oauth feature)
#[cfg(feature = "oauth")]
pub use oauth::{
    OAuthUserCapsule,
    OAuthUserError,
    OAuthUserStats,
    fnv1a_hash_oauth,
    USER_TABLE_SLOTS,
    AuthorizationCodeCapsule,
    AuthCodeError,
    AuthCodeStats,
    fnv1a_hash_code,
    sha256_to_fnv,
    generate_secure_code,
    CODE_TABLE_SLOTS,
    CODE_TTL_SECS,
};

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
    AnomalyStats as AnomalyDetectorStats,
    BehavioralFeatureVector,
    DetectionResult,
    // SOTA 2025 Heuristic Detection
    HeuristicDetectorCapsule,
    HeuristicResult,
    HeuristicStats,
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

#[cfg(feature = "hsm")]
pub use hsm_capsule::{
    HsmCapsule,
    HsmError as HsmCapsuleError,
    HsmResult,
    ED25519_PUBLIC_KEY_SIZE as HSM_CAPSULE_PUBLIC_KEY_SIZE,
    ED25519_SIGNATURE_SIZE,
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
pub use http_transport::HttpTransportCapsule;

#[cfg(feature = "tool-executor")]
pub use tool_executor::{ToolExecutorCapsule, ExecutionState};

#[cfg(feature = "feature-flags")]
pub use feature_flags::FeatureFlagsCapsule;

#[cfg(feature = "connection-pool")]
pub use connection_pool::ConnectionPoolCapsule;

#[cfg(feature = "sse-transport")]
pub use sse_connection_pool::{
    SseConnectionPoolCapsule,
    SseConnectionPoolHeader,
    SseConnectionSlot,
    SlotState,
    SsePoolError,
    MAX_CONNECTIONS as SSE_MAX_CONNECTIONS,
};

#[cfg(feature = "sse-transport")]
pub use sse_transport::{
    SseTransportCapsule,
    SseTransportConfig,
    TransportState,
    TransportError,
    TransportSnapshot,
    HttpResponse as SseHttpResponse,
    // SSE event formatting (MCP 2024-11-05 spec)
    format_sse_event,
    format_endpoint_event,
    format_message_event,
    format_ping_event,
    // HTTP response helpers
    build_sse_response_headers,
    build_204_response,
    build_error_response as build_sse_error_response,
    build_json_response,
    build_cors_preflight_response,
    // Header extraction helpers
    extract_api_key,
    extract_session_id,
    // Constants
    DEFAULT_MAX_CONNECTIONS as SSE_DEFAULT_MAX_CONNECTIONS,
    DEFAULT_HEARTBEAT_INTERVAL_MS,
    DEFAULT_CONNECTION_TIMEOUT_MS,
    DEFAULT_MESSAGE_QUEUE_SIZE,
    DEFAULT_PORT as SSE_DEFAULT_PORT,
};

#[cfg(feature = "streamable-http")]
pub use streamable_http::{
    StreamableHttpTransportCapsule,
    StreamableHttpError,
    McpResponse,
    McpHeaders,
    ResponseType,
    TransportState as StreamableHttpTransportState,
    // Constants
    DEFAULT_PORT as STREAMABLE_HTTP_DEFAULT_PORT,
    DEFAULT_MAX_BODY_SIZE,
    DEFAULT_REQUEST_TIMEOUT_MS,
    MCP_PROTOCOL_VERSION,
    MCP_PROTOCOL_VERSION_STR,
    FLAG_CORS_ENABLED,
    FLAG_STREAMING_ENABLED,
};

// OAuth 2.1 re-exports (all items exported via oauth::mod already)
#[cfg(feature = "oauth")]
pub use oauth::*;

// Note: ab_testing module doesn't define AbTestingCapsule yet
// #[cfg(feature = "ab-testing")]
// pub use ab_testing::AbTestingCapsule;

// ============================================================================
// Public Re-exports (Session Pool & Memory Replay - kdb integration)
// ============================================================================

// Session Pool types (from kdb::session_pool)
pub use kdb::session_pool::{
    SessionPoolCapsule, SessionTierType, SessionId as KdbSessionId,
    PoolConfig, PoolError, PoolStats,
    DEFAULT_LIGHT_CAPACITY, DEFAULT_MEDIUM_CAPACITY, DEFAULT_HEAVY_CAPACITY,
    DEFAULT_UPGRADE_LIGHT_TO_MEDIUM, DEFAULT_UPGRADE_MEDIUM_TO_HEAVY,
    DEFAULT_DOWNGRADE_IDLE_SECONDS,
};

// Memory Replay types (from kdb::memory_replay)
pub use kdb::memory_replay::{
    MemoryReplayCapsule, ReplayConfig, ReplayError, ReplayState, ReplayStats,
    MAX_TRACKED_PAGES, MAX_DELTAS_PER_SNAPSHOT,
};

// ============================================================================
// Public Re-exports (Configuration - T0 Auditable)
// ============================================================================

#[cfg(feature = "configure")]
pub use configure::{
    EnvResolutionCapsule,
    EnvSource,
    ResolvedVariable,
    EnvStats,
    EnvResolutionError,
};

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
