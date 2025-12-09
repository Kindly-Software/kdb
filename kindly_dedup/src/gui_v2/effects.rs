// Copyright (c) 2025 Kindly Dedup Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// gui_v2/effects.rs - GPUI Effect System for Deduplication Pipeline
//
// UCE34 Compliance:
// - Q1: Effects for deferred UI operations (file dialogs, background tasks)
// - Q10: T0 Auditable (effect queue logging for debugging)
// - Q33: 100% safe (no mutex, atomic coordination only)
// - Q34: Effect audit trail (debugging/compliance)
//
// Chaos Compliance:
// - Fire-and-forget effects (no return values in hot path)
// - AtomicU64 for effect queue coordination
// - Packed structs (32B max for ProgressStats)
// - Small enums (ErrorKind, UrlKind)

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// Effect kinds for deferred processing in GPUI
///
/// Effects are dispatched from event handlers and processed asynchronously
/// by the effect executor. This pattern keeps event handlers synchronous
/// and non-blocking while enabling complex background operations.
///
/// # Design Principles
///
/// 1. **Fire-and-Forget**: Effects have no return values in the hot path
/// 2. **Small Payloads**: Effects carry minimal data (<32B when possible)
/// 3. **Enum Indirection**: ErrorKind/UrlKind avoid String allocations
/// 4. **Atomic Coordination**: Effect queue uses AtomicU64 for lockfree coordination
///
/// # Framework Compliance
///
/// - **UCE34 Q10**: T0 Auditable (effect logging for debugging)
/// - **Chaos**: 100% lockfree (no mutex in effect dispatch)
/// - **ASSUM**: 99.99% safe (no unsafe in effect handling)
#[derive(Debug, Clone, PartialEq)]
pub enum GuiEffect {
    /// Open native file picker dialog (async via rfd)
    ///
    /// Dispatched when user clicks "Select Corpus" button.
    /// Result delivered via FileSelected message.
    OpenFilePicker,

    /// Calculate file/directory size in background thread
    ///
    /// Dispatched after FileSelected to show corpus statistics.
    /// Result delivered via FileSizeCalculated message.
    UpdateFileSize(PathBuf),

    /// Start deduplication pipeline worker thread
    ///
    /// Dispatched when user clicks "Start Processing" button.
    /// Progress delivered via UpdateProgress effects.
    /// Completion delivered via ShowResults effect.
    StartProcessing(PipelineConfig),

    /// Cancel running deduplication pipeline
    ///
    /// Dispatched when user clicks "Cancel" button during processing.
    /// Sends cancellation signal to worker thread.
    CancelProcessing,

    /// Clear selected file/directory
    ///
    /// Dispatched when user clicks "Clear" button to reset file selection.
    /// Clears corpus path and resets UI to initial state.
    ClearFile,

    /// Start deduplication pipeline (alias for StartProcessing)
    ///
    /// Dispatched when user clicks "Start Processing" button.
    /// Identical to StartProcessing, provided for API consistency.
    StartPipeline(PipelineConfig),

    /// Cancel running pipeline (alias for CancelProcessing)
    ///
    /// Dispatched when user clicks "Cancel" button during processing.
    /// Identical to CancelProcessing, provided for API consistency.
    CancelPipeline,

    /// Export deduplication results to file
    ///
    /// Dispatched when user clicks "Export Results" button.
    /// Writes duplicate clusters and statistics to specified path.
    /// Supports JSON, CSV, and JSONL formats based on file extension.
    ExportResults(PathBuf),

    /// Copy results to system clipboard
    ///
    /// Dispatched when user clicks "Copy Results" button.
    /// Formats results as plain text and copies to clipboard.
    /// Includes summary statistics and top duplicate clusters.
    CopyResults,

    /// Update progress UI (from worker thread)
    ///
    /// Dispatched periodically (every 100ms) during processing.
    /// Updates progress bar, throughput meter, ETA.
    UpdateProgress(ProgressStats),

    /// Show deduplication results in UI
    ///
    /// Dispatched when processing completes successfully.
    /// Displays duplicate clusters, statistics, export options.
    ShowResults(DedupResults),

    /// Show error modal dialog
    ///
    /// Dispatched on pipeline errors (file I/O, OOM, validation).
    /// Displays user-friendly error message with recovery options.
    ShowError(ErrorKind),

    /// Advance animation state by delta milliseconds
    ///
    /// Dispatched from GPUI animation callback (60 FPS typical).
    /// Updates spinner rotation, progress bar fill, fade effects.
    AnimationStep(u32),

    /// Open URL in system browser
    ///
    /// Dispatched when user clicks documentation/report links.
    /// Uses system default browser (webbrowser crate).
    OpenUrl(UrlKind),

    /// Export compliance report as PDF
    ///
    /// Dispatched when user clicks "Export Report" button.
    /// Generates Q34 audit trail PDF with hash chain verification.
    ExportReport(PathBuf),
}

/// Pipeline configuration for StartProcessing effect
///
/// Compact representation (24B) of deduplication parameters.
/// Packed into u64 fields to minimize allocation overhead.
///
/// # Layout (24 bytes)
///
/// ```text
/// [0-7]:   corpus_path_id (u64, interned path ID)
/// [8-15]:  config_bits (u64, packed configuration)
///          [0-15]:   threshold_q16 (u16, Q16.16 Jaccard threshold × 65536)
///          [16-23]:  num_hashes (u8, MinHash permutations)
///          [24-31]:  num_bands (u8, LSH bands)
///          [32-47]:  batch_size (u16, documents per batch)
///          [48-55]:  flags (u8, ExecutionMode + BloomFilter + PersistentMode)
/// [16-23]: output_path_id (u64, interned path ID)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineConfig {
    /// Interned corpus path ID (to avoid allocating PathBuf in effect)
    pub corpus_path_id: u64,

    /// Packed configuration bits (threshold, hashes, bands, batch_size, flags)
    pub config_bits: u64,

    /// Interned output path ID (for results/audit trail)
    pub output_path_id: u64,
}

impl PipelineConfig {
    /// Create new pipeline configuration
    ///
    /// # Arguments
    ///
    /// - `corpus_path_id`: Interned path ID from path interner
    /// - `threshold`: Jaccard threshold (0.0-1.0)
    /// - `num_hashes`: MinHash permutations (64-512)
    /// - `num_bands`: LSH bands (16-128)
    /// - `batch_size`: Documents per batch (100-10000)
    /// - `execution_mode`: CPU/GPU/Auto
    /// - `use_bloom`: Enable Bloom pre-filter
    /// - `use_persistent`: Enable persistent mode (T9)
    /// - `output_path_id`: Interned output path ID
    #[inline]
    pub fn new(
        corpus_path_id: u64,
        threshold: f64,
        num_hashes: u8,
        num_bands: u8,
        batch_size: u16,
        execution_mode: ExecutionMode,
        use_bloom: bool,
        use_persistent: bool,
        output_path_id: u64,
    ) -> Self {
        // Convert threshold to Q16.16 fixed-point (0.0-1.0 → 0-65536)
        let threshold_q16 = (threshold.clamp(0.0, 1.0) * 65536.0) as u16;

        // Pack flags into single byte
        let mut flags: u8 = 0;
        flags |= (execution_mode as u8) & 0x03; // bits 0-1: ExecutionMode (0=CPU, 1=GPU, 2=Auto)
        if use_bloom {
            flags |= 0x04; // bit 2: Bloom filter enabled
        }
        if use_persistent {
            flags |= 0x08; // bit 3: Persistent mode enabled
        }

        // Pack config_bits (8 bytes total)
        let config_bits = (threshold_q16 as u64)
            | ((num_hashes as u64) << 16)
            | ((num_bands as u64) << 24)
            | ((batch_size as u64) << 32)
            | ((flags as u64) << 48);

        Self {
            corpus_path_id,
            config_bits,
            output_path_id,
        }
    }

    /// Extract Jaccard threshold from config_bits
    #[inline]
    pub fn threshold(&self) -> f64 {
        let threshold_q16 = (self.config_bits & 0xFFFF) as u16;
        (threshold_q16 as f64) / 65536.0
    }

    /// Extract num_hashes from config_bits
    #[inline]
    pub fn num_hashes(&self) -> u8 {
        ((self.config_bits >> 16) & 0xFF) as u8
    }

    /// Extract num_bands from config_bits
    #[inline]
    pub fn num_bands(&self) -> u8 {
        ((self.config_bits >> 24) & 0xFF) as u8
    }

    /// Extract batch_size from config_bits
    #[inline]
    pub fn batch_size(&self) -> u16 {
        ((self.config_bits >> 32) & 0xFFFF) as u16
    }

    /// Extract ExecutionMode from config_bits
    #[inline]
    pub fn execution_mode(&self) -> ExecutionMode {
        let mode_bits = ((self.config_bits >> 48) & 0x03) as u8;
        match mode_bits {
            0 => ExecutionMode::Cpu,
            1 => ExecutionMode::Gpu,
            2 => ExecutionMode::Auto,
            _ => ExecutionMode::Auto, // invalid → default to Auto
        }
    }

    /// Check if Bloom filter is enabled
    #[inline]
    pub fn use_bloom(&self) -> bool {
        ((self.config_bits >> 50) & 0x01) != 0
    }

    /// Check if persistent mode is enabled
    #[inline]
    pub fn use_persistent(&self) -> bool {
        ((self.config_bits >> 51) & 0x01) != 0
    }
}

/// Execution mode for deduplication pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExecutionMode {
    /// CPU-only execution (SIMD optimized)
    Cpu = 0,

    /// GPU-accelerated execution (wgpu)
    Gpu = 1,

    /// Adaptive CPU/GPU mode switching (T6 Mixed)
    Auto = 2,
}

/// Progress statistics for UpdateProgress effect
///
/// Compact representation (32B) of pipeline progress metrics.
/// Updated every 100ms from worker thread, displayed in progress UI.
///
/// # Layout (32 bytes)
///
/// ```text
/// [0-7]:   docs_processed (u64, cumulative document count)
/// [8-15]:  docs_total (u64, estimated total documents)
/// [16-23]: bytes_processed (u64, cumulative bytes)
/// [24-27]: throughput_docs_per_sec (u32, recent throughput)
/// [28-31]: eta_seconds (u32, estimated time remaining)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, align(8))]
pub struct ProgressStats {
    /// Cumulative documents processed
    pub docs_processed: u64,

    /// Total documents (estimated from corpus scan)
    pub docs_total: u64,

    /// Cumulative bytes processed
    pub bytes_processed: u64,

    /// Recent throughput (documents per second)
    pub throughput_docs_per_sec: u32,

    /// Estimated time remaining (seconds)
    pub eta_seconds: u32,
}

impl ProgressStats {
    /// Create new progress stats
    #[inline]
    pub fn new(
        docs_processed: u64,
        docs_total: u64,
        bytes_processed: u64,
        throughput_docs_per_sec: u32,
        eta_seconds: u32,
    ) -> Self {
        Self {
            docs_processed,
            docs_total,
            bytes_processed,
            throughput_docs_per_sec,
            eta_seconds,
        }
    }

    /// Calculate progress percentage (0-100)
    #[inline]
    pub fn progress_percent(&self) -> u8 {
        if self.docs_total == 0 {
            0
        } else {
            ((self.docs_processed as f64 / self.docs_total as f64) * 100.0)
                .clamp(0.0, 100.0) as u8
        }
    }

    /// Calculate megabytes processed
    #[inline]
    pub fn megabytes_processed(&self) -> f64 {
        (self.bytes_processed as f64) / (1024.0 * 1024.0)
    }

    /// Format ETA as human-readable string (e.g., "2m 30s")
    pub fn format_eta(&self) -> String {
        let minutes = self.eta_seconds / 60;
        let seconds = self.eta_seconds % 60;

        if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }

    /// Format throughput as human-readable string (e.g., "60.0K docs/sec")
    pub fn format_throughput(&self) -> String {
        if self.throughput_docs_per_sec >= 1_000_000 {
            format!("{:.1}M docs/sec", self.throughput_docs_per_sec as f64 / 1_000_000.0)
        } else if self.throughput_docs_per_sec >= 1_000 {
            format!("{:.1}K docs/sec", self.throughput_docs_per_sec as f64 / 1_000.0)
        } else {
            format!("{} docs/sec", self.throughput_docs_per_sec)
        }
    }
}

/// Deduplication results for ShowResults effect
///
/// Contains summary statistics and cluster information.
/// Displayed in results panel after processing completes.
#[derive(Debug, Clone, PartialEq)]
pub struct DedupResults {
    /// Total documents processed
    pub total_docs: u64,

    /// Unique documents (no duplicates)
    pub unique_docs: u64,

    /// Duplicate documents (in clusters)
    pub duplicate_docs: u64,

    /// Number of duplicate clusters
    pub num_clusters: u64,

    /// Largest cluster size
    pub max_cluster_size: u64,

    /// Processing time (milliseconds)
    pub processing_time_ms: u64,

    /// Average throughput (documents per second)
    pub avg_throughput: u32,

    /// Memory usage peak (megabytes)
    pub peak_memory_mb: u32,

    /// Execution mode used (CPU/GPU/Auto)
    pub execution_mode: ExecutionMode,
}

impl DedupResults {
    /// Calculate deduplication ratio (0.0-1.0)
    #[inline]
    pub fn dedup_ratio(&self) -> f64 {
        if self.total_docs == 0 {
            0.0
        } else {
            (self.duplicate_docs as f64) / (self.total_docs as f64)
        }
    }

    /// Format deduplication ratio as percentage string
    pub fn format_dedup_ratio(&self) -> String {
        format!("{:.1}%", self.dedup_ratio() * 100.0)
    }

    /// Format processing time as human-readable string
    pub fn format_processing_time(&self) -> String {
        let seconds = self.processing_time_ms / 1000;
        let minutes = seconds / 60;
        let seconds = seconds % 60;

        if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }
}

/// Error kinds for ShowError effect
///
/// Small enum (1 byte) to avoid String allocations in effects.
/// Error details stored in UI model, indexed by ErrorKind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorKind {
    /// File I/O error (corpus not found, permission denied, etc.)
    FileIo = 0,

    /// Out of memory error (corpus too large for available RAM)
    OutOfMemory = 1,

    /// Invalid configuration (threshold out of range, invalid parameters)
    InvalidConfig = 2,

    /// Pipeline validation error (corrupt data, hash mismatch, etc.)
    ValidationFailed = 3,

    /// GPU initialization error (no device, driver issue, etc.)
    GpuInitFailed = 4,

    /// Processing cancelled by user
    Cancelled = 5,

    /// Unknown/unexpected error
    Unknown = 6,
}

impl ErrorKind {
    /// Get user-friendly error title
    pub fn title(&self) -> &'static str {
        match self {
            ErrorKind::FileIo => "File Error",
            ErrorKind::OutOfMemory => "Out of Memory",
            ErrorKind::InvalidConfig => "Invalid Configuration",
            ErrorKind::ValidationFailed => "Validation Failed",
            ErrorKind::GpuInitFailed => "GPU Initialization Failed",
            ErrorKind::Cancelled => "Processing Cancelled",
            ErrorKind::Unknown => "Unknown Error",
        }
    }

    /// Get user-friendly error message
    pub fn message(&self) -> &'static str {
        match self {
            ErrorKind::FileIo => "Failed to read corpus file. Check file permissions and path.",
            ErrorKind::OutOfMemory => "Corpus too large for available RAM. Try persistent mode or reduce batch size.",
            ErrorKind::InvalidConfig => "Invalid pipeline configuration. Check threshold and parameter ranges.",
            ErrorKind::ValidationFailed => "Pipeline validation failed. Data may be corrupt.",
            ErrorKind::GpuInitFailed => "Failed to initialize GPU. Falling back to CPU mode.",
            ErrorKind::Cancelled => "Processing cancelled by user.",
            ErrorKind::Unknown => "An unexpected error occurred. Check logs for details.",
        }
    }

    /// Get recovery suggestion
    pub fn suggestion(&self) -> &'static str {
        match self {
            ErrorKind::FileIo => "Verify file path and permissions.",
            ErrorKind::OutOfMemory => "Enable persistent mode or reduce batch size.",
            ErrorKind::InvalidConfig => "Reset to default configuration.",
            ErrorKind::ValidationFailed => "Re-scan corpus or check file integrity.",
            ErrorKind::GpuInitFailed => "Update GPU drivers or use CPU mode.",
            ErrorKind::Cancelled => "No action needed.",
            ErrorKind::Unknown => "Check logs for detailed error information.",
        }
    }
}

/// URL kinds for OpenUrl effect
///
/// Small enum (1 byte) to avoid String allocations in effects.
/// Actual URLs stored in UI model, indexed by UrlKind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UrlKind {
    /// Documentation (algorithm overview, usage guide)
    Documentation = 0,

    /// Benchmark report (performance metrics, validation)
    BenchmarkReport = 1,

    /// Compliance report (Q34 audit trail, hash chain verification)
    ComplianceReport = 2,

    /// GitHub repository (source code, issues)
    GitHubRepo = 3,

    /// Support page (contact, troubleshooting)
    Support = 4,
}

impl UrlKind {
    /// Get URL for this kind
    pub fn url(&self) -> &'static str {
        match self {
            UrlKind::Documentation => "https://kindly.ai/docs/deduplication",
            UrlKind::BenchmarkReport => "https://kindly.ai/benchmarks/deduplication",
            UrlKind::ComplianceReport => "https://kindly.ai/compliance/q34-audit",
            UrlKind::GitHubRepo => "https://github.com/kindly-ai/kindly_dedup",
            UrlKind::Support => "https://kindly.ai/support",
        }
    }
}

/// Effect queue coordination state (lockfree)
///
/// Uses AtomicU64 for lockfree effect queue coordination.
/// Shared between effect dispatcher (UI thread) and effect executor (background thread).
///
/// # Layout (8 bytes)
///
/// ```text
/// [0-31]:  pending_count (u32, number of pending effects)
/// [32-63]: sequence_number (u32, effect sequence for ordering)
/// ```
#[derive(Debug)]
#[repr(C, align(8))]
pub struct EffectQueueState {
    /// Packed state (pending_count | sequence_number)
    state: AtomicU64,
}

impl EffectQueueState {
    /// Create new effect queue state
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
        }
    }

    /// Increment pending count and get sequence number (atomic)
    ///
    /// Returns sequence number for this effect (monotonically increasing).
    #[inline]
    pub fn push_effect(&self) -> u32 {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let pending_count = (current & 0xFFFF_FFFF) as u32;
            let sequence_number = ((current >> 32) & 0xFFFF_FFFF) as u32;

            let new_pending_count = pending_count.wrapping_add(1);
            let new_sequence_number = sequence_number.wrapping_add(1);

            let new_state =
                (new_pending_count as u64) | ((new_sequence_number as u64) << 32);

            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return new_sequence_number,
                Err(actual) => current = actual,
            }
        }
    }

    /// Decrement pending count (atomic)
    #[inline]
    pub fn pop_effect(&self) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let pending_count = (current & 0xFFFF_FFFF) as u32;
            let sequence_number = ((current >> 32) & 0xFFFF_FFFF) as u32;

            let new_pending_count = pending_count.saturating_sub(1);
            let new_state =
                (new_pending_count as u64) | ((sequence_number as u64) << 32);

            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(actual) => current = actual,
            }
        }
    }

    /// Get current pending count (atomic snapshot)
    #[inline]
    pub fn pending_count(&self) -> u32 {
        let state = self.state.load(Ordering::Acquire);
        (state & 0xFFFF_FFFF) as u32
    }

    /// Get current sequence number (atomic snapshot)
    #[inline]
    pub fn sequence_number(&self) -> u32 {
        let state = self.state.load(Ordering::Acquire);
        ((state >> 32) & 0xFFFF_FFFF) as u32
    }

    /// Check if queue is empty (atomic snapshot)
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pending_count() == 0
    }
}

impl Default for EffectQueueState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_config_packing() {
        let config = PipelineConfig::new(
            42,       // corpus_path_id
            0.85,     // threshold
            128,      // num_hashes
            64,       // num_bands
            1000,     // batch_size
            ExecutionMode::Auto,
            true,     // use_bloom
            false,    // use_persistent
            99,       // output_path_id
        );

        assert_eq!(config.corpus_path_id, 42);
        assert_eq!(config.output_path_id, 99);
        assert!((config.threshold() - 0.85).abs() < 0.001);
        assert_eq!(config.num_hashes(), 128);
        assert_eq!(config.num_bands(), 64);
        assert_eq!(config.batch_size(), 1000);
        assert_eq!(config.execution_mode(), ExecutionMode::Auto);
        assert!(config.use_bloom());
        assert!(!config.use_persistent());
    }

    #[test]
    fn test_pipeline_config_threshold_clamping() {
        let config = PipelineConfig::new(
            1, 1.5, 64, 32, 500,
            ExecutionMode::Cpu, false, false, 2
        );
        assert!((config.threshold() - 1.0).abs() < 0.001);

        let config = PipelineConfig::new(
            1, -0.5, 64, 32, 500,
            ExecutionMode::Cpu, false, false, 2
        );
        assert!(config.threshold() < 0.001);
    }

    #[test]
    fn test_pipeline_config_size() {
        use std::mem::size_of;
        assert_eq!(size_of::<PipelineConfig>(), 24);
    }

    #[test]
    fn test_progress_stats_calculations() {
        let stats = ProgressStats::new(
            5000,     // docs_processed
            10000,    // docs_total
            104857600, // bytes_processed (100 MB)
            60000,    // throughput_docs_per_sec
            83,       // eta_seconds
        );

        assert_eq!(stats.progress_percent(), 50);
        assert!((stats.megabytes_processed() - 100.0).abs() < 0.1);
        assert_eq!(stats.format_eta(), "1m 23s");
        assert_eq!(stats.format_throughput(), "60.0K docs/sec");
    }

    #[test]
    fn test_progress_stats_edge_cases() {
        let stats = ProgressStats::new(0, 0, 0, 0, 0);
        assert_eq!(stats.progress_percent(), 0);

        let stats = ProgressStats::new(10000, 10000, 0, 0, 0);
        assert_eq!(stats.progress_percent(), 100);

        let stats = ProgressStats::new(11000, 10000, 0, 0, 0);
        assert_eq!(stats.progress_percent(), 100); // clamped
    }

    #[test]
    fn test_progress_stats_size() {
        use std::mem::size_of;
        assert_eq!(size_of::<ProgressStats>(), 32);
    }

    #[test]
    fn test_dedup_results_calculations() {
        let results = DedupResults {
            total_docs: 10000,
            unique_docs: 7000,
            duplicate_docs: 3000,
            num_clusters: 500,
            max_cluster_size: 25,
            processing_time_ms: 125000, // 2m 5s
            avg_throughput: 80,
            peak_memory_mb: 512,
            execution_mode: ExecutionMode::Auto,
        };

        assert!((results.dedup_ratio() - 0.3).abs() < 0.001);
        assert_eq!(results.format_dedup_ratio(), "30.0%");
        assert_eq!(results.format_processing_time(), "2m 5s");
    }

    #[test]
    fn test_error_kind_messages() {
        assert_eq!(ErrorKind::FileIo.title(), "File Error");
        assert!(ErrorKind::FileIo.message().contains("permissions"));
        assert!(ErrorKind::FileIo.suggestion().contains("Verify"));

        assert_eq!(ErrorKind::OutOfMemory.title(), "Out of Memory");
        assert!(ErrorKind::OutOfMemory.message().contains("RAM"));
        assert!(ErrorKind::OutOfMemory.suggestion().contains("persistent"));
    }

    #[test]
    fn test_url_kind_urls() {
        assert!(UrlKind::Documentation.url().contains("docs"));
        assert!(UrlKind::BenchmarkReport.url().contains("benchmarks"));
        assert!(UrlKind::ComplianceReport.url().contains("compliance"));
        assert!(UrlKind::GitHubRepo.url().contains("github"));
        assert!(UrlKind::Support.url().contains("support"));
    }

    #[test]
    fn test_effect_queue_state_push_pop() {
        let queue = EffectQueueState::new();
        assert_eq!(queue.pending_count(), 0);
        assert_eq!(queue.sequence_number(), 0);
        assert!(queue.is_empty());

        let seq1 = queue.push_effect();
        assert_eq!(seq1, 1);
        assert_eq!(queue.pending_count(), 1);
        assert!(!queue.is_empty());

        let seq2 = queue.push_effect();
        assert_eq!(seq2, 2);
        assert_eq!(queue.pending_count(), 2);

        queue.pop_effect();
        assert_eq!(queue.pending_count(), 1);

        queue.pop_effect();
        assert_eq!(queue.pending_count(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_effect_queue_state_underflow() {
        let queue = EffectQueueState::new();
        queue.pop_effect(); // should saturate at 0
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn test_effect_queue_state_concurrent() {
        use std::sync::Arc;
        use std::thread;

        let queue = Arc::new(EffectQueueState::new());
        let mut handles = vec![];

        for _ in 0..10 {
            let queue = Arc::clone(&queue);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    queue.push_effect();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(queue.pending_count(), 10000);
        assert_eq!(queue.sequence_number(), 10000);
    }

    #[test]
    fn test_effect_variants() {
        let effect1 = GuiEffect::OpenFilePicker;
        let effect2 = GuiEffect::CancelProcessing;
        assert_ne!(effect1, effect2);

        let config = PipelineConfig::new(
            1, 0.8, 64, 32, 500,
            ExecutionMode::Cpu, false, false, 2
        );
        let effect3 = GuiEffect::StartProcessing(config);
        assert!(matches!(effect3, GuiEffect::StartProcessing(_)));
    }
}
