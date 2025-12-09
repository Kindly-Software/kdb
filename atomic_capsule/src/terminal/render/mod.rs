//! Terminal Rendering Capsules
//!
//! High-performance rendering primitives for terminal emulators:
//! - Glyph caching with LRU eviction
//! - Double-buffered cell grid for smooth updates
//! - GPU texture atlasing (future)
//! - Shader compilation (future)
//!
//! ## Design Principles
//!
//! - **UCE34 Framework**: Systematic rendering discovery
//! - **Chaos Compliant**: 100% lockfree, cache-aligned capsules
//! - **T4+T5 Tiers**: Batch rasterization + Streaming updates
//! - **GPU Acceleration**: Prepare for future GPU rendering backend
//!
//! ## Module Organization
//!
//! - `glyph_cache`: T4+T5 LRU glyph cache with batch rasterization
//! - `cell_buffer`: T1+T4 Double-buffered cell grid for smooth GPU updates
//! - `pipeline`: T6 Mixed pipeline orchestration (GPU state machine)
//! - `effects`: T7 GPU post-processing effects (CRT, bloom, chromatic aberration, vignette, noise)
//! - `atlas`: T7 GPU glyph atlas management
//!
//! ## Feature Flags
//!
//! - `terminal-gpu`: Enable GPU rendering capsules

// All modules enabled (error types renamed to avoid conflicts)
pub mod atlas;
pub mod cell_buffer;
pub mod effects;
pub mod glyph_cache;
pub mod pipeline;

// Atlas exports (T7 GPU glyph atlas)
pub use atlas::{AtlasError, AtlasRegion, TerminalAtlasCapsule};

// Cell buffer exports (T1+T4+T7 double-buffered grid with GPU support)
pub use cell_buffer::{
    TerminalCell, TerminalCellBufferCapsule,
    // GPU double-buffering (T7 Heterogeneous)
    GpuCellBuffer, GpuCellBufferSnapshot, CombinedBufferSnapshot,
    // Error types (T0 Auditable)
    CellBufferError, CellBufferResult,
};

// Effects exports (T7 post-processing)
pub use effects::{EffectFlags, EffectUniforms, TerminalEffectsCapsule};

// Glyph cache exports (T4+T5+T7 LRU cache with GPU upload)
pub use glyph_cache::{
    GlyphCacheCapsule, GlyphEntry, GlyphId, GlyphMetrics,
    // GPU staging buffer (T7 Heterogeneous)
    GpuStagingBufferCapsule, GlyphUploadRequest,
    // Constants
    MAX_BATCH_UPLOAD_SIZE, DEFAULT_STAGING_BUFFER_SIZE,
};

#[cfg(feature = "std")]
pub use glyph_cache::RenderError;

// Pipeline exports (T6 orchestrator with kgpu integration)
pub use pipeline::{
    // Core types
    PipelineError, PipelineState, RecordingStats, ShaderProgram, TerminalPipelineManager,
    // GPU integration types
    FrameStats, ResourceType,
};
