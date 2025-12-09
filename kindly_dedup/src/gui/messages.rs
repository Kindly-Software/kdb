//! Message types for Elm architecture

use std::path::PathBuf;

/// Execution mode for deduplication
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Automatically select best mode (CPU vs GPU)
    Auto,
    /// Force CPU-only processing
    Cpu,
    /// Force GPU processing (if available)
    Gpu,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        ExecutionMode::Auto
    }
}

impl ExecutionMode {
    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            ExecutionMode::Auto => "Auto (Recommended)",
            ExecutionMode::Cpu => "CPU Only",
            ExecutionMode::Gpu => "GPU Accelerated",
        }
    }
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// All possible messages in the application
#[derive(Debug, Clone)]
pub enum Message {
    // File selection
    FilePickerClicked,
    FileSelected(Option<PathBuf>),
    FileDropped(PathBuf),

    // Settings
    ThresholdChanged(f32),
    ModeChanged(ExecutionMode),

    // Actions
    StartDeduplication,
    CancelDeduplication,
    PauseDeduplication,  // Toggle pause/resume
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
    pub actual_mode: ExecutionMode,
    pub gpu_available: bool,
}
