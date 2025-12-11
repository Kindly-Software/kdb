//! kdb Auto-Configuration System
//!
//! Universal MCP client auto-configuration for 90+ clients.
//!
//! ## Architecture
//! - **T6 Mixed Orchestrator**: Multi-stage pipeline
//! - **T1 Atomic Detectors**: Client detection capsules
//! - **T0 Auditable**: Config generation with Q34 audit trail
//!
//! ## Modules
//! - `platform`: Platform detection (OS, arch, paths)
//! - `env`: Environment variable resolution
//! - `detectors`: MCP client detectors (Phase 2+)
//! - `generators`: Config format generators (Phase 2+)
//!
//! ## Usage
//! ```bash
//! # Auto-configure all detected clients
//! kdb-configure --auto
//!
//! # Dry-run mode
//! kdb-configure --dry-run
//!
//! # Specific clients
//! kdb-configure --clients=claude_code,cursor
//! ```
//!
//! ## Tier Selection (UCE35 Q10)
//! - Platform detection: T1 Atomic (caching, lockfree)
//! - Environment resolution: T0 Auditable (deterministic, traceable)
//! - Client detection: T1 Atomic (fast lookup)
//! - Config generation: T0 Auditable (reproducible output)
//! - Orchestration: T6 Mixed (multi-stage pipeline)

pub mod platform;
pub mod env;
pub mod permission;
pub mod merger;
pub mod detectors;
pub mod generators;
pub mod orchestrator;
pub mod rollback;
pub mod audit;

// ============================================================================
// Re-exports for convenience
// ============================================================================

// Platform detection capsule and types
pub use platform::{
    // Core capsule
    PlatformDetectorCapsule,
    // Types
    PlatformInfo,
    Platform,
    Architecture,
    DetectionState,
    // Path utilities - auto-detecting (no Platform argument)
    get_config_dir,
    // Path utilities - platform-specific (take Platform argument)
    get_config_dir_for_platform,
    get_data_dir,
    get_cache_dir,
    get_system_config_dir,
    get_kdb_config_dir,
    get_kdb_data_dir,
    get_kdb_cache_dir,
    get_kdb_env_path,
    get_kdb_license_path,
    expand_path,
    expand_env_vars,
    set_secure_permissions,
    ensure_secure_dir,
};

// Environment resolution capsule and types
pub use env::{
    // Core capsule
    EnvResolutionCapsule,
    // Types
    EnvSource,
    ResolvedVariable,
    EnvStats,
    EnvResolutionError,
    // Utilities
    fnv1a_hash,
    is_secret_key,
    // Dotenv parser
    DotenvParserCapsule,
    ParsedEnvFile,
    ParseError,
    ErrorSeverity,
};

// Permission/consent management capsule and types
pub use permission::{
    // Core capsule
    PermissionGuardCapsule,
    // Types
    PermissionState,
    PermissionRequest,
    PermissionResponse,
    PermissionReason,
    PermissionStats,
};

// Config merger capsule and types
pub use merger::{
    // Core capsule
    ConfigMergerCapsule,
    // Types
    MergeState,
    MergeResult,
    MergeError,
    ConfigChange,
    KdbConfig,
    MergerStats,
};

// Detector registry capsule and types
pub use detectors::{
    // Core capsule
    DetectorRegistryCapsule,
    // Trait
    McpClientDetector,
    // Types
    ConfigFormat,
    TransportType,
    DetectionMethod,
    DetectedClient,
    DetectorEntry,
    DetectorHandle,
    RegistryStats as DetectorRegistryStats,
    DetectionResult,
    // Constants
    MAX_DETECTORS,
    // Utilities
    fnv1a_hash as detector_fnv1a_hash,
    // Built-in detectors
    ClaudeCodeDetector,
    ClaudeDesktopDetector,
    CursorDetector,
    VSCodeDetector,
    GenericHttpDetector,
    // Static detector instances
    CURSOR_DETECTOR,
    VSCODE_DETECTOR,
    GENERIC_HTTP_DETECTOR,
};

// Config generators (pure utilities, not capsules)
pub use generators::{
    // Core functions
    generate_stdio_config,
    generate_http_config,
    generate_mcp_config_file,
    // Merge utilities
    merge_kdb_into_config,
    merge_kdb_into_config_with_transport,
    // Constants
    KDB_MCP_BASE_URL,
    KDB_NPM_PACKAGE,
    LICENSE_KEY_ENV_VAR,
};

// Orchestrator capsule and types (T6 Mixed, 1024B)
pub use orchestrator::{
    // Core capsule
    AutoConfigOrchestratorCapsule,
    // Types
    OrchestratorState,
    ConfigOptions,
    ConfigReport,
    ConfigError,
    OrchestratorStats,
};

// Rollback capsule and types (T1 Atomic, 64B)
pub use rollback::{
    // Core capsule
    BackupManagerCapsule,
    // Session
    BackupSession,
    // Types
    Manifest,
    ClientBackup,
    BackupInfo,
    BackupState,
    BackupStats,
    RollbackResult,
    VerifyResult,
    RollbackError,
    // Constants
    MAX_BACKUP_COUNT,
    BACKUP_DIR_NAME,
    MANIFEST_FILENAME,
    CHECKSUMS_FILENAME,
    KDB_VERSION,
    // Utility
    sha256_hash,
};

// Audit logger capsule and types (T0+T1, 64B)
pub use audit::{
    // Core capsule
    AuditLoggerCapsule,
    // Types
    AuditEntry,
    AuditOperation,
    AuditStats,
    AuditError,
    // Constants
    AUDIT_DIR_NAME,
    MAX_ENTRIES_PER_FILE,
    MAX_LOG_FILES,
    GENESIS_HASH,
    // Utility
    fnv1a_hash as audit_fnv1a_hash,
    hash_with_prev,
};
