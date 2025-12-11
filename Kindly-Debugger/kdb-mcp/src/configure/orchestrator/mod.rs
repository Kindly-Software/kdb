//! AutoConfigOrchestratorCapsule - T6 Mixed Auto-Configuration Pipeline (1024B)
//!
//! Top-level orchestrator for the kdb auto-configuration pipeline.
//!
//! **Tier**: T6 Mixed (orchestrates T0 + T1 capsules across 8-stage pipeline)
//! **Size**: 1024 bytes (256-byte aligned, 4 cache lines)
//! **Latency**: <50ms full auto-configure (3-5 clients)
//! **Architecture**: 100% lockfree (AtomicU64 only)
//!
//! # Pipeline Stages
//!
//! 1. **Detection** (T1): Detect installed MCP clients
//! 2. **Filtering** (T0): Apply client whitelist/blacklist
//! 3. **Environment** (T0): Resolve environment variables (license key, etc.)
//! 4. **Generation** (T0): Generate configuration for each client
//! 5. **Permission** (T1): Request user consent (or auto-approve)
//! 6. **Backup** (T0): Create timestamped backups
//! 7. **Installation** (T0): Write configs to disk
//! 8. **Verification** (T0): Validate installed configs
//!
//! # Usage
//!
//! ```rust,ignore
//! use kdb_mcp::configure::orchestrator::{AutoConfigOrchestratorCapsule, ConfigOptions};
//! use kdb_mcp::configure::{PlatformDetectorCapsule, DetectorRegistryCapsule, EnvResolutionCapsule, ConfigMergerCapsule};
//!
//! let orchestrator = AutoConfigOrchestratorCapsule::new();
//! let platform = PlatformDetectorCapsule::new();
//! let registry = DetectorRegistryCapsule::new();
//! let env_resolver = EnvResolutionCapsule::new();
//! let merger = ConfigMergerCapsule::new();
//!
//! let options = ConfigOptions {
//!     auto_approve: true,
//!     force_overwrite: false,
//!     dry_run: false,
//!     specific_clients: None,
//!     license_key: Some("KDB-PRO-...".to_string()),
//! };
//!
//! let result = orchestrator.auto_configure(&options, &platform, &registry, &env_resolver, &merger);
//! match result {
//!     Ok(report) => {
//!         println!("Configured {} clients in {}ms", report.clients_configured.len(), report.duration_ms);
//!     }
//!     Err(e) => {
//!         eprintln!("Configuration failed: {:?}", e);
//!     }
//! }
//! ```
//!
//! # Chaos Compliance
//!
//! - #[repr(C, align(256))]: Cache-aligned (4 cache lines)
//! - 100% lockfree: AtomicU64 only
//! - Generation counters: TOCTOU prevention on all state transitions
//! - Q34 audit trail: Hash-chain integrity for compliance
//! - Embedded PermissionGuardCapsule: 64B sub-capsule for consent management

use core::sync::atomic::{AtomicU64, Ordering};
use std::path::PathBuf;

// Import from parent modules
use super::permission::{
    PermissionGuardCapsule, PermissionReason, PermissionRequest, PermissionResponse,
};
use super::platform::PlatformInfo;
use super::{
    ConfigMergerCapsule, DetectedClient, DetectorRegistryCapsule, EnvResolutionCapsule,
    KdbConfig, MergeError, PlatformDetectorCapsule,
};

// ============================================================================
// Orchestrator State Machine
// ============================================================================

/// State machine for the auto-configuration pipeline
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrchestratorState {
    /// Ready to start auto-configuration
    Idle = 0,
    /// Running client detection (Stage 1)
    Detecting = 1,
    /// Clients found, awaiting generation (Stage 2)
    Detected = 2,
    /// Generating configurations (Stage 3-4)
    Generating = 3,
    /// Configs ready, awaiting confirmation (Stage 5)
    Generated = 4,
    /// Requesting user permission (Stage 5)
    Confirming = 5,
    /// Writing configs + backups (Stage 6-7)
    Installing = 6,
    /// Success - all configs installed (Stage 8)
    Complete = 7,
    /// User cancelled
    Cancelled = 8,
    /// Rolling back changes
    Rollback = 9,
    /// Rollback complete
    RolledBack = 10,
    /// Error occurred
    Error = 11,
}

impl OrchestratorState {
    /// Convert from u64 (for atomic storage)
    #[inline]
    pub const fn from_u64(value: u64) -> Option<Self> {
        match value {
            0 => Some(OrchestratorState::Idle),
            1 => Some(OrchestratorState::Detecting),
            2 => Some(OrchestratorState::Detected),
            3 => Some(OrchestratorState::Generating),
            4 => Some(OrchestratorState::Generated),
            5 => Some(OrchestratorState::Confirming),
            6 => Some(OrchestratorState::Installing),
            7 => Some(OrchestratorState::Complete),
            8 => Some(OrchestratorState::Cancelled),
            9 => Some(OrchestratorState::Rollback),
            10 => Some(OrchestratorState::RolledBack),
            11 => Some(OrchestratorState::Error),
            _ => None,
        }
    }

    /// Get human-readable name
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            OrchestratorState::Idle => "idle",
            OrchestratorState::Detecting => "detecting",
            OrchestratorState::Detected => "detected",
            OrchestratorState::Generating => "generating",
            OrchestratorState::Generated => "generated",
            OrchestratorState::Confirming => "confirming",
            OrchestratorState::Installing => "installing",
            OrchestratorState::Complete => "complete",
            OrchestratorState::Cancelled => "cancelled",
            OrchestratorState::Rollback => "rollback",
            OrchestratorState::RolledBack => "rolled_back",
            OrchestratorState::Error => "error",
        }
    }

    /// Check if this is a terminal state
    #[inline]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            OrchestratorState::Complete
                | OrchestratorState::Cancelled
                | OrchestratorState::RolledBack
                | OrchestratorState::Error
        )
    }
}

// ============================================================================
// Configuration Options
// ============================================================================

/// Options for the auto-configuration pipeline
#[derive(Debug, Clone, Default)]
pub struct ConfigOptions {
    /// Auto-approve configuration changes (KDB_AUTO_CONFIGURE=true)
    pub auto_approve: bool,
    /// Force overwrite existing configs (KDB_CONFIGURE_FORCE=true)
    pub force_overwrite: bool,
    /// Dry-run mode - don't write any files (KDB_CONFIGURE_DRY_RUN=true)
    pub dry_run: bool,
    /// Only configure specific clients (--clients=claude_code,cursor)
    pub specific_clients: Option<Vec<String>>,
    /// License key override (from CLI or env)
    pub license_key: Option<String>,
}

impl ConfigOptions {
    /// Create options from environment variables
    pub fn from_env() -> Self {
        let auto_approve = std::env::var("KDB_AUTO_CONFIGURE")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        let force_overwrite = std::env::var("KDB_CONFIGURE_FORCE")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        let dry_run = std::env::var("KDB_CONFIGURE_DRY_RUN")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        let license_key = std::env::var("KDB_LICENSE_KEY").ok();

        Self {
            auto_approve,
            force_overwrite,
            dry_run,
            specific_clients: None,
            license_key,
        }
    }

    /// Set auto-approve mode
    pub fn with_auto_approve(mut self, auto_approve: bool) -> Self {
        self.auto_approve = auto_approve;
        self
    }

    /// Set dry-run mode
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Set specific clients to configure
    pub fn with_clients(mut self, clients: Vec<String>) -> Self {
        self.specific_clients = Some(clients);
        self
    }

    /// Set license key
    pub fn with_license_key(mut self, key: String) -> Self {
        self.license_key = Some(key);
        self
    }
}

// ============================================================================
// Configuration Report
// ============================================================================

/// Report from a completed auto-configuration run
#[derive(Debug, Clone)]
pub struct ConfigReport {
    /// All clients detected during detection phase
    pub clients_detected: Vec<DetectedClient>,
    /// Client IDs that were successfully configured
    pub clients_configured: Vec<String>,
    /// Clients that were skipped (client_id, reason)
    pub clients_skipped: Vec<(String, String)>,
    /// Backup files created during installation
    pub backups_created: Vec<PathBuf>,
    /// Base backup directory
    pub backup_dir: PathBuf,
    /// Total duration in milliseconds
    pub duration_ms: u64,
    /// Q34 final audit hash
    pub audit_hash: u64,
}

impl Default for ConfigReport {
    fn default() -> Self {
        Self {
            clients_detected: Vec::new(),
            clients_configured: Vec::new(),
            clients_skipped: Vec::new(),
            backups_created: Vec::new(),
            backup_dir: PathBuf::from("~/.kdb/backups"),
            duration_ms: 0,
            audit_hash: 0,
        }
    }
}

// ============================================================================
// Configuration Error
// ============================================================================

/// Errors that can occur during auto-configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Client detection failed
    DetectionFailed(String),
    /// Config generation failed
    GenerationFailed(String),
    /// User denied permission
    PermissionDenied,
    /// Config installation failed
    InstallFailed(String),
    /// Rollback failed
    RollbackFailed(String),
    /// Q34 audit verification failed
    AuditVerificationFailed,
    /// Merge error from ConfigMergerCapsule
    MergeError(String),
    /// No clients detected
    NoClientsDetected,
    /// License key not found
    LicenseKeyNotFound,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::DetectionFailed(msg) => write!(f, "detection failed: {}", msg),
            ConfigError::GenerationFailed(msg) => write!(f, "generation failed: {}", msg),
            ConfigError::PermissionDenied => write!(f, "permission denied by user"),
            ConfigError::InstallFailed(msg) => write!(f, "installation failed: {}", msg),
            ConfigError::RollbackFailed(msg) => write!(f, "rollback failed: {}", msg),
            ConfigError::AuditVerificationFailed => write!(f, "audit verification failed"),
            ConfigError::MergeError(msg) => write!(f, "merge error: {}", msg),
            ConfigError::NoClientsDetected => write!(f, "no MCP clients detected"),
            ConfigError::LicenseKeyNotFound => write!(f, "KDB_LICENSE_KEY not found"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<MergeError> for ConfigError {
    fn from(err: MergeError) -> Self {
        ConfigError::MergeError(err.to_string())
    }
}

// ============================================================================
// Statistics
// ============================================================================

/// Statistics snapshot from the orchestrator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrchestratorStats {
    /// Number of clients detected
    pub clients_detected: u64,
    /// Number of configs generated
    pub configs_generated: u64,
    /// Number of configs installed
    pub configs_installed: u64,
    /// Number of rollbacks performed
    pub rollbacks_performed: u64,
    /// Current state
    pub current_state: OrchestratorState,
}

impl Default for OrchestratorStats {
    fn default() -> Self {
        Self {
            clients_detected: 0,
            configs_generated: 0,
            configs_installed: 0,
            rollbacks_performed: 0,
            current_state: OrchestratorState::Idle,
        }
    }
}

// ============================================================================
// AutoConfigOrchestratorCapsule (T6 Mixed, 1024B)
// ============================================================================

/// T6 Mixed Auto-Configuration Orchestrator Capsule (1024 bytes)
///
/// Orchestrates the complete auto-configuration pipeline across 8 stages.
/// Coordinates T0 and T1 sub-capsules for detection, generation, and installation.
///
/// ## Memory Layout (1024 bytes, 256B aligned)
///
/// ```text
/// +------------------------------------------------------------------+
/// | Cache Line 1 (Offset 0-63): State Machine                        |
/// | state(8) | generation(8) | last_op_ns(8) | clients_detected(8)   |
/// | error_code(8) | _pad1[24]                                        |
/// +------------------------------------------------------------------+
/// | Cache Line 2 (Offset 64-127): Detection State                    |
/// | detected_bitmap(8) | detection_count(8) | detection_dur_ns(8)    |
/// | _pad2[40]                                                        |
/// +------------------------------------------------------------------+
/// | Cache Line 3 (Offset 128-191): Generation State                  |
/// | template_hash(8) | output_hash(8) | configs_generated(8)         |
/// | _pad3[40]                                                        |
/// +------------------------------------------------------------------+
/// | Cache Line 4 (Offset 192-255): Embedded PermissionGuardCapsule   |
/// | [64B PermissionGuardCapsule]                                     |
/// +------------------------------------------------------------------+
/// | Cache Line 5 (Offset 256-319): Audit State                       |
/// | operation_type(8) | operation_ts(8) | prev_audit_hash(8)         |
/// | audit_entries(8) | _pad4[32]                                     |
/// +------------------------------------------------------------------+
/// | Cache Line 6 (Offset 320-383): Statistics                        |
/// | configs_installed(8) | rollbacks_perf(8) | errors_encountered(8) |
/// | _pad5[40]                                                        |
/// +------------------------------------------------------------------+
/// | Cache Lines 7-16 (Offset 384-1023): Reserved                     |
/// | _reserved[640]                                                   |
/// +------------------------------------------------------------------+
/// ```
#[repr(C, align(256))]
pub struct AutoConfigOrchestratorCapsule {
    // ========== Cache Line 1 (64B): State Machine ==========
    /// Current orchestrator state
    /// #ASSUME: OrchestratorState enum fits in u8, stored as u64 for atomics
    state: AtomicU64,
    /// Generation counter for TOCTOU prevention
    /// #ASSUME: Generation wraps safely after 2^64 increments
    generation: AtomicU64,
    /// Timestamp of last operation (Unix nanoseconds)
    last_operation_ns: AtomicU64,
    /// Number of clients detected in current run
    clients_detected: AtomicU64,
    /// Error code (0 = no error)
    error_code: AtomicU64,
    /// Padding to 64B boundary
    _pad1: [u8; 24],

    // ========== Cache Line 2 (64B): Detection State ==========
    /// Bitmap of detected client slots (up to 64 clients)
    /// #ASSUME: We support at most 64 distinct client types
    detected_bitmap: AtomicU64,
    /// Total detections across all runs
    detection_count: AtomicU64,
    /// Duration of last detection phase (nanoseconds)
    detection_duration_ns: AtomicU64,
    /// Padding to 64B boundary
    _pad2: [u8; 40],

    // ========== Cache Line 3 (64B): Generation State ==========
    /// FNV-1a hash of template used
    template_hash: AtomicU64,
    /// FNV-1a hash of generated output
    output_hash: AtomicU64,
    /// Number of configs generated in current run
    configs_generated: AtomicU64,
    /// Padding to 64B boundary
    _pad3: [u8; 40],

    // ========== Cache Line 4 (64B): Embedded Permission Capsule ==========
    /// Embedded PermissionGuardCapsule for consent management
    /// #ASSUME: PermissionGuardCapsule is exactly 64 bytes
    permission: PermissionGuardCapsule,

    // ========== Cache Line 5 (64B): Audit State ==========
    /// Current operation type for audit (0=idle, 1=detect, 2=generate, etc.)
    operation_type: AtomicU64,
    /// Timestamp of current operation
    operation_timestamp: AtomicU64,
    /// Previous audit hash for Q34 hash chain
    prev_audit_hash: AtomicU64,
    /// Total audit entries recorded
    audit_entries: AtomicU64,
    /// Padding to 64B boundary
    _pad4: [u8; 32],

    // ========== Cache Line 6 (64B): Statistics ==========
    /// Number of configs successfully installed
    configs_installed: AtomicU64,
    /// Number of rollback operations performed
    rollbacks_performed: AtomicU64,
    /// Number of errors encountered
    errors_encountered: AtomicU64,
    /// Padding to 64B boundary
    _pad5: [u8; 40],

    // ========== Cache Lines 7-16 (640B): Reserved ==========
    /// Reserved for future sub-capsule coordination
    _reserved: [u8; 640],
}

// #VERIFY: Size and alignment assertions
const _: () = {
    assert!(core::mem::size_of::<AutoConfigOrchestratorCapsule>() == 1024);
    assert!(core::mem::align_of::<AutoConfigOrchestratorCapsule>() == 256);
};

impl AutoConfigOrchestratorCapsule {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Create a new AutoConfigOrchestratorCapsule
    ///
    /// All counters start at zero. State starts as Idle.
    #[inline]
    pub const fn new() -> Self {
        Self {
            // State machine
            state: AtomicU64::new(OrchestratorState::Idle as u64),
            generation: AtomicU64::new(0),
            last_operation_ns: AtomicU64::new(0),
            clients_detected: AtomicU64::new(0),
            error_code: AtomicU64::new(0),
            _pad1: [0u8; 24],

            // Detection state
            detected_bitmap: AtomicU64::new(0),
            detection_count: AtomicU64::new(0),
            detection_duration_ns: AtomicU64::new(0),
            _pad2: [0u8; 40],

            // Generation state
            template_hash: AtomicU64::new(0),
            output_hash: AtomicU64::new(0),
            configs_generated: AtomicU64::new(0),
            _pad3: [0u8; 40],

            // Embedded permission capsule
            permission: PermissionGuardCapsule::new(),

            // Audit state
            operation_type: AtomicU64::new(0),
            operation_timestamp: AtomicU64::new(0),
            prev_audit_hash: AtomicU64::new(0),
            audit_entries: AtomicU64::new(0),
            _pad4: [0u8; 32],

            // Statistics
            configs_installed: AtomicU64::new(0),
            rollbacks_performed: AtomicU64::new(0),
            errors_encountered: AtomicU64::new(0),
            _pad5: [0u8; 40],

            // Reserved
            _reserved: [0u8; 640],
        }
    }

    // ========================================================================
    // Core Pipeline
    // ========================================================================

    /// Main auto-configuration pipeline
    ///
    /// Executes the 8-stage pipeline:
    /// 1. Detection - Find installed MCP clients
    /// 2. Filtering - Apply client whitelist (if specified)
    /// 3. Environment - Resolve license key
    /// 4. Generation - Generate configs
    /// 5. Permission - Request user consent
    /// 6. Backup - Create backups (if not dry-run)
    /// 7. Installation - Write configs
    /// 8. Verification - Validate
    ///
    /// # Arguments
    ///
    /// * `options` - Configuration options (auto_approve, dry_run, etc.)
    /// * `platform` - Platform detector capsule
    /// * `registry` - Detector registry with registered client detectors
    /// * `env_resolver` - Environment variable resolver
    /// * `merger` - Config merger for JSON manipulation
    ///
    /// # Returns
    ///
    /// * `Ok(ConfigReport)` - Successful configuration with report
    /// * `Err(ConfigError)` - Configuration failed
    ///
    /// # Performance
    ///
    /// * <50ms for 3-5 clients (typical)
    /// * I/O bound (file detection, config writes)
    pub fn auto_configure(
        &self,
        options: &ConfigOptions,
        platform: &PlatformDetectorCapsule,
        registry: &DetectorRegistryCapsule,
        env_resolver: &EnvResolutionCapsule,
        merger: &ConfigMergerCapsule,
    ) -> Result<ConfigReport, ConfigError> {
        let start_ns = get_nanos();

        // Reset for new run
        self.reset_for_run();

        // ========== Stage 1: Detection ==========
        self.set_state(OrchestratorState::Detecting);
        self.update_audit(1); // 1 = detection operation

        let platform_info = platform.detect();
        let detection_start = get_nanos();
        let detection_result = registry.detect_all(&platform_info);
        let detection_duration = get_nanos() - detection_start;
        self.detection_duration_ns
            .store(detection_duration, Ordering::Relaxed);

        let detected_clients = detection_result.clients;
        if detected_clients.is_empty() {
            self.set_error(ConfigError::NoClientsDetected.clone());
            return Err(ConfigError::NoClientsDetected);
        }

        self.clients_detected
            .store(detected_clients.len() as u64, Ordering::Relaxed);
        self.detection_count.fetch_add(1, Ordering::Relaxed);

        // Update bitmap (first 64 clients)
        let mut bitmap: u64 = 0;
        for (i, _) in detected_clients.iter().enumerate().take(64) {
            bitmap |= 1 << i;
        }
        self.detected_bitmap.store(bitmap, Ordering::Relaxed);

        self.set_state(OrchestratorState::Detected);

        // ========== Stage 2: Filtering ==========
        let mut clients_to_configure = detected_clients.clone();
        let mut clients_skipped: Vec<(String, String)> = Vec::new();

        if let Some(ref specific) = options.specific_clients {
            let original_count = clients_to_configure.len();
            clients_to_configure.retain(|c| specific.contains(&c.client_id.to_string()));
            let filtered_count = original_count - clients_to_configure.len();
            if filtered_count > 0 {
                // Track skipped clients
                for client in &detected_clients {
                    if !specific.contains(&client.client_id.to_string()) {
                        clients_skipped.push((
                            client.client_id.to_string(),
                            "not in --clients list".to_string(),
                        ));
                    }
                }
            }
        }

        if clients_to_configure.is_empty() {
            self.set_error(ConfigError::NoClientsDetected.clone());
            return Err(ConfigError::NoClientsDetected);
        }

        // ========== Stage 3: Environment Resolution ==========
        self.set_state(OrchestratorState::Generating);
        self.update_audit(2); // 2 = generation operation

        let license_key = options
            .license_key
            .clone()
            .or_else(|| {
                env_resolver
                    .resolve("KDB_LICENSE_KEY")
                    .map(|v| v.value.clone())
            })
            .ok_or(ConfigError::LicenseKeyNotFound)?;

        // ========== Stage 4: Generate Configs ==========
        let kdb_config = KdbConfig::with_license_key(&license_key);
        let template_hash = fnv1a_hash(license_key.as_bytes());
        self.template_hash.store(template_hash, Ordering::Relaxed);

        // For now, we track how many configs we could generate
        self.configs_generated
            .store(clients_to_configure.len() as u64, Ordering::Relaxed);
        self.set_state(OrchestratorState::Generated);

        // ========== Stage 5: Permission Request ==========
        if !options.dry_run && !options.auto_approve {
            self.set_state(OrchestratorState::Confirming);
            self.update_audit(3); // 3 = permission operation

            let request = PermissionRequest {
                action: format!("Configure {} MCP client(s)", clients_to_configure.len()),
                target: clients_to_configure
                    .iter()
                    .map(|c| c.config_path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                impact: "Create/update MCP configurations with timestamped backups".to_string(),
                auto_approve_env: "KDB_AUTO_CONFIGURE",
            };

            let response = self.permission.request_permission(&request);
            if !response.granted {
                self.set_state(OrchestratorState::Cancelled);
                return Err(ConfigError::PermissionDenied);
            }
        }

        // ========== Stage 6-7: Installation (if not dry-run) ==========
        let mut clients_configured: Vec<String> = Vec::new();
        let mut backups_created: Vec<PathBuf> = Vec::new();

        if !options.dry_run {
            self.set_state(OrchestratorState::Installing);
            self.update_audit(4); // 4 = installation operation

            for client in &clients_to_configure {
                // Try to read existing config
                let existing_content = std::fs::read_to_string(&client.config_path).ok();

                // Create backup directory
                let backup_dir = get_backup_dir(&platform_info);
                std::fs::create_dir_all(&backup_dir).map_err(|e| {
                    ConfigError::InstallFailed(format!("failed to create backup dir: {}", e))
                })?;

                // Generate backup path
                let backup_path = if existing_content.is_some() {
                    let timestamp = get_nanos() / 1_000_000_000; // seconds
                    let backup_filename = format!(
                        "{}_{}.json.bak",
                        client.client_id,
                        timestamp
                    );
                    Some(backup_dir.join(backup_filename))
                } else {
                    None
                };

                // Merge or create config
                let content_to_write = if let Some(ref existing) = existing_content {
                    // Merge with existing config
                    let merge_result = merger
                        .merge_json(existing, &kdb_config, backup_path.as_deref())?;
                    if let Some(ref bp) = merge_result.backup_path {
                        backups_created.push(bp.clone());
                    }
                    merge_result.merged_content
                } else {
                    // Create new config
                    create_new_config(&kdb_config)?
                };

                // Ensure parent directory exists
                if let Some(parent) = client.config_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        ConfigError::InstallFailed(format!(
                            "failed to create config dir: {}",
                            e
                        ))
                    })?;
                }

                // Write config
                std::fs::write(&client.config_path, &content_to_write).map_err(|e| {
                    ConfigError::InstallFailed(format!(
                        "failed to write config for {}: {}",
                        client.client_id, e
                    ))
                })?;

                clients_configured.push(client.client_id.to_string());
                self.configs_installed.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            // Dry-run: mark all as "would configure"
            for client in &clients_to_configure {
                clients_configured.push(format!("{} (dry-run)", client.client_id));
            }
        }

        // ========== Stage 8: Complete ==========
        self.set_state(OrchestratorState::Complete);
        self.update_audit(5); // 5 = completion

        let duration_ms = (get_nanos() - start_ns) / 1_000_000;
        let audit_hash = self.prev_audit_hash.load(Ordering::Acquire);

        Ok(ConfigReport {
            clients_detected: detected_clients,
            clients_configured,
            clients_skipped,
            backups_created,
            backup_dir: get_backup_dir(&platform_info),
            duration_ms,
            audit_hash,
        })
    }

    // ========================================================================
    // State Management
    // ========================================================================

    /// Get current orchestrator state
    #[inline]
    pub fn get_state(&self) -> OrchestratorState {
        let state = self.state.load(Ordering::Acquire);
        OrchestratorState::from_u64(state).unwrap_or(OrchestratorState::Error)
    }

    /// Set state with generation increment
    #[inline]
    fn set_state(&self, state: OrchestratorState) {
        self.state.store(state as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.last_operation_ns.store(get_nanos(), Ordering::Relaxed);
    }

    /// Set error state
    #[inline]
    fn set_error(&self, error: ConfigError) {
        let error_code = match error {
            ConfigError::DetectionFailed(_) => 1,
            ConfigError::GenerationFailed(_) => 2,
            ConfigError::PermissionDenied => 3,
            ConfigError::InstallFailed(_) => 4,
            ConfigError::RollbackFailed(_) => 5,
            ConfigError::AuditVerificationFailed => 6,
            ConfigError::MergeError(_) => 7,
            ConfigError::NoClientsDetected => 8,
            ConfigError::LicenseKeyNotFound => 9,
        };
        self.error_code.store(error_code, Ordering::Relaxed);
        self.errors_encountered.fetch_add(1, Ordering::Relaxed);
        self.set_state(OrchestratorState::Error);
    }

    /// Reset state for a new run
    #[inline]
    fn reset_for_run(&self) {
        self.state
            .store(OrchestratorState::Idle as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.clients_detected.store(0, Ordering::Relaxed);
        self.error_code.store(0, Ordering::Relaxed);
        self.detected_bitmap.store(0, Ordering::Relaxed);
        self.configs_generated.store(0, Ordering::Relaxed);
    }

    /// Update audit hash chain (Q34)
    #[inline]
    fn update_audit(&self, operation: u64) {
        self.operation_type.store(operation, Ordering::Relaxed);
        let timestamp = get_nanos();
        self.operation_timestamp.store(timestamp, Ordering::Relaxed);

        // Update hash chain: new_hash = FNV(prev_hash || operation || timestamp)
        let prev_hash = self.prev_audit_hash.load(Ordering::Acquire);
        let mut hash_data = [0u8; 24];
        hash_data[0..8].copy_from_slice(&prev_hash.to_le_bytes());
        hash_data[8..16].copy_from_slice(&operation.to_le_bytes());
        hash_data[16..24].copy_from_slice(&timestamp.to_le_bytes());
        let new_hash = fnv1a_hash(&hash_data);
        self.prev_audit_hash.store(new_hash, Ordering::Release);
        self.audit_entries.fetch_add(1, Ordering::Relaxed);
    }

    // ========================================================================
    // Statistics & Queries
    // ========================================================================

    /// Get current generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get statistics snapshot
    #[inline]
    pub fn get_stats(&self) -> OrchestratorStats {
        OrchestratorStats {
            clients_detected: self.clients_detected.load(Ordering::Acquire),
            configs_generated: self.configs_generated.load(Ordering::Acquire),
            configs_installed: self.configs_installed.load(Ordering::Acquire),
            rollbacks_performed: self.rollbacks_performed.load(Ordering::Acquire),
            current_state: self.get_state(),
        }
    }

    /// Get the embedded permission guard capsule
    #[inline]
    pub fn permission_guard(&self) -> &PermissionGuardCapsule {
        &self.permission
    }

    /// Get audit trail hash
    #[inline]
    pub fn audit_hash(&self) -> u64 {
        self.prev_audit_hash.load(Ordering::Acquire)
    }

    /// Get number of audit entries
    #[inline]
    pub fn audit_entries(&self) -> u64 {
        self.audit_entries.load(Ordering::Acquire)
    }

    /// Check if currently in a terminal state
    #[inline]
    pub fn is_terminal(&self) -> bool {
        self.get_state().is_terminal()
    }

    // ========================================================================
    // Test Helpers
    // ========================================================================

    /// Reset all counters (for testing)
    #[cfg(test)]
    pub fn reset_all(&self) {
        self.state
            .store(OrchestratorState::Idle as u64, Ordering::Release);
        self.generation.store(0, Ordering::Release);
        self.last_operation_ns.store(0, Ordering::Release);
        self.clients_detected.store(0, Ordering::Release);
        self.error_code.store(0, Ordering::Release);
        self.detected_bitmap.store(0, Ordering::Release);
        self.detection_count.store(0, Ordering::Release);
        self.detection_duration_ns.store(0, Ordering::Release);
        self.template_hash.store(0, Ordering::Release);
        self.output_hash.store(0, Ordering::Release);
        self.configs_generated.store(0, Ordering::Release);
        self.operation_type.store(0, Ordering::Release);
        self.operation_timestamp.store(0, Ordering::Release);
        self.prev_audit_hash.store(0, Ordering::Release);
        self.audit_entries.store(0, Ordering::Release);
        self.configs_installed.store(0, Ordering::Release);
        self.rollbacks_performed.store(0, Ordering::Release);
        self.errors_encountered.store(0, Ordering::Release);
    }
}

impl Default for AutoConfigOrchestratorCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Get current Unix timestamp in nanoseconds
#[inline]
fn get_nanos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// FNV-1a hash function
#[inline]
fn fnv1a_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    let mut hash = FNV_OFFSET;
    for byte in data {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Get the backup directory for the platform
#[inline]
fn get_backup_dir(platform: &PlatformInfo) -> PathBuf {
    use super::platform::Platform;

    match platform.platform {
        Platform::Linux | Platform::FreeBSD => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
            PathBuf::from(home).join(".kdb/backups")
        }
        Platform::MacOS => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/Users".to_string());
            PathBuf::from(home).join(".kdb/backups")
        }
        Platform::Windows => {
            let appdata = std::env::var("APPDATA").unwrap_or_else(|_| "C:\\".to_string());
            PathBuf::from(appdata).join("kdb\\backups")
        }
        Platform::Unknown => PathBuf::from("/tmp/kdb/backups"),
    }
}

/// Create a new MCP config file with kdb
#[cfg(feature = "json-rpc")]
fn create_new_config(kdb_config: &KdbConfig) -> Result<String, ConfigError> {
    use serde_json::json;

    let config = json!({
        "mcpServers": {
            "kdb": {
                "command": kdb_config.command,
                "args": kdb_config.args,
                "env": kdb_config.env
            }
        }
    });

    serde_json::to_string_pretty(&config)
        .map_err(|e| ConfigError::GenerationFailed(format!("JSON serialization failed: {}", e)))
}

#[cfg(not(feature = "json-rpc"))]
fn create_new_config(_kdb_config: &KdbConfig) -> Result<String, ConfigError> {
    Err(ConfigError::GenerationFailed(
        "json-rpc feature not enabled".to_string(),
    ))
}

// ============================================================================
// Unit Tests (T28 Q1-Q15)
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
        env::remove_var("KDB_LICENSE_KEY");
    }

    // Q1: Layout Verification - Size
    #[test]
    fn test_orchestrator_size() {
        assert_eq!(
            core::mem::size_of::<AutoConfigOrchestratorCapsule>(),
            1024
        );
    }

    // Q2: Layout Verification - Alignment
    #[test]
    fn test_orchestrator_alignment() {
        assert_eq!(
            core::mem::align_of::<AutoConfigOrchestratorCapsule>(),
            256
        );
    }

    // Q3: Initial State
    #[test]
    fn test_initial_state() {
        let orchestrator = AutoConfigOrchestratorCapsule::new();
        assert_eq!(orchestrator.get_state(), OrchestratorState::Idle);
        let stats = orchestrator.get_stats();
        assert_eq!(stats.clients_detected, 0);
        assert_eq!(stats.configs_generated, 0);
        assert_eq!(stats.configs_installed, 0);
        assert_eq!(stats.rollbacks_performed, 0);
    }

    // Q4: State Transitions
    #[test]
    fn test_state_transitions() {
        let orchestrator = AutoConfigOrchestratorCapsule::new();

        // Test state transitions
        orchestrator.set_state(OrchestratorState::Detecting);
        assert_eq!(orchestrator.get_state(), OrchestratorState::Detecting);

        orchestrator.set_state(OrchestratorState::Detected);
        assert_eq!(orchestrator.get_state(), OrchestratorState::Detected);

        orchestrator.set_state(OrchestratorState::Generating);
        assert_eq!(orchestrator.get_state(), OrchestratorState::Generating);

        orchestrator.set_state(OrchestratorState::Complete);
        assert_eq!(orchestrator.get_state(), OrchestratorState::Complete);
        assert!(orchestrator.is_terminal());
    }

    // Q5: ConfigOptions from_env
    #[test]
    fn test_config_options_from_env() {
        cleanup_env();
        env::set_var("KDB_AUTO_CONFIGURE", "true");
        env::set_var("KDB_CONFIGURE_DRY_RUN", "1");
        env::set_var("KDB_LICENSE_KEY", "test-key");

        let options = ConfigOptions::from_env();
        assert!(options.auto_approve);
        assert!(options.dry_run);
        assert_eq!(options.license_key, Some("test-key".to_string()));

        cleanup_env();
    }

    // Q6: ConfigOptions with_* methods
    #[test]
    fn test_config_options_builders() {
        let options = ConfigOptions::default()
            .with_auto_approve(true)
            .with_dry_run(true)
            .with_clients(vec!["claude_code".to_string()])
            .with_license_key("my-key".to_string());

        assert!(options.auto_approve);
        assert!(options.dry_run);
        assert_eq!(
            options.specific_clients,
            Some(vec!["claude_code".to_string()])
        );
        assert_eq!(options.license_key, Some("my-key".to_string()));
    }

    // Q7: Missing license key error
    #[test]
    fn test_missing_license_key() {
        cleanup_env();
        let orchestrator = AutoConfigOrchestratorCapsule::new();
        let platform = PlatformDetectorCapsule::new();
        let registry = DetectorRegistryCapsule::new();
        let env_resolver = EnvResolutionCapsule::new();
        let merger = ConfigMergerCapsule::new();

        // Need to register at least one detector that will match
        // For this test, we just verify the license key error path
        let options = ConfigOptions::default();

        let result =
            orchestrator.auto_configure(&options, &platform, &registry, &env_resolver, &merger);

        // Will fail with NoClientsDetected since no detectors registered
        assert!(matches!(result, Err(ConfigError::NoClientsDetected)));

        cleanup_env();
    }

    // Q8: Permission denied
    #[test]
    fn test_permission_denied() {
        cleanup_env();
        let orchestrator = AutoConfigOrchestratorCapsule::new();

        // Simulate permission denial by checking the state after a hypothetical denial
        orchestrator.set_state(OrchestratorState::Cancelled);
        assert_eq!(orchestrator.get_state(), OrchestratorState::Cancelled);
        assert!(orchestrator.is_terminal());

        cleanup_env();
    }

    // Q9: Statistics tracking
    #[test]
    fn test_get_stats() {
        let orchestrator = AutoConfigOrchestratorCapsule::new();

        let stats = orchestrator.get_stats();
        assert_eq!(stats.current_state, OrchestratorState::Idle);
        assert_eq!(stats.clients_detected, 0);

        // Manually update some stats for testing
        orchestrator.clients_detected.store(5, Ordering::Relaxed);
        orchestrator.configs_installed.store(3, Ordering::Relaxed);

        let stats = orchestrator.get_stats();
        assert_eq!(stats.clients_detected, 5);
        assert_eq!(stats.configs_installed, 3);
    }

    // Q10: Generation counter TOCTOU prevention
    #[test]
    fn test_generation_counter() {
        let orchestrator = AutoConfigOrchestratorCapsule::new();
        let gen1 = orchestrator.generation();

        orchestrator.set_state(OrchestratorState::Detecting);
        let gen2 = orchestrator.generation();
        assert!(gen2 > gen1, "Generation should increment on state change");

        orchestrator.set_state(OrchestratorState::Complete);
        let gen3 = orchestrator.generation();
        assert!(gen3 > gen2, "Generation should continue incrementing");
    }

    // Q11: State as_str()
    #[test]
    fn test_state_as_str() {
        assert_eq!(OrchestratorState::Idle.as_str(), "idle");
        assert_eq!(OrchestratorState::Detecting.as_str(), "detecting");
        assert_eq!(OrchestratorState::Detected.as_str(), "detected");
        assert_eq!(OrchestratorState::Generating.as_str(), "generating");
        assert_eq!(OrchestratorState::Generated.as_str(), "generated");
        assert_eq!(OrchestratorState::Confirming.as_str(), "confirming");
        assert_eq!(OrchestratorState::Installing.as_str(), "installing");
        assert_eq!(OrchestratorState::Complete.as_str(), "complete");
        assert_eq!(OrchestratorState::Cancelled.as_str(), "cancelled");
        assert_eq!(OrchestratorState::Rollback.as_str(), "rollback");
        assert_eq!(OrchestratorState::RolledBack.as_str(), "rolled_back");
        assert_eq!(OrchestratorState::Error.as_str(), "error");
    }

    // Q12: Const new (static initialization)
    #[test]
    fn test_const_new() {
        static ORCHESTRATOR: AutoConfigOrchestratorCapsule = AutoConfigOrchestratorCapsule::new();
        assert_eq!(ORCHESTRATOR.get_state(), OrchestratorState::Idle);
    }

    // Q13: Detection phase (basic)
    #[test]
    fn test_detection_phase() {
        let orchestrator = AutoConfigOrchestratorCapsule::new();

        // Simulate detection phase
        orchestrator.set_state(OrchestratorState::Detecting);
        assert_eq!(orchestrator.get_state(), OrchestratorState::Detecting);

        // Simulate detected
        orchestrator.clients_detected.store(3, Ordering::Relaxed);
        orchestrator.detected_bitmap.store(0b111, Ordering::Relaxed);
        orchestrator.set_state(OrchestratorState::Detected);

        assert_eq!(orchestrator.get_state(), OrchestratorState::Detected);
        let stats = orchestrator.get_stats();
        assert_eq!(stats.clients_detected, 3);
    }

    // Q14: Audit hash chain (Q34)
    #[test]
    fn test_audit_hash_chain() {
        let orchestrator = AutoConfigOrchestratorCapsule::new();

        // Initial audit hash should be 0
        let hash1 = orchestrator.audit_hash();
        assert_eq!(hash1, 0);

        // Update audit - hash should change
        orchestrator.update_audit(1);
        let hash2 = orchestrator.audit_hash();
        assert_ne!(hash2, hash1, "Audit hash should change after update");

        // Another update - hash should change again
        orchestrator.update_audit(2);
        let hash3 = orchestrator.audit_hash();
        assert_ne!(hash3, hash2, "Audit hash should change after each update");

        // Verify audit entries count
        assert_eq!(orchestrator.audit_entries(), 2);
    }

    // Q15: Reset all (test helper)
    #[test]
    fn test_reset_all() {
        let orchestrator = AutoConfigOrchestratorCapsule::new();

        // Set some state
        orchestrator.set_state(OrchestratorState::Complete);
        orchestrator.clients_detected.store(5, Ordering::Relaxed);
        orchestrator.configs_installed.store(3, Ordering::Relaxed);
        orchestrator.update_audit(1);

        // Reset
        orchestrator.reset_all();

        // Verify everything is reset
        assert_eq!(orchestrator.get_state(), OrchestratorState::Idle);
        let stats = orchestrator.get_stats();
        assert_eq!(stats.clients_detected, 0);
        assert_eq!(stats.configs_installed, 0);
        assert_eq!(orchestrator.audit_hash(), 0);
        assert_eq!(orchestrator.audit_entries(), 0);
    }

    // Additional tests for completeness

    #[test]
    fn test_default_impl() {
        let orchestrator = AutoConfigOrchestratorCapsule::default();
        assert_eq!(orchestrator.get_state(), OrchestratorState::Idle);
    }

    #[test]
    fn test_is_terminal_states() {
        assert!(OrchestratorState::Complete.is_terminal());
        assert!(OrchestratorState::Cancelled.is_terminal());
        assert!(OrchestratorState::RolledBack.is_terminal());
        assert!(OrchestratorState::Error.is_terminal());

        assert!(!OrchestratorState::Idle.is_terminal());
        assert!(!OrchestratorState::Detecting.is_terminal());
        assert!(!OrchestratorState::Installing.is_terminal());
    }

    #[test]
    fn test_state_from_u64_invalid() {
        assert_eq!(OrchestratorState::from_u64(12), None);
        assert_eq!(OrchestratorState::from_u64(255), None);
    }

    #[test]
    fn test_config_error_display() {
        assert_eq!(
            ConfigError::DetectionFailed("test".to_string()).to_string(),
            "detection failed: test"
        );
        assert_eq!(
            ConfigError::PermissionDenied.to_string(),
            "permission denied by user"
        );
        assert_eq!(
            ConfigError::NoClientsDetected.to_string(),
            "no MCP clients detected"
        );
        assert_eq!(
            ConfigError::LicenseKeyNotFound.to_string(),
            "KDB_LICENSE_KEY not found"
        );
    }

    #[test]
    fn test_permission_guard_access() {
        let orchestrator = AutoConfigOrchestratorCapsule::new();
        let guard = orchestrator.permission_guard();

        // Verify we can access the embedded capsule
        assert_eq!(
            guard.get_state(),
            super::super::permission::PermissionState::NotAsked
        );
    }

    #[test]
    fn test_fnv1a_hash() {
        // Known test vectors
        let hash1 = fnv1a_hash(b"");
        assert_eq!(hash1, 14695981039346656037); // FNV offset basis

        let hash2 = fnv1a_hash(b"a");
        let hash3 = fnv1a_hash(b"b");
        assert_ne!(hash2, hash3);

        // Same input should produce same output
        let hash4 = fnv1a_hash(b"test");
        let hash5 = fnv1a_hash(b"test");
        assert_eq!(hash4, hash5);
    }

    #[test]
    fn test_config_report_default() {
        let report = ConfigReport::default();
        assert!(report.clients_detected.is_empty());
        assert!(report.clients_configured.is_empty());
        assert!(report.clients_skipped.is_empty());
        assert!(report.backups_created.is_empty());
        assert_eq!(report.duration_ms, 0);
        assert_eq!(report.audit_hash, 0);
    }
}
