//! kindly_dedup - Premium GUI (DEFAULT BINARY)
//!
//! Mac-level UX with Byzantine purple + gold branding
//!
//! # Architecture
//! - Iced Elm architecture: Predictable state management
//! - Native file dialogs: Professional UX (rfd crate)
//! - Background processing: tokio::spawn_blocking (non-blocking UI)
//! - Progress updates: Atomic counters (lockfree)
//! - Real pipeline: DedupPipeline with CPU detection
//!
//! # Performance
//! - Startup: <500ms (instant)
//! - UI refresh: 60 FPS (smooth)
//! - Processing: 60K docs/sec sequential, 576K docs/sec parallel (16 cores)
//!
//! # Design Philosophy
//! - 70% neutral backgrounds (professional)
//! - 20% purple accents (branding)
//! - 10% gold highlights (premium)
//! - Mac-level polish: Large text, generous spacing, smooth animations
//!
//! # Binaries
//! - `kindly_dedup` - **PREMIUM DEFAULT GUI (iced)** - Use this!
//! - `kindly_dedup_cli` - Command-line interface for scripting
//! - `kindly_dedup_iced` - Explicit iced GUI (same as default)
//! - `kindly_dedup_gui` - DEPRECATED egui GUI (removed in v1.15.0)

use iced::{window, Application, Settings};
use kindly_dedup::gui::KindlyDedupApp;

fn main() -> iced::Result {
    KindlyDedupApp::run(Settings {
        window: window::Settings {
            size: (900, 1000),
            min_size: Some((600, 800)),
            resizable: true,
            decorations: true,
            ..Default::default()
        },
        antialiasing: true,
        default_font: iced::Font::with_name("system-ui"),
        default_text_size: 14.0.into(),
        ..Default::default()
    })
}
