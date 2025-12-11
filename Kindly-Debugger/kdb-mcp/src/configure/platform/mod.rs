//! Platform Detection Module
//!
//! Cross-platform OS, architecture, and path resolution.
//!
//! ## Capsules
//! - `PlatformDetectorCapsule` (T1 Atomic, 64B): OS/arch detection with caching
//!
//! ## Utilities
//! - `paths`: XDG/AppData/Library path resolution
//!
//! ## Platform Support
//! - Linux: XDG Base Directory Specification (XDG_CONFIG_HOME)
//! - macOS: ~/Library/Application Support
//! - Windows: %APPDATA% (C:\Users\{user}\AppData\Roaming)
//!
//! ## Architecture Support
//! - x86_64 (Intel/AMD 64-bit)
//! - aarch64 (ARM 64-bit, Apple Silicon)
//! - x86 (Intel/AMD 32-bit, legacy)
//! - arm (ARM 32-bit, embedded)
//!
//! ## UCE35 Compliance
//! - T1 Atomic tier for detection capsule
//! - Cache-aligned (64B) for lockfree access
//! - Generation counter for TOCTOU prevention
//! - Q34 audit trail for platform detection events

pub mod detector;
pub mod paths;

// Re-export detector types
pub use detector::{PlatformDetectorCapsule, PlatformInfo, Architecture, DetectionState};
// Note: Platform enum is defined in both detector.rs and paths.rs
// We re-export from detector as the canonical source for the capsule
pub use detector::Platform;

// Re-export path utilities
// Note: Most functions take a Platform argument for cross-platform support
pub use paths::{
    // Auto-detecting (no Platform argument needed)
    get_config_dir,
    // Platform-specific (take Platform argument)
    get_config_dir_for_platform,
    get_data_dir,
    get_cache_dir,
    get_system_config_dir,
    // KDB-specific directories (take Platform argument)
    get_kdb_config_dir,
    get_kdb_data_dir,
    get_kdb_cache_dir,
    get_kdb_env_path,
    get_kdb_license_path,
    // Path expansion utilities (take Platform argument)
    expand_path,
    expand_env_vars,
    // Security utilities
    set_secure_permissions,
    ensure_secure_dir,
};
