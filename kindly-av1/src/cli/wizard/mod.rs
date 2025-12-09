//! Kindly-AV1 Wizard Module
//!
//! User-friendly guided setup for non-technical users.
//!
//! ## Features
//! - Persistent user preferences (`~/.kindly-av1/preferences.json`)
//! - Recent files history (last 5 files in `~/.kindly-av1/recent.json`)
//! - User choice to technical parameter mapping
//! - Zero external dependencies (hand-written JSON parsing)
//! - Interactive TUI with arrow key navigation (cli-kindly-term feature)
//!
//! ## Framework Compliance
//! - **Chaos**: Simple file I/O, no complex coordination needed
//! - **ASSUM**: All file I/O assumptions documented in implementations
//! - **UCE34**: Correctness over optimization (Q1-Q28 simple-coding)

pub mod file_browser;
pub mod flow;
pub mod mapping;
pub mod preferences;
pub mod recent;
pub mod steps;
pub mod terminal;
pub mod tui;

pub use flow::{WizardFlowCapsule, WizardState};
pub use mapping::{
    estimate_output_size, estimate_time, format_size, format_time, map_to_encoding_options,
    EncodingOptions, QualityGoal, SpeedChoice,
};
pub use preferences::{PreferencesError, UserPreferences};
pub use recent::{RecentFile, RecentFiles, RecentFilesError};
pub use steps::{render_step_0, render_step_1, render_step_2, render_step_3, render_step_4, WizardContext};
pub use terminal::TerminalStateCapsule;
pub use tui::{WizardTuiCapsule, SelectionListCapsule, keys, box_chars, read_key, enable_raw_mode, disable_raw_mode};
pub use file_browser::{FileBrowserCapsule, FileBrowserState, FileEntry};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all types are exported
        let _prefs = UserPreferences::default();
        let _recent = RecentFiles::default();
    }
}
