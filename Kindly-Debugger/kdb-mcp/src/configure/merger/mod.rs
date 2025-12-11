//! Config Merger Module
//!
//! T1 Atomic configuration merging with rollback capability.
//!
//! ## Modules
//!
//! - `capsule` - ConfigMergerCapsule (128B, T1 Atomic)
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use kdb_mcp::configure::merger::{ConfigMergerCapsule, KdbConfig, MergeResult};
//!
//! let merger = ConfigMergerCapsule::new();
//!
//! let kdb_config = KdbConfig {
//!     command: "npx".to_string(),
//!     args: vec!["@kindly-software-inc/kdb".to_string()],
//!     env: std::collections::HashMap::from([
//!         ("KDB_LICENSE_KEY".to_string(), "your-key".to_string())
//!     ]),
//! };
//!
//! let result = merger.merge_json(
//!     r#"{"mcpServers": {}}"#,
//!     &kdb_config,
//!     Some(std::path::Path::new("/tmp/backup.json")),
//! );
//! ```

mod capsule;

pub use capsule::{
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
