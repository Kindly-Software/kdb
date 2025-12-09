//! 💜 Kindly-AV1 Progress Display Module
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! T1 Atomic tier progress tracking with Kindly-branded TUI display.
//!
//! ## Architecture
//!
//! ```text
//! ProgressCapsule (64B, T1 Atomic)
//! ├── current_frame (AtomicU64) - Frame counter
//! ├── total_frames (AtomicU64) - Total frame count
//! ├── bytes_written (AtomicU64) - Output size
//! ├── input_bytes (AtomicU64) - Input size
//! ├── start_time_ns (AtomicU64) - Encoding start
//! ├── last_update_ns (AtomicU64) - Last update timestamp
//! ├── frames_last_second (AtomicU64) - Recent FPS tracking
//! └── _padding (8B) - Cache alignment
//! ```
//!
//! ## Display Components
//!
//! - `ProgressDisplay` - Real-time TUI rendering with branded output
//! - `VideoInfo` - Video metadata for header display
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T1 Atomic tier, <10ns atomic operations
//! - **Chaos**: 64B cache-aligned, 100% lockfree, generation counters
//! - **ASSUM**: All atomics use Acquire/Release for visibility
//! - **B32**: Validated <10ns increment, <50ns snapshot
//! - **T28**: Unit/property/integration tests included

mod capsule;
mod dashboard;
mod display;
mod interactive;
mod keyboard;
mod menu;
mod runner;
mod metrics_capsule;
mod rolling_eta;

pub use capsule::ProgressCapsule;
pub use dashboard::{DashboardRendererCapsule, DashboardState, FinalStats, ProgressSnapshot};
pub use display::{DisplayConfig, ProgressDisplay, VideoInfo};
pub use interactive::{InteractiveSnapshot, InteractiveStateCapsule};
pub use keyboard::{DefaultKeyboardHandler, KeyAction, KeyboardInput};
pub use menu::{CommandMenuCapsule, MenuItem};
pub use runner::DashboardRunner;
pub use metrics_capsule::{MetricsCapsule, MetricsSnapshot};
pub use rolling_eta::RollingEtaCapsule;

#[cfg(feature = "cli-kindly-term")]
pub use keyboard::KindlyTermKeyboardHandler;

#[cfg(feature = "cli-crossterm")]
pub use keyboard::CrosstermKeyboardHandler;

#[cfg(not(any(feature = "cli-kindly-term", feature = "cli-crossterm")))]
pub use keyboard::StubKeyboardHandler;

/// Re-export TuiDashboardCapsule for backwards compatibility
pub type TuiDashboardCapsule = ProgressDisplay;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all public types are accessible
        let progress = ProgressCapsule::new();
        let _display = ProgressDisplay::new(DisplayConfig::default());
        let _video_info = VideoInfo::default();

        // Verify capsule size compliance
        assert_eq!(std::mem::size_of::<ProgressCapsule>(), 64);
        assert_eq!(std::mem::align_of::<ProgressCapsule>(), 64);
    }
}
