//! Pipeline Coordination Capsules
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! This module provides high-level pipeline coordination capsules for
//! orchestrating parallel video decoding and encoding operations.
//!
//! # Architecture
//!
//! The pipeline module implements capsules for parallel processing of video data:
//!
//! - **Full pipeline orchestration**: DecoderPipelineCapsule coordinates demux -> decode -> output
//! - **Tile-based parallelism**: Modern codecs (VP9, AV1) support spatial
//!   partitioning into independent tiles for parallel decode
//! - **Frame buffer pooling**: Zero-copy frame buffer management for decode output
//! - **Output formatting**: SIMD-accelerated color space conversion
//! - **Lockfree coordination**: All capsules use AtomicU64/AtomicU32 with
//!   Acquire/Release ordering for thread-safe state management
//!
//! # Capsules
//!
//! | Capsule | Tier | Size | Purpose | Speedup |
//! |---------|------|------|---------|---------|
//! | DecoderPipelineCapsule | T6 Mixed | 1024B | Full decode pipeline orchestration | Multi-stage |
//! | TileDecoderCapsule | T4 Batch | 512B | Parallel tile decode coordination | 4-16x |
//! | FrameBufferPoolCapsule | T4 Batch | 512B | Zero-copy frame buffer management | <100ns |
//! | OutputFormatterCapsule | T2 SIMD | 256B | YUV to RGB color conversion | 2-4x |
//!
//! # UCE34/Chaos Compliance
//!
//! - **Q10**: T6 Mixed (DecoderPipelineCapsule), T4 Batch (TileDecoder, FramePool), T2 SIMD (OutputFormatter)
//! - **Q33**: 100% lockfree (AtomicU64/AtomicU32 only)
//! - **Q34**: Generation counters for audit trails
//!
//! # Usage
//!
//! ## Tile Decoder
//!
//! ```rust,ignore
//! use kindly_av1::pipeline::{TileDecoderCapsule, TileGrid, TileInfo};
//!
//! // Configure tile decoder
//! let mut decoder = TileDecoderCapsule::new();
//! let grid = TileGrid::new(4, 4, 256, 256);
//! decoder.configure(&grid)?;
//! decoder.set_worker_count(8);
//!
//! // Decode frame
//! decoder.begin_frame(0)?;
//! for id in 0..16 {
//!     let tile = TileInfo::from_grid(&grid, id);
//!     decoder.add_tile(&tile)?;
//! }
//! decoder.decode_all()?;
//!
//! // Workers process tiles
//! while let Some(tile_id) = decoder.next_tile() {
//!     // Decode tile data...
//!     decoder.complete_tile(tile_id)?;
//! }
//! ```
//!
//! ## Frame Buffer Pool
//!
//! ```rust,ignore
//! use kindly_av1::pipeline::{FrameBufferPoolCapsule, PoolConfig, ChromaFormat};
//!
//! // Create pool for 1080p decode
//! let config = PoolConfig::preset_1080p();
//! let pool = FrameBufferPoolCapsule::new(&config)?;
//!
//! // Acquire buffer for decode
//! let handle = pool.try_acquire().expect("Should have buffer");
//!
//! // Mark as ready after decode
//! pool.mark_ready(handle.id)?;
//!
//! // Display and release
//! pool.mark_display(handle.id)?;
//! pool.mark_free(handle.id)?;
//! ```

pub mod decoder_pipeline;
pub mod frame_pool;
pub mod output_formatter;
pub mod tile_decoder;

// Re-export decoder pipeline capsule and types
pub use decoder_pipeline::{
    // Core capsule
    DecoderPipelineCapsule,
    // State machine
    PipelineState,
    // Format types
    ContainerFormat as PipelineContainerFormat,
    VideoCodec as PipelineVideoCodec,
    ChromaFormat as PipelineChromaFormat,
    // Output types
    DecodedFrame, VideoInfo,
    // Statistics
    PipelineStats,
    // Error type
    PipelineError,
    // Phase flags
    phase_flags as pipeline_phase_flags,
};

// Re-export tile decoder capsule and types
pub use tile_decoder::{
    // Core capsule
    TileDecoderCapsule,
    // Configuration types
    TileGrid, TileInfo, TileWork,
    // State enums
    TileState, DecoderState,
    // Statistics
    TileDecoderStats,
    // Error type
    TileDecoderError,
    // Constants
    MAX_TILE_COLS, MAX_TILE_ROWS, MAX_INLINE_TILES,
    WORK_QUEUE_CAPACITY, DEFAULT_WORKERS,
};

// Re-export frame buffer pool capsule and types
pub use frame_pool::{
    // Core capsule
    FrameBufferPoolCapsule,
    // Configuration
    PoolConfig,
    // Handle type
    FrameBufferHandle,
    // State types
    FrameBufferState,
    ChromaFormat,
    // Buffer metadata
    FrameBuffer,
    // Statistics
    FramePoolStats,
    // Error type
    FramePoolError,
};

// Re-export output formatter capsule and types
pub use output_formatter::{
    // Core capsule
    OutputFormatterCapsule,
    // Output formats
    OutputFormat,
    // Color space and range
    ColorSpace, ColorRange,
    // Statistics
    OutputFormatterStats,
    // Error type
    OutputError,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_decoder_exports() {
        // Verify tile decoder types are accessible
        let _decoder = TileDecoderCapsule::new();
        let _grid = TileGrid::new(4, 4, 256, 256);
        let _state = TileState::Pending;
        let _decoder_state = DecoderState::Idle;
    }

    #[test]
    fn test_tile_decoder_size() {
        assert_eq!(core::mem::size_of::<TileDecoderCapsule>(), 512);
        assert_eq!(core::mem::align_of::<TileDecoderCapsule>(), 512);
    }

    #[test]
    fn test_frame_pool_exports() {
        // Verify frame pool types are accessible
        let config = PoolConfig::preset_1080p();
        let pool = FrameBufferPoolCapsule::new(&config).unwrap();
        let _state = FrameBufferState::Free;
        let _chroma = ChromaFormat::Yuv420;
        let _stats = pool.stats();
    }

    #[test]
    fn test_frame_pool_size() {
        assert_eq!(core::mem::size_of::<FrameBufferPoolCapsule>(), 512);
        assert_eq!(core::mem::align_of::<FrameBufferPoolCapsule>(), 512);
    }

    #[test]
    fn test_output_formatter_exports() {
        // Verify output formatter types are accessible
        let _formatter = OutputFormatterCapsule::new();
        let _format = OutputFormat::Rgb24;
        let _space = ColorSpace::BT709;
        let _range = ColorRange::Limited;
        let _stats = _formatter.stats();
    }

    #[test]
    fn test_output_formatter_size() {
        assert_eq!(core::mem::size_of::<OutputFormatterCapsule>(), 256);
        assert_eq!(core::mem::align_of::<OutputFormatterCapsule>(), 256);
    }

    #[test]
    fn test_decoder_pipeline_exports() {
        // Verify decoder pipeline types are accessible
        let _pipeline = DecoderPipelineCapsule::new();
        let _state = PipelineState::Idle;
        let _format = PipelineContainerFormat::Mp4;
        let _codec = PipelineVideoCodec::H264;
        let _chroma = PipelineChromaFormat::Yuv420;
        let _info = VideoInfo::default();
        let _frame = DecodedFrame::default();
        let _stats = _pipeline.stats();
    }

    #[test]
    fn test_decoder_pipeline_size() {
        assert_eq!(core::mem::size_of::<DecoderPipelineCapsule>(), 1024);
        assert_eq!(core::mem::align_of::<DecoderPipelineCapsule>(), 1024);
    }

    #[test]
    fn test_decoder_pipeline_state_transitions() {
        let mut pipeline = DecoderPipelineCapsule::new();
        assert_eq!(pipeline.state(), PipelineState::Idle);

        // Initialize with valid data
        let mkv_data: Vec<u8> = vec![
            0x1A, 0x45, 0xDF, 0xA3, // EBML header
            0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x1F,
            0x00, 0x00, 0x00, 0x00,
        ];
        pipeline.open_data(&mkv_data).unwrap();
        assert_eq!(pipeline.state(), PipelineState::DecoderReady);
    }
}
