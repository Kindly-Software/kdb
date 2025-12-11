//! MCP Client Detector System
//!
//! Pluggable detection system for auto-configuring MCP clients.
//!
//! ## Architecture
//! - **McpClientDetector trait**: Interface for client detection
//! - **DetectorRegistryCapsule** (T1 Atomic, 16KB): Lockfree registry with 128 slots
//! - **Built-in detectors** (Phase 2+): Claude Code, Cursor, VSCode, etc.
//!
//! ## Usage
//! ```rust,ignore
//! use kdb_mcp::configure::detectors::{
//!     DetectorRegistryCapsule,
//!     McpClientDetector,
//!     ConfigFormat,
//!     TransportType,
//! };
//! use kdb_mcp::configure::PlatformInfo;
//!
//! // Create registry
//! let registry = DetectorRegistryCapsule::new();
//!
//! // Register detectors
//! static CLAUDE_DETECTOR: ClaudeCodeDetector = ClaudeCodeDetector;
//! registry.register(&CLAUDE_DETECTOR)?;
//!
//! // Detect all clients
//! let platform = PlatformInfo::default();
//! let result = registry.detect_all(&platform);
//!
//! for client in result.clients {
//!     println!("Found: {} at {}", client.client_name, client.config_path.display());
//! }
//! ```
//!
//! ## Priority Tiers
//! | Range | Category | Examples |
//! |-------|----------|----------|
//! | 1500+ | Enterprise | Paid/commercial clients |
//! | 1000-1499 | IDE | Cursor, VSCode, JetBrains |
//! | 500-999 | AI Assistant | Claude Code, ChatGPT, Copilot |
//! | 100-499 | Community | Open-source clients |
//! | 0-99 | Experimental | Development/testing |
//!
//! ## UCE35 Compliance
//! - T1 Atomic tier for registry (lockfree, cache-aligned)
//! - FNV-1a hash for O(1) lookup
//! - Priority-based conflict resolution
//! - Q34 audit trail for detection events
//!
//! ## Performance
//! - Registration: <100ns
//! - Lookup: <120ns (FNV-1a hash + linear probe)
//! - Detect all: <10ms (128 detectors, I/O bound)

// Trait definition (McpClientDetector, enums, DetectedClient)
mod trait_def;

// Registry capsule (DetectorRegistryCapsule)
mod registry;

// ============================================================================
// Built-in Detectors
// ============================================================================

// Claude Code CLI/VSCode detector (Priority 1500 - Enterprise)
mod claude_code;

// Claude Desktop detector (Priority 1400)
mod claude_desktop;

// Cursor IDE detector (Priority 1000 - IDE tier)
mod cursor;

// VS Code detector (Priority 900 - IDE tier)
mod vscode;

// Generic HTTP/SSE fallback detector (Priority 100 - Fallback)
mod generic_http;

// ============================================================================
// Re-exports
// ============================================================================

// Trait and types from trait_def
pub use trait_def::{
    // Core trait
    McpClientDetector,
    // Types
    ConfigFormat,
    TransportType,
    DetectionMethod,
    DetectedClient,
};

// Registry from registry module
pub use registry::{
    // Core capsule
    DetectorRegistryCapsule,
    // Types
    DetectorEntry,
    DetectorHandle,
    RegistryStats,
    DetectionResult,
    // Constants
    MAX_DETECTORS,
    // Utilities
    fnv1a_hash,
};

// Built-in detectors
pub use claude_code::ClaudeCodeDetector;
pub use claude_desktop::ClaudeDesktopDetector;
pub use cursor::CursorDetector;
pub use vscode::VSCodeDetector;

// Generic fallback detector
pub use generic_http::{GenericConfig, GenericHttpDetector, GENERIC_HTTP_DETECTOR};

// Static detector instances for registry
pub use cursor::CURSOR_DETECTOR;
pub use vscode::VSCODE_DETECTOR;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use super::super::platform::PlatformInfo;

    // Simple test detector
    struct SimpleDetector;

    impl McpClientDetector for SimpleDetector {
        fn client_id(&self) -> &'static str { "simple_detector" }
        fn client_name(&self) -> &'static str { "Simple Detector" }
        fn priority(&self) -> u32 { 500 }

        fn detect(&self, _platform: &PlatformInfo) -> Option<DetectedClient> {
            Some(DetectedClient::from_parts(
                "simple_detector",
                "Simple Detector",
                PathBuf::from("/tmp/simple.json"),
                false,
                false,
                DetectionMethod::Binary,
                ConfigFormat::Json,
                TransportType::Stdio,
                500,
            ))
        }

        fn config_path(&self, _platform: &PlatformInfo) -> PathBuf {
            PathBuf::from("/tmp/simple.json")
        }

        fn config_format(&self) -> ConfigFormat { ConfigFormat::Json }
        fn transport_type(&self) -> TransportType { TransportType::Stdio }
    }

    static SIMPLE_DETECTOR: SimpleDetector = SimpleDetector;

    #[test]
    fn test_module_integration() {
        // Test that all re-exports work together
        let registry = DetectorRegistryCapsule::new();
        registry.register(&SIMPLE_DETECTOR).unwrap();

        let handle = registry.lookup("simple_detector").unwrap();
        assert_eq!(handle.detector.client_id(), "simple_detector");
        assert_eq!(handle.detector.config_format(), ConfigFormat::Json);
        assert_eq!(handle.detector.transport_type(), TransportType::Stdio);

        let platform = PlatformInfo::default();
        let result = registry.detect_all(&platform);
        assert_eq!(result.detections, 1);
    }

    #[test]
    fn test_fnv1a_hash_exported() {
        // Verify hash function is accessible
        let hash = fnv1a_hash(b"test");
        assert_ne!(hash, 0);
    }

    #[test]
    fn test_max_detectors_constant() {
        // Verify constant is exported
        assert_eq!(MAX_DETECTORS, 128);
    }
}
