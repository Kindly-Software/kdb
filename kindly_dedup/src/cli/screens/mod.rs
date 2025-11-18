//! Screen rendering modules for CLI TUI
//!
//! Phase 3: Complete deduplication workflow with 6 screens
//!
//! ## Screens
//! - **Phase 1**: welcome - Welcome screen with pulsing hearts
//! - **Phase 2**: menu - Main menu with interactive options
//! - **Phase 3.1**: file_selection - File/directory browser
//! - **Phase 3.2**: configuration - Settings panel (threshold, threads, features)
//! - **Phase 3.3**: confirmation - Pre-processing review
//! - **Phase 3.4**: processing - Real-time progress with metrics
//! - **Phase 3.5**: results - Success summary with achievements
//!
//! ## UCE34 Framework
//! - **Q10 (Tier)**: T1 Atomic state (MenuStateCapsule, ProgressTrackerCapsule)
//! - **Q13 (Architecture)**: Clear screen hierarchy + state management
//! - **Q14 (Pattern)**: Capsule-based state coordination
//! - **Q28 (Simplicity)**: Each screen has single responsibility
//! - **Q31 (Rust Transform)**: 100% safe Rust, modular architecture
//! - **Q33 (Verification)**: All state verified at compile-time

pub mod configuration;
pub mod confirmation;
pub mod file_selection;
pub mod license_info;
pub mod menu;
pub mod processing;
pub mod results;
pub mod welcome;

pub use configuration::{ConfigurationScreen, DedupConfig};
pub use confirmation::{ConfirmationAction, ConfirmationScreen};
pub use file_selection::FileSelectionScreen;
pub use license_info::render_license_info_screen;
pub use menu::render_main_menu;
pub use processing::ProcessingScreen;
pub use results::{DedupResults, ResultsScreen};
pub use welcome::render_welcome_screen;
