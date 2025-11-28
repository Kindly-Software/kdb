//! OBS Studio Integration Module for kindly-av1
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! This module provides OBS integration features for streamers and content creators
//! to display real-time encoding progress in their streams without requiring an OBS plugin.
//!
//! ## Architecture
//!
//! Three-phase integration with progressive capabilities:
//!
//! ### Phase 1: Text File Output (T1 Atomic)
//! - `ObsStatusWriterCapsule` - Rate-limited file output for OBS Text (GDI+)
//! - Atomic write-then-rename for corruption prevention
//! - Three formats: simple, multiline, JSON
//! - ~1s update latency, works with any OBS version
//!
//! ### Phase 2: HTTP Overlay Server (T8+T1)
//! - `ObsOverlayServerCapsule` - HTTP server for browser source overlays
//! - WebSocket push for <100ms real-time updates
//! - Pre-built branded HTML templates
//! - Zero-config OBS setup (just add browser source URL)
//!
//! ### Phase 3: OBS WebSocket Client (T8+T1)
//! - `ObsWebSocketCapsule` - Direct OBS WebSocket control
//! - Scene automation (auto-switch on start/complete/error)
//! - Direct text source updates (no file/HTTP overhead)
//! - Requires OBS 28.0+ with WebSocket enabled
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 Tier-appropriate (T1/T6/T8)
//! - **COCA**: 100% lockfree, cache-aligned capsules
//! - **ASSUM**: File I/O documented, 99.5%+ safe
//! - **B32**: <1% CPU overhead target
//! - **T28**: Unit/property/integration tests per phase
//!
//! ## Example Usage
//!
//! ```ignore
//! // Phase 1: Text file output
//! kindly-av1 encode input.mp4 -o output.av1 --obs-status ~/obs-status.txt
//!
//! // Phase 2: HTTP overlay server
//! kindly-av1 encode input.mp4 -o output.av1 --obs-server 9876
//!
//! // Phase 3: OBS WebSocket control
//! kindly-av1 encode input.mp4 -o output.av1 --obs-websocket localhost:4455
//! ```

mod status_writer;

pub use status_writer::{
    ObsStatusWriterCapsule, ObsStatusFormat, ObsStatusError, ObsStatusSnapshot,
};

// Phase 2 exports (HTTP Overlay Server)
#[cfg(feature = "obs-overlay")]
mod server;
#[cfg(feature = "obs-overlay")]
mod templates;
#[cfg(feature = "obs-overlay")]
pub use server::{ObsOverlayServerCapsule, ProgressSender, ServerError, ServerSnapshot, ServerState};
#[cfg(feature = "obs-overlay")]
pub use templates::{render_overlay_html, OverlayStyle};

// Phase 3 exports (OBS WebSocket Client)
#[cfg(feature = "obs-websocket")]
mod websocket;
#[cfg(feature = "obs-websocket")]
pub use websocket::{ConnectionState as ObsConnectionState, ObsError, ObsSceneConfig, ObsWebSocketCapsule};

/// OBS integration options parsed from CLI
#[derive(Debug, Clone, Default)]
pub struct ObsOptions {
    /// Phase 1: Text file output path (None = disabled)
    pub status_file: Option<std::path::PathBuf>,

    /// Phase 1: Output format (simple/multiline/json)
    pub status_format: ObsStatusFormat,

    /// Phase 1: Update interval in milliseconds (100-5000)
    pub status_interval_ms: u32,

    /// Phase 2: HTTP overlay server port (0 = disabled)
    #[cfg(feature = "obs-overlay")]
    pub server_port: u16,

    /// Phase 3: OBS WebSocket URL (None = disabled)
    #[cfg(feature = "obs-websocket")]
    pub websocket_url: Option<String>,

    /// Phase 3: OBS WebSocket password
    #[cfg(feature = "obs-websocket")]
    pub websocket_password: Option<String>,

    /// Phase 3: Text source name to update
    #[cfg(feature = "obs-websocket")]
    pub text_source: Option<String>,

    /// Phase 3: Scene to switch to on encoding start
    #[cfg(feature = "obs-websocket")]
    pub scene_encoding: Option<String>,

    /// Phase 3: Scene to switch to on encoding complete
    #[cfg(feature = "obs-websocket")]
    pub scene_complete: Option<String>,

    /// Phase 3: Scene to switch to on encoding error
    #[cfg(feature = "obs-websocket")]
    pub scene_error: Option<String>,
}

impl ObsOptions {
    /// Check if any OBS integration is enabled
    pub fn is_enabled(&self) -> bool {
        if self.status_file.is_some() {
            return true;
        }

        #[cfg(feature = "obs-overlay")]
        if self.server_port > 0 {
            return true;
        }

        #[cfg(feature = "obs-websocket")]
        if self.websocket_url.is_some() {
            return true;
        }

        false
    }

    /// Check if text file output is enabled
    pub fn has_status_file(&self) -> bool {
        self.status_file.is_some()
    }

    /// Check if HTTP server is enabled
    #[cfg(feature = "obs-overlay")]
    pub fn has_server(&self) -> bool {
        self.server_port > 0
    }

    /// Check if OBS WebSocket is enabled
    #[cfg(feature = "obs-websocket")]
    pub fn has_websocket(&self) -> bool {
        self.websocket_url.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obs_options_default_disabled() {
        let opts = ObsOptions::default();
        assert!(!opts.is_enabled());
        assert!(!opts.has_status_file());
    }

    #[test]
    fn test_obs_options_status_file_enabled() {
        let opts = ObsOptions {
            status_file: Some(std::path::PathBuf::from("/tmp/obs-status.txt")),
            ..Default::default()
        };
        assert!(opts.is_enabled());
        assert!(opts.has_status_file());
    }
}
