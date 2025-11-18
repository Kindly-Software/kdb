//! Progress Viewer Component - Multi-gauge Display with Live Metrics
//!
//! # UCE34 Framework
//! - Q1-Q9: Real-time progress visualization with multi-phase tracking
//! - Q10: Tier 1 (Atomic) - ProgressCapsule for lockfree metrics updates
//! - Q11: Rust AtomicU64 for counters, AtomicU32 for percentages
//! - Q12: Nightly N/A (stable atomics sufficient)
//! - Q13-Q21: Ratatui rendering, indicatif integration, live metrics
//! - Q31: Simplicity - Clean progress bar API with atomic state
//! - Q33: Validation - #[derive(cache-optimized data structure)] compile-time verification
//! - Q34: Auditability N/A (ephemeral progress display)
//!
//! # Architecture
//! ```text
//! ProgressCapsule (256B, cache-aligned)
//! ├─ phase_packed: AtomicU64          // phase:8 + progress:32 + total:24
//! ├─ throughput_docs_sec: AtomicU64   // Documents per second
//! ├─ cpu_usage_permille: AtomicU32    // CPU % × 1000 (e.g., 45.3% = 45300)
//! ├─ ram_usage_mb: AtomicU64          // RAM usage in MB
//! ├─ eta_seconds: AtomicU64           // Estimated time to completion
//! └─ _padding: [u8; N]                // Complete 256B cache line
//! ```
//!
//! # Phases
//! 1. Corpus Loading - Reading input files
//! 2. Pipeline Processing - MinHash + LSH + Union-Find
//! 3. Ground Truth Generation - Writing output files

// cache-optimized data structure
use atomic_capsule_derive::ComputationalCapsule;
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Progress phase enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProgressPhase {
    CorpusLoading = 0,
    PipelineProcessing = 1,
    GroundTruthGeneration = 2,
    Complete = 3,
}

impl ProgressPhase {
    /// Convert from u8
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => ProgressPhase::CorpusLoading,
            1 => ProgressPhase::PipelineProcessing,
            2 => ProgressPhase::GroundTruthGeneration,
            _ => ProgressPhase::Complete,
        }
    }

    /// Get phase name
    pub fn name(&self) -> &'static str {
        match self {
            ProgressPhase::CorpusLoading => "Corpus Loading",
            ProgressPhase::PipelineProcessing => "Pipeline Processing",
            ProgressPhase::GroundTruthGeneration => "Ground Truth Generation",
            ProgressPhase::Complete => "Complete",
        }
    }

    /// Get phase color
    pub fn color(&self) -> Color {
        match self {
            ProgressPhase::CorpusLoading => Color::Cyan,
            ProgressPhase::PipelineProcessing => Color::Yellow,
            ProgressPhase::GroundTruthGeneration => Color::Magenta,
            ProgressPhase::Complete => Color::Green,
        }
    }
}

/// Progress state capsule (256B aligned)
///
/// # Memory Layout
/// - 8 bytes: phase_packed (phase:8 + progress:32 + total:24)
/// - 8 bytes: throughput_docs_sec
/// - 4 bytes: cpu_usage_permille
/// - 4 bytes: _pad1
/// - 8 bytes: ram_usage_mb
/// - 8 bytes: eta_seconds
/// - 216 bytes: padding (complete 256B cache line)
#[derive(Debug, ComputationalCapsule)]
#[capsule(alignment = 64, size = 256, tier = "Atomic")]
#[repr(C, align(64))]
pub struct ProgressCapsule {
    /// Packed state: phase (upper 8) + progress (next 32) + total (lower 24)
    /// #ASSUME: u8 sufficient for phase (max 255 phases)
    /// #ASSUME: u32 sufficient for progress count (4B items)
    /// #ASSUME: u24 (via u32 mask) sufficient for total count (16M items)
    /// #VERIFY: Atomic operations maintain consistency
    phase_packed: AtomicU64,

    /// Throughput in documents per second
    /// #ASSUME: u64 sufficient for throughput (realistic max ~1M docs/sec)
    throughput_docs_sec: AtomicU64,

    /// CPU usage in permille (0-100000 representing 0.0%-100.0%)
    /// #ASSUME: u32 sufficient for CPU usage with 0.001% precision
    cpu_usage_permille: AtomicU32,

    /// Padding for alignment
    _pad1: u32,

    /// RAM usage in MB
    /// #ASSUME: u64 sufficient for RAM (realistic max ~1TB = 1M MB)
    ram_usage_mb: AtomicU64,

    /// Estimated time to completion in seconds
    /// #ASSUME: u64 sufficient for ETA (max ~584 billion years)
    eta_seconds: AtomicU64,

    /// Padding to 256B
    _padding: [u8; 216],
}

impl ProgressCapsule {
    /// Create new progress capsule
    pub fn new() -> Self {
        Self {
            phase_packed: AtomicU64::new(0),
            throughput_docs_sec: AtomicU64::new(0),
            cpu_usage_permille: AtomicU32::new(0),
            _pad1: 0,
            ram_usage_mb: AtomicU64::new(0),
            eta_seconds: AtomicU64::new(0),
            _padding: [0u8; 216],
        }
    }

    /// Get current phase
    #[inline(always)]
    pub fn phase(&self) -> ProgressPhase {
        let packed = self.phase_packed.load(Ordering::Acquire);
        let phase_u8 = (packed >> 56) as u8;
        ProgressPhase::from_u8(phase_u8)
    }

    /// Get progress count
    #[inline(always)]
    pub fn progress(&self) -> u32 {
        let packed = self.phase_packed.load(Ordering::Acquire);
        ((packed >> 24) & 0xFFFFFFFF) as u32
    }

    /// Get total count
    #[inline(always)]
    pub fn total(&self) -> u32 {
        let packed = self.phase_packed.load(Ordering::Acquire);
        (packed & 0xFFFFFF) as u32
    }

    /// Set phase, progress, and total
    #[inline(always)]
    pub fn set_state(&self, phase: ProgressPhase, progress: u32, total: u32) {
        let phase_u64 = (phase as u64) << 56;
        let progress_u64 = ((progress as u64) & 0xFFFFFFFF) << 24;
        let total_u64 = (total as u64) & 0xFFFFFF;
        let packed = phase_u64 | progress_u64 | total_u64;
        self.phase_packed.store(packed, Ordering::Release);
    }

    /// Increment progress
    #[inline(always)]
    pub fn increment_progress(&self, delta: u32) {
        loop {
            let current = self.phase_packed.load(Ordering::Acquire);
            let phase = (current >> 56) as u8;
            let progress = ((current >> 24) & 0xFFFFFFFF) as u32;
            let total = (current & 0xFFFFFF) as u32;

            let new_progress = progress.saturating_add(delta).min(total);
            let new_packed = ((phase as u64) << 56) | (((new_progress as u64) & 0xFFFFFFFF) << 24) | (total as u64);

            if self
                .phase_packed
                .compare_exchange_weak(current, new_packed, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Set throughput
    pub fn set_throughput(&self, docs_per_sec: u64) {
        self.throughput_docs_sec.store(docs_per_sec, Ordering::Release);
    }

    /// Get throughput
    pub fn throughput(&self) -> u64 {
        self.throughput_docs_sec.load(Ordering::Acquire)
    }

    /// Set CPU usage (percentage × 1000, e.g., 45.3% = 45300)
    pub fn set_cpu_usage(&self, permille: u32) {
        self.cpu_usage_permille.store(permille, Ordering::Release);
    }

    /// Get CPU usage percentage
    pub fn cpu_usage_percent(&self) -> f64 {
        let permille = self.cpu_usage_permille.load(Ordering::Acquire);
        permille as f64 / 1000.0
    }

    /// Set RAM usage in MB
    pub fn set_ram_usage(&self, mb: u64) {
        self.ram_usage_mb.store(mb, Ordering::Release);
    }

    /// Get RAM usage in MB
    pub fn ram_usage_mb(&self) -> u64 {
        self.ram_usage_mb.load(Ordering::Acquire)
    }

    /// Set ETA in seconds
    pub fn set_eta(&self, seconds: u64) {
        self.eta_seconds.store(seconds, Ordering::Release);
    }

    /// Get ETA in seconds
    pub fn eta_seconds(&self) -> u64 {
        self.eta_seconds.load(Ordering::Acquire)
    }

    /// Get progress percentage (0-100)
    pub fn percentage(&self) -> f64 {
        let progress = self.progress();
        let total = self.total();

        if total == 0 {
            0.0
        } else {
            (progress as f64 / total as f64) * 100.0
        }
    }
}

impl Default for ProgressCapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// Progress viewer component
pub struct ProgressViewer {
    /// Atomic progress capsule
    capsule: ProgressCapsule,

    /// Start time for throughput calculation
    start_time: Instant,

    /// Phase start times
    phase_start_times: [Option<Instant>; 4],
}

impl ProgressViewer {
    /// Create new progress viewer
    pub fn new() -> Self {
        Self {
            capsule: ProgressCapsule::new(),
            start_time: Instant::now(),
            phase_start_times: [None; 4],
        }
    }

    /// Get capsule reference (for external updates)
    pub fn capsule(&self) -> &ProgressCapsule {
        &self.capsule
    }

    /// Start phase
    pub fn start_phase(&mut self, phase: ProgressPhase, total: u32) {
        self.capsule.set_state(phase, 0, total);
        self.phase_start_times[phase as usize] = Some(Instant::now());
    }

    /// Update progress
    pub fn update_progress(&self, progress: u32) {
        let phase = self.capsule.phase();
        let total = self.capsule.total();
        self.capsule.set_state(phase, progress, total);

        // Calculate throughput
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            let throughput = (progress as f64 / elapsed) as u64;
            self.capsule.set_throughput(throughput);

            // Calculate ETA
            let remaining = total.saturating_sub(progress);
            if throughput > 0 {
                let eta = (remaining as f64 / throughput as f64) as u64;
                self.capsule.set_eta(eta);
            }
        }
    }

    /// Increment progress
    pub fn increment(&self, delta: u32) {
        self.capsule.increment_progress(delta);

        // Recalculate throughput and ETA
        let progress = self.capsule.progress();
        let total = self.capsule.total();
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            let throughput = (progress as f64 / elapsed) as u64;
            self.capsule.set_throughput(throughput);

            let remaining = total.saturating_sub(progress);
            if throughput > 0 {
                let eta = (remaining as f64 / throughput as f64) as u64;
                self.capsule.set_eta(eta);
            }
        }
    }

    /// Update system metrics (CPU, RAM)
    pub fn update_metrics(&self, cpu_percent: f64, ram_mb: u64) {
        let cpu_permille = (cpu_percent * 1000.0) as u32;
        self.capsule.set_cpu_usage(cpu_permille);
        self.capsule.set_ram_usage(ram_mb);
    }

    /// Mark complete
    pub fn complete(&mut self) {
        let total = self.capsule.total();
        self.capsule.set_state(ProgressPhase::Complete, total, total);
        self.capsule.set_eta(0);
    }

    /// Render progress viewer to frame
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Create layout: phase gauge + metrics + details
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Phase gauge
                Constraint::Length(3), // Metrics (throughput, ETA)
                Constraint::Length(3), // System metrics (CPU, RAM)
                Constraint::Min(2),    // Details/logs
            ])
            .split(area);

        // Render phase gauge
        let phase = self.capsule.phase();
        let percentage = self.capsule.percentage();
        let progress = self.capsule.progress();
        let total = self.capsule.total();

        let gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Phase: {}", phase.name())),
            )
            .gauge_style(Style::default().fg(phase.color()).add_modifier(Modifier::BOLD))
            .percent(percentage as u16)
            .label(format!("{:.1}% ({}/{})", percentage, progress, total));
        frame.render_widget(gauge, chunks[0]);

        // Render throughput and ETA
        let throughput = self.capsule.throughput();
        let eta_secs = self.capsule.eta_seconds();
        let eta_str = format_duration(Duration::from_secs(eta_secs));

        let metrics_text = vec![Line::from(vec![
            Span::styled("Throughput: ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{} docs/sec", throughput)),
            Span::raw("  |  "),
            Span::styled("ETA: ", Style::default().fg(Color::Yellow)),
            Span::raw(eta_str),
        ])];
        let metrics = Paragraph::new(metrics_text).block(Block::default().borders(Borders::ALL));
        frame.render_widget(metrics, chunks[1]);

        // Render system metrics
        let cpu_percent = self.capsule.cpu_usage_percent();
        let ram_mb = self.capsule.ram_usage_mb();

        let system_text = vec![Line::from(vec![
            Span::styled("CPU: ", Style::default().fg(Color::Magenta)),
            Span::raw(format!("{:.1}%", cpu_percent)),
            Span::raw("  |  "),
            Span::styled("RAM: ", Style::default().fg(Color::Blue)),
            Span::raw(format!("{} MB", ram_mb)),
        ])];
        let system = Paragraph::new(system_text).block(Block::default().borders(Borders::ALL));
        frame.render_widget(system, chunks[2]);

        // Render elapsed time
        let elapsed = self.start_time.elapsed();
        let elapsed_str = format_duration(elapsed);

        let details_text = vec![Line::from(vec![
            Span::styled("Elapsed: ", Style::default().fg(Color::Green)),
            Span::raw(elapsed_str),
        ])];
        let details = Paragraph::new(details_text).block(Block::default().borders(Borders::ALL).title("Details"));
        frame.render_widget(details, chunks[3]);
    }
}

impl Default for ProgressViewer {
    fn default() -> Self {
        Self::new()
    }
}

/// Format duration as human-readable string
fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_state() {
        let capsule = ProgressCapsule::new();

        capsule.set_state(ProgressPhase::PipelineProcessing, 50, 100);

        assert_eq!(capsule.phase(), ProgressPhase::PipelineProcessing);
        assert_eq!(capsule.progress(), 50);
        assert_eq!(capsule.total(), 100);
        assert_eq!(capsule.percentage(), 50.0);
    }

    #[test]
    fn test_increment_progress() {
        let capsule = ProgressCapsule::new();
        capsule.set_state(ProgressPhase::CorpusLoading, 0, 100);

        capsule.increment_progress(10);
        assert_eq!(capsule.progress(), 10);

        capsule.increment_progress(20);
        assert_eq!(capsule.progress(), 30);
    }

    #[test]
    fn test_metrics() {
        let capsule = ProgressCapsule::new();

        capsule.set_throughput(1000);
        assert_eq!(capsule.throughput(), 1000);

        capsule.set_cpu_usage(45300); // 45.3%
        assert!((capsule.cpu_usage_percent() - 45.3).abs() < 0.01);

        capsule.set_ram_usage(512);
        assert_eq!(capsule.ram_usage_mb(), 512);

        capsule.set_eta(120);
        assert_eq!(capsule.eta_seconds(), 120);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3665)), "1h 1m 5s");
    }
}
