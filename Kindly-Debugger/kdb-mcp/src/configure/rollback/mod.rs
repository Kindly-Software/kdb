//! Rollback Module - Backup and Restore Operations
//!
//! T1 Atomic backup/rollback system with SHA-256 checksum verification.
//!
//! ## Modules
//!
//! - `capsule` - BackupManagerCapsule (64B, T1 Atomic)
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use kdb_mcp::configure::rollback::{BackupManagerCapsule, BackupState};
//!
//! let manager = BackupManagerCapsule::new();
//!
//! // Create backup session
//! let mut session = manager.create_backup_session("auto-configure")?;
//!
//! // Backup a config file
//! let backup_path = session.backup_file("claude-code", &config_path)?;
//!
//! // Finalize session (writes manifest.json + checksums.sha256)
//! session.finalize()?;
//!
//! // Later: rollback to most recent backup
//! let result = manager.rollback_latest()?;
//! println!("Restored {} files", result.restored);
//! ```
//!
//! ## Backup Structure
//!
//! ```text
//! ~/.kdb/backups/
//! └── 2025-12-11T14-30-00/
//!     ├── manifest.json             # What was modified
//!     ├── claude_code/
//!     │   └── mcp.json.bak          # Original config
//!     ├── cursor/
//!     │   └── mcp.json.bak
//!     └── checksums.sha256          # SHA-256 of all backups
//! ```

mod capsule;

pub use capsule::{
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
