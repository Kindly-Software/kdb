//! MCP Configuration Generators
//!
//! Pure utility modules for generating MCP configuration files in various formats.
//!
//! ## Modules
//! - **json_mcp**: JSON MCP configuration generator (most common format)
//!
//! ## Future Modules (Phase 2+)
//! - **yaml_mcp**: YAML configuration generator
//! - **toml_mcp**: TOML configuration generator
//!
//! ## Usage
//! ```rust,ignore
//! use kdb_mcp::configure::generators::{
//!     generate_stdio_config,
//!     generate_mcp_config_file,
//!     merge_kdb_into_config,
//!     KDB_MCP_BASE_URL,
//! };
//! use kdb_mcp::configure::detectors::TransportType;
//!
//! // Generate stdio config (recommended)
//! let config = generate_stdio_config("kdb", "KDB-PRO-...", false);
//!
//! // Generate complete config file
//! let content = generate_mcp_config_file("KDB-PRO-...", true, TransportType::Stdio);
//!
//! // Merge into existing config
//! let merged = merge_kdb_into_config(existing, "KDB-PRO-...", true)?;
//! ```
//!
//! ## Design
//! - **Pure utilities**: No capsule architecture (stateless, side-effect free)
//! - **Format-specific**: Each format in its own module
//! - **Environment variable support**: Template support for ${VAR} placeholders
//! - **Non-destructive merge**: Preserves existing config entries

// JSON MCP configuration generator
pub mod json_mcp;

// ============================================================================
// Re-exports
// ============================================================================

// Core functions from json_mcp
pub use json_mcp::{
    // Config generators
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configure::detectors::TransportType;

    #[test]
    fn test_module_reexports() {
        // Verify all re-exports are accessible
        let _ = generate_stdio_config("kdb", "KEY", false);
        let _ = generate_http_config("kdb", "KEY", false);
        let _ = generate_mcp_config_file("KEY", false, TransportType::Stdio);
        let _ = merge_kdb_into_config("{}", "KEY", false);
        let _ = merge_kdb_into_config_with_transport("{}", "KEY", false, TransportType::Http);

        // Verify constants
        assert!(!KDB_MCP_BASE_URL.is_empty());
        assert!(!KDB_NPM_PACKAGE.is_empty());
        assert!(!LICENSE_KEY_ENV_VAR.is_empty());
    }
}
