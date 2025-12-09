//! Job Type Definitions
//!
//! Core types for video encoding job queue system.

use std::path::PathBuf;
use std::time::Duration;

/// Unique job identifier (32-bit counter + 32-bit generation for ABA prevention)
///
/// Packed as u64: [generation:32 | job_id:32]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub u64);

impl JobId {
    /// Create new JobId from generation and counter
    #[inline]
    pub fn new(generation: u32, counter: u32) -> Self {
        Self(((generation as u64) << 32) | (counter as u64))
    }

    /// Extract generation counter (upper 32 bits)
    #[inline]
    pub fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// Extract job counter (lower 32 bits)
    #[inline]
    pub fn counter(self) -> u32 {
        (self.0 & 0xFFFF_FFFF) as u32
    }
}

/// Job priority level (higher = processed first)
///
/// Premium users get priority processing to reduce queue wait time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JobPriority {
    /// Free tier (lowest priority)
    Free = 0,
    /// Creator tier ($49)
    Creator = 1,
    /// Professional tier ($149)
    Professional = 2,
    /// Enterprise tier ($499, highest priority)
    Premium = 3,
}

impl Default for JobPriority {
    fn default() -> Self {
        Self::Free
    }
}

/// Video encoding job specification
///
/// Contains all parameters needed to encode a video file using kindly-av1.
#[derive(Debug, Clone)]
pub struct EncodingJob {
    /// Input video file path
    pub input_path: PathBuf,

    /// Output AV1 file path
    pub output_path: PathBuf,

    /// Encoding preset (ultrafast/superfast/veryfast/faster/fast/medium/slow/slower/veryslow)
    pub preset: String,

    /// Constant Rate Factor (0-63, lower = higher quality)
    pub crf: u8,

    /// Job priority (determines processing order)
    pub priority: JobPriority,

    /// Optional GPU backend (auto/rocm/vulkan/cpu)
    pub gpu: Option<String>,

    /// Optional thread count (defaults to auto-detect)
    pub threads: Option<usize>,

    /// Optional keyframe interval (default: 250)
    pub keyint: Option<u32>,

    /// Optional tile columns for parallelism
    pub tile_columns: Option<u8>,

    /// Optional tile rows for parallelism
    pub tile_rows: Option<u8>,
}

impl EncodingJob {
    /// Create new encoding job with default settings
    pub fn new(input: PathBuf, output: PathBuf) -> Self {
        Self {
            input_path: input,
            output_path: output,
            preset: "medium".into(),
            crf: 28,
            priority: JobPriority::default(),
            gpu: None,
            threads: None,
            keyint: None,
            tile_columns: None,
            tile_rows: None,
        }
    }

    /// Set encoding preset
    pub fn with_preset(mut self, preset: impl Into<String>) -> Self {
        self.preset = preset.into();
        self
    }

    /// Set CRF quality
    pub fn with_crf(mut self, crf: u8) -> Self {
        self.crf = crf;
        self
    }

    /// Set job priority
    pub fn with_priority(mut self, priority: JobPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set GPU backend
    pub fn with_gpu(mut self, gpu: impl Into<String>) -> Self {
        self.gpu = Some(gpu.into());
        self
    }

    /// Set thread count
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = Some(threads);
        self
    }
}

/// Job execution result
#[derive(Debug, Clone)]
pub struct EncodingResult {
    /// Job ID
    pub job_id: JobId,

    /// Encoding success/failure
    pub success: bool,

    /// Error message (if failed)
    pub error: Option<String>,

    /// Output file size (bytes)
    pub output_size: u64,

    /// Encoding duration
    pub duration: Duration,

    /// Average encoding FPS
    pub avg_fps: f64,

    /// Total frames encoded
    pub frames: u64,
}

impl EncodingResult {
    /// Create successful result
    pub fn success(
        job_id: JobId,
        output_size: u64,
        duration: Duration,
        frames: u64,
    ) -> Self {
        let avg_fps = if duration.as_secs_f64() > 0.0 {
            frames as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        Self {
            job_id,
            success: true,
            error: None,
            output_size,
            duration,
            avg_fps,
            frames,
        }
    }

    /// Create failed result
    pub fn failure(job_id: JobId, error: String) -> Self {
        Self {
            job_id,
            success: false,
            error: Some(error),
            output_size: 0,
            duration: Duration::ZERO,
            avg_fps: 0.0,
            frames: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_id_packing() {
        let id = JobId::new(42, 12345);
        assert_eq!(id.generation(), 42);
        assert_eq!(id.counter(), 12345);
    }

    #[test]
    fn test_job_priority_ordering() {
        assert!(JobPriority::Premium > JobPriority::Professional);
        assert!(JobPriority::Professional > JobPriority::Creator);
        assert!(JobPriority::Creator > JobPriority::Free);
    }

    #[test]
    fn test_encoding_job_builder() {
        let job = EncodingJob::new("input.mp4".into(), "output.av1".into())
            .with_preset("fast")
            .with_crf(24)
            .with_priority(JobPriority::Premium)
            .with_gpu("rocm")
            .with_threads(8);

        assert_eq!(job.preset, "fast");
        assert_eq!(job.crf, 24);
        assert_eq!(job.priority, JobPriority::Premium);
        assert_eq!(job.gpu, Some("rocm".into()));
        assert_eq!(job.threads, Some(8));
    }
}
