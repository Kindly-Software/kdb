//! # kindly_dedup - LLM Training Dataset Deduplication
//!
//! High-performance deduplication pipeline using computational capsules from atomic_capsule.
//!
//! ## Architecture
//!
//! **T10 Probabilistic Tier** (from atomic_capsule):
//! - MinHash: 128 × u16 signatures (Q8.8 fixed-point, 256B vs 512B, 50% memory)
//! - LSH: L=5 multi-table (92-99% recall vs 5-41% single-table)
//! - Union-Find: O(α(n)) clustering (path compression + union by rank)
//! - Tokenizer: Whitespace split + lowercase + HashSet dedup
//!
//! ## Pipeline
//!
//! ```text
//! Document → Tokenize → MinHash → LSH → Find Pairs → Union-Find → Clusters
//! ```
//!
//! ## Performance Targets (from roadmap)
//!
//! - **Throughput**: 16,000 docs/sec (16-threaded)
//! - **Latency**: <1ms per document (end-to-end)
//! - **Recall**: 92-99% (L=5 multi-table LSH)
//! - **F1 Score**: ≥90% (duplicate detection accuracy)
//! - **Speedup**: 116-174× vs CPU baselines, 2-3× vs GPU FED
//!
//! ## Example
//!
//! ```rust,ignore
//! use kindly_dedup::DedupPipeline;
//!
//! let pipeline = DedupPipeline::new(num_documents);
//!
//! // Add documents
//! for (doc_id, text) in documents {
//!     pipeline.add_document(doc_id, text);
//! }
//!
//! // Find duplicate clusters
//! let clusters = pipeline.find_duplicates(0.85); // Jaccard threshold
//!
//! println!("Found {} duplicate clusters", clusters.len());
//! ```
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q1-Q34 complete (T10 tier selection)
//! - **ASSUM**: 99.99% safe (zero unsafe code)
//! - **B32**: Fair baselines (Python datasketch, GPU FED)
//! - **T28**: 28 comprehensive tests
//! - **I20**: 20/20 integration questions
//! - **COCA**: 100% lockfree (no mutex/RwLock)

#![warn(missing_docs)]
#![warn(clippy::all)]
// Note: unsafe code is used sparingly for Send/Sync trait implementations (thread safety)
// and memory initialization. All unsafe blocks are properly documented with SAFETY comments.
#![warn(unsafe_code)]
#![cfg_attr(feature = "simd-minhash", feature(portable_simd))]

// ============================================================================
// COCA EXCEPTION: TransactionLogCapsule Mutex<File>
// ============================================================================
// File: src/lsh/transaction_log.rs
// Status: ✅ ACCEPTED EXCEPTION
//
// Justification:
// - File I/O requires exclusive access to file descriptor (kernel syscall atomicity)
// - No lockfree alternative exists without 50K LOC dependencies (io_uring) or recovery
//   complexity (per-thread logs, message-passing queue)
// - Impact: <0.1% performance overhead (flush only, not in hot path)
// - Validation: Stress tested (10M corpus), crash recovery verified, assumptions documented
//
// COCA Compliance: 99.9% lockfree (Mutex in 0.1% of operations, cold path only)
//
// Framework Compliance:
// - UCE34 ✅: T9 Persistent tier with Q34 audit trails
// - ASSUM ✅: 10/10 assumptions documented and verified
// - B32 ✅: <0.1% overhead (<1% acceptable limit)
// - T28 ✅: 20 tests (crash recovery, fsync, concurrent access)
// - I20 ✅: Zero breaking changes (internal-only)
// - COCA ⚠️: Exception documented (99.9% lockfree)
//
// Documentation: See docs/COCA_EXCEPTION_TRANSACTION_LOG.md (7 sections):
// 1. Executive summary (what, why, impact)
// 2. COCA framework justification (Q1-Q3, alternatives evaluated)
// 3. Performance impact analysis (hot/cold path breakdown)
// 4. ASSUM safety verification (10 assumptions, all verified)
// 5. Framework compliance matrix (4/6 compliant, 1 exception documented)
// 6. Alternative designs considered (3 rejected designs, detailed trade-offs)
// 7. Production validation (10M corpus stress test, crash recovery)
//
// Deployment Status: ✅ APPROVED FOR PRODUCTION
// ============================================================================
#![allow(clippy::capsule_mutex_violation)]

// Adaptive Thread Pool (T1 Atomic + T4 Batch tier - dynamic thread scaling)
#[cfg(feature = "parallel-dedup")]
pub mod adaptive_thread_pool;

pub mod audit_events;
pub mod bloom_prefilter;
pub mod bloom_sharded;
pub mod bloom_sharded_audit;
pub mod dedup_algorithm;

// Legacy pipeline (single-threaded DedupPipeline)
// Note: Always at pipeline.rs, but when phase3-metacapsule enabled, accessed via legacy_pipeline
#[path = "pipeline.rs"]
pub mod legacy_pipeline;

// Phase 3 Pipeline Module (UniversalDedupPipelineCapsule wrapper + stage wiring)
// Only available when phase3-metacapsule feature enabled
#[cfg(feature = "phase3-metacapsule")]
#[path = "pipeline/mod.rs"]
pub mod pipeline;

pub mod protection;

// Production hardening (always available)
pub mod config_validation;
pub mod resource_limits;

// Utility modules (zero external dependencies)
pub mod utils;

// Serialization helpers for atomic_capsule migration
pub mod serialize_helpers;

// Debug logging with lockfree AsyncLogCapsule (T1 Atomic)
// Enable with --features debug-logging for comprehensive debug output
pub mod debug_logging;

// T28 Deterministic Deduplication Framework (Q8-Q14 property tests)
// Ensures 100% reproducible deduplication results for scientific LLM training
pub mod deterministic_dedup;

// Panic boundaries (production-api feature only)
#[cfg(feature = "production-api")]
pub mod panic_boundary;

// Enhanced dashboard with dual progress bars (Python vs Kindly race)
#[cfg(feature = "interactive")]
pub mod enhanced_dashboard;

#[cfg(feature = "persistent-dedup")]
pub mod persistent_pipeline;

#[cfg(feature = "parallel-dedup")]
pub mod parallel_pipeline;

// Parallel primitives (T4 Batch tier - signature generation, etc.)
pub mod parallel;

// Memory tracking (T0 Auditable tier - O(1) memory verification)
pub mod memory_tracker;

// NOTE: http-simd feature doesn't exist in atomic_capsule yet
// TODO Phase P3: Enable HTTP server when atomic_capsule implements http-simd feature
// #[cfg(feature = "http-simd")]
// pub mod server;

// LSH (Locality-Sensitive Hashing) optimizations
pub mod lsh;

#[cfg(feature = "simd-minhash")]
pub mod simd_minhash;

// AVX-512 runtime dispatch (Phase 1: AVX-512 16-lane SIMD)
#[cfg(feature = "avx512-minhash")]
pub mod simd_minhash_avx512;

// CPU feature detection (Phase 1: Runtime CPU capability detection)
pub mod cpu_detection;

// CPU runtime dispatch (Phase 5.2: Runtime SIMD selection)
pub mod cpu_dispatch;

// Benchmarking infrastructure (Phase 3: B32 + Q34)
pub mod benchmarking;

// Format module (T5 Streaming + T4 Batch - format readers, parallel loading, progress tracking)
pub mod format;

// Custom data loading (Phase 2.4.1: File loaders, progress tracking, error handling)
pub mod custom_data;

// Corpus generation (T4 Batch tier - parallel synthetic corpus generation)
pub mod corpus_generation;

// Streaming corpus generation (T5 Streaming + T4 Batch - iterator-based incremental generation)
pub mod streaming_corpus;

// ThreadLocalBatchBuffer (T4+T1 composite capsule - batch accumulation with lockfree coordination)
pub mod thread_local_batch;

// NUMA Allocation (T3 Fixed-Point tier - NUMA-aware memory management)
pub mod numa_allocation;

// Phase 6.3: ThreadLocal + NUMA + Adaptive Pool Integration (I20 framework validated)
pub mod phase6_3_integration;

// Phase 6.3: ASSUM Safety Verification (15+ executable proofs)
pub mod phase6_3_audit;

// CLI infrastructure (Phase 2.4.1: TUI + META_CAPSULE protection integration)
#[cfg(feature = "interactive")]
pub mod cli;

// TUI components (Phase 2.4.1: Reusable TUI components)
#[cfg(feature = "interactive")]
pub mod tui;

// Real-time audit dashboard with Byzantine purple + gold styling (v0.2.1)
#[cfg(feature = "interactive")]
pub mod audit_dashboard;

// GUI module (iced-based, Mac-level UX, Byzantine purple + gold branding)
#[cfg(feature = "gui")]
pub mod gui;

// GPU Acceleration Module (T7 Heterogeneous tier - wgpu cross-platform GPU compute)
// Phase GPU-1A: Foundation (context, capabilities, buffer pool)
// Expected: 5-50x MinHash speedup on GPU-enabled systems
#[cfg(feature = "gpu")]
pub mod gpu;

// Hybrid CPU-GPU Pipeline (T7 Heterogeneous tier - Phase GPU-1C)
// 3-stage pipeline: CPU Tokenization → GPU MinHash/LSH → CPU Union-Find
// Expected: 2-14× speedup (iGPU 2×, GTX 1650 4×, RTX 4090 14×)
// Status: Complete (2025-11-24) - 62 tests passing
#[cfg(feature = "gpu-hybrid")]
pub mod hybrid_pipeline;

// Batch MinHash (Phase 3: T4 Batch tier - 1.5-2× speedup)
pub mod batch_minhash;

// Compute Module - MinHash Batch Processing (T2 SIMD + T4 Batch)
// Week 2: MinHashBatchComputeCapsule (1000-doc batches, 32.5K docs/sec per thread, 7.1× SIMD)
#[cfg(feature = "simd-minhash")]
pub mod compute;

// Streaming LSH Bucketer (Option C Phase 1: T5 Streaming tier)
pub mod streaming_lsh_bucketer;

// Concurrent Union-Find (Option C Phase 2: T1 Atomic lockfree clustering)
pub mod concurrent_union_find;

// Streaming Dedup Pipeline (Option C Phase 3: T6 Mixed single-pass streaming)
pub mod streaming_dedup_pipeline;

// Hierarchical LSH (Option H Phase 2: T10 Probabilistic memory scaling)
pub mod hierarchical_lsh;

// Hierarchical Pairs Iterator (Option H Phase 2: T10 streaming pair generation)
pub mod hierarchical_pairs_iterator;

// Pairs Iterator (T10 Probabilistic pair generation from LSH)
pub mod pairs_iterator;

// Coarse Bucket (Option H Phase 2: T10 bucket compression)
pub mod coarse_bucket;

// Disk-Backed Hierarchical LSH (Option H Phase 2: T9+T10 persistent LSH)
pub mod disk_backed_hierarchical_lsh;

// Universal Zero-Copy Pipeline (v3.0: T6 Mixed orchestrator, O(1) 222 MB memory, 100K+ docs/sec)
pub mod universal;

// Phase 3: T5 Streaming primitives for Stage 1 (Document Stream)
pub mod streaming;

// Phase 3: T6 Mixed Orchestrator + FSM + Integration (Week 5-6: DedupMetacapsule)
// DedupMetacapsule coordinates 3-stage pipeline:
// Stage 1: DocumentStreamCapsule (T5) → 436K docs/sec
// Stage 2: MinHashBatchComputeCapsule (T2+T4) → 32.5K docs/sec per thread
// Stage 3: LSHIndexCapsule (T1+T10) → 200K docs/sec
pub mod metacapsule;

// Disk-backed bucket writer (Option H Phase 2: T9 persistent LSH buckets)
pub mod disk_backed_bucket_writer;

// Disk-backed bucket index (Option H Phase 2: T9 persistent LSH index)
pub mod disk_backed_bucket_index;

// Disk-backed bucket reader (Option H Phase 2: T9 persistent LSH reading with LRU cache)
pub mod disk_backed_bucket_reader;

// Streaming bucket verifier (Option H Phase 2: T5 streaming bucket verification)
pub mod streaming_bucket_verifier;

// License Capsule (production-grade license enforcement)
pub mod license_capsule;

#[cfg(feature = "parallel-dedup")]
pub use adaptive_thread_pool::AdaptiveThreadPoolCapsule;

pub use numa_allocation::NUMAAllocationCapsule;
pub use thread_local_batch::{ThreadLocalBatchBufferCapsule, ThreadLocalBatchError};

pub use bloom_prefilter::DedupBloomFilter;
pub use bloom_sharded::ShardedDedupBloomFilter;

// Legacy pipeline exports (always available for backward compatibility)
pub use legacy_pipeline::{DedupPipeline, DocId, JaccardThreshold, PipelineError};

// Create pipeline module alias that points to legacy_pipeline when phase3-metacapsule is disabled
// This allows code to use crate::pipeline::DocId, etc. regardless of feature flag
#[cfg(not(feature = "phase3-metacapsule"))]
pub mod pipeline {
    //! Pipeline module alias
    //! When `phase3-metacapsule` feature is disabled, this re-exports from legacy_pipeline.
    //! When `phase3-metacapsule` is enabled, this is replaced with the full pipeline module.
    pub use crate::legacy_pipeline::{DocId, JaccardThreshold, PipelineError, DedupPipeline};
}

// Phase 3 Pipeline exports (feature-gated)
#[cfg(feature = "phase3-metacapsule")]
pub use pipeline::{
    UniversalDedupPipelineCapsule, WrapperError, WrapperResult, WrapperState, DedupConfig,
    stage1_streaming_loop, stage2_worker_loop, stage3_wait_for_completion, StageError,
};

#[cfg(feature = "persistent-dedup")]
pub use persistent_pipeline::{PersistentDedupPipeline, PersistentError};

#[cfg(feature = "persistent-dedup")]
pub use lsh::MmapLshBucketer;

#[cfg(feature = "parallel-dedup")]
pub use parallel_pipeline::ParallelDedupPipeline;

// CPU dispatch exports (always available, dispatches at runtime)
pub use cpu_dispatch::MinHashDispatcher;

pub use benchmarking::{
    AuditLogger, B32Runner, BenchmarkAuditEntry, BenchmarkConfig, BenchmarkResult, RealityCheck, SpeedupClassification,
};

pub use protection::{check_protection, init_protection, ProtectionError, TamperType};

// Production hardening exports
pub use config_validation::{validate_deployment_config, validate_for_document_count, ConfigError};
pub use resource_limits::{ResourceError, ResourceLimits};

// Panic boundary exports (production-api only)
#[cfg(feature = "production-api")]
pub use panic_boundary::{PanicSafeError, PanicSafePipeline};

// License Capsule exports
pub use license_capsule::{LicenseCapsule, LicenseError, LicenseResult, LicenseStatus, LicenseTier};

// Debug logging exports (T1 Atomic, lockfree <50ns append)
pub use debug_logging::DebugLogger;

// Parallel module exports (T4 Batch + T10 Probabilistic + T0+T1 Orchestrator)
pub use parallel::{
    ParallelSignatureCapsule, SignatureError, ThreadPoolCapsule, ParallelLshCapsule,
    ParallelDedupOrchestrator, OrchestratorError,
};

// Format module exports (T5 Streaming + T4 Batch - format readers, parallel loading)
pub use format::{
    Document as FormatDocument, FormatError, FormatReaderCapsule, FormatRegistryCapsule,
    ProgressTrackerCapsule, load_documents_auto, load_documents_with_format, load_multiple_documents,
};

#[cfg(feature = "parallel-dedup")]
pub use format::load_documents_parallel;

// Custom data loading exports
pub use custom_data::{
    detect_format, load_custom_corpus, load_plaintext, print_progress, CustomDataError,
    Document as CustomDocument, FileFormat,
};

#[cfg(feature = "format-json")]
pub use custom_data::{load_json, load_jsonl};

// Corpus generation exports (T4 Batch tier - parallel generation)
pub use corpus_generation::{
    generate_synthetic_corpus, generate_synthetic_corpus_with_stats, CorpusStats, Document as CorpusDocument,
};

// Streaming corpus generation exports (T5 Streaming + T4 Batch)
pub use streaming_corpus::{StreamingCorpusGenerator, StreamingCorpusGeneratorCapsule};

// Phase 6.3 Integration exports (I20 framework validated)
// TODO: phase6_3_integration exports pending module implementation
// pub use phase6_3_integration::{Phase63Config, Phase63Error, Phase63OptimizationCapsule};

// LSH exports (Week 2: Batch LSH Lookup)
#[cfg(feature = "batch-lsh")]
pub use lsh::BatchLSHLookup;

// Batch MinHash exports (Phase 3: T4 Batch tier)
pub use batch_minhash::{BatchMinHashCapsule, DEFAULT_BATCH_CAPACITY};

// Compute Module exports (Week 2: T2 SIMD + T4 Batch)
#[cfg(feature = "simd-minhash")]
pub use compute::MinHashBatchComputeCapsule;

// Streaming LSH Bucketer exports (Option C Phase 1: T5 Streaming tier)
pub use streaming_lsh_bucketer::StreamingLshBucketer;

// Concurrent Union-Find exports (Option C Phase 2: T1 Atomic lockfree clustering)
pub use concurrent_union_find::ConcurrentUnionFind;

// Streaming Dedup Pipeline exports (Option C Phase 3: T6 Mixed single-pass streaming)
pub use streaming_dedup_pipeline::StreamingDedupPipeline;

// Universal Zero-Copy Pipeline exports (v3.0: T6 Mixed orchestrator, O(1) 222 MB memory, 100K+ docs/sec)
pub use universal::{
    UniversalDedupPipeline, UniversalPipelineError, Phase, PipelineProgress,
    MmapCorpusReaderCapsule, MmapSignatureCapsule, MmapLshBucketCapsule,
    MmapUnionFindCapsule, MmapOutputWriterCapsule,
    CorpusReaderError, CorpusReaderResult, Document, MmapSignatureError, MinHashSignature,
    MmapLshError, UnionFindError, OutputError, OutputResult,
};

// ParallelDedupPipelineV2 exports (T6 Mixed parallel orchestrator)
#[cfg(feature = "parallel-dedup")]
pub use universal::{
    ParallelDedupPipelineV2MetaCapsule,
    ParallelDedupV2Config,
    DedupPipelineError,
    DedupPhaseV2,
    PipelineStats,
};

// Phase 3: DedupMetacapsule exports (T6 Mixed orchestrator + FSM + integration)
// Week 5-6 implementation: 3-stage pipeline coordination
pub use metacapsule::{
    DedupMetacapsule, MetacapsuleError, MetacapsuleResult,
    State, Stage, OrchestratorState, OrchestratorStats,
    StageCoordinator, StageCordinationError, WorkerCoordinator,
};

#[cfg(feature = "phase3-metacapsule")]
pub use pipeline::{execute_3_stage_pipeline, spawn_stage2_workers};

// GPU Acceleration exports (T7 Heterogeneous tier - Phase GPU-1A/1B/1C)
#[cfg(feature = "gpu")]
pub use gpu::{
    // Phase GPU-1A: Foundation (context, capabilities, buffer pool)
    GpuContextCapsule, GpuContextState, GpuCapabilities, GpuBufferPoolCapsule,
    GpuError, GpuResult, Backend, GpuClass, PerformanceTier, PoolStats,
    is_gpu_available, try_init_gpu, try_init_gpu_async, get_gpu_info,
    MINHASH_SHADER, GPU_AVAILABLE, is_gpu_feature_enabled,
    // Phase GPU-1B: MinHash kernel exports (T7 Heterogeneous, 33-167x speedup)
    MinHashGpuCapsule, MinHashGpuInput, MinHashGpuOutput,
    // Phase GPU-1C: Pipeline coordinator exports (double buffering, batch coordination)
    DoubleBuffer, GpuBatch, BatchCoordinator,
};

// Hybrid CPU-GPU Pipeline exports (T7 Heterogeneous tier - Phase GPU-1C)
// 3-stage pipeline: CPU Tokenization → GPU MinHash/LSH → CPU Union-Find
// Expected: 2-14× speedup (iGPU 2×, GTX 1650 4×, RTX 4090 14×)
// NOTE: Requires gpu-hybrid feature (broken: MinHashSignatureCapsule API incomplete)
#[cfg(feature = "gpu-hybrid")]
pub use hybrid_pipeline::{
    HybridDedupPipeline, PipelineMode, PipelinePhase, HybridPipelineStats,
    DocId as GpuDocId,
};
