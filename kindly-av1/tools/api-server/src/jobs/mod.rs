//! Job Queue System for Video Encoding (T6 Mixed: T1+T4+T5)
//!
//! **SOTA Architecture**: Based on 2024 research into high-performance video transcoding systems.
//!
//! ## Research Foundations
//!
//! 1. **Work-Stealing Queues** (St3, Tokio): Fixed-capacity lockfree queues with minimal RMW operations
//! 2. **CDN Transcoding** (Linode, OpenVisualCloud): Media Ingest Queue + Worker pattern
//! 3. **GNU Parallel FFmpeg**: 1 thread/encode, N parallel = N cores
//! 4. **Queue-Based Autoscaling** (Egnyte): Monitor backlog, scale workers dynamically
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                        JobQueueSystem                                │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │ WorkStealingQueue<EncodingJob> (T4 Batch)                           │
//! │   - 1024 capacity (lockfree bounded queue)                           │
//! │   - <100ns enqueue/dequeue                                           │
//! │   - Priority handling (premium users first)                          │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │ ParallelBatchProcessor<EncodingJob, EncoderFn> (T6 Mixed)           │
//! │   - 8 encoder worker threads                                         │
//! │   - Each calls kindly-av1 encoder                                    │
//! │   - Progress via DualAtomicU64                                       │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │ JobStatusTracker (T1 Atomic + T9 Persistent)                        │
//! │   - SQLite job tracking                                              │
//! │   - States: queued → encoding → complete/failed                      │
//! │   - Progress percentage (0-100)                                      │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Performance Targets (B32 Framework)
//!
//! - Job submission: <100ns (lockfree enqueue)
//! - Status query: <50ns (atomic load)
//! - Worker dispatch: <1μs (work-stealing coordination)
//! - Concurrent throughput: 10K+ jobs/sec submission
//! - Encoding latency: Depends on video (1-60 seconds typical)
//!
//! ## Chaos Compliance
//!
//! - 100% lockfree job queue (WorkStealingQueue)
//! - Atomic progress tracking (DualAtomicU64)
//! - Cache-aligned coordination (64B/128B)
//! - Generation counters for ABA prevention
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use kindly_av1_api_server::jobs::{JobQueueSystem, EncodingJob, JobPriority};
//!
//! // Create job queue system (8 workers)
//! let queue = JobQueueSystem::new(8)?;
//!
//! // Submit job
//! let job = EncodingJob {
//!     input_path: "/tmp/input.mp4".into(),
//!     output_path: "/tmp/output.av1".into(),
//!     preset: "medium".into(),
//!     crf: 28,
//!     priority: JobPriority::Premium,
//! };
//! let job_id = queue.submit(job)?;
//!
//! // Query status
//! let status = queue.get_status(job_id)?;
//! println!("Progress: {}%", status.progress);
//!
//! // Wait for completion
//! let result = queue.wait_for_job(job_id)?;
//! ```

pub mod queue;
pub mod worker;
pub mod status;
pub mod types;

pub use queue::JobQueueSystem;
pub use status::{JobStatus, JobState, JobResult};
pub use types::{EncodingJob, JobPriority, JobId};
pub use worker::EncoderWorker;
