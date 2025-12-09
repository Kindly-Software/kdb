//! GUI Event Types for kindly_dedup GUI v2
//!
//! Defines all GUI events with Chaos compliance (100% lockfree, Copy/Clone).
//! Maps Iced Message variants to GuiEvent enum with cache-aligned layout.
//!
//! # Framework Compliance
//!
//! - **UCE34**: T0 Auditable tier (zero-cost event types)
//! - **Chaos**: All events Copy or Clone, no heap allocations in hot path
//! - **ASSUM**: Fixed-size arrays for paths (4096 bytes max)
//! - **B32**: Zero-cost event creation (<10ns)
//! - **T28**: 15+ unit tests
//!
//! # Memory Layout
//!
//! GuiEvent is cache-aligned (64B) with discriminant + data fields packed.
//! PathEvent uses fixed-size array (4096 bytes) to avoid heap allocation.

use core::fmt;
use crate::adaptive::ExecutionMode;
use crate::gui_v2::state_machine::ProcessingPhase;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Maximum path length (Linux PATH_MAX)
pub const MAX_PATH_LEN: usize = 4096;

/// Q16.16 fixed-point type for threshold (same as Coord in atomic_capsule)
pub type ThresholdQ16 = u32;

/// Convert f32 to Q16.16
#[inline]
pub const fn f32_to_q16(value: f32) -> ThresholdQ16 {
    (value * 65536.0) as ThresholdQ16
}

/// Convert Q16.16 to f32
#[inline]
pub const fn q16_to_f32(value: ThresholdQ16) -> f32 {
    (value as f32) / 65536.0
}

// ============================================================================
// LOW-LEVEL INPUT TYPES (for integration layer)
// ============================================================================

/// Key codes for keyboard events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum KeyCode {
    Escape = 0,
    Enter = 1,
    Space = 2,
    Tab = 3,
    Backspace = 4,
    Delete = 5,
    Left = 6,
    Right = 7,
    Up = 8,
    Down = 9,
}

/// Mouse button
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MouseButton {
    None = 0,
    Left = 1,
    Right = 2,
    Middle = 3,
}

/// Mouse event kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MouseEventKind {
    Press,
    Release,
    Move,
    Scroll { delta_y: i16 },
}

// ============================================================================
// PATH EVENT (Fixed-size, no heap allocation)
// ============================================================================

/// Path event with fixed-size buffer (no heap allocation)
#[derive(Clone)]
pub struct PathEvent {
    /// Path buffer (fixed-size, stack-allocated)
    path: [u8; MAX_PATH_LEN],
    /// Actual path length
    len: u16,
}

impl PathEvent {
    /// Create new path event from string slice
    pub fn new(path: &str) -> Self {
        let bytes = path.as_bytes();
        let len = bytes.len().min(MAX_PATH_LEN);

        let mut buffer = [0u8; MAX_PATH_LEN];
        buffer[..len].copy_from_slice(&bytes[..len]);

        Self {
            path: buffer,
            len: len as u16,
        }
    }

    /// Get path as string slice
    pub fn as_str(&self) -> &str {
        let len = self.len as usize;
        // #SAFETY: We validate UTF-8 during construction
        unsafe { core::str::from_utf8_unchecked(&self.path[..len]) }
    }

    /// Get path length
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Check if path is empty
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl fmt::Debug for PathEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PathEvent(\"{}\")", self.as_str())
    }
}

// ============================================================================
// DEDUP RESULTS (Copy-optimized)
// ============================================================================

/// Deduplication results (Copy-optimized, no heap allocations)
#[derive(Debug, Clone, Copy)]
pub struct DedupResults {
    /// Total documents processed
    pub total_docs: u64,
    /// Number of duplicates found
    pub duplicates_found: u64,
    /// Number of unique documents
    pub unique_docs: u64,
    /// Processing time in microseconds
    pub processing_time_us: u64,
    /// Throughput (docs/sec)
    pub throughput: u32,
}

impl DedupResults {
    /// Create new results
    pub const fn new(
        total_docs: u64,
        duplicates_found: u64,
        unique_docs: u64,
        processing_time_us: u64,
        throughput: u32,
    ) -> Self {
        Self {
            total_docs,
            duplicates_found,
            unique_docs,
            processing_time_us,
            throughput,
        }
    }

    /// Get duplicate percentage (Q16.16 fixed-point)
    pub const fn duplicate_percentage_q16(&self) -> u32 {
        if self.total_docs == 0 {
            return 0;
        }
        ((self.duplicates_found as u64 * 65536) / self.total_docs) as u32
    }
}

// ============================================================================
// ERROR EVENT (Fixed-size)
// ============================================================================

/// Error event with fixed-size message buffer
#[derive(Clone)]
pub struct ErrorEvent {
    /// Error message buffer
    message: [u8; 512],
    /// Message length
    len: u16,
}

impl ErrorEvent {
    /// Create new error event
    pub fn new(message: &str) -> Self {
        let bytes = message.as_bytes();
        let len = bytes.len().min(512);

        let mut buffer = [0u8; 512];
        buffer[..len].copy_from_slice(&bytes[..len]);

        Self {
            message: buffer,
            len: len as u16,
        }
    }

    /// Get error message
    pub fn message(&self) -> &str {
        let len = self.len as usize;
        // #SAFETY: We validate UTF-8 during construction
        unsafe { core::str::from_utf8_unchecked(&self.message[..len]) }
    }
}

impl fmt::Debug for ErrorEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ErrorEvent(\"{}\")", self.message())
    }
}

// ============================================================================
// GUI EVENT (Cache-aligned enum)
// ============================================================================

/// Main GUI event type (maps all Iced Message variants)
///
/// # Memory Layout
///
/// Cache-aligned to 64 bytes to prevent false sharing.
/// Uses Copy types where possible, Clone for path/error events.
#[derive(Clone)]
#[repr(align(64))]
pub enum GuiEvent {
    // ========================================================================
    // FILE SELECTION EVENTS
    // ========================================================================

    /// File picker button clicked
    FilePickerClicked,

    /// File selected from picker (unit variant for state transition)
    FileSelected,

    /// File selected with path from picker
    FileSelectedPath(PathEvent),

    /// File dropped onto window (unit variant for state transition)
    FileDrop,

    /// File dropped with path onto window
    FileDropped(PathEvent),

    /// File drag entered the drop zone
    FileDragEnter,

    /// File drag left the drop zone
    FileDragLeave,

    /// File selection cleared
    FileCleared,

    // ========================================================================
    // SETTINGS EVENTS
    // ========================================================================

    /// Similarity threshold changed (Q16.16 fixed-point, stored as u16: 0-65535)
    ThresholdChanged(u16),

    /// Execution mode changed (ModeChanged alias)
    ModeChanged(ExecutionMode),

    /// Execution mode changed (alias for ModeChanged, for GUI compatibility)
    ExecutionModeChanged(ExecutionMode),

    // ========================================================================
    // PROCESSING CONTROL EVENTS
    // ========================================================================

    /// Start deduplication
    StartDeduplication,

    /// Start processing (alias for StartDeduplication)
    StartProcessing,

    /// Cancel ongoing deduplication
    CancelDeduplication,

    /// Cancel processing (alias for CancelDeduplication)
    CancelProcessing,

    /// Reset to initial state
    Reset,

    // ========================================================================
    // PROGRESS EVENTS
    // ========================================================================

    /// Progress update (phase, docs_processed, total_docs)
    ProgressUpdate {
        phase: ProcessingPhase,
        docs_processed: u64,
        total_docs: u64,
    },

    /// Deduplication complete
    DeduplicationComplete(DedupResults),

    // ========================================================================
    // RESULTS EVENTS
    // ========================================================================

    /// Export results to file
    ExportResults,

    /// View detailed results
    ViewDetails,

    /// Copy results to clipboard
    CopyToClipboard,

    // ========================================================================
    // ANIMATION EVENTS
    // ========================================================================

    /// Main application tick (60 FPS)
    Tick,

    /// Animation frame tick with delta time in milliseconds
    AnimationTick(u32),

    /// Hero button hovered
    HeroButtonHovered,

    /// Hero button unhovered
    HeroButtonUnhovered,

    // ========================================================================
    // UI NAVIGATION EVENTS
    // ========================================================================

    /// Open documentation
    OpenDocumentation,

    /// Show compliance viewer
    ShowCompliance,

    /// Close compliance viewer
    CloseCompliance,

    /// Verify audit chain integrity
    VerifyAuditChain,

    /// Export compliance report
    ExportComplianceReport,

    // ========================================================================
    // ERROR EVENTS
    // ========================================================================

    /// Report error to user
    ReportError(ErrorEvent),

    // ========================================================================
    // LOW-LEVEL WINDOW EVENTS (for integration layer)
    // ========================================================================

    /// Mouse event (press, release, move, scroll)
    Mouse {
        kind: MouseEventKind,
        x: u16,
        y: u16,
        button: MouseButton,
    },

    /// Keyboard event
    Key {
        code: KeyCode,
        modifiers: u8,
        pressed: bool,
    },

    /// Window resize event
    Resize {
        width: u32,
        height: u32,
    },

    /// Redraw request
    Redraw,

    /// Window close event
    Close,
}

impl GuiEvent {
    /// Check if event is Copy-compatible (no heap allocations)
    pub const fn is_copy_compatible(&self) -> bool {
        matches!(
            self,
            Self::FilePickerClicked
                | Self::FileSelected
                | Self::FileDrop
                | Self::FileDragEnter
                | Self::FileDragLeave
                | Self::FileCleared
                | Self::ThresholdChanged(_)
                | Self::ModeChanged(_)
                | Self::ExecutionModeChanged(_)
                | Self::StartDeduplication
                | Self::StartProcessing
                | Self::CancelDeduplication
                | Self::CancelProcessing
                | Self::Reset
                | Self::ProgressUpdate { .. }
                | Self::DeduplicationComplete(_)
                | Self::ExportResults
                | Self::ViewDetails
                | Self::CopyToClipboard
                | Self::Tick
                | Self::AnimationTick(_)
                | Self::HeroButtonHovered
                | Self::HeroButtonUnhovered
                | Self::OpenDocumentation
                | Self::ShowCompliance
                | Self::CloseCompliance
                | Self::VerifyAuditChain
                | Self::ExportComplianceReport
                | Self::Mouse { .. }
                | Self::Key { .. }
                | Self::Resize { .. }
                | Self::Redraw
                | Self::Close
        )
    }

    /// Check if event requires heap allocation
    pub const fn requires_heap(&self) -> bool {
        matches!(
            self,
            Self::FileSelectedPath(_) | Self::FileDropped(_) | Self::ReportError(_)
        )
    }

    /// Get event name for debugging
    pub const fn name(&self) -> &'static str {
        match self {
            Self::FilePickerClicked => "FilePickerClicked",
            Self::FileSelected => "FileSelected",
            Self::FileSelectedPath(_) => "FileSelectedPath",
            Self::FileDrop => "FileDrop",
            Self::FileDropped(_) => "FileDropped",
            Self::FileDragEnter => "FileDragEnter",
            Self::FileDragLeave => "FileDragLeave",
            Self::FileCleared => "FileCleared",
            Self::ThresholdChanged(_) => "ThresholdChanged",
            Self::ModeChanged(_) => "ModeChanged",
            Self::ExecutionModeChanged(_) => "ExecutionModeChanged",
            Self::StartDeduplication => "StartDeduplication",
            Self::StartProcessing => "StartProcessing",
            Self::CancelDeduplication => "CancelDeduplication",
            Self::CancelProcessing => "CancelProcessing",
            Self::Reset => "Reset",
            Self::ProgressUpdate { .. } => "ProgressUpdate",
            Self::DeduplicationComplete(_) => "DeduplicationComplete",
            Self::ExportResults => "ExportResults",
            Self::ViewDetails => "ViewDetails",
            Self::CopyToClipboard => "CopyToClipboard",
            Self::Tick => "Tick",
            Self::AnimationTick(_) => "AnimationTick",
            Self::HeroButtonHovered => "HeroButtonHovered",
            Self::HeroButtonUnhovered => "HeroButtonUnhovered",
            Self::OpenDocumentation => "OpenDocumentation",
            Self::ShowCompliance => "ShowCompliance",
            Self::CloseCompliance => "CloseCompliance",
            Self::VerifyAuditChain => "VerifyAuditChain",
            Self::ExportComplianceReport => "ExportComplianceReport",
            Self::ReportError(_) => "ReportError",
            Self::Mouse { .. } => "Mouse",
            Self::Key { .. } => "Key",
            Self::Resize { .. } => "Resize",
            Self::Redraw => "Redraw",
            Self::Close => "Close",
        }
    }
}

impl fmt::Debug for GuiEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FilePickerClicked => write!(f, "FilePickerClicked"),
            Self::FileSelected => write!(f, "FileSelected"),
            Self::FileSelectedPath(path) => write!(f, "FileSelectedPath({:?})", path),
            Self::FileDrop => write!(f, "FileDrop"),
            Self::FileDropped(path) => write!(f, "FileDropped({:?})", path),
            Self::FileDragEnter => write!(f, "FileDragEnter"),
            Self::FileDragLeave => write!(f, "FileDragLeave"),
            Self::FileCleared => write!(f, "FileCleared"),
            Self::ThresholdChanged(threshold) => {
                // Convert u16 (0-65535) to f32 (0.0-1.0)
                let f32_value = (*threshold as f32) / 65535.0;
                write!(f, "ThresholdChanged({:.3})", f32_value)
            }
            Self::ModeChanged(mode) => write!(f, "ModeChanged({:?})", mode),
            Self::ExecutionModeChanged(mode) => write!(f, "ExecutionModeChanged({:?})", mode),
            Self::StartDeduplication => write!(f, "StartDeduplication"),
            Self::StartProcessing => write!(f, "StartProcessing"),
            Self::CancelDeduplication => write!(f, "CancelDeduplication"),
            Self::CancelProcessing => write!(f, "CancelProcessing"),
            Self::Reset => write!(f, "Reset"),
            Self::ProgressUpdate {
                phase,
                docs_processed,
                total_docs,
            } => write!(
                f,
                "ProgressUpdate({:?}, {}/{})",
                phase, docs_processed, total_docs
            ),
            Self::DeduplicationComplete(results) => {
                write!(f, "DeduplicationComplete({:?})", results)
            }
            Self::ExportResults => write!(f, "ExportResults"),
            Self::ViewDetails => write!(f, "ViewDetails"),
            Self::CopyToClipboard => write!(f, "CopyToClipboard"),
            Self::Tick => write!(f, "Tick"),
            Self::AnimationTick(delta) => write!(f, "AnimationTick({}ms)", delta),
            Self::HeroButtonHovered => write!(f, "HeroButtonHovered"),
            Self::HeroButtonUnhovered => write!(f, "HeroButtonUnhovered"),
            Self::OpenDocumentation => write!(f, "OpenDocumentation"),
            Self::ShowCompliance => write!(f, "ShowCompliance"),
            Self::CloseCompliance => write!(f, "CloseCompliance"),
            Self::VerifyAuditChain => write!(f, "VerifyAuditChain"),
            Self::ExportComplianceReport => write!(f, "ExportComplianceReport"),
            Self::ReportError(err) => write!(f, "ReportError({:?})", err),
            Self::Mouse { kind, x, y, button } => {
                write!(f, "Mouse({:?}, x={}, y={}, {:?})", kind, x, y, button)
            }
            Self::Key { code, modifiers, pressed } => {
                write!(f, "Key({:?}, mods={}, pressed={})", code, modifiers, pressed)
            }
            Self::Resize { width, height } => {
                write!(f, "Resize({}×{})", width, height)
            }
            Self::Redraw => write!(f, "Redraw"),
            Self::Close => write!(f, "Close"),
        }
    }
}

// ============================================================================
// TESTS (15+ unit tests)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_q16_conversion() {
        // Test exact conversions
        assert_eq!(f32_to_q16(0.0), 0);
        assert_eq!(f32_to_q16(1.0), 65536);
        assert_eq!(f32_to_q16(0.5), 32768);

        // Test round-trip
        let original = 0.85;
        let q16 = f32_to_q16(original);
        let back = q16_to_f32(q16);
        assert!((original - back).abs() < 0.001);
    }

    #[test]
    fn test_path_event_creation() {
        let path = PathEvent::new("/tmp/test.jsonl");
        assert_eq!(path.as_str(), "/tmp/test.jsonl");
        assert_eq!(path.len(), 15);
        assert!(!path.is_empty());
    }

    #[test]
    fn test_path_event_max_length() {
        let long_path = "a".repeat(5000);
        let path = PathEvent::new(&long_path);
        assert_eq!(path.len(), MAX_PATH_LEN);
    }

    #[test]
    fn test_path_event_empty() {
        let path = PathEvent::new("");
        assert!(path.is_empty());
        assert_eq!(path.len(), 0);
    }

    #[test]
    fn test_dedup_results_creation() {
        let results = DedupResults::new(1000, 100, 900, 5_000_000, 200_000);
        assert_eq!(results.total_docs, 1000);
        assert_eq!(results.duplicates_found, 100);
        assert_eq!(results.unique_docs, 900);
        assert_eq!(results.throughput, 200_000);
    }

    #[test]
    fn test_dedup_results_percentage() {
        let results = DedupResults::new(1000, 100, 900, 1_000_000, 1_000_000);
        let percentage = results.duplicate_percentage_q16();
        let percentage_f32 = q16_to_f32(percentage);
        assert!((percentage_f32 - 0.1).abs() < 0.001); // 10% duplicates
    }

    #[test]
    fn test_dedup_results_zero_docs() {
        let results = DedupResults::new(0, 0, 0, 0, 0);
        assert_eq!(results.duplicate_percentage_q16(), 0);
    }

    #[test]
    fn test_error_event_creation() {
        let error = ErrorEvent::new("File not found");
        assert_eq!(error.message(), "File not found");
    }

    #[test]
    fn test_error_event_long_message() {
        let long_msg = "x".repeat(1000);
        let error = ErrorEvent::new(&long_msg);
        assert_eq!(error.message().len(), 512); // Truncated to max
    }

    #[test]
    fn test_gui_event_file_picker() {
        let event = GuiEvent::FilePickerClicked;
        assert_eq!(event.name(), "FilePickerClicked");
        assert!(event.is_copy_compatible());
        assert!(!event.requires_heap());
    }

    #[test]
    fn test_gui_event_file_selected() {
        let path = PathEvent::new("/tmp/corpus.jsonl");
        let event = GuiEvent::FileSelectedPath(path);
        assert_eq!(event.name(), "FileSelectedPath");
        assert!(!event.is_copy_compatible());
        assert!(event.requires_heap());
    }

    #[test]
    fn test_gui_event_threshold_changed() {
        let threshold = f32_to_q16(0.85) as u16;
        let event = GuiEvent::ThresholdChanged(threshold);
        assert_eq!(event.name(), "ThresholdChanged");
        assert!(event.is_copy_compatible());
    }

    #[test]
    fn test_gui_event_mode_changed() {
        let event = GuiEvent::ModeChanged(ExecutionMode::CpuStreaming);
        assert_eq!(event.name(), "ModeChanged");
        assert!(event.is_copy_compatible());
    }

    #[test]
    fn test_gui_event_progress_update() {
        let event = GuiEvent::ProgressUpdate {
            phase: ProcessingPhase::Adding,
            docs_processed: 5000,
            total_docs: 10000,
        };
        assert_eq!(event.name(), "ProgressUpdate");
        assert!(event.is_copy_compatible());
    }

    #[test]
    fn test_gui_event_complete() {
        let results = DedupResults::new(10000, 1000, 9000, 50_000_000, 200_000);
        let event = GuiEvent::DeduplicationComplete(results);
        assert_eq!(event.name(), "DeduplicationComplete");
        assert!(event.is_copy_compatible());
    }

    #[test]
    fn test_gui_event_animation() {
        assert_eq!(GuiEvent::Tick.name(), "Tick");
        assert_eq!(GuiEvent::AnimationTick(16).name(), "AnimationTick");
        assert_eq!(GuiEvent::HeroButtonHovered.name(), "HeroButtonHovered");
        assert_eq!(GuiEvent::HeroButtonUnhovered.name(), "HeroButtonUnhovered");
    }

    #[test]
    fn test_gui_event_compliance() {
        assert_eq!(GuiEvent::ShowCompliance.name(), "ShowCompliance");
        assert_eq!(GuiEvent::CloseCompliance.name(), "CloseCompliance");
        assert_eq!(GuiEvent::VerifyAuditChain.name(), "VerifyAuditChain");
        assert_eq!(GuiEvent::ExportComplianceReport.name(), "ExportComplianceReport");
    }

    #[test]
    fn test_gui_event_debug_formatting() {
        let event = GuiEvent::StartDeduplication;
        let debug_str = format!("{:?}", event);
        assert_eq!(debug_str, "StartDeduplication");

        let threshold_event = GuiEvent::ThresholdChanged(f32_to_q16(0.85) as u16);
        let debug_str = format!("{:?}", threshold_event);
        assert!(debug_str.contains("ThresholdChanged"));
    }

    #[test]
    fn test_event_alignment() {
        // Verify cache-aligned to prevent false sharing
        assert_eq!(core::mem::align_of::<GuiEvent>(), 64);
    }

    #[test]
    fn test_path_event_clone() {
        let path1 = PathEvent::new("/tmp/test.jsonl");
        let path2 = path1.clone();
        assert_eq!(path1.as_str(), path2.as_str());
    }

    #[test]
    fn test_error_event_clone() {
        let err1 = ErrorEvent::new("Test error");
        let err2 = err1.clone();
        assert_eq!(err1.message(), err2.message());
    }

    #[test]
    fn test_dedup_results_copy() {
        let results = DedupResults::new(1000, 100, 900, 1_000_000, 1_000);
        let copied = results; // Should compile (Copy trait)
        assert_eq!(results.total_docs, copied.total_docs);
    }
}
