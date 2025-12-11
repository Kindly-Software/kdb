//! Claude Desktop Detector (Priority 1400)
//!
//! Auto-detection for Anthropic's Claude Desktop application.
//!
//! ## Detection Methods
//! - **macOS**: `/Applications/Claude.app` app bundle exists
//! - **Windows**: `%APPDATA%\Claude` directory exists (registry check TBD)
//! - **Linux**: `~/.config/Claude` directory exists (if Linux build released)
//!
//! ## Configuration
//! - **Format**: JSON
//! - **Transport**: stdio
//! - **Config Locations**:
//!   - macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
//!   - Windows: `%APPDATA%\Claude\claude_desktop_config.json`
//!   - Linux: `~/.config/Claude/claude_desktop_config.json`
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
//! ## Priority
//! 1400 (IDE tier - Claude Desktop is a standalone app, higher than CLI)
//!
//! ## UCE35 Compliance
//! - Stateless detector (const-constructible)
//! - Deterministic detection
//! - O(1) platform-specific checks

use std::path::{Path, PathBuf};

use super::super::platform::{Platform, PlatformInfo};
use super::{ConfigFormat, DetectedClient, DetectionMethod, McpClientDetector, TransportType};

/// Claude Desktop application detector
///
/// Detects Anthropic's Claude Desktop app across macOS, Windows, and Linux.
/// Uses platform-specific detection methods for reliability.
pub struct ClaudeDesktopDetector;

impl ClaudeDesktopDetector {
    /// Create a new Claude Desktop detector
    #[inline]
    pub const fn new() -> Self {
        Self
    }

    /// Detect Claude Desktop on macOS
    ///
    /// Checks for `/Applications/Claude.app` bundle.
    #[inline]
    fn detect_macos(&self) -> bool {
        Path::new("/Applications/Claude.app").exists()
    }

    /// Detect Claude Desktop on Windows
    ///
    /// Checks for `%APPDATA%\Claude` directory.
    /// TODO: Add Windows registry check for more reliable detection.
    #[inline]
    fn detect_windows(&self) -> bool {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let claude_dir = PathBuf::from(appdata).join("Claude");
            return claude_dir.exists();
        }
        false
    }

    /// Detect Claude Desktop on Linux
    ///
    /// Checks for `~/.config/Claude` directory.
    /// Note: Claude Desktop for Linux may not exist yet.
    #[inline]
    fn detect_linux(&self) -> bool {
        if let Ok(home) = std::env::var("HOME") {
            let claude_dir = PathBuf::from(home).join(".config").join("Claude");
            return claude_dir.exists();
        }
        false
    }

    /// Get the detection method used for this platform
    #[inline]
    fn get_detection_method(&self, platform: &PlatformInfo) -> DetectionMethod {
        match platform.platform {
            Platform::MacOS => DetectionMethod::AppBundle,
            Platform::Windows => DetectionMethod::ConfigDir, // Registry if implemented
            _ => DetectionMethod::ConfigDir,
        }
    }

    /// Build a DetectedClient from detection results
    #[inline]
    fn build_detected_client(
        &self,
        platform: &PlatformInfo,
        detection_method: DetectionMethod,
    ) -> DetectedClient {
        let config_path = self.config_path(platform);
        DetectedClient::new(
            self.client_id(),
            self.client_name(),
            config_path,
            detection_method,
            self.config_format(),
            self.transport_type(),
            self.priority(),
        )
    }
}

impl Default for ClaudeDesktopDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl McpClientDetector for ClaudeDesktopDetector {
    /// Unique identifier: "claude_desktop"
    #[inline]
    fn client_id(&self) -> &'static str {
        "claude_desktop"
    }

    /// Human-readable name: "Claude Desktop"
    #[inline]
    fn client_name(&self) -> &'static str {
        "Claude Desktop"
    }

    /// Priority: 1400 (IDE tier for standalone app)
    #[inline]
    fn priority(&self) -> u32 {
        1400
    }

    /// Configuration format: JSON
    #[inline]
    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Json
    }

    /// Transport type: stdio
    #[inline]
    fn transport_type(&self) -> TransportType {
        TransportType::Stdio
    }

    /// Detect Claude Desktop on the current platform
    ///
    /// Returns `Some(DetectedClient)` if Claude Desktop is installed,
    /// `None` otherwise.
    fn detect(&self, platform: &PlatformInfo) -> Option<DetectedClient> {
        let detected = match platform.platform {
            Platform::MacOS => self.detect_macos(),
            Platform::Windows => self.detect_windows(),
            Platform::Linux => self.detect_linux(),
            Platform::FreeBSD => self.detect_linux(), // Use Linux detection for FreeBSD
            Platform::Unknown => false,
        };

        if detected {
            Some(self.build_detected_client(platform, self.get_detection_method(platform)))
        } else {
            None
        }
    }

    /// Get the configuration file path for Claude Desktop
    ///
    /// Platform-specific paths:
    /// - macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
    /// - Windows: `%APPDATA%\Claude\claude_desktop_config.json`
    /// - Linux: `~/.config/Claude/claude_desktop_config.json`
    fn config_path(&self, platform: &PlatformInfo) -> PathBuf {
        match platform.platform {
            Platform::MacOS => {
                // ~/Library/Application Support/Claude/claude_desktop_config.json
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("Claude")
                    .join("claude_desktop_config.json")
            }
            Platform::Windows => {
                // %APPDATA%\Claude\claude_desktop_config.json
                let appdata = std::env::var("APPDATA").unwrap_or_else(|_| {
                    let userprofile =
                        std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users".to_string());
                    format!("{}\\AppData\\Roaming", userprofile)
                });
                PathBuf::from(appdata)
                    .join("Claude")
                    .join("claude_desktop_config.json")
            }
            Platform::Linux | Platform::FreeBSD | Platform::Unknown => {
                // ~/.config/Claude/claude_desktop_config.json
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                PathBuf::from(home)
                    .join(".config")
                    .join("Claude")
                    .join("claude_desktop_config.json")
            }
        }
    }

    /// Check if Claude Desktop supports this platform
    ///
    /// Currently supported on macOS and Windows.
    /// Linux support is conditional (directory must exist).
    #[inline]
    fn supports_platform(&self, platform: &PlatformInfo) -> bool {
        matches!(
            platform.platform,
            Platform::MacOS | Platform::Windows | Platform::Linux
        )
    }
}

// SAFETY: ClaudeDesktopDetector is stateless (no interior mutability)
unsafe impl Send for ClaudeDesktopDetector {}
unsafe impl Sync for ClaudeDesktopDetector {}

// ============================================================================
// Tests (8 tests as specified)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a mock PlatformInfo for testing
    fn mock_platform(platform: Platform) -> PlatformInfo {
        use super::super::super::platform::Architecture;
        PlatformInfo::new(
            platform,
            Architecture::X86_64,
            true,  // home_valid
            true,  // config_valid
            false, // is_wsl
        )
    }

    // -------------------------------------------------------------------------
    // Test 1: test_claude_desktop_priority()
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_desktop_priority() {
        let detector = ClaudeDesktopDetector::new();
        assert_eq!(detector.priority(), 1400);
    }

    // -------------------------------------------------------------------------
    // Test 2: test_claude_desktop_config_format()
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_desktop_config_format() {
        let detector = ClaudeDesktopDetector::new();
        assert_eq!(detector.config_format(), ConfigFormat::Json);
    }

    // -------------------------------------------------------------------------
    // Test 3: test_claude_desktop_transport()
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_desktop_transport() {
        let detector = ClaudeDesktopDetector::new();
        assert_eq!(detector.transport_type(), TransportType::Stdio);
    }

    // -------------------------------------------------------------------------
    // Test 4: test_claude_desktop_config_path_macos()
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_desktop_config_path_macos() {
        let detector = ClaudeDesktopDetector::new();
        let platform = mock_platform(Platform::MacOS);

        let config_path = detector.config_path(&platform);
        let path_str = config_path.to_string_lossy();

        // Should contain Library/Application Support/Claude
        assert!(
            path_str.contains("Library/Application Support/Claude"),
            "macOS config path should contain Library/Application Support/Claude, got: {}",
            path_str
        );
        // Should end with claude_desktop_config.json
        assert!(
            path_str.ends_with("claude_desktop_config.json"),
            "Config path should end with claude_desktop_config.json, got: {}",
            path_str
        );
    }

    // -------------------------------------------------------------------------
    // Test 5: test_claude_desktop_config_path_windows()
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_desktop_config_path_windows() {
        // Save original APPDATA
        let original_appdata = std::env::var("APPDATA").ok();

        // Set test APPDATA
        std::env::set_var("APPDATA", "C:\\Users\\Test\\AppData\\Roaming");

        let detector = ClaudeDesktopDetector::new();
        let platform = mock_platform(Platform::Windows);

        let config_path = detector.config_path(&platform);
        let path_str = config_path.to_string_lossy();

        // Should contain Claude directory
        assert!(
            path_str.contains("Claude"),
            "Windows config path should contain Claude, got: {}",
            path_str
        );
        // Should end with claude_desktop_config.json
        assert!(
            path_str.ends_with("claude_desktop_config.json"),
            "Config path should end with claude_desktop_config.json, got: {}",
            path_str
        );
        // Should use Windows path with APPDATA
        assert!(
            path_str.contains("AppData") || path_str.contains("Roaming"),
            "Windows config path should be in AppData, got: {}",
            path_str
        );

        // Restore original APPDATA
        if let Some(val) = original_appdata {
            std::env::set_var("APPDATA", val);
        } else {
            std::env::remove_var("APPDATA");
        }
    }

    // -------------------------------------------------------------------------
    // Test 6: test_claude_desktop_config_path_linux()
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_desktop_config_path_linux() {
        let detector = ClaudeDesktopDetector::new();
        let platform = mock_platform(Platform::Linux);

        let config_path = detector.config_path(&platform);
        let path_str = config_path.to_string_lossy();

        // Should contain .config/Claude
        assert!(
            path_str.contains(".config/Claude"),
            "Linux config path should contain .config/Claude, got: {}",
            path_str
        );
        // Should end with claude_desktop_config.json
        assert!(
            path_str.ends_with("claude_desktop_config.json"),
            "Config path should end with claude_desktop_config.json, got: {}",
            path_str
        );
    }

    // -------------------------------------------------------------------------
    // Test 7: test_claude_desktop_detection_macos()
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_desktop_detection_macos() {
        let detector = ClaudeDesktopDetector::new();

        // Test detection method for macOS
        let platform = mock_platform(Platform::MacOS);
        let detection_method = detector.get_detection_method(&platform);
        assert_eq!(
            detection_method,
            DetectionMethod::AppBundle,
            "macOS should use AppBundle detection method"
        );

        // Note: Actual detection depends on whether /Applications/Claude.app exists
        // We test the detection method, not the actual presence
    }

    // -------------------------------------------------------------------------
    // Test 8: test_claude_desktop_detection_windows()
    // -------------------------------------------------------------------------
    #[test]
    fn test_claude_desktop_detection_windows() {
        let detector = ClaudeDesktopDetector::new();

        // Test detection method for Windows
        let platform = mock_platform(Platform::Windows);
        let detection_method = detector.get_detection_method(&platform);
        assert_eq!(
            detection_method,
            DetectionMethod::ConfigDir,
            "Windows should use ConfigDir detection method (until registry support added)"
        );

        // Note: Actual detection depends on %APPDATA%\Claude existence
        // We test the detection method, not the actual presence
    }

    // -------------------------------------------------------------------------
    // Additional tests for completeness
    // -------------------------------------------------------------------------

    #[test]
    fn test_claude_desktop_client_id() {
        let detector = ClaudeDesktopDetector::new();
        assert_eq!(detector.client_id(), "claude_desktop");
    }

    #[test]
    fn test_claude_desktop_client_name() {
        let detector = ClaudeDesktopDetector::new();
        assert_eq!(detector.client_name(), "Claude Desktop");
    }

    #[test]
    fn test_claude_desktop_supports_platform() {
        let detector = ClaudeDesktopDetector::new();

        // Should support macOS, Windows, and Linux
        assert!(detector.supports_platform(&mock_platform(Platform::MacOS)));
        assert!(detector.supports_platform(&mock_platform(Platform::Windows)));
        assert!(detector.supports_platform(&mock_platform(Platform::Linux)));

        // Should not support Unknown
        assert!(!detector.supports_platform(&mock_platform(Platform::Unknown)));
    }

    #[test]
    fn test_claude_desktop_default() {
        let detector = ClaudeDesktopDetector::default();
        assert_eq!(detector.priority(), 1400);
        assert_eq!(detector.client_id(), "claude_desktop");
    }

    #[test]
    fn test_claude_desktop_detection_linux() {
        let detector = ClaudeDesktopDetector::new();

        // Test detection method for Linux
        let platform = mock_platform(Platform::Linux);
        let detection_method = detector.get_detection_method(&platform);
        assert_eq!(
            detection_method,
            DetectionMethod::ConfigDir,
            "Linux should use ConfigDir detection method"
        );
    }

    #[test]
    fn test_claude_desktop_freebsd_fallback() {
        let detector = ClaudeDesktopDetector::new();

        // FreeBSD should use Linux-style paths
        let platform = mock_platform(Platform::FreeBSD);
        let config_path = detector.config_path(&platform);
        let path_str = config_path.to_string_lossy();

        assert!(
            path_str.contains(".config/Claude"),
            "FreeBSD should use Linux-style config path, got: {}",
            path_str
        );
    }
}
