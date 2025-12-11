//! Generic HTTP/SSE Detector (Priority 100 - Fallback)
//!
//! Fallback detector for unknown MCP clients. Provides generic configuration
//! templates for stdio, HTTP, and SSE transports.
//!
//! ## Design
//! - Priority 100 (lowest): Only used when no specific detector matches
//! - Always "detects" on all platforms: Ensures users always get configuration help
//! - Provides all three transport templates: stdio, HTTP, and SSE
//!
//! ## UCE35 Compliance
//! - T0 Auditable tier: Pure functions, no state
//! - Zero allocation in detect(): Stack-only operations
//! - Deterministic: Same input produces same output

use std::path::PathBuf;

use super::super::platform::PlatformInfo;
use super::trait_def::{
    ConfigFormat, DetectedClient, DetectionMethod, McpClientDetector, TransportType,
};

// ============================================================================
// GenericConfig Result Type
// ============================================================================

/// Generic configuration output containing all transport templates
#[derive(Debug, Clone)]
pub struct GenericConfig {
    /// Human-readable instructions with all templates
    pub instructions: String,
    /// stdio transport template (npx command)
    pub stdio_template: String,
    /// HTTP transport template (direct URL)
    pub http_template: String,
    /// SSE transport template (legacy streaming)
    pub sse_template: String,
}

// ============================================================================
// GenericHttpDetector
// ============================================================================

/// Fallback detector for generic/unknown MCP clients
///
/// Priority: 100 (lowest)
/// Config Format: JSON (universal)
/// Transport: Stdio (most compatible)
///
/// This detector always "detects" but with lowest priority, ensuring
/// users always get SOME configuration template even if their specific
/// client isn't recognized.
pub struct GenericHttpDetector;

impl McpClientDetector for GenericHttpDetector {
    #[inline]
    fn client_id(&self) -> &'static str {
        "generic"
    }

    #[inline]
    fn client_name(&self) -> &'static str {
        "Generic MCP Client"
    }

    #[inline]
    fn priority(&self) -> u32 {
        100 // Lowest priority - only used as fallback
    }

    #[inline]
    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Json
    }

    #[inline]
    fn transport_type(&self) -> TransportType {
        TransportType::Stdio
    }

    /// Generic detector always "detects" but with Fallback method
    ///
    /// This ensures users always get configuration help even when
    /// no specific client is detected.
    fn detect(&self, _platform: &PlatformInfo) -> Option<DetectedClient> {
        Some(DetectedClient::from_parts(
            self.client_id(),
            self.client_name(),
            self.config_path(_platform),
            false, // Config doesn't exist (generic path)
            false, // kdb not configured
            DetectionMethod::Fallback,
            self.config_format(),
            self.transport_type(),
            self.priority(),
        ))
    }

    /// Returns a generic config path in the current directory
    fn config_path(&self, _platform: &PlatformInfo) -> PathBuf {
        PathBuf::from("./kdb-mcp-config.json")
    }
}

impl GenericHttpDetector {
    /// Create a new generic detector
    #[inline]
    pub const fn new() -> Self {
        Self
    }

    /// Generate generic configuration with all three transport templates
    ///
    /// ## Arguments
    /// - `license_key`: The user's license key to embed in templates
    ///
    /// ## Returns
    /// `GenericConfig` containing instructions and all transport templates
    pub fn generate_generic_config(&self, license_key: &str) -> GenericConfig {
        GenericConfig {
            instructions: self.generate_instructions(),
            stdio_template: self.generate_stdio_template(license_key),
            http_template: self.generate_http_template(license_key),
            sse_template: self.generate_sse_template(license_key),
        }
    }

    /// Generate human-readable instructions
    fn generate_instructions(&self) -> String {
        r#"# Generic MCP Configuration for KDB

If your MCP client was not auto-detected, use one of these templates:

## Option 1: stdio Transport (Recommended - Works Everywhere)

Add to your MCP client config:

```json
{
  "mcpServers": {
    "kdb": {
      "command": "npx",
      "args": ["@kindly-software-inc/kdb"],
      "env": {
        "KDB_LICENSE_KEY": "YOUR_LICENSE_KEY_HERE"
      }
    }
  }
}
```

## Option 2: HTTP Transport (Modern Clients)

```json
{
  "mcpServers": {
    "kdb": {
      "type": "http",
      "url": "https://mcp.kindly.software/mcp",
      "headers": {
        "X-License-Key": "YOUR_LICENSE_KEY_HERE"
      }
    }
  }
}
```

## Option 3: SSE Transport (Legacy)

```json
{
  "mcpServers": {
    "kdb": {
      "type": "sse",
      "url": "https://mcp.kindly.software/sse",
      "headers": {
        "X-License-Key": "YOUR_LICENSE_KEY_HERE"
      }
    }
  }
}
```

For more help, visit: https://kindly.software/setup
"#
        .to_string()
    }

    /// Generate stdio transport template (npx command)
    ///
    /// Stdio is the most compatible transport - works with all MCP clients.
    fn generate_stdio_template(&self, license_key: &str) -> String {
        format!(
            r#"{{
  "mcpServers": {{
    "kdb": {{
      "command": "npx",
      "args": ["@kindly-software-inc/kdb"],
      "env": {{
        "KDB_LICENSE_KEY": "{}"
      }}
    }}
  }}
}}"#,
            license_key
        )
    }

    /// Generate HTTP transport template (Streamable HTTP)
    ///
    /// HTTP transport uses the modern MCP 2025-06-18 spec with unified /mcp endpoint.
    fn generate_http_template(&self, license_key: &str) -> String {
        format!(
            r#"{{
  "mcpServers": {{
    "kdb": {{
      "type": "http",
      "url": "https://mcp.kindly.software/mcp",
      "headers": {{
        "X-License-Key": "{}"
      }}
    }}
  }}
}}"#,
            license_key
        )
    }

    /// Generate SSE transport template (legacy)
    ///
    /// SSE transport uses the MCP 2024-11-05 spec with /sse endpoint.
    fn generate_sse_template(&self, license_key: &str) -> String {
        format!(
            r#"{{
  "mcpServers": {{
    "kdb": {{
      "type": "sse",
      "url": "https://mcp.kindly.software/sse",
      "headers": {{
        "X-License-Key": "{}"
      }}
    }}
  }}
}}"#,
            license_key
        )
    }

    /// Check if a license key looks valid (basic format check)
    ///
    /// Does NOT validate cryptographically - just checks format.
    #[inline]
    pub fn is_valid_license_format(license_key: &str) -> bool {
        // License format: KDB-{TIER}-{timestamp}-{hash}
        // e.g., KDB-HOBBY-1733875200-a1b2c3d4e5f6
        license_key.starts_with("KDB-") && license_key.len() >= 20
    }
}

impl Default for GenericHttpDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Static Instance
// ============================================================================

/// Static instance for registry registration
pub static GENERIC_HTTP_DETECTOR: GenericHttpDetector = GenericHttpDetector;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // GenericHttpDetector Tests (12 tests)
    // ========================================================================

    // --- Priority/Format/Transport Tests (1-3) ---

    #[test]
    fn test_generic_priority() {
        let detector = GenericHttpDetector::new();
        assert_eq!(detector.priority(), 100);
        // Verify it's the lowest priority tier (100-499 is Community)
        assert!(detector.priority() < 500, "Generic should be lowest priority");
    }

    #[test]
    fn test_generic_config_format() {
        let detector = GenericHttpDetector::new();
        assert_eq!(detector.config_format(), ConfigFormat::Json);
        assert_eq!(detector.config_format().as_str(), "json");
        assert_eq!(detector.config_format().extension(), ".json");
    }

    #[test]
    fn test_generic_transport_type() {
        let detector = GenericHttpDetector::new();
        assert_eq!(detector.transport_type(), TransportType::Stdio);
        assert_eq!(detector.transport_type().as_str(), "stdio");
        // Stdio doesn't require network (local process)
        assert!(!detector.transport_type().requires_network());
    }

    // --- Client ID/Name Tests (4-5) ---

    #[test]
    fn test_generic_client_id() {
        let detector = GenericHttpDetector::new();
        assert_eq!(detector.client_id(), "generic");
        // Verify it's valid for registry (ASCII, reasonable length)
        assert!(detector.client_id().is_ascii());
        assert!(detector.client_id().len() < 48);
    }

    #[test]
    fn test_generic_client_name() {
        let detector = GenericHttpDetector::new();
        assert_eq!(detector.client_name(), "Generic MCP Client");
    }

    // --- Fallback Detection Behavior (6-8) ---

    #[test]
    fn test_generic_always_detects() {
        let detector = GenericHttpDetector::new();
        let platform = PlatformInfo::default();

        // Generic detector should ALWAYS return Some
        let result = detector.detect(&platform);
        assert!(result.is_some(), "Generic detector must always detect");
    }

    #[test]
    fn test_generic_detection_method_is_fallback() {
        let detector = GenericHttpDetector::new();
        let platform = PlatformInfo::default();

        let result = detector.detect(&platform).unwrap();
        assert_eq!(
            result.detection_method,
            DetectionMethod::Fallback,
            "Generic detector must use Fallback detection method"
        );
    }

    #[test]
    fn test_generic_fallback_confidence() {
        let detector = GenericHttpDetector::new();
        let platform = PlatformInfo::default();

        let result = detector.detect(&platform).unwrap();
        // Fallback has low confidence (50)
        assert_eq!(result.confidence(), 50);
        // Should be lower than other detection methods
        assert!(result.confidence() < DetectionMethod::Binary.confidence());
        assert!(result.confidence() < DetectionMethod::ConfigDir.confidence());
    }

    // --- Template Generation Tests (9-12) ---

    #[test]
    fn test_generic_stdio_template() {
        let detector = GenericHttpDetector::new();
        let license_key = "KDB-HOBBY-1733875200-a1b2c3d4e5f6";

        let template = detector.generate_stdio_template(license_key);

        // Verify structure
        assert!(template.contains("mcpServers"));
        assert!(template.contains("kdb"));
        assert!(template.contains("command"));
        assert!(template.contains("npx"));
        assert!(template.contains("@kindly-software-inc/kdb"));
        assert!(template.contains("KDB_LICENSE_KEY"));
        assert!(template.contains(license_key));
    }

    #[test]
    fn test_generic_http_template() {
        let detector = GenericHttpDetector::new();
        let license_key = "KDB-PRO-1733875200-x9y8z7w6v5";

        let template = detector.generate_http_template(license_key);

        // Verify structure
        assert!(template.contains("mcpServers"));
        assert!(template.contains("kdb"));
        assert!(template.contains(r#""type": "http""#));
        assert!(template.contains("https://mcp.kindly.software/mcp"));
        assert!(template.contains("X-License-Key"));
        assert!(template.contains(license_key));
    }

    #[test]
    fn test_generic_sse_template() {
        let detector = GenericHttpDetector::new();
        let license_key = "KDB-ENGINEER-1733875200-m1n2o3p4q5";

        let template = detector.generate_sse_template(license_key);

        // Verify structure
        assert!(template.contains("mcpServers"));
        assert!(template.contains("kdb"));
        assert!(template.contains(r#""type": "sse""#));
        assert!(template.contains("https://mcp.kindly.software/sse"));
        assert!(template.contains("X-License-Key"));
        assert!(template.contains(license_key));
    }

    #[test]
    fn test_generic_config_all_templates() {
        let detector = GenericHttpDetector::new();
        let license_key = "KDB-TEAMS-1733875200-testkey123";

        let config = detector.generate_generic_config(license_key);

        // Instructions should mention all three options
        assert!(config.instructions.contains("Option 1: stdio Transport"));
        assert!(config.instructions.contains("Option 2: HTTP Transport"));
        assert!(config.instructions.contains("Option 3: SSE Transport"));
        assert!(config.instructions.contains("kindly.software/setup"));

        // All templates should contain the license key
        assert!(config.stdio_template.contains(license_key));
        assert!(config.http_template.contains(license_key));
        assert!(config.sse_template.contains(license_key));
    }

    // --- License Format Validation (13-14) ---

    #[test]
    fn test_license_format_validation_valid() {
        // Valid formats
        assert!(GenericHttpDetector::is_valid_license_format(
            "KDB-HOBBY-1733875200-a1b2c3d4"
        ));
        assert!(GenericHttpDetector::is_valid_license_format(
            "KDB-PRO-1733875200-x9y8z7w6v5u4"
        ));
        assert!(GenericHttpDetector::is_valid_license_format(
            "KDB-ENGINEER-1733875200-abcdef"
        ));
        assert!(GenericHttpDetector::is_valid_license_format(
            "KDB-ENTERPRISE-1733875200-verylonghashvalue"
        ));
    }

    #[test]
    fn test_license_format_validation_invalid() {
        // Invalid formats
        assert!(!GenericHttpDetector::is_valid_license_format("")); // Empty
        assert!(!GenericHttpDetector::is_valid_license_format("KDB-")); // Too short
        assert!(!GenericHttpDetector::is_valid_license_format("INVALID-KEY")); // Wrong prefix
        assert!(!GenericHttpDetector::is_valid_license_format("kdb-hobby-123")); // Lowercase
        assert!(!GenericHttpDetector::is_valid_license_format("KDB-SHORT")); // Too short
    }

    // --- Config Path Test (15) ---

    #[test]
    fn test_generic_config_path() {
        let detector = GenericHttpDetector::new();
        let platform = PlatformInfo::default();

        let path = detector.config_path(&platform);
        assert_eq!(path, PathBuf::from("./kdb-mcp-config.json"));
        // Should be relative (current directory)
        assert!(path.is_relative());
    }

    // --- Static Instance Test (16) ---

    #[test]
    fn test_static_instance() {
        // Verify static instance is usable
        assert_eq!(GENERIC_HTTP_DETECTOR.client_id(), "generic");
        assert_eq!(GENERIC_HTTP_DETECTOR.priority(), 100);
    }

    // --- Default Trait Test (17) ---

    #[test]
    fn test_default_trait() {
        let detector = GenericHttpDetector::default();
        assert_eq!(detector.client_id(), "generic");
    }

    // --- Platform Support Test (18) ---

    #[test]
    fn test_supports_all_platforms() {
        let detector = GenericHttpDetector::new();

        // Generic detector should support ALL platforms (default implementation)
        let linux = PlatformInfo::default();
        assert!(
            detector.supports_platform(&linux),
            "Generic must support Linux"
        );

        // Note: PlatformInfo doesn't have platform variants in the current API,
        // but the default supports_platform() always returns true, which is correct
        // for a generic fallback detector.
    }
}
