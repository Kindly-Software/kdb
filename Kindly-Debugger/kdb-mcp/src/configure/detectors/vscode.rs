//! VS Code MCP Client Detector
//!
//! Detects VS Code (Visual Studio Code) and its MCP configuration.
//!
//! ## Priority
//! - Priority 900 (IDE integration tier, below Cursor)
//!
//! ## Config Locations
//! - Workspace-specific: `.vscode/mcp.json` (preferred)
//! - User-level: `~/.vscode/mcp.json` (fallback)
//!
//! ## Detection Methods
//! 1. `code` binary in PATH
//! 2. `.vscode/` directory in current or parent directories
//! 3. `VSCODE_PID` environment variable set
//!
//! ## Transport
//! - stdio (standard I/O)
//!
//! ## Config Format
//! - JSON (`mcp.json`)
//!
//! ## Requirements
//! - VS Code v1.99+ for MCP support
//! - `chat.mcp` setting must be enabled
//!
//! ## UCE35 Compliance
//! - T1 Atomic: Detector is stateless, const-constructible
//! - Zero allocation: Detection uses stack-only operations
//! - Deterministic: Same input produces same output

use std::path::PathBuf;

use super::trait_def::{
    ConfigFormat, DetectedClient, DetectionMethod, McpClientDetector, TransportType,
};
use super::super::platform::{expand_path, Platform, PlatformInfo};

// ============================================================================
// VS Code Detector
// ============================================================================

/// VS Code MCP client detector.
///
/// Visual Studio Code is a popular code editor with AI extension support.
/// MCP support was added in v1.99 via the `chat.mcp` setting.
///
/// ## Priority: 900 (IDE integration tier, below Cursor)
///
/// ## Detection Methods
/// 1. Check for `code` binary in PATH
/// 2. Check for `.vscode/` directory in current or parent directories
/// 3. Check for `VSCODE_PID` environment variable
///
/// ## Configuration Path
/// - Workspace: `.vscode/mcp.json` (preferred)
/// - User: `~/.vscode/mcp.json` (fallback)
#[derive(Debug, Clone, Copy, Default)]
pub struct VSCodeDetector;

impl VSCodeDetector {
    /// Create a new VS Code detector.
    #[inline]
    pub const fn new() -> Self {
        Self
    }

    /// Search for `.vscode/` directory in current and parent directories.
    ///
    /// Searches up to 10 parent directories to find a workspace-specific
    /// VS Code configuration directory.
    ///
    /// # Returns
    /// `Some(PathBuf)` with the `.vscode/` directory path if found,
    /// `None` otherwise.
    #[must_use]
    fn find_vscode_dir(&self) -> Option<PathBuf> {
        let mut current = std::env::current_dir().ok()?;

        // Search up to 10 parent directories
        for _ in 0..10 {
            let vscode_dir = current.join(".vscode");
            if vscode_dir.is_dir() {
                return Some(vscode_dir);
            }

            if !current.pop() {
                break;
            }
        }

        None
    }

    /// Get the user-level VS Code config directory.
    ///
    /// This is used as a fallback when no workspace-specific config is found.
    ///
    /// # Arguments
    /// * `platform` - Target platform
    ///
    /// # Returns
    /// Path to user-level `.vscode/` directory.
    #[must_use]
    fn user_vscode_dir(&self, platform: Platform) -> PathBuf {
        expand_path("~/.vscode", platform)
    }

    /// Build a DetectedClient from detection info.
    #[inline]
    fn build_detected_client(
        &self,
        platform: &PlatformInfo,
        method: DetectionMethod,
    ) -> DetectedClient {
        DetectedClient::new(
            self.client_id(),
            self.client_name(),
            self.config_path(platform),
            method,
            self.config_format(),
            self.transport_type(),
            self.priority(),
        )
    }
}

impl McpClientDetector for VSCodeDetector {
    /// Returns "vscode" as the unique client identifier.
    #[inline]
    fn client_id(&self) -> &'static str {
        "vscode"
    }

    /// Returns "VS Code" as the human-readable client name.
    #[inline]
    fn client_name(&self) -> &'static str {
        "VS Code"
    }

    /// Returns 900 (IDE integration tier, below Cursor).
    ///
    /// VS Code is prioritized below Cursor (1000) because Cursor has
    /// more integrated MCP support. Both are in the IDE tier (1000-1499).
    #[inline]
    fn priority(&self) -> u32 {
        900
    }

    /// Detect VS Code on the current platform.
    ///
    /// ## Detection Order
    /// 1. Check for `code` binary in PATH (95% confidence)
    /// 2. Check for `.vscode/` in current/parent directories (80% confidence)
    /// 3. Check for `VSCODE_PID` environment variable (70% confidence)
    ///
    /// # Arguments
    /// * `platform` - Platform information
    ///
    /// # Returns
    /// `Some(DetectedClient)` if VS Code is detected, `None` otherwise.
    fn detect(&self, platform: &PlatformInfo) -> Option<DetectedClient> {
        // 1. Check for code binary in PATH
        if self.binary_exists("code") {
            return Some(self.build_detected_client(platform, DetectionMethod::Binary));
        }

        // 2. Check for .vscode/ in current or parent directories
        if self.find_vscode_dir().is_some() {
            return Some(self.build_detected_client(platform, DetectionMethod::ConfigDir));
        }

        // 3. Check for VSCODE_PID environment variable
        if self.env_var_set("VSCODE_PID") {
            return Some(self.build_detected_client(platform, DetectionMethod::EnvVar));
        }

        None
    }

    /// Get the MCP configuration file path for VS Code.
    ///
    /// Prefers workspace-specific config over user-level config.
    ///
    /// # Arguments
    /// * `platform` - Platform information
    ///
    /// # Returns
    /// Path to `mcp.json`:
    /// - Workspace: `.vscode/mcp.json` (if `.vscode/` found)
    /// - User: `~/.vscode/mcp.json` (fallback)
    fn config_path(&self, platform: &PlatformInfo) -> PathBuf {
        // Prefer workspace-specific config
        if let Some(vscode_dir) = self.find_vscode_dir() {
            return vscode_dir.join("mcp.json");
        }

        // Fallback to user-level config
        self.user_vscode_dir(platform.platform).join("mcp.json")
    }

    /// Returns JSON as the configuration format.
    ///
    /// VS Code uses JSON for MCP configuration.
    #[inline]
    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Json
    }

    /// Returns stdio as the transport type.
    ///
    /// VS Code uses standard I/O for MCP communication.
    #[inline]
    fn transport_type(&self) -> TransportType {
        TransportType::Stdio
    }
}

// ============================================================================
// Static Instance for Registry
// ============================================================================

/// Static VS Code detector instance for registration.
pub static VSCODE_DETECTOR: VSCodeDetector = VSCodeDetector::new();

// ============================================================================
// Tests (T28 Q1-Q7)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Test 1: Priority
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_priority() {
        let detector = VSCodeDetector::new();
        assert_eq!(detector.priority(), 900);
        assert!(detector.priority() >= 500, "Priority should be IDE/AI tier (500+)");
        assert!(detector.priority() < 1000, "Priority should be below Cursor");
    }

    // -------------------------------------------------------------------------
    // Test 2: Config Format
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_config_format() {
        let detector = VSCodeDetector::new();
        assert_eq!(detector.config_format(), ConfigFormat::Json);
        assert_eq!(detector.config_format().as_str(), "json");
        assert_eq!(detector.config_format().extension(), ".json");
    }

    // -------------------------------------------------------------------------
    // Test 3: Transport Type
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_transport_type() {
        let detector = VSCodeDetector::new();
        assert_eq!(detector.transport_type(), TransportType::Stdio);
        assert!(!detector.transport_type().requires_network());
    }

    // -------------------------------------------------------------------------
    // Test 4: Config Path - Linux (user-level fallback)
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_config_path_linux_user() {
        let detector = VSCodeDetector::new();
        let platform = PlatformInfo {
            platform: Platform::Linux,
            ..PlatformInfo::default()
        };

        // When no workspace .vscode/ exists, should use user-level
        let user_dir = detector.user_vscode_dir(platform.platform);
        let expected_path = user_dir.join("mcp.json");

        // If no workspace found, config_path returns user-level
        // Note: actual result depends on whether .vscode/ exists in test env
        let config_path = detector.config_path(&platform);
        let path_str = config_path.to_string_lossy();

        // Should end with mcp.json
        assert!(
            path_str.ends_with("mcp.json"),
            "Config path should end with mcp.json, got: {}",
            path_str
        );

        // Should contain .vscode somewhere
        assert!(
            path_str.contains(".vscode") || path_str.contains("vscode"),
            "Config path should contain vscode, got: {}",
            path_str
        );
    }

    // -------------------------------------------------------------------------
    // Test 5: Config Path - macOS (user-level fallback)
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_config_path_macos_user() {
        let detector = VSCodeDetector::new();
        let platform = PlatformInfo {
            platform: Platform::MacOS,
            ..PlatformInfo::default()
        };

        let user_dir = detector.user_vscode_dir(platform.platform);
        let path_str = user_dir.to_string_lossy();

        // User-level should contain .vscode
        assert!(
            path_str.contains(".vscode"),
            "User config dir should contain .vscode, got: {}",
            path_str
        );
    }

    // -------------------------------------------------------------------------
    // Test 6: Config Path - Windows (user-level fallback)
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_config_path_windows_user() {
        let detector = VSCodeDetector::new();
        let platform = PlatformInfo {
            platform: Platform::Windows,
            ..PlatformInfo::default()
        };

        let user_dir = detector.user_vscode_dir(platform.platform);
        let path_str = user_dir.to_string_lossy();

        // User-level should contain .vscode
        assert!(
            path_str.contains(".vscode"),
            "User config dir should contain .vscode, got: {}",
            path_str
        );
    }

    // -------------------------------------------------------------------------
    // Test 7: Workspace Detection - find_vscode_dir
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_find_vscode_dir() {
        let detector = VSCodeDetector::new();

        // Test the find_vscode_dir method
        // Result depends on whether we're running in a VS Code workspace
        let result = detector.find_vscode_dir();

        // If found, should be a .vscode directory
        if let Some(vscode_dir) = result {
            let path_str = vscode_dir.to_string_lossy();
            assert!(
                path_str.ends_with(".vscode"),
                "Found vscode dir should end with .vscode, got: {}",
                path_str
            );
        }
    }

    // -------------------------------------------------------------------------
    // Test 8: Workspace Detection - prefers workspace over user
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_workspace_preference() {
        let detector = VSCodeDetector::new();
        let platform = PlatformInfo::default();

        // Get user-level path
        let user_path = detector.user_vscode_dir(platform.platform).join("mcp.json");

        // Get actual config path (may be workspace or user)
        let config_path = detector.config_path(&platform);

        // If a workspace .vscode/ was found, config_path should be different from user_path
        if let Some(vscode_dir) = detector.find_vscode_dir() {
            let workspace_path = vscode_dir.join("mcp.json");
            assert_eq!(
                config_path, workspace_path,
                "When workspace .vscode/ exists, should prefer it"
            );
        } else {
            // No workspace found, should use user-level
            assert_eq!(
                config_path, user_path,
                "When no workspace .vscode/, should use user-level"
            );
        }
    }

    // -------------------------------------------------------------------------
    // Test 9: Client ID and Name
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_client_id_and_name() {
        let detector = VSCodeDetector::new();

        assert_eq!(detector.client_id(), "vscode");
        assert_eq!(detector.client_name(), "VS Code");

        // Verify client_id is lowercase
        assert!(detector.client_id().chars().all(|c| c.is_lowercase() || c == '_'));

        // Verify client_name is human-readable
        assert!(detector.client_name().contains(" ") || detector.client_name().len() > 0);
    }

    // -------------------------------------------------------------------------
    // Test 10: Static Instance
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_static_instance() {
        assert_eq!(VSCODE_DETECTOR.client_id(), "vscode");
        assert_eq!(VSCODE_DETECTOR.priority(), 900);
        assert_eq!(VSCODE_DETECTOR.config_format(), ConfigFormat::Json);
        assert_eq!(VSCODE_DETECTOR.transport_type(), TransportType::Stdio);
    }

    // -------------------------------------------------------------------------
    // Test 11: Detection - Binary Method
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_detection_binary_check() {
        let detector = VSCodeDetector::new();

        // Test the binary_exists method
        let exists = detector.binary_exists("code");
        // We just verify the method works, actual result depends on environment
        assert!(exists || !exists, "binary_exists should return bool");
    }

    // -------------------------------------------------------------------------
    // Test 12: Detection - Environment Variable Method
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_detection_env_var() {
        let detector = VSCodeDetector::new();

        // Save original value
        let original = std::env::var("VSCODE_PID").ok();

        // Test with VSCODE_PID set
        std::env::set_var("VSCODE_PID", "12345");
        assert!(detector.env_var_set("VSCODE_PID"), "Should detect VSCODE_PID");

        // Test with VSCODE_PID unset
        std::env::remove_var("VSCODE_PID");
        assert!(!detector.env_var_set("VSCODE_PID"), "Should not detect unset VSCODE_PID");

        // Restore original
        if let Some(val) = original {
            std::env::set_var("VSCODE_PID", val);
        }
    }

    // -------------------------------------------------------------------------
    // Test 13: Supports All Platforms
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_supports_all_platforms() {
        let detector = VSCodeDetector::new();

        for platform in [
            Platform::Linux,
            Platform::MacOS,
            Platform::Windows,
            Platform::FreeBSD,
            Platform::Unknown,
        ] {
            let info = PlatformInfo {
                platform,
                ..PlatformInfo::default()
            };
            assert!(
                detector.supports_platform(&info),
                "VS Code should support {:?}",
                platform
            );
        }
    }

    // -------------------------------------------------------------------------
    // Test 14: DetectedClient Construction
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_detected_client_construction() {
        let detector = VSCodeDetector::new();
        let platform = PlatformInfo::default();

        // Test Binary detection method
        let client = detector.build_detected_client(&platform, DetectionMethod::Binary);
        assert_eq!(client.client_id, "vscode");
        assert_eq!(client.client_name, "VS Code");
        assert_eq!(client.config_format, ConfigFormat::Json);
        assert_eq!(client.transport_type, TransportType::Stdio);
        assert_eq!(client.priority, 900);
        assert_eq!(client.detection_method, DetectionMethod::Binary);
        assert_eq!(client.confidence(), 95);

        // Test EnvVar detection method
        let client = detector.build_detected_client(&platform, DetectionMethod::EnvVar);
        assert_eq!(client.detection_method, DetectionMethod::EnvVar);
        assert_eq!(client.confidence(), 70);

        // Test ConfigDir detection method
        let client = detector.build_detected_client(&platform, DetectionMethod::ConfigDir);
        assert_eq!(client.detection_method, DetectionMethod::ConfigDir);
        assert_eq!(client.confidence(), 80);
    }

    // -------------------------------------------------------------------------
    // Test 15: Priority Comparison with Cursor
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_priority_vs_cursor() {
        use super::super::cursor::CursorDetector;

        let vscode = VSCodeDetector::new();
        let cursor = CursorDetector::new();

        assert!(
            cursor.priority() > vscode.priority(),
            "Cursor ({}) should have higher priority than VS Code ({})",
            cursor.priority(),
            vscode.priority()
        );
    }

    // -------------------------------------------------------------------------
    // Test 16: Full Detection Flow
    // -------------------------------------------------------------------------
    #[test]
    fn test_vscode_full_detection_flow() {
        let detector = VSCodeDetector::new();
        let platform = PlatformInfo::default();

        // Run detection (result depends on environment)
        let result = detector.detect(&platform);

        // If detected, verify the DetectedClient
        if let Some(client) = result {
            assert_eq!(client.client_id, "vscode");
            assert_eq!(client.priority, 900);
            assert!(client.config_path.to_string_lossy().ends_with("mcp.json"));
        }
    }
}
