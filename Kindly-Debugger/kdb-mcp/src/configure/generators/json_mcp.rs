//! JSON MCP Configuration Generator
//!
//! Pure utility module for generating JSON MCP configurations for various clients.
//!
//! ## Features
//! - Generate stdio MCP server config (recommended)
//! - Generate HTTP/SSE MCP server config
//! - Environment variable templating support
//! - Merge into existing MCP configuration files
//!
//! ## Usage
//! ```rust,ignore
//! use kdb_mcp::configure::generators::json_mcp::{
//!     generate_stdio_config,
//!     generate_mcp_config_file,
//!     merge_kdb_into_config,
//!     TransportType,
//! };
//!
//! // Generate stdio config (recommended for Claude Code)
//! let config = generate_stdio_config("kdb", "KDB-PRO-...", false);
//!
//! // Generate complete config file
//! let file_content = generate_mcp_config_file("KDB-PRO-...", true, TransportType::Stdio);
//!
//! // Merge into existing config
//! let merged = merge_kdb_into_config(existing_json, "KDB-PRO-...", true)?;
//! ```
//!
//! ## Transport Types
//! - **Stdio** (recommended): npm package bridges to remote HTTPS server
//! - **HTTP**: Direct HTTP transport (for clients that support it)
//! - **SSE**: Server-Sent Events transport (legacy)
//! - **StreamableHttp**: MCP 2025-06-18 unified endpoint
//!
//! ## Environment Variables
//! When `use_env_vars` is true, license keys use `${KDB_LICENSE_KEY}` template
//! instead of hardcoded values. This is recommended for:
//! - Version-controlled configs
//! - CI/CD pipelines
//! - Multi-user environments

use serde_json::{json, Value};
use crate::configure::detectors::TransportType;

// ============================================================================
// Configuration Constants
// ============================================================================

/// Default MCP server base URL
pub const KDB_MCP_BASE_URL: &str = "https://mcp.kindly.software";

/// Default npm package name
pub const KDB_NPM_PACKAGE: &str = "@kindly-software-inc/kdb";

/// Environment variable placeholder for license key
pub const LICENSE_KEY_ENV_VAR: &str = "${KDB_LICENSE_KEY}";

// ============================================================================
// Stdio Config Generator
// ============================================================================

/// Generate stdio MCP server config (recommended)
///
/// This is the recommended configuration for Claude Code and similar clients.
/// Uses the npm package `@kindly-software-inc/kdb` which bridges stdio to
/// the remote HTTPS server.
///
/// # Arguments
/// * `_server_name` - Server name (currently unused, reserved for future)
/// * `license_key` - KDB license key or empty string if using env vars
/// * `use_env_vars` - If true, use `${KDB_LICENSE_KEY}` placeholder
///
/// # Returns
/// JSON Value representing the MCP server configuration
///
/// # Example Output (env vars)
/// ```json
/// {
///   "command": "npx",
///   "args": ["@kindly-software-inc/kdb"],
///   "env": {
///     "KDB_LICENSE_KEY": "${KDB_LICENSE_KEY}"
///   }
/// }
/// ```
pub fn generate_stdio_config(
    _server_name: &str,
    license_key: &str,
    use_env_vars: bool,
) -> Value {
    let key_value = if use_env_vars {
        LICENSE_KEY_ENV_VAR
    } else {
        license_key
    };

    json!({
        "command": "npx",
        "args": [KDB_NPM_PACKAGE],
        "env": {
            "KDB_LICENSE_KEY": key_value
        }
    })
}

// ============================================================================
// HTTP Config Generator
// ============================================================================

/// Generate HTTP MCP server config
///
/// Uses direct HTTP transport to the MCP server. Some MCP clients support
/// this natively without needing the npm bridge.
///
/// # Arguments
/// * `_server_name` - Server name (currently unused, reserved for future)
/// * `license_key` - KDB license key or empty string if using env vars
/// * `use_env_vars` - If true, use `${KDB_LICENSE_KEY}` placeholder
///
/// # Returns
/// JSON Value representing the HTTP MCP server configuration
///
/// # Example Output (env vars)
/// ```json
/// {
///   "type": "http",
///   "url": "https://mcp.kindly.software/mcp",
///   "headers": {
///     "X-License-Key": "${KDB_LICENSE_KEY}"
///   }
/// }
/// ```
pub fn generate_http_config(
    _server_name: &str,
    license_key: &str,
    use_env_vars: bool,
) -> Value {
    let key_value = if use_env_vars {
        LICENSE_KEY_ENV_VAR
    } else {
        license_key
    };

    json!({
        "type": "http",
        "url": format!("{}/mcp", KDB_MCP_BASE_URL),
        "headers": {
            "X-License-Key": key_value
        }
    })
}

// ============================================================================
// SSE Config Generator
// ============================================================================

/// Generate SSE MCP server config (legacy)
///
/// Uses Server-Sent Events transport to the MCP server.
/// This is the legacy transport, prefer StreamableHttp or Stdio.
///
/// # Arguments
/// * `_server_name` - Server name (currently unused, reserved for future)
/// * `license_key` - KDB license key or empty string if using env vars
/// * `use_env_vars` - If true, use `${KDB_LICENSE_KEY}` placeholder
///
/// # Returns
/// JSON Value representing the SSE MCP server configuration
fn generate_sse_config(
    _server_name: &str,
    license_key: &str,
    use_env_vars: bool,
) -> Value {
    let key_value = if use_env_vars {
        LICENSE_KEY_ENV_VAR
    } else {
        license_key
    };

    json!({
        "type": "sse",
        "url": format!("{}/sse", KDB_MCP_BASE_URL),
        "headers": {
            "X-License-Key": key_value
        }
    })
}

/// Generate StreamableHttp MCP server config (MCP 2025-06-18)
///
/// Uses the unified /mcp endpoint per MCP spec 2025-06-18.
///
/// # Arguments
/// * `_server_name` - Server name (currently unused, reserved for future)
/// * `license_key` - KDB license key or empty string if using env vars
/// * `use_env_vars` - If true, use `${KDB_LICENSE_KEY}` placeholder
///
/// # Returns
/// JSON Value representing the StreamableHttp MCP server configuration
fn generate_streamable_http_config(
    _server_name: &str,
    license_key: &str,
    use_env_vars: bool,
) -> Value {
    let key_value = if use_env_vars {
        LICENSE_KEY_ENV_VAR
    } else {
        license_key
    };

    json!({
        "type": "streamable-http",
        "url": format!("{}/mcp", KDB_MCP_BASE_URL),
        "headers": {
            "X-License-Key": key_value
        }
    })
}

// ============================================================================
// Complete Config File Generator
// ============================================================================

/// Generate complete MCP config file with kdb server
///
/// Creates a full MCP configuration file containing only the kdb server.
/// For adding kdb to an existing config, use `merge_kdb_into_config` instead.
///
/// # Arguments
/// * `license_key` - KDB license key or empty string if using env vars
/// * `use_env_vars` - If true, use `${KDB_LICENSE_KEY}` placeholder
/// * `transport` - Which transport type to use
///
/// # Returns
/// Pretty-printed JSON string representing the complete MCP config file
///
/// # Example Output
/// ```json
/// {
///   "mcpServers": {
///     "kdb": {
///       "command": "npx",
///       "args": ["@kindly-software-inc/kdb"],
///       "env": {
///         "KDB_LICENSE_KEY": "${KDB_LICENSE_KEY}"
///       }
///     }
///   }
/// }
/// ```
pub fn generate_mcp_config_file(
    license_key: &str,
    use_env_vars: bool,
    transport: TransportType,
) -> String {
    let kdb_config = match transport {
        TransportType::Stdio => generate_stdio_config("kdb", license_key, use_env_vars),
        TransportType::Http => generate_http_config("kdb", license_key, use_env_vars),
        TransportType::Sse => generate_sse_config("kdb", license_key, use_env_vars),
        TransportType::StreamableHttp => generate_streamable_http_config("kdb", license_key, use_env_vars),
        TransportType::WebSocket | TransportType::Unknown => {
            // Default to stdio for unknown/unsupported transports
            generate_stdio_config("kdb", license_key, use_env_vars)
        }
    };

    let config = json!({
        "mcpServers": {
            "kdb": kdb_config
        }
    });

    // serde_json::to_string_pretty never fails for valid Value
    serde_json::to_string_pretty(&config).expect("JSON serialization should never fail for Value")
}

// ============================================================================
// Config Merger
// ============================================================================

/// Merge kdb config into existing MCP config
///
/// Parses an existing MCP configuration JSON file and adds or updates
/// the kdb server entry. Preserves all other servers and settings.
///
/// # Arguments
/// * `existing_json` - Existing MCP config file content as JSON string
/// * `license_key` - KDB license key or empty string if using env vars
/// * `use_env_vars` - If true, use `${KDB_LICENSE_KEY}` placeholder
///
/// # Returns
/// - `Ok(String)` - Pretty-printed JSON with kdb merged in
/// - `Err(String)` - Error message if JSON is invalid
///
/// # Example
/// ```rust,ignore
/// let existing = r#"{"mcpServers": {"other": {"command": "other"}}}"#;
/// let merged = merge_kdb_into_config(existing, "KDB-PRO-...", true)?;
/// // Result: {"mcpServers": {"other": {...}, "kdb": {...}}}
/// ```
pub fn merge_kdb_into_config(
    existing_json: &str,
    license_key: &str,
    use_env_vars: bool,
) -> Result<String, String> {
    // Parse existing JSON
    let mut config: Value = serde_json::from_str(existing_json)
        .map_err(|e| format!("Invalid JSON: {}", e))?;

    // Ensure root is an object
    if !config.is_object() {
        return Err("Config root must be object".to_string());
    }

    let obj = config.as_object_mut().unwrap();

    // Get or create mcpServers object
    if !obj.contains_key("mcpServers") {
        obj.insert("mcpServers".to_string(), json!({}));
    }

    let servers = obj
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
        .ok_or("mcpServers must be object")?;

    // Add/update kdb server (stdio config by default)
    let kdb_config = generate_stdio_config("kdb", license_key, use_env_vars);
    servers.insert("kdb".to_string(), kdb_config);

    // Serialize back to pretty JSON
    serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Serialize failed: {}", e))
}

/// Merge kdb config into existing MCP config with specific transport
///
/// Same as `merge_kdb_into_config` but allows specifying transport type.
///
/// # Arguments
/// * `existing_json` - Existing MCP config file content as JSON string
/// * `license_key` - KDB license key or empty string if using env vars
/// * `use_env_vars` - If true, use `${KDB_LICENSE_KEY}` placeholder
/// * `transport` - Which transport type to use
///
/// # Returns
/// - `Ok(String)` - Pretty-printed JSON with kdb merged in
/// - `Err(String)` - Error message if JSON is invalid
pub fn merge_kdb_into_config_with_transport(
    existing_json: &str,
    license_key: &str,
    use_env_vars: bool,
    transport: TransportType,
) -> Result<String, String> {
    // Parse existing JSON
    let mut config: Value = serde_json::from_str(existing_json)
        .map_err(|e| format!("Invalid JSON: {}", e))?;

    // Ensure root is an object
    if !config.is_object() {
        return Err("Config root must be object".to_string());
    }

    let obj = config.as_object_mut().unwrap();

    // Get or create mcpServers object
    if !obj.contains_key("mcpServers") {
        obj.insert("mcpServers".to_string(), json!({}));
    }

    let servers = obj
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
        .ok_or("mcpServers must be object")?;

    // Add/update kdb server with specified transport
    let kdb_config = match transport {
        TransportType::Stdio => generate_stdio_config("kdb", license_key, use_env_vars),
        TransportType::Http => generate_http_config("kdb", license_key, use_env_vars),
        TransportType::Sse => generate_sse_config("kdb", license_key, use_env_vars),
        TransportType::StreamableHttp => generate_streamable_http_config("kdb", license_key, use_env_vars),
        TransportType::WebSocket | TransportType::Unknown => {
            generate_stdio_config("kdb", license_key, use_env_vars)
        }
    };
    servers.insert("kdb".to_string(), kdb_config);

    // Serialize back to pretty JSON
    serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Serialize failed: {}", e))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test 1: Basic stdio config
    #[test]
    fn test_generate_stdio_config() {
        let config = generate_stdio_config("kdb", "KDB-PRO-123456", false);

        assert_eq!(config["command"], "npx");
        assert_eq!(config["args"][0], KDB_NPM_PACKAGE);
        assert_eq!(config["env"]["KDB_LICENSE_KEY"], "KDB-PRO-123456");
    }

    // Test 2: Stdio config with env vars
    #[test]
    fn test_generate_stdio_with_env_vars() {
        let config = generate_stdio_config("kdb", "", true);

        assert_eq!(config["command"], "npx");
        assert_eq!(config["args"][0], KDB_NPM_PACKAGE);
        assert_eq!(config["env"]["KDB_LICENSE_KEY"], LICENSE_KEY_ENV_VAR);
        // Verify the actual template string
        assert_eq!(config["env"]["KDB_LICENSE_KEY"].as_str().unwrap(), "${KDB_LICENSE_KEY}");
    }

    // Test 3: HTTP config
    #[test]
    fn test_generate_http_config() {
        let config = generate_http_config("kdb", "KDB-HOBBY-999", false);

        assert_eq!(config["type"], "http");
        assert_eq!(config["url"], format!("{}/mcp", KDB_MCP_BASE_URL));
        assert_eq!(config["headers"]["X-License-Key"], "KDB-HOBBY-999");
    }

    // Test 4: SSE config
    #[test]
    fn test_generate_sse_config() {
        let config = generate_sse_config("kdb", "KDB-ENTERPRISE-ABC", false);

        assert_eq!(config["type"], "sse");
        assert_eq!(config["url"], format!("{}/sse", KDB_MCP_BASE_URL));
        assert_eq!(config["headers"]["X-License-Key"], "KDB-ENTERPRISE-ABC");
    }

    // Test 5: Complete MCP config file
    #[test]
    fn test_generate_mcp_config_file() {
        let content = generate_mcp_config_file("KDB-PRO-XYZ", false, TransportType::Stdio);

        // Parse the output to verify structure
        let parsed: Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["mcpServers"]["kdb"].is_object());
        assert_eq!(parsed["mcpServers"]["kdb"]["command"], "npx");
        assert_eq!(parsed["mcpServers"]["kdb"]["env"]["KDB_LICENSE_KEY"], "KDB-PRO-XYZ");
    }

    // Test 6: Merge into empty config
    #[test]
    fn test_merge_empty_config() {
        let result = merge_kdb_into_config("{}", "KDB-TEST-123", false);

        assert!(result.is_ok());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(merged["mcpServers"]["kdb"].is_object());
        assert_eq!(merged["mcpServers"]["kdb"]["env"]["KDB_LICENSE_KEY"], "KDB-TEST-123");
    }

    // Test 7: Merge preserves existing servers
    #[test]
    fn test_merge_existing_servers() {
        let existing = r#"{
            "mcpServers": {
                "other_server": {
                    "command": "other-cmd",
                    "args": ["--flag"]
                }
            }
        }"#;

        let result = merge_kdb_into_config(existing, "KDB-MERGE-TEST", true);
        assert!(result.is_ok());

        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();

        // Check kdb was added
        assert!(merged["mcpServers"]["kdb"].is_object());

        // Check other_server was preserved
        assert_eq!(merged["mcpServers"]["other_server"]["command"], "other-cmd");
        assert_eq!(merged["mcpServers"]["other_server"]["args"][0], "--flag");
    }

    // Test 8: Merge updates existing kdb entry
    #[test]
    fn test_merge_update_kdb() {
        let existing = r#"{
            "mcpServers": {
                "kdb": {
                    "command": "old-kdb",
                    "env": { "OLD_KEY": "old-value" }
                }
            }
        }"#;

        let result = merge_kdb_into_config(existing, "KDB-NEW-KEY", false);
        assert!(result.is_ok());

        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();

        // Verify kdb was updated (not old values)
        assert_eq!(merged["mcpServers"]["kdb"]["command"], "npx");
        assert_eq!(merged["mcpServers"]["kdb"]["env"]["KDB_LICENSE_KEY"], "KDB-NEW-KEY");
        // Old key should be gone (replaced, not merged)
        assert!(merged["mcpServers"]["kdb"]["env"]["OLD_KEY"].is_null());
    }

    // Test 9: Merge handles invalid JSON
    #[test]
    fn test_merge_invalid_json() {
        let result = merge_kdb_into_config("not valid json {{{", "KEY", false);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid JSON"));
    }

    // Test 10: Pretty formatting
    #[test]
    fn test_pretty_formatting() {
        let content = generate_mcp_config_file("KEY", false, TransportType::Stdio);

        // Pretty JSON should contain newlines
        assert!(content.contains('\n'));
        // Pretty JSON should contain indentation
        assert!(content.contains("  "));
        // Verify it's valid JSON by parsing
        let _: Value = serde_json::from_str(&content).unwrap();
    }

    // Test 11: Env var not evaluated (literal template)
    #[test]
    fn test_env_var_escaping() {
        let config = generate_stdio_config("kdb", "", true);
        let key = config["env"]["KDB_LICENSE_KEY"].as_str().unwrap();

        // Must be literal "${KDB_LICENSE_KEY}", not an empty/resolved value
        assert_eq!(key, "${KDB_LICENSE_KEY}");
        assert!(key.starts_with("${"));
        assert!(key.ends_with("}"));
        assert!(key.contains("KDB_LICENSE_KEY"));
    }

    // Test 12: Transport type selection
    #[test]
    fn test_transport_type_selection() {
        // Stdio
        let stdio_content = generate_mcp_config_file("KEY", false, TransportType::Stdio);
        let stdio: Value = serde_json::from_str(&stdio_content).unwrap();
        assert_eq!(stdio["mcpServers"]["kdb"]["command"], "npx");
        assert!(stdio["mcpServers"]["kdb"]["type"].is_null()); // stdio has no "type" field

        // HTTP
        let http_content = generate_mcp_config_file("KEY", false, TransportType::Http);
        let http: Value = serde_json::from_str(&http_content).unwrap();
        assert_eq!(http["mcpServers"]["kdb"]["type"], "http");
        assert!(http["mcpServers"]["kdb"]["url"].as_str().unwrap().contains("/mcp"));

        // SSE
        let sse_content = generate_mcp_config_file("KEY", false, TransportType::Sse);
        let sse: Value = serde_json::from_str(&sse_content).unwrap();
        assert_eq!(sse["mcpServers"]["kdb"]["type"], "sse");
        assert!(sse["mcpServers"]["kdb"]["url"].as_str().unwrap().contains("/sse"));

        // StreamableHttp
        let streamable_content = generate_mcp_config_file("KEY", false, TransportType::StreamableHttp);
        let streamable: Value = serde_json::from_str(&streamable_content).unwrap();
        assert_eq!(streamable["mcpServers"]["kdb"]["type"], "streamable-http");

        // Unknown defaults to stdio
        let unknown_content = generate_mcp_config_file("KEY", false, TransportType::Unknown);
        let unknown: Value = serde_json::from_str(&unknown_content).unwrap();
        assert_eq!(unknown["mcpServers"]["kdb"]["command"], "npx");
    }

    // Additional tests for edge cases

    #[test]
    fn test_merge_non_object_root() {
        let result = merge_kdb_into_config("[1, 2, 3]", "KEY", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("root must be object"));
    }

    #[test]
    fn test_merge_non_object_mcp_servers() {
        let existing = r#"{"mcpServers": "not an object"}"#;
        let result = merge_kdb_into_config(existing, "KEY", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mcpServers must be object"));
    }

    #[test]
    fn test_constants() {
        assert!(KDB_MCP_BASE_URL.starts_with("https://"));
        assert!(KDB_NPM_PACKAGE.contains("kdb"));
        assert!(LICENSE_KEY_ENV_VAR.starts_with("${"));
    }

    #[test]
    fn test_http_config_with_env_vars() {
        let config = generate_http_config("kdb", "", true);
        assert_eq!(config["headers"]["X-License-Key"], LICENSE_KEY_ENV_VAR);
    }

    #[test]
    fn test_streamable_http_config() {
        let config = generate_streamable_http_config("kdb", "KDB-TEST", false);
        assert_eq!(config["type"], "streamable-http");
        assert!(config["url"].as_str().unwrap().ends_with("/mcp"));
    }

    #[test]
    fn test_merge_with_transport() {
        let existing = "{}";
        let result = merge_kdb_into_config_with_transport(existing, "KEY", false, TransportType::Http);

        assert!(result.is_ok());
        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(merged["mcpServers"]["kdb"]["type"], "http");
    }

    #[test]
    fn test_websocket_defaults_to_stdio() {
        let content = generate_mcp_config_file("KEY", false, TransportType::WebSocket);
        let parsed: Value = serde_json::from_str(&content).unwrap();
        // WebSocket not implemented, should fallback to stdio
        assert_eq!(parsed["mcpServers"]["kdb"]["command"], "npx");
    }
}
