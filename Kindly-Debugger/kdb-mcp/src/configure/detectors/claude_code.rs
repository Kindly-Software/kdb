//! Claude Code MCP Client Detector (Priority 1500)
//!
//! Detects Claude Code CLI/VSCode extension and returns configuration paths.
//!
//! ## Detection Methods (in order of confidence)
//! 1. `claude` binary in PATH (95% confidence)
//! 2. `~/.claude.json` global config file (80% confidence)
//! 3. `~/.claude/` directory exists (80% confidence)
//! 4. `CLAUDE_CODE_VERSION` environment variable (70% confidence)
//!
//! ## Config Locations
//! - Linux: `~/.config/claude-code/mcp.json` OR `~/.claude.json` (projects)
//! - macOS: `~/Library/Application Support/claude-code/mcp.json` OR `~/.claude.json`
//! - Windows: `%APPDATA%\claude-code\mcp.json` OR `%USERPROFILE%\.claude.json`
//!
//! ## Transport
//! Uses stdio transport (recommended due to HTTP header bug in Claude Code)
//!
//! ## Config Schema
//! ```json
//! {
//!   "mcpServers": {
//!     "kdb": {
//!       "command": "npx",
//!       "args": ["@kindly-software-inc/kdb"],
//!       "env": {
//!         "KDB_LICENSE_KEY": "${KDB_LICENSE_KEY}"
//!       }
//!     }
//!   }
//! }
//! ```
//!
//! ## UCE35 Compliance
//! - T0 utility module (stateless, deterministic)
//! - No atomic operations (detection is pure functions)
//! - No allocations in hot path

use super::trait_def::{
    ConfigFormat, DetectedClient, DetectionMethod, McpClientDetector, TransportType,
};
use crate::configure::platform::{expand_path, Platform, PlatformInfo};
use std::path::{Path, PathBuf};

/// Claude Code MCP client detector
///
/// Detects Claude Code CLI and VSCode extension installations.
/// Priority 1500 (Enterprise tier) due to commercial AI assistant status.
pub struct ClaudeCodeDetector;

impl McpClientDetector for ClaudeCodeDetector {
    /// Unique identifier for Claude Code
    #[inline]
    fn client_id(&self) -> &'static str {
        "claude_code"
    }

    /// Human-readable client name
    #[inline]
    fn client_name(&self) -> &'static str {
        "Claude Code"
    }

    /// Enterprise priority (1500+)
    ///
    /// Claude Code is Anthropic's official CLI and primary commercial AI assistant,
    /// warranting highest priority tier.
    #[inline]
    fn priority(&self) -> u32 {
        1500
    }

    /// JSON configuration format
    #[inline]
    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Json
    }

    /// Stdio transport (recommended)
    ///
    /// Uses stdio transport due to HTTP header bug in Claude Code's HTTP transport.
    /// The npm package @kindly-software-inc/kdb bridges stdio to remote HTTPS.
    #[inline]
    fn transport_type(&self) -> TransportType {
        TransportType::Stdio
    }

    /// Detect Claude Code installation
    ///
    /// Tries detection methods in order of confidence:
    /// 1. Binary in PATH (95% confidence)
    /// 2. Global config file ~/.claude.json (80% confidence)
    /// 3. Claude directory ~/.claude/ (80% confidence)
    /// 4. Environment variable CLAUDE_CODE_VERSION (70% confidence)
    fn detect(&self, platform: &PlatformInfo) -> Option<DetectedClient> {
        // 1. Check for claude binary (highest confidence)
        if self.binary_exists("claude") {
            return Some(self.build_detected_client(platform, DetectionMethod::Binary));
        }

        // 2. Check for .claude.json (global config)
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            let global_config = PathBuf::from(home).join(".claude.json");
            if global_config.exists() {
                return Some(self.build_detected_client(platform, DetectionMethod::ConfigDir));
            }
        }

        // 3. Check for .claude directory
        if let Some(claude_dir) = self.get_claude_dir() {
            if claude_dir.exists() {
                return Some(self.build_detected_client(platform, DetectionMethod::ConfigDir));
            }
        }

        // 4. Check environment variable
        if self.env_var_set("CLAUDE_CODE_VERSION") {
            return Some(self.build_detected_client(platform, DetectionMethod::EnvVar));
        }

        None
    }

    /// Get configuration file path for Claude Code MCP servers
    ///
    /// Returns platform-specific path:
    /// - Linux: `~/.config/claude-code/mcp.json`
    /// - macOS: `~/Library/Application Support/claude-code/mcp.json`
    /// - Windows: `%APPDATA%\claude-code\mcp.json`
    fn config_path(&self, platform: &PlatformInfo) -> PathBuf {
        let base = match platform.platform {
            Platform::Linux => "~/.config/claude-code/mcp.json",
            Platform::MacOS => "~/Library/Application Support/claude-code/mcp.json",
            Platform::Windows => "%APPDATA%\\claude-code\\mcp.json",
            Platform::FreeBSD => "~/.config/claude-code/mcp.json", // XDG fallback
            Platform::Unknown => "~/.config/claude-code/mcp.json", // XDG fallback
        };

        expand_path(base, platform.platform)
    }
}

impl ClaudeCodeDetector {
    /// Create a new Claude Code detector instance
    #[inline]
    pub const fn new() -> Self {
        Self
    }

    /// Get the Claude directory path (~/.claude/)
    fn get_claude_dir(&self) -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(|home| PathBuf::from(home).join(".claude"))
    }

    /// Build a DetectedClient struct with common fields
    fn build_detected_client(
        &self,
        platform: &PlatformInfo,
        method: DetectionMethod,
    ) -> DetectedClient {
        let config_path = self.config_path(platform);
        let config_exists = config_path.exists();
        let kdb_configured = if config_exists {
            self.check_kdb_configured(&config_path)
        } else {
            false
        };

        DetectedClient::from_parts(
            self.client_id(),
            self.client_name(),
            config_path,
            config_exists,
            kdb_configured,
            method,
            self.config_format(),
            self.transport_type(),
            self.priority(),
        )
    }

    /// Check if kdb is already configured in the config file
    fn check_kdb_configured(&self, config_path: &Path) -> bool {
        std::fs::read_to_string(config_path)
            .ok()
            .and_then(|content| {
                // Parse JSON and check for kdb in mcpServers
                let json: serde_json::Value = serde_json::from_str(&content).ok()?;
                json.get("mcpServers")?.get("kdb").map(|_| true)
            })
            .unwrap_or(false)
    }
}

impl Default for ClaudeCodeDetector {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// UNIT TESTS (T28 Q1-Q7)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configure::platform::Architecture;
    use std::env;

    /// Helper to create a PlatformInfo for testing
    fn make_platform(platform: Platform) -> PlatformInfo {
        PlatformInfo::new(platform, Architecture::X86_64, true, true, false)
    }

    // -------------------------------------------------------------------------
    // Test 1: Priority is 1500 (Enterprise tier)
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_code_priority() {
        let detector = ClaudeCodeDetector::new();
        assert_eq!(detector.priority(), 1500);
        assert!(detector.priority() >= 1500, "Should be Enterprise tier (1500+)");
    }

    // -------------------------------------------------------------------------
    // Test 2: Config format is JSON
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_code_config_format() {
        let detector = ClaudeCodeDetector::new();
        assert_eq!(detector.config_format(), ConfigFormat::Json);
        assert_eq!(detector.config_format().as_str(), "json");
        assert_eq!(detector.config_format().extension(), ".json");
    }

    // -------------------------------------------------------------------------
    // Test 3: Transport type is stdio
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_code_transport() {
        let detector = ClaudeCodeDetector::new();
        assert_eq!(detector.transport_type(), TransportType::Stdio);
        assert!(!detector.transport_type().requires_network());
    }

    // -------------------------------------------------------------------------
    // Test 4: Config path for Linux (XDG path)
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_code_config_path_linux() {
        let detector = ClaudeCodeDetector::new();
        let platform = make_platform(Platform::Linux);
        let config_path = detector.config_path(&platform);

        // Should contain .config/claude-code/mcp.json
        let path_str = config_path.to_string_lossy();
        assert!(
            path_str.contains(".config/claude-code/mcp.json")
                || path_str.contains("claude-code") && path_str.ends_with("mcp.json"),
            "Linux config path should use XDG: {}",
            path_str
        );
    }

    // -------------------------------------------------------------------------
    // Test 5: Config path for macOS (Library/Application Support)
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_code_config_path_macos() {
        let detector = ClaudeCodeDetector::new();
        let platform = make_platform(Platform::MacOS);
        let config_path = detector.config_path(&platform);

        // Should contain Library/Application Support/claude-code/mcp.json
        let path_str = config_path.to_string_lossy();
        assert!(
            path_str.contains("Library/Application Support/claude-code/mcp.json")
                || (path_str.contains("claude-code") && path_str.ends_with("mcp.json")),
            "macOS config path should use Library: {}",
            path_str
        );
    }

    // -------------------------------------------------------------------------
    // Test 6: Config path for Windows (APPDATA)
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_code_config_path_windows() {
        let detector = ClaudeCodeDetector::new();

        // Save and set APPDATA for test
        let original = env::var("APPDATA").ok();
        env::set_var("APPDATA", "C:\\Users\\Test\\AppData\\Roaming");

        let platform = make_platform(Platform::Windows);
        let config_path = detector.config_path(&platform);

        // Should contain claude-code\mcp.json
        let path_str = config_path.to_string_lossy();
        assert!(
            path_str.contains("claude-code") && path_str.ends_with("mcp.json"),
            "Windows config path should use APPDATA: {}",
            path_str
        );

        // Restore original
        if let Some(val) = original {
            env::set_var("APPDATA", val);
        } else {
            env::remove_var("APPDATA");
        }
    }

    // -------------------------------------------------------------------------
    // Test 7: Detection via binary
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_code_detection_binary() {
        let detector = ClaudeCodeDetector::new();
        let platform = make_platform(Platform::Linux);

        // Detection depends on whether claude binary is actually installed
        // We test that detection doesn't panic and returns consistent results
        let result = detector.detect(&platform);

        // If claude binary exists, detection should succeed
        if which::which("claude").is_ok() {
            assert!(result.is_some(), "Should detect when claude binary exists");
            let client = result.unwrap();
            assert_eq!(client.client_id, "claude_code");
            assert_eq!(client.client_name, "Claude Code");
            assert_eq!(client.detection_method, DetectionMethod::Binary);
            assert_eq!(client.priority, 1500);
        }
    }

    // -------------------------------------------------------------------------
    // Test 8: Check kdb configured detection
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_code_kdb_configured_check() {
        use std::io::Write;

        let detector = ClaudeCodeDetector::new();

        // Create a temporary config file with kdb configured
        let temp_dir = env::temp_dir();
        let test_config = temp_dir.join("test_claude_mcp.json");

        // Write config with kdb
        let config_with_kdb = r#"{
            "mcpServers": {
                "kdb": {
                    "command": "npx",
                    "args": ["@kindly-software-inc/kdb"],
                    "env": {
                        "KDB_LICENSE_KEY": "test-key"
                    }
                }
            }
        }"#;

        let mut file = std::fs::File::create(&test_config).expect("Failed to create test file");
        file.write_all(config_with_kdb.as_bytes())
            .expect("Failed to write test config");
        drop(file);

        // Check that kdb is detected as configured
        let is_configured = detector.check_kdb_configured(&test_config);
        assert!(is_configured, "Should detect kdb in config");

        // Write config without kdb
        let config_without_kdb = r#"{
            "mcpServers": {
                "other-server": {
                    "command": "other"
                }
            }
        }"#;

        std::fs::write(&test_config, config_without_kdb).expect("Failed to write test config");

        // Check that kdb is NOT detected as configured
        let is_configured = detector.check_kdb_configured(&test_config);
        assert!(!is_configured, "Should not detect kdb when not in config");

        // Cleanup
        std::fs::remove_file(&test_config).ok();
    }

    // -------------------------------------------------------------------------
    // Additional tests for completeness
    // -------------------------------------------------------------------------

    #[test]
    fn test_claude_code_client_id() {
        let detector = ClaudeCodeDetector::new();
        assert_eq!(detector.client_id(), "claude_code");
    }

    #[test]
    fn test_claude_code_client_name() {
        let detector = ClaudeCodeDetector::new();
        assert_eq!(detector.client_name(), "Claude Code");
    }

    #[test]
    fn test_claude_code_supports_all_platforms() {
        let detector = ClaudeCodeDetector::new();

        // Claude Code should support all platforms
        for platform in [
            Platform::Linux,
            Platform::MacOS,
            Platform::Windows,
            Platform::FreeBSD,
            Platform::Unknown,
        ] {
            let platform_info = make_platform(platform);
            assert!(
                detector.supports_platform(&platform_info),
                "Should support {}",
                platform.as_str()
            );
        }
    }

    #[test]
    fn test_claude_code_get_claude_dir() {
        let detector = ClaudeCodeDetector::new();

        // Should return Some path when HOME is set
        if env::var_os("HOME").is_some() || env::var_os("USERPROFILE").is_some() {
            let claude_dir = detector.get_claude_dir();
            assert!(claude_dir.is_some(), "Should return claude dir path");
            let path = claude_dir.unwrap();
            assert!(
                path.to_string_lossy().ends_with(".claude"),
                "Path should end with .claude: {}",
                path.display()
            );
        }
    }

    #[test]
    fn test_claude_code_env_var_detection() {
        let detector = ClaudeCodeDetector::new();
        let platform = make_platform(Platform::Linux);

        // Save original value
        let original = env::var("CLAUDE_CODE_VERSION").ok();

        // Set the environment variable
        env::set_var("CLAUDE_CODE_VERSION", "1.0.0");

        // If no binary or config dir exists, should detect via env var
        // (This test may not trigger env var detection if binary exists)
        let result = detector.detect(&platform);

        // If binary doesn't exist and no config dir, should use env var
        if which::which("claude").is_err() {
            // Check if .claude dir or .claude.json exists
            let claude_dir_exists = detector.get_claude_dir().map(|p| p.exists()).unwrap_or(false);
            let claude_json_exists = env::var_os("HOME")
                .or_else(|| env::var_os("USERPROFILE"))
                .map(|h| PathBuf::from(h).join(".claude.json").exists())
                .unwrap_or(false);

            if !claude_dir_exists && !claude_json_exists {
                assert!(result.is_some(), "Should detect via env var");
                let client = result.unwrap();
                assert_eq!(client.detection_method, DetectionMethod::EnvVar);
            }
        }

        // Restore original
        if let Some(val) = original {
            env::set_var("CLAUDE_CODE_VERSION", val);
        } else {
            env::remove_var("CLAUDE_CODE_VERSION");
        }
    }

    #[test]
    fn test_claude_code_default_impl() {
        // Test Default trait implementation
        let detector: ClaudeCodeDetector = Default::default();
        assert_eq!(detector.client_id(), "claude_code");
    }

    #[test]
    fn test_claude_code_freebsd_uses_xdg() {
        let detector = ClaudeCodeDetector::new();
        let platform = make_platform(Platform::FreeBSD);
        let config_path = detector.config_path(&platform);

        // FreeBSD should use XDG-style path like Linux
        let path_str = config_path.to_string_lossy();
        assert!(
            path_str.contains(".config/claude-code/mcp.json")
                || (path_str.contains("claude-code") && path_str.ends_with("mcp.json")),
            "FreeBSD should use XDG path: {}",
            path_str
        );
    }
}
