//! McpClientDetector Trait Definition
//!
//! Defines the pluggable interface for MCP client auto-detection.
//!
//! ## Design
//! - Each detector is a stateless, const-constructible struct
//! - Detectors implement `McpClientDetector` trait
//! - Priority-based conflict resolution (higher priority wins)
//! - Detection methods are O(1) or O(log n) for fast iteration
//!
//! ## Priority Tiers
//! - 1500+ : Enterprise/commercial clients (priority support)
//! - 1000-1499: IDE integrations (Cursor, VSCode, etc.)
//! - 500-999 : Major AI assistants (Claude Code, ChatGPT, etc.)
//! - 100-499 : Community/open-source clients
//! - 0-99    : Experimental/development clients
//!
//! ## UCE35 Compliance
//! - T1 Atomic: Trait is used in lockfree registry
//! - Zero allocation: Detection uses stack-only operations
//! - Deterministic: Same input produces same output

use std::path::PathBuf;
use super::super::platform::PlatformInfo;

// ============================================================================
// ConfigFormat Enum
// ============================================================================

/// Configuration file format for MCP clients
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ConfigFormat {
    /// JSON format (most common for MCP clients)
    Json = 0,
    /// YAML format (Kubernetes-style configs)
    Yaml = 1,
    /// TOML format (Rust ecosystem preference)
    Toml = 2,
    /// Unknown/custom format
    Unknown = 255,
}

impl ConfigFormat {
    /// Get format name as string
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            ConfigFormat::Json => "json",
            ConfigFormat::Yaml => "yaml",
            ConfigFormat::Toml => "toml",
            ConfigFormat::Unknown => "unknown",
        }
    }

    /// Get file extension for this format
    #[inline]
    pub const fn extension(&self) -> &'static str {
        match self {
            ConfigFormat::Json => ".json",
            ConfigFormat::Yaml => ".yaml",
            ConfigFormat::Toml => ".toml",
            ConfigFormat::Unknown => "",
        }
    }

    /// Convert from u8
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => ConfigFormat::Json,
            1 => ConfigFormat::Yaml,
            2 => ConfigFormat::Toml,
            _ => ConfigFormat::Unknown,
        }
    }
}

impl From<u8> for ConfigFormat {
    fn from(value: u8) -> Self {
        Self::from_u8(value)
    }
}

// ============================================================================
// TransportType Enum
// ============================================================================

/// MCP transport type supported by client
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TransportType {
    /// Standard I/O transport (most common)
    Stdio = 0,
    /// HTTP transport (REST-style)
    Http = 1,
    /// Server-Sent Events transport (streaming)
    Sse = 2,
    /// Streamable HTTP transport (MCP 2025-06-18)
    StreamableHttp = 3,
    /// WebSocket transport
    WebSocket = 4,
    /// Unknown/custom transport
    Unknown = 255,
}

impl TransportType {
    /// Get transport name as string
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            TransportType::Stdio => "stdio",
            TransportType::Http => "http",
            TransportType::Sse => "sse",
            TransportType::StreamableHttp => "streamable-http",
            TransportType::WebSocket => "websocket",
            TransportType::Unknown => "unknown",
        }
    }

    /// Check if transport requires network connectivity
    #[inline]
    pub const fn requires_network(&self) -> bool {
        matches!(
            self,
            TransportType::Http | TransportType::Sse | TransportType::StreamableHttp | TransportType::WebSocket
        )
    }

    /// Convert from u8
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => TransportType::Stdio,
            1 => TransportType::Http,
            2 => TransportType::Sse,
            3 => TransportType::StreamableHttp,
            4 => TransportType::WebSocket,
            _ => TransportType::Unknown,
        }
    }
}

impl From<u8> for TransportType {
    fn from(value: u8) -> Self {
        Self::from_u8(value)
    }
}

// ============================================================================
// DetectionMethod Enum
// ============================================================================

/// How the client was detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DetectionMethod {
    /// Found executable in PATH
    Binary = 0,
    /// Found configuration directory
    ConfigDir = 1,
    /// Environment variable set
    EnvVar = 2,
    /// Process currently running
    ProcessList = 3,
    /// Registry entry (Windows)
    Registry = 4,
    /// Application bundle (macOS)
    AppBundle = 5,
    /// Desktop entry (Linux)
    DesktopEntry = 6,
    /// Fallback detection (generic detector)
    Fallback = 7,
    /// Unknown detection method
    Unknown = 255,
}

impl DetectionMethod {
    /// Get method name as string
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            DetectionMethod::Binary => "binary",
            DetectionMethod::ConfigDir => "config_dir",
            DetectionMethod::EnvVar => "env_var",
            DetectionMethod::ProcessList => "process_list",
            DetectionMethod::Registry => "registry",
            DetectionMethod::AppBundle => "app_bundle",
            DetectionMethod::DesktopEntry => "desktop_entry",
            DetectionMethod::Fallback => "fallback",
            DetectionMethod::Unknown => "unknown",
        }
    }

    /// Get confidence level (0-100)
    #[inline]
    pub const fn confidence(&self) -> u8 {
        match self {
            DetectionMethod::Binary => 95,       // High confidence
            DetectionMethod::ProcessList => 99,  // Very high confidence
            DetectionMethod::ConfigDir => 80,    // Good confidence
            DetectionMethod::Registry => 90,     // High confidence (Windows)
            DetectionMethod::AppBundle => 90,    // High confidence (macOS)
            DetectionMethod::DesktopEntry => 80, // Good confidence (Linux)
            DetectionMethod::EnvVar => 70,       // Moderate confidence
            DetectionMethod::Fallback => 50,     // Low confidence (generic)
            DetectionMethod::Unknown => 0,       // No confidence
        }
    }

    /// Convert from u8
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => DetectionMethod::Binary,
            1 => DetectionMethod::ConfigDir,
            2 => DetectionMethod::EnvVar,
            3 => DetectionMethod::ProcessList,
            4 => DetectionMethod::Registry,
            5 => DetectionMethod::AppBundle,
            6 => DetectionMethod::DesktopEntry,
            7 => DetectionMethod::Fallback,
            _ => DetectionMethod::Unknown,
        }
    }
}

impl From<u8> for DetectionMethod {
    fn from(value: u8) -> Self {
        Self::from_u8(value)
    }
}

// ============================================================================
// DetectedClient Struct
// ============================================================================

/// Information about a detected MCP client
#[derive(Debug, Clone)]
pub struct DetectedClient {
    /// Unique client identifier (e.g., "claude_code", "cursor")
    pub client_id: &'static str,
    /// Human-readable client name (e.g., "Claude Code", "Cursor")
    pub client_name: &'static str,
    /// Path to configuration file
    pub config_path: PathBuf,
    /// Whether config file exists
    pub config_exists: bool,
    /// Whether kdb is already configured in this client
    pub kdb_configured: bool,
    /// How the client was detected
    pub detection_method: DetectionMethod,
    /// Configuration file format
    pub config_format: ConfigFormat,
    /// Transport type supported
    pub transport_type: TransportType,
    /// Detector priority (higher wins conflicts)
    pub priority: u32,
}

impl DetectedClient {
    /// Create a new detected client
    #[inline]
    pub fn new(
        client_id: &'static str,
        client_name: &'static str,
        config_path: PathBuf,
        detection_method: DetectionMethod,
        config_format: ConfigFormat,
        transport_type: TransportType,
        priority: u32,
    ) -> Self {
        let config_exists = config_path.exists();
        let kdb_configured = if config_exists {
            // Check if kdb is already in the config
            std::fs::read_to_string(&config_path)
                .map(|content| content.contains("kdb") || content.contains("kindly"))
                .unwrap_or(false)
        } else {
            false
        };

        Self {
            client_id,
            client_name,
            config_path,
            config_exists,
            kdb_configured,
            detection_method,
            config_format,
            transport_type,
            priority,
        }
    }

    /// Create from basic info (for testing)
    #[inline]
    pub fn from_parts(
        client_id: &'static str,
        client_name: &'static str,
        config_path: PathBuf,
        config_exists: bool,
        kdb_configured: bool,
        detection_method: DetectionMethod,
        config_format: ConfigFormat,
        transport_type: TransportType,
        priority: u32,
    ) -> Self {
        Self {
            client_id,
            client_name,
            config_path,
            config_exists,
            kdb_configured,
            detection_method,
            config_format,
            transport_type,
            priority,
        }
    }

    /// Get detection confidence (0-100)
    #[inline]
    pub fn confidence(&self) -> u8 {
        self.detection_method.confidence()
    }
}

// ============================================================================
// McpClientDetector Trait
// ============================================================================

/// Trait for pluggable MCP client detection
///
/// ## Implementation Guidelines
/// - Detectors should be stateless (const-constructible)
/// - Detection must be deterministic (same input = same output)
/// - Use O(1) or O(log n) algorithms for detection
/// - Avoid blocking I/O where possible
///
/// ## Priority Guidelines
/// - 1500+ : Enterprise clients (paid support)
/// - 1000-1499: IDE integrations
/// - 500-999 : AI assistants
/// - 100-499 : Community clients
/// - 0-99    : Experimental
pub trait McpClientDetector: Send + Sync {
    /// Unique identifier for this client (lowercase, snake_case)
    ///
    /// Example: "claude_code", "cursor", "vscode_copilot"
    fn client_id(&self) -> &'static str;

    /// Human-readable client name
    ///
    /// Example: "Claude Code", "Cursor", "VS Code Copilot"
    fn client_name(&self) -> &'static str;

    /// Detection priority (higher wins in conflict resolution)
    ///
    /// See priority guidelines in trait documentation.
    fn priority(&self) -> u32;

    /// Attempt to detect the client on this platform
    ///
    /// Returns `Some(DetectedClient)` if client is detected,
    /// `None` if client is not present on this system.
    fn detect(&self, platform: &PlatformInfo) -> Option<DetectedClient>;

    /// Get the configuration file path for this client on the given platform
    fn config_path(&self, platform: &PlatformInfo) -> PathBuf;

    /// Configuration format used by this client
    fn config_format(&self) -> ConfigFormat;

    /// Transport type supported by this client
    fn transport_type(&self) -> TransportType;

    /// Check if this detector supports the given platform
    ///
    /// Default implementation returns true for all platforms.
    /// Override for platform-specific clients.
    #[inline]
    fn supports_platform(&self, _platform: &PlatformInfo) -> bool {
        true
    }

    /// Check if the client binary exists in PATH
    ///
    /// Utility method for binary-based detection.
    #[inline]
    fn binary_exists(&self, binary_name: &str) -> bool {
        which::which(binary_name).is_ok()
    }

    /// Check if a directory exists
    #[inline]
    fn dir_exists(&self, path: &std::path::Path) -> bool {
        path.is_dir()
    }

    /// Check if a file exists
    #[inline]
    fn file_exists(&self, path: &std::path::Path) -> bool {
        path.is_file()
    }

    /// Check if an environment variable is set
    #[inline]
    fn env_var_set(&self, name: &str) -> bool {
        std::env::var(name).is_ok()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_format_as_str() {
        assert_eq!(ConfigFormat::Json.as_str(), "json");
        assert_eq!(ConfigFormat::Yaml.as_str(), "yaml");
        assert_eq!(ConfigFormat::Toml.as_str(), "toml");
        assert_eq!(ConfigFormat::Unknown.as_str(), "unknown");
    }

    #[test]
    fn test_config_format_extension() {
        assert_eq!(ConfigFormat::Json.extension(), ".json");
        assert_eq!(ConfigFormat::Yaml.extension(), ".yaml");
        assert_eq!(ConfigFormat::Toml.extension(), ".toml");
    }

    #[test]
    fn test_transport_type_as_str() {
        assert_eq!(TransportType::Stdio.as_str(), "stdio");
        assert_eq!(TransportType::Http.as_str(), "http");
        assert_eq!(TransportType::Sse.as_str(), "sse");
        assert_eq!(TransportType::StreamableHttp.as_str(), "streamable-http");
    }

    #[test]
    fn test_transport_type_requires_network() {
        assert!(!TransportType::Stdio.requires_network());
        assert!(TransportType::Http.requires_network());
        assert!(TransportType::Sse.requires_network());
        assert!(TransportType::StreamableHttp.requires_network());
        assert!(TransportType::WebSocket.requires_network());
    }

    #[test]
    fn test_detection_method_confidence() {
        assert_eq!(DetectionMethod::Binary.confidence(), 95);
        assert_eq!(DetectionMethod::ProcessList.confidence(), 99);
        assert_eq!(DetectionMethod::ConfigDir.confidence(), 80);
        assert_eq!(DetectionMethod::EnvVar.confidence(), 70);
        assert_eq!(DetectionMethod::Unknown.confidence(), 0);
    }

    #[test]
    fn test_detected_client_from_parts() {
        let client = DetectedClient::from_parts(
            "test_client",
            "Test Client",
            PathBuf::from("/tmp/test.json"),
            false,
            false,
            DetectionMethod::Binary,
            ConfigFormat::Json,
            TransportType::Stdio,
            500,
        );

        assert_eq!(client.client_id, "test_client");
        assert_eq!(client.client_name, "Test Client");
        assert_eq!(client.config_format, ConfigFormat::Json);
        assert_eq!(client.transport_type, TransportType::Stdio);
        assert_eq!(client.priority, 500);
        assert_eq!(client.confidence(), 95);
    }

    #[test]
    fn test_config_format_roundtrip() {
        for i in 0..=3 {
            let format = ConfigFormat::from_u8(i);
            let s = format.as_str();
            assert!(!s.is_empty() || format == ConfigFormat::Unknown);
        }
        assert_eq!(ConfigFormat::from_u8(255), ConfigFormat::Unknown);
    }

    #[test]
    fn test_transport_type_roundtrip() {
        for i in 0..=5 {
            let transport = TransportType::from_u8(i);
            let s = transport.as_str();
            assert!(!s.is_empty() || transport == TransportType::Unknown);
        }
        assert_eq!(TransportType::from_u8(255), TransportType::Unknown);
    }

    #[test]
    fn test_detection_method_roundtrip() {
        for i in 0..=7 {
            let method = DetectionMethod::from_u8(i);
            let s = method.as_str();
            assert!(!s.is_empty() || method == DetectionMethod::Unknown);
        }
        assert_eq!(DetectionMethod::from_u8(255), DetectionMethod::Unknown);
    }

    #[test]
    fn test_detection_method_fallback() {
        assert_eq!(DetectionMethod::Fallback.as_str(), "fallback");
        assert_eq!(DetectionMethod::Fallback.confidence(), 50);
        assert_eq!(DetectionMethod::from_u8(7), DetectionMethod::Fallback);
    }
}
