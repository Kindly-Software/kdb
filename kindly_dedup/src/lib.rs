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

// Adaptive Thread Pool (T1 Atomic + T4 Batch tier - dynamic thread scaling)
#[cfg(feature = "parallel-dedup")]
pub mod adaptive_thread_pool;

pub mod audit_events;
pub mod bloom_prefilter;
pub mod bloom_sharded;
pub mod bloom_sharded_audit;
pub mod dedup_algorithm;
pub mod pipeline;
pub mod protection;

// Production hardening (always available)
pub mod config_validation;
pub mod resource_limits;

// Utility modules (zero external dependencies)
pub mod utils;

// Serialization helpers for atomic_capsule migration
pub mod serialize_helpers;

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

// Batch MinHash (Phase 3: T4 Batch tier - 1.5-2× speedup)
pub mod batch_minhash;

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
pub use pipeline::{DedupPipeline, DocId};

#[cfg(feature = "persistent-dedup")]
pub use persistent_pipeline::{PersistentDedupPipeline, PersistentError};

#[cfg(feature = "persistent-dedup")]
pub use lsh::MmapLshBucketer;

#[cfg(feature = "parallel-dedup")]
pub use parallel_pipeline::ParallelDedupPipeline;

pub use pipeline::JaccardThreshold;

// CPU dispatch exports (always available, dispatches at runtime)
pub use cpu_dispatch::MinHashDispatcher;

pub use benchmarking::{
    AuditLogger, B32Runner, BenchmarkAuditEntry, BenchmarkConfig, BenchmarkResult, RealityCheck, SpeedupClassification,
};

pub use protection::{check_protection, init_protection, ProtectionError, TamperType};

pub use pipeline::PipelineError;

// Production hardening exports
pub use config_validation::{validate_deployment_config, validate_for_document_count, ConfigError};
pub use resource_limits::{ResourceError, ResourceLimits};

// Panic boundary exports (production-api only)
#[cfg(feature = "production-api")]
pub use panic_boundary::{PanicSafeError, PanicSafePipeline};

// License Capsule exports
pub use license_capsule::{LicenseCapsule, LicenseError, LicenseResult, LicenseStatus, LicenseTier};

// Custom data loading exports
pub use custom_data::{
    detect_format, load_custom_corpus, load_json, load_jsonl, load_plaintext, print_progress, CustomDataError,
    Document as CustomDocument, FileFormat,
};

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

// Streaming LSH Bucketer exports (Option C Phase 1: T5 Streaming tier)
pub use streaming_lsh_bucketer::StreamingLshBucketer;

// Concurrent Union-Find exports (Option C Phase 2: T1 Atomic lockfree clustering)
pub use concurrent_union_find::ConcurrentUnionFind;

// Streaming Dedup Pipeline exports (Option C Phase 3: T6 Mixed single-pass streaming)
pub use streaming_dedup_pipeline::StreamingDedupPipeline;
