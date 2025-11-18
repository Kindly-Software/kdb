//! Message types for Elm architecture

use std::path::PathBuf;

/// All possible messages in the application
#[derive(Debug, Clone)]
pub enum Message {
    // File selection
    FilePickerClicked,
    FileSelected(Option<PathBuf>),
    FileDropped(PathBuf),

    // Settings
    ThresholdChanged(f32),

    // Actions
    StartDeduplication,
    CancelDeduplication,
    Reset,

    // Background processing updates
    ProgressUpdate,
    DeduplicationComplete(Result<DedupResults, String>),

    // UI events
    Tick,
    AnimationTick,

    // Button hover animations
    HeroButtonHovered,
    HeroButtonUnhovered,

    // Badge hover (no-op, enables hover states)
    BadgeHovered,

    // Documentation link
    OpenDocumentation,

    // Error reporting
    ReportError,

    // Compliance dashboard
    ShowCompliance,
    CloseCompliance,
    VerifyAuditChain,
    ExportComplianceReport,
}

/// Results from deduplication
#[derive(Debug, Clone)]
pub struct DedupResults {
    pub total_documents: usize,
    pub unique_documents: usize,
    pub duplicate_clusters: usize,
    pub processing_time_sec: f64,
    pub throughput_docs_sec: f64,
    pub speedup_vs_python: f64,
    pub output_file: PathBuf,
}
