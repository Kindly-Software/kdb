//! Native Demuxer Stack for kindly-av1
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Provides native container format parsing without external dependencies.
//! Implements T5 Streaming architecture for O(1) memory incremental parsing.
//!
//! ## Supported Formats
//!
//! | Format | Extension | Module | Status |
//! |--------|-----------|--------|--------|
//! | MP4/ISO BMFF | .mp4, .m4v, .mov | `mp4` | Production |
//! | Matroska | .mkv, .webm | `mkv_cluster` | Production |
//! | AVI | .avi | `avi` | Planned |
//! | Transport Stream | .ts, .mts | `ts` | Planned |
//!
//! ## Architecture
//!
//! ```text
//! +-------------------------------------------------------------------+
//! |                    Native Demuxer Stack (T6 Mixed)                |
//! +-------------------------------------------------------------------+
//! |                                                                   |
//! |  +---------------------+     +-----------------------------+     |
//! |  | ContainerDetector   |---->| Format-Specific Demuxer     |     |
//! |  | (T2 SIMD, 128B)     |     | (MP4/MKV/WebM/AVI/TS)       |     |
//! |  +---------------------+     +-----------------------------+     |
//! |          |                              |                        |
//! |          v                              v                        |
//! |  +---------------------+     +-----------------------------+     |
//! |  | Magic Byte Match    |     | Track Parsing               |     |
//! |  | - MP4: ftyp@4       |     | - Video/Audio/Subtitle      |     |
//! |  | - MKV: EBML@0       |     | - Codec Detection           |     |
//! |  | - AVI: RIFF+AVI     |     | - Sample Table              |     |
//! |  | - TS: 0x47 sync     |     +-----------------------------+     |
//! |  +---------------------+                |                        |
//! |                                         v                        |
//! |                              +-----------------------------+     |
//! |                              | Frame Extraction            |     |
//! |                              | - Sample Table Parsing      |     |
//! |                              | - Keyframe Detection        |     |
//! |                              | - Timestamp Mapping         |     |
//! |                              +-----------------------------+     |
//! |                                                                   |
//! +-------------------------------------------------------------------+
//! ```
//!
//! ## Capsule Tiers
//!
//! | Capsule | Tier | Size | Purpose |
//! |---------|------|------|---------|
//! | ContainerDetectorCapsule | T2 SIMD | 128B | Magic byte detection |
//! | Mp4DemuxerCapsule | T5 Streaming | 512B | ISO BMFF parsing |
//! | MkvClusterCapsule | T4 Batch | 512B | MKV/WebM cluster block parsing |
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kindly_av1::demux::{ContainerDetectorCapsule, ContainerFormat, Mp4DemuxerCapsule};
//!
//! // Detect container format
//! let detector = ContainerDetectorCapsule::new();
//! let format = detector.detect(&header);
//!
//! match format {
//!     ContainerFormat::Mp4 => {
//!         let demuxer = Mp4DemuxerCapsule::new();
//!         // Parse MP4 container...
//!     }
//!     _ => println!("Unsupported: {}", format),
//! }
//! ```

// Sub-modules
pub mod avi;
pub mod detector;
pub mod mkv;
pub mod mkv_cluster;
pub mod mkv_track;
pub mod mkv_cues;
pub mod mp4;
pub mod mp4_sample_table;
pub mod mp4_track;
pub mod webm_validator;

// Re-export container detection (Phase 1)
pub use detector::{
    ContainerDetectorCapsule,
    ContainerFormat,
    DetectorStats,
    // Magic byte constants
    MP4_FTYP,
    MKV_EBML,
    WEBM_EBML,
    AVI_RIFF,
    AVI_TYPE,
    TS_SYNC,
    MIN_HEADER_SIZE,
};

// Re-export MP4 demuxer types
pub use mp4::{
    box_types, BoxInfo, DemuxError, DemuxerState, DemuxerStats, FileTypeBox, Mp4DemuxerCapsule,
    CONTAINER_BOXES, FULL_BOXES,
};

// Re-export MP4 sample table types
pub use mp4_sample_table::{
    CttsEntry, Mp4SampleTableCapsule, SampleLocation, SampleTableError, StscEntry, SttsEntry,
    sample_table_flags,
};

// Re-export MP4 track types
pub use mp4_track::{
    AudioCodec, Mp4TrackCapsule, TrackError, TrackSnapshot, TrackType, VideoCodec, track_flags,
};

// Re-export WebM validator types
pub use webm_validator::{
    // Capsule
    WebMValidatorCapsule,
    // Error types
    WebMValidationError,
    // Supporting types
    EbmlHeader, MkvTrackCapsule, LacingType as WebmLacingType, ValidationSnapshot,
    // Codec constants
    WEBM_VIDEO_CODECS, WEBM_AUDIO_CODECS,
    // Element ID constants
    EBML_HEADER, EBML_DOC_TYPE, EBML_DOC_TYPE_VERSION, EBML_DOC_TYPE_READ_VERSION,
    SEGMENT, TRACKS, TRACK_ENTRY, CODEC_ID, TRACK_TYPE,
    CUES, CLUSTER, SIMPLE_BLOCK, BLOCK, BLOCK_GROUP, BLOCK_ADDITIONS,
    CHAPTERS, ATTACHMENTS, TAGS,
    // Validation flags
    validation_flags,
};

// Re-export MKV cluster types (T4 Batch tier)
pub use mkv_cluster::{
    // Capsule
    MkvClusterCapsule,
    // Error types
    MkvClusterError,
    // Block types
    BlockHeader, BlockInfo, ClusterHeader, LacingType, ClusterStats,
    // Element ID constants
    CLUSTER as MKV_CLUSTER, TIMECODE, POSITION, PREV_SIZE,
    SIMPLE_BLOCK as MKV_SIMPLE_BLOCK, BLOCK_GROUP as MKV_BLOCK_GROUP,
    BLOCK as MKV_BLOCK, BLOCK_DURATION, REFERENCE_BLOCK,
    // State flags
    cluster_state,
};

// Re-export MKV cues types (T4 Batch tier)
pub use mkv_cues::{
    // Capsule
    MkvCuesCapsule,
    // Error types
    MkvCuesError,
    // Supporting types
    CuePoint, SeekTarget, MkvCuesState, MkvCuesStats,
    // Element ID constants
    CUES as MKV_CUES, CUE_POINT, CUE_TIME, CUE_TRACK_POSITIONS,
    CUE_TRACK, CUE_CLUSTER_POSITION, CUE_RELATIVE_POSITION,
    CUE_DURATION, CUE_BLOCK_NUMBER,
    // Limits
    MAX_INLINE_CUES,
};

// Re-export MKV track atomic capsule types (T1 Atomic tier, 256B cache-aligned)
pub use mkv_track::{
    // Capsule (renamed to avoid conflict with webm_validator::MkvTrackCapsule)
    MkvTrackCapsule as MkvTrackAtomicCapsule,
    // Snapshot
    MkvTrackSnapshot,
    // Error types
    MkvTrackError,
    // Track types
    MkvTrackType, MkvVideoCodec, MkvAudioCodec,
    // Element ID constants
    element_ids as mkv_track_element_ids,
    // Codec ID constants
    video_codec_ids, audio_codec_ids,
    // Track flags
    mkv_track_flags,
};

// Re-export MKV demuxer types (T5 Streaming tier - EBML header/segment parsing)
pub use mkv::{
    // Capsule
    MkvDemuxerCapsule,
    // Error types
    MkvError,
    // State machine
    MkvDemuxerState,
    // Parsed types
    EbmlHeader as MkvEbmlHeader, SegmentInfo as MkvSegmentInfo, MkvInfo, EbmlElement,
    // Statistics
    MkvDemuxerStats,
    // Element ID constants
    element_ids as mkv_element_ids,
    // Utility functions
    is_master_element, MASTER_ELEMENTS,
};

// Re-export AVI demuxer types (T5 Streaming tier)
pub use avi::{
    // Capsule
    AviDemuxerCapsule,
    // Error types
    DemuxError as AviDemuxError,
    // State machine
    DemuxerState as AviDemuxerState,
    // Parsed types
    ChunkInfo, AviMainHeader, StreamHeader,
    // Statistics
    DemuxerStats as AviDemuxerStats,
    // FourCC constants
    fourcc as avi_fourcc,
    stream_type as avi_stream_type,
    video_codec as avi_video_codec,
    // Utility constants
    LIST_CHUNKS as AVI_LIST_CHUNKS,
};
