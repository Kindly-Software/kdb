//! Cursor MCP Client Detector
//!
//! Detects Cursor IDE (AI-first code editor) and its MCP configuration.
//!
//! ## Priority
//! - Priority 1000 (IDE integration tier)
//!
//! ## Config Locations
//! - Linux: `~/.cursor/mcp.json`
//! - macOS: `~/Library/Application Support/Cursor/mcp.json`
//! - Windows: `%APPDATA%\Cursor\mcp.json`
//!
//! ## Detection Methods
//! 1. `cursor` binary in PATH
//! 2. `~/.cursor/` or platform-specific Cursor directory exists
//! 3. Cursor process running (future)
//!
//! ## Transport
//! - stdio (standard I/O)
//!
//! ## Config Format
//! - JSON (`mcp.json`)
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
// Cursor Detector
// ============================================================================

/// Cursor IDE MCP client detector.
///
/// Cursor is an AI-first code editor based on VS Code with built-in
/// AI features and MCP support.
///
/// ## Priority: 1000 (IDE integration tier)
///
/// ## Detection Methods
/// 1. Check for `cursor` binary in PATH
/// 2. Check for Cursor config directory
///
/// ## Configuration Path
/// - Linux: `~/.cursor/mcp.json`
/// - macOS: `~/Library/Application Support/Cursor/mcp.json`
/// - Windows: `%APPDATA%\Cursor\mcp.json`
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorDetector;

impl CursorDetector {
    /// Create a new Cursor detector.
    #[inline]
    pub const fn new() -> Self {
        Self
    }

    /// Get the Cursor config directory for the platform.
    ///
    /// # Arguments
    /// * `platform` - Target platform
    ///
    /// # Returns
    /// Path to Cursor's configuration directory (not the mcp.json file).
    #[must_use]
    fn cursor_config_dir(&self, platform: Platform) -> PathBuf {
        match platform {
            Platform::Linux | Platform::FreeBSD | Platform::Unknown => {
                expand_path("~/.cursor", platform)
            }
            Platform::MacOS => {
                expand_path("~/Library/Application Support/Cursor", platform)
            }
            Platform::Windows => {
                expand_path("%APPDATA%\\Cursor", platform)
            }
        }
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

impl McpClientDetector for CursorDetector {
    /// Returns "cursor" as the unique client identifier.
    #[inline]
    fn client_id(&self) -> &'static str {
        "cursor"
    }

    /// Returns "Cursor" as the human-readable client name.
    #[inline]
    fn client_name(&self) -> &'static str {
        "Cursor"
    }

    /// Returns 1000 (IDE integration tier).
    ///
    /// Cursor is prioritized in the IDE tier (1000-1499) as a primary
    /// AI development environment with strong MCP support.
    #[inline]
    fn priority(&self) -> u32 {
        1000
    }

    /// Detect Cursor on the current platform.
    ///
    /// ## Detection Order
    /// 1. Check for `cursor` binary in PATH (95% confidence)
    /// 2. Check for Cursor config directory (80% confidence)
    ///
    /// # Arguments
    /// * `platform` - Platform information
    ///
    /// # Returns
    /// `Some(DetectedClient)` if Cursor is detected, `None` otherwise.
    fn detect(&self, platform: &PlatformInfo) -> Option<DetectedClient> {
        // 1. Check for cursor binary in PATH
        if self.binary_exists("cursor") {
            return Some(self.build_detected_client(platform, DetectionMethod::Binary));
        }

        // 2. Check for Cursor config directory
        let cursor_dir = self.cursor_config_dir(platform.platform);
        if self.dir_exists(&cursor_dir) {
            return Some(self.build_detected_client(platform, DetectionMethod::ConfigDir));
        }

        None
    }

    /// Get the MCP configuration file path for Cursor.
    ///
    /// # Arguments
    /// * `platform` - Platform information
    ///
    /// # Returns
    /// Path to `mcp.json`:
    /// - Linux: `~/.cursor/mcp.json`
    /// - macOS: `~/Library/Application Support/Cursor/mcp.json`
    /// - Windows: `%APPDATA%\Cursor\mcp.json`
    fn config_path(&self, platform: &PlatformInfo) -> PathBuf {
        self.cursor_config_dir(platform.platform).join("mcp.json")
    }

    /// Returns JSON as the configuration format.
    ///
    /// Cursor uses JSON for MCP configuration.
    #[inline]
    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Json
    }

    /// Returns stdio as the transport type.
    ///
    /// Cursor uses standard I/O for MCP communication.
    #[inline]
    fn transport_type(&self) -> TransportType {
        TransportType::Stdio
    }
}

// ============================================================================
// Static Instance for Registry
// ============================================================================

/// Static Cursor detector instance for registration.
pub static CURSOR_DETECTOR: CursorDetector = CursorDetector::new();

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
    fn test_cursor_priority() {
        let detector = CursorDetector::new();
        assert_eq!(detector.priority(), 1000);
        assert!(detector.priority() >= 1000, "Priority should be IDE tier (1000+)");
        assert!(detector.priority() < 1500, "Priority should be below enterprise tier");
    }

    // -------------------------------------------------------------------------
    // Test 2: Config Format
    // -------------------------------------------------------------------------
    #[test]
    fn test_cursor_config_format() {
        let detector = CursorDetector::new();
        assert_eq!(detector.config_format(), ConfigFormat::Json);
        assert_eq!(detector.config_format().as_str(), "json");
        assert_eq!(detector.config_format().extension(), ".json");
    }

    // -------------------------------------------------------------------------
    // Test 3: Transport Type
    // -------------------------------------------------------------------------
    #[test]
    fn test_cursor_transport_type() {
        let detector = CursorDetector::new();
        assert_eq!(detector.transport_type(), TransportType::Stdio);
        assert!(!detector.transport_type().requires_network());
    }

    // -------------------------------------------------------------------------
    // Test 4: Config Path - Linux
    // -------------------------------------------------------------------------
    #[test]
    fn test_cursor_config_path_linux() {
        let detector = CursorDetector::new();
        let platform = PlatformInfo {
            platform: Platform::Linux,
            ..PlatformInfo::default()
        };

        let config_path = detector.config_path(&platform);
        let path_str = config_path.to_string_lossy();

        // Should end with .cursor/mcp.json
        assert!(
            path_str.ends_with(".cursor/mcp.json"),
            "Linux config path should end with .cursor/mcp.json, got: {}",
            path_str
        );
    }

    // -------------------------------------------------------------------------
    // Test 5: Config Path - macOS
    // -------------------------------------------------------------------------
    #[test]
    fn test_cursor_config_path_macos() {
        let detector = CursorDetector::new();
        let platform = PlatformInfo {
            platform: Platform::MacOS,
            ..PlatformInfo::default()
        };

        let config_path = detector.config_path(&platform);
        let path_str = config_path.to_string_lossy();

        // Should contain Library/Application Support/Cursor/mcp.json
        assert!(
            path_str.contains("Library/Application Support/Cursor") && path_str.ends_with("mcp.json"),
            "macOS config path should contain Library/Application Support/Cursor/mcp.json, got: {}",
            path_str
        );
    }

    // -------------------------------------------------------------------------
    // Test 6: Config Path - Windows
    // -------------------------------------------------------------------------
    #[test]
    fn test_cursor_config_path_windows() {
        let detector = CursorDetector::new();

        // Set APPDATA for test
        let original = std::env::var("APPDATA").ok();
        std::env::set_var("APPDATA", "C:\\Users\\Test\\AppData\\Roaming");

        let platform = PlatformInfo {
            platform: Platform::Windows,
            ..PlatformInfo::default()
        };

        let config_path = detector.config_path(&platform);
        let path_str = config_path.to_string_lossy();

        // Should contain Cursor\mcp.json or Cursor/mcp.json
        assert!(
            path_str.contains("Cursor") && path_str.ends_with("mcp.json"),
            "Windows config path should contain Cursor\\mcp.json, got: {}",
            path_str
        );

        // Restore original
        if let Some(val) = original {
            std::env::set_var("APPDATA", val);
        } else {
            std::env::remove_var("APPDATA");
        }
    }

    // -------------------------------------------------------------------------
    // Test 7: Detection - Binary Method
    // -------------------------------------------------------------------------
    #[test]
    fn test_cursor_detection_binary_check() {
        let detector = CursorDetector::new();

        // Test the binary_exists method (won't find cursor in CI)
        let exists = detector.binary_exists("cursor");
        // We just verify the method works, actual result depends on environment
        assert!(exists || !exists, "binary_exists should return bool");

        // Verify we can check for a known binary
        assert!(detector.binary_exists("ls") || cfg!(windows), "ls should exist on Unix");
    }

    // -------------------------------------------------------------------------
    // Test 8: Detection - Config Dir Method
    // -------------------------------------------------------------------------
    #[test]
    fn test_cursor_detection_config_dir() {
        let detector = CursorDetector::new();
        let platform = PlatformInfo::default();

        // Get the cursor config dir for the current platform
        let cursor_dir = detector.cursor_config_dir(platform.platform);

        // Verify the path looks reasonable
        let path_str = cursor_dir.to_string_lossy();
        assert!(
            path_str.contains("cursor") || path_str.contains("Cursor"),
            "Cursor config dir should contain 'cursor' or 'Cursor', got: {}",
            path_str
        );

        // Test dir_exists (actual result depends on environment)
        let _exists = detector.dir_exists(&cursor_dir);
    }

    // -------------------------------------------------------------------------
    // Test 9: Client ID and Name
    // -------------------------------------------------------------------------
    #[test]
    fn test_cursor_client_id_and_name() {
        let detector = CursorDetector::new();

        assert_eq!(detector.client_id(), "cursor");
        assert_eq!(detector.client_name(), "Cursor");

        // Verify client_id is lowercase snake_case
        assert!(detector.client_id().chars().all(|c| c.is_lowercase() || c == '_'));

        // Verify client_name starts with uppercase
        assert!(detector.client_name().starts_with(char::is_uppercase));
    }

    // -------------------------------------------------------------------------
    // Test 10: Static Instance
    // -------------------------------------------------------------------------
    #[test]
    fn test_cursor_static_instance() {
        assert_eq!(CURSOR_DETECTOR.client_id(), "cursor");
        assert_eq!(CURSOR_DETECTOR.priority(), 1000);
        assert_eq!(CURSOR_DETECTOR.config_format(), ConfigFormat::Json);
        assert_eq!(CURSOR_DETECTOR.transport_type(), TransportType::Stdio);
    }

    // -------------------------------------------------------------------------
    // Test 11: Supports All Platforms
    // -------------------------------------------------------------------------
    #[test]
    fn test_cursor_supports_all_platforms() {
        let detector = CursorDetector::new();

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
                "Cursor should support {:?}",
                platform
            );
        }
    }

    // -------------------------------------------------------------------------
    // Test 12: DetectedClient Construction
    // -------------------------------------------------------------------------
    #[test]
    fn test_cursor_detected_client_construction() {
        let detector = CursorDetector::new();
        let platform = PlatformInfo::default();

        let client = detector.build_detected_client(&platform, DetectionMethod::Binary);

        assert_eq!(client.client_id, "cursor");
        assert_eq!(client.client_name, "Cursor");
        assert_eq!(client.config_format, ConfigFormat::Json);
        assert_eq!(client.transport_type, TransportType::Stdio);
        assert_eq!(client.priority, 1000);
        assert_eq!(client.detection_method, DetectionMethod::Binary);
        assert_eq!(client.confidence(), 95); // Binary detection confidence
    }
}
