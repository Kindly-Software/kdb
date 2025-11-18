//! TUI Components Module
//!
//! Reusable terminal UI components for kindly_dedup:
//! - File browser with tree navigation and multi-select
//! - Form builder with inquire integration
//! - Progress viewer with multi-gauge display
//! - Protection status viewer with 4-layer visualization
//! - Result viewer with tables and charts
//! - Recent files LRU cache
//! - Optimization table with contribution breakdown (educational)
//! - Metrics dashboard with real-time system monitoring

pub mod file_browser;
pub mod form_builder;
pub mod metrics_dashboard;
pub mod optimization_table;
pub mod progress_viewer;
pub mod protection_status;
pub mod recent_files;
pub mod result_viewer;

pub use file_browser::{FileBrowser, FileBrowserAction, FileBrowserCapsule, FileEntry};
pub use form_builder::{FieldValue, Form, FormBuilder, FormResults};
pub use metrics_dashboard::MetricsDashboardCapsule;
pub use optimization_table::{OptimizationEntry, OptimizationTableCapsule};
pub use progress_viewer::{ProgressCapsule, ProgressPhase, ProgressViewer};
pub use protection_status::{LayerStatus, ProtectionStatusCapsule, ProtectionStatusViewer};
pub use recent_files::{RecentFileEntry, RecentFilesCapsule, RecentFilesManager, RecentFilesMenu};
pub use result_viewer::{ClusterSample, DedupResults, ResultViewer, ResultViewerAction};
