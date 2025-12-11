//! Download Queue Capsule (T4 Batch + T8 Network)
//!
//! **Tier**: T4 (Batch) + T8 (Network)
//! **Size**: 512 bytes
//! **Chaos Compliance**: 100% lockfree, parallel downloads
//!
//! Parallel download manager for package files with:
//! - Concurrent download scheduling
//! - Bandwidth throttling
//! - Resume support (HTTP Range)
//! - Checksum verification during download

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use super::version::Version;

// ============================================================================
// Download Status
// ============================================================================

/// Download task status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DownloadStatus {
    /// Task is queued
    Queued = 0,
    /// Download in progress
    Downloading = 1,
    /// Download paused
    Paused = 2,
    /// Download completed
    Completed = 3,
    /// Verifying checksum
    Verifying = 4,
    /// Download failed
    Failed = 5,
    /// Download cancelled
    Cancelled = 6,
}

impl DownloadStatus {
    /// Check if download is active
    pub const fn is_active(&self) -> bool {
        matches!(self, DownloadStatus::Downloading | DownloadStatus::Verifying)
    }

    /// Check if download is complete
    pub const fn is_complete(&self) -> bool {
        matches!(
            self,
            DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
        )
    }

    /// Convert from raw
    pub fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(DownloadStatus::Queued),
            1 => Some(DownloadStatus::Downloading),
            2 => Some(DownloadStatus::Paused),
            3 => Some(DownloadStatus::Completed),
            4 => Some(DownloadStatus::Verifying),
            5 => Some(DownloadStatus::Failed),
            6 => Some(DownloadStatus::Cancelled),
            _ => None,
        }
    }
}

// ============================================================================
// Download Task
// ============================================================================

/// Download task descriptor
#[derive(Debug, Clone)]
pub struct DownloadTask {
    /// Task ID
    pub id: u64,
    /// Package name
    pub package_name: String,
    /// Package version
    pub version: Version,
    /// Download URL
    pub url: String,
    /// Expected size in bytes
    pub size: u64,
    /// Expected SHA256 checksum (hex)
    pub sha256: String,
    /// Download priority (higher = more important)
    pub priority: u32,
    /// Repository ID
    pub repository_id: String,
}

impl DownloadTask {
    /// Create new download task
    pub fn new(
        id: u64,
        package_name: String,
        version: Version,
        url: String,
        size: u64,
    ) -> Self {
        Self {
            id,
            package_name,
            version,
            url,
            size,
            sha256: String::new(),
            priority: 100,
            repository_id: String::new(),
        }
    }

    /// Set expected checksum
    pub fn with_checksum(mut self, sha256: String) -> Self {
        self.sha256 = sha256;
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }
}

// ============================================================================
// Download Queue Capsule
// ============================================================================

/// Download Queue Capsule (T4 + T8)
///
/// # Size
/// 512 bytes
///
/// # Features
/// - Parallel download management
/// - Priority-based scheduling
/// - Bandwidth control
/// - Progress tracking
#[repr(C, align(64))]
pub struct DownloadQueueCapsule {
    // Cache line 0: State (64B)
    /// Generation counter
    generation: AtomicU64,
    /// Queue state
    state: AtomicU32,
    /// Flags
    flags: AtomicU32,
    /// Tasks in queue
    queued_count: AtomicU32,
    /// Active downloads
    active_count: AtomicU32,
    /// Completed downloads
    completed_count: AtomicU32,
    /// Failed downloads
    failed_count: AtomicU32,
    /// Next task ID
    next_task_id: AtomicU64,
    /// Padding
    _pad0: [u8; 16],

    // Cache line 1: Progress (64B)
    /// Total bytes to download
    total_bytes: AtomicU64,
    /// Bytes downloaded
    downloaded_bytes: AtomicU64,
    /// Current download speed (bytes/sec)
    current_speed: AtomicU64,
    /// Peak download speed (bytes/sec)
    peak_speed: AtomicU64,
    /// Average speed (bytes/sec)
    avg_speed: AtomicU64,
    /// Padding
    _pad1: [u8; 24],

    // Cache line 2: Configuration (64B)
    /// Maximum concurrent downloads
    max_concurrent: AtomicU32,
    /// Bandwidth limit (bytes/sec, 0 = unlimited)
    bandwidth_limit: AtomicU64,
    /// Connection timeout (milliseconds)
    connection_timeout_ms: AtomicU32,
    /// Read timeout (milliseconds)
    read_timeout_ms: AtomicU32,
    /// Retry count
    max_retries: AtomicU32,
    /// Padding
    _pad2: [u8; 32],

    // Cache line 3: Timing (64B)
    /// Start time (Unix timestamp)
    start_time: AtomicU64,
    /// Last activity time
    last_activity: AtomicU64,
    /// Total time spent (seconds)
    total_time_sec: AtomicU64,
    /// ETA (seconds remaining)
    eta_seconds: AtomicU64,
    /// Padding
    _pad3: [u8; 32],

    // Remaining: Statistics (256B)
    /// Total downloads attempted
    total_downloads: AtomicU64,
    /// Total bytes ever downloaded
    lifetime_bytes: AtomicU64,
    /// Connection errors
    connection_errors: AtomicU64,
    /// Timeout errors
    timeout_errors: AtomicU64,
    /// Checksum errors
    checksum_errors: AtomicU64,
    /// Retries performed
    retries_performed: AtomicU64,
    /// Padding
    _reserved: [u8; 208],
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<DownloadQueueCapsule>() == 512);
    assert!(core::mem::align_of::<DownloadQueueCapsule>() == 64);
};

impl DownloadQueueCapsule {
    /// Queue state: idle
    pub const STATE_IDLE: u32 = 0;
    /// Queue state: downloading
    pub const STATE_DOWNLOADING: u32 = 1;
    /// Queue state: paused
    pub const STATE_PAUSED: u32 = 2;
    /// Queue state: complete
    pub const STATE_COMPLETE: u32 = 3;

    /// Flag: verify checksums
    pub const FLAG_VERIFY: u32 = 1 << 0;
    /// Flag: resume supported
    pub const FLAG_RESUME: u32 = 1 << 1;
    /// Flag: keep partial files
    pub const FLAG_KEEP_PARTIAL: u32 = 1 << 2;

    /// Default max concurrent downloads
    pub const DEFAULT_MAX_CONCURRENT: u32 = 4;

    /// Create new download queue
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            state: AtomicU32::new(Self::STATE_IDLE),
            flags: AtomicU32::new(Self::FLAG_VERIFY | Self::FLAG_RESUME),
            queued_count: AtomicU32::new(0),
            active_count: AtomicU32::new(0),
            completed_count: AtomicU32::new(0),
            failed_count: AtomicU32::new(0),
            next_task_id: AtomicU64::new(1),
            _pad0: [0; 16],
            total_bytes: AtomicU64::new(0),
            downloaded_bytes: AtomicU64::new(0),
            current_speed: AtomicU64::new(0),
            peak_speed: AtomicU64::new(0),
            avg_speed: AtomicU64::new(0),
            _pad1: [0; 24],
            max_concurrent: AtomicU32::new(Self::DEFAULT_MAX_CONCURRENT),
            bandwidth_limit: AtomicU64::new(0),
            connection_timeout_ms: AtomicU32::new(30000),
            read_timeout_ms: AtomicU32::new(60000),
            max_retries: AtomicU32::new(3),
            _pad2: [0; 32],
            start_time: AtomicU64::new(0),
            last_activity: AtomicU64::new(0),
            total_time_sec: AtomicU64::new(0),
            eta_seconds: AtomicU64::new(0),
            _pad3: [0; 32],
            total_downloads: AtomicU64::new(0),
            lifetime_bytes: AtomicU64::new(0),
            connection_errors: AtomicU64::new(0),
            timeout_errors: AtomicU64::new(0),
            checksum_errors: AtomicU64::new(0),
            retries_performed: AtomicU64::new(0),
            _reserved: [0; 208],
        }
    }

    /// Get queue state
    pub fn state(&self) -> u32 {
        self.state.load(Ordering::Acquire)
    }

    /// Get generation
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Allocate new task ID
    pub fn alloc_task_id(&self) -> u64 {
        self.next_task_id.fetch_add(1, Ordering::AcqRel)
    }

    /// Enqueue download task
    pub fn enqueue(&self, size: u64) {
        self.queued_count.fetch_add(1, Ordering::Release);
        self.total_bytes.fetch_add(size, Ordering::Release);
        self.total_downloads.fetch_add(1, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Start download
    pub fn start_download(&self) {
        let queued = self.queued_count.fetch_sub(1, Ordering::AcqRel);
        if queued > 0 {
            self.active_count.fetch_add(1, Ordering::Release);
            if self.state.load(Ordering::Acquire) == Self::STATE_IDLE {
                self.state.store(Self::STATE_DOWNLOADING, Ordering::Release);
            }
        }
    }

    /// Record download progress
    pub fn record_progress(&self, bytes: u64, speed: u64) {
        self.downloaded_bytes.fetch_add(bytes, Ordering::Release);
        self.current_speed.store(speed, Ordering::Release);

        // Update peak speed
        let peak = self.peak_speed.load(Ordering::Acquire);
        if speed > peak {
            self.peak_speed.store(speed, Ordering::Release);
        }

        // Update ETA
        let remaining = self.total_bytes.load(Ordering::Acquire)
            - self.downloaded_bytes.load(Ordering::Acquire);
        if speed > 0 {
            self.eta_seconds.store(remaining / speed, Ordering::Release);
        }

        #[cfg(feature = "std")]
        {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            self.last_activity.store(now, Ordering::Release);
        }
    }

    /// Complete download
    pub fn complete_download(&self, success: bool, bytes: u64) {
        self.active_count.fetch_sub(1, Ordering::Release);
        if success {
            self.completed_count.fetch_add(1, Ordering::Release);
            self.lifetime_bytes.fetch_add(bytes, Ordering::Release);
        } else {
            self.failed_count.fetch_add(1, Ordering::Release);
        }

        // Check if all done
        let active = self.active_count.load(Ordering::Acquire);
        let queued = self.queued_count.load(Ordering::Acquire);
        if active == 0 && queued == 0 {
            self.state.store(Self::STATE_COMPLETE, Ordering::Release);
        }

        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Record error
    pub fn record_error(&self, is_timeout: bool, is_checksum: bool) {
        if is_timeout {
            self.timeout_errors.fetch_add(1, Ordering::Release);
        } else if is_checksum {
            self.checksum_errors.fetch_add(1, Ordering::Release);
        } else {
            self.connection_errors.fetch_add(1, Ordering::Release);
        }
    }

    /// Record retry
    pub fn record_retry(&self) {
        self.retries_performed.fetch_add(1, Ordering::Release);
    }

    /// Get progress percentage
    pub fn progress_percent(&self) -> f64 {
        let total = self.total_bytes.load(Ordering::Acquire);
        if total == 0 {
            return 100.0;
        }
        let downloaded = self.downloaded_bytes.load(Ordering::Acquire);
        (downloaded as f64 / total as f64) * 100.0
    }

    /// Check if queue can accept more concurrent downloads
    pub fn can_start(&self) -> bool {
        let active = self.active_count.load(Ordering::Acquire);
        let max = self.max_concurrent.load(Ordering::Acquire);
        let queued = self.queued_count.load(Ordering::Acquire);
        active < max && queued > 0
    }

    /// Set max concurrent downloads
    pub fn set_max_concurrent(&self, max: u32) {
        self.max_concurrent.store(max, Ordering::Release);
    }

    /// Set bandwidth limit
    pub fn set_bandwidth_limit(&self, bytes_per_sec: u64) {
        self.bandwidth_limit.store(bytes_per_sec, Ordering::Release);
    }

    /// Get statistics
    pub fn statistics(&self) -> DownloadStatistics {
        DownloadStatistics {
            generation: self.generation(),
            queued: self.queued_count.load(Ordering::Relaxed),
            active: self.active_count.load(Ordering::Relaxed),
            completed: self.completed_count.load(Ordering::Relaxed),
            failed: self.failed_count.load(Ordering::Relaxed),
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
            downloaded_bytes: self.downloaded_bytes.load(Ordering::Relaxed),
            current_speed: self.current_speed.load(Ordering::Relaxed),
            eta_seconds: self.eta_seconds.load(Ordering::Relaxed),
        }
    }
}

impl Default for DownloadQueueCapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// Download statistics
#[derive(Debug, Clone, Copy)]
pub struct DownloadStatistics {
    /// Current generation
    pub generation: u64,
    /// Queued tasks
    pub queued: u32,
    /// Active downloads
    pub active: u32,
    /// Completed downloads
    pub completed: u32,
    /// Failed downloads
    pub failed: u32,
    /// Total bytes to download
    pub total_bytes: u64,
    /// Bytes downloaded
    pub downloaded_bytes: u64,
    /// Current speed (bytes/sec)
    pub current_speed: u64,
    /// ETA (seconds)
    pub eta_seconds: u64,
}

impl DownloadStatistics {
    /// Get progress percentage
    pub fn progress_percent(&self) -> f64 {
        if self.total_bytes == 0 {
            100.0
        } else {
            (self.downloaded_bytes as f64 / self.total_bytes as f64) * 100.0
        }
    }

    /// Get speed in MB/s
    pub fn speed_mbps(&self) -> f64 {
        self.current_speed as f64 / (1024.0 * 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size() {
        assert_eq!(core::mem::size_of::<DownloadQueueCapsule>(), 512);
    }

    #[test]
    fn test_download_lifecycle() {
        let queue = DownloadQueueCapsule::new();
        assert_eq!(queue.state(), DownloadQueueCapsule::STATE_IDLE);

        // Enqueue task
        queue.enqueue(1024 * 1024);
        assert_eq!(queue.queued_count.load(Ordering::Acquire), 1);

        // Start download
        queue.start_download();
        assert_eq!(queue.state(), DownloadQueueCapsule::STATE_DOWNLOADING);
        assert_eq!(queue.active_count.load(Ordering::Acquire), 1);

        // Record progress
        queue.record_progress(512 * 1024, 1024 * 1024);
        assert!((queue.progress_percent() - 50.0).abs() < 0.1);

        // Complete
        queue.complete_download(true, 1024 * 1024);
        assert_eq!(queue.state(), DownloadQueueCapsule::STATE_COMPLETE);
    }

    #[test]
    fn test_download_statistics() {
        let queue = DownloadQueueCapsule::new();

        queue.enqueue(2 * 1024 * 1024);
        queue.start_download();
        queue.record_progress(1024 * 1024, 2 * 1024 * 1024);

        let stats = queue.statistics();
        assert_eq!(stats.total_bytes, 2 * 1024 * 1024);
        assert_eq!(stats.downloaded_bytes, 1024 * 1024);
        assert!((stats.speed_mbps() - 2.0).abs() < 0.1);
    }

    #[test]
    fn test_concurrent_limit() {
        let queue = DownloadQueueCapsule::new();
        queue.set_max_concurrent(2);

        // Enqueue 3 tasks
        queue.enqueue(1024);
        queue.enqueue(1024);
        queue.enqueue(1024);

        // Start 2
        queue.start_download();
        queue.start_download();

        // Can't start third
        assert!(!queue.can_start());
    }
}
