//! TUI Module - Terminal User Interface for kindly_dedup
//!
//! Interactive terminal interface with:
//! - File browser for corpus selection
//! - Interactive forms for configuration
//! - Real-time progress tracking
//! - Results visualization
//! - Recent files quick access

// TODO: Command workflows not yet implemented
// pub mod commands;

pub mod components;

pub use components::{
    ClusterSample, DedupResults, FieldValue, FileBrowser, FileBrowserAction, FileBrowserCapsule, FileEntry, Form,
    FormBuilder, FormResults, ProgressCapsule, ProgressPhase, ProgressViewer, RecentFileEntry, RecentFilesCapsule,
    RecentFilesManager, RecentFilesMenu, ResultViewer, ResultViewerAction,
};
