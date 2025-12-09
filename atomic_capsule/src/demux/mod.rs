//! Container Demuxer Capsules
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Provides native container format parsing without external dependencies.
//! Implements T5 Streaming architecture for O(1) memory incremental parsing.
//!
//! ## Supported Formats
//!
//! | Format | Extension | Feature | Status |
//! |--------|-----------|---------|--------|
//! | MP4/ISO BMFF | .mp4, .m4v, .mov | `demux-mp4` | Production |
//! | Matroska | .mkv, .webm | `demux-mkv` | Production |
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
//! |  | (T2 SIMD, 128B)     |     | (MP4/MKV/WebM)              |     |
//! |  +---------------------+     +-----------------------------+     |
//! |          |                              |                        |
//! |          v                              v                        |
//! |  +---------------------+     +-----------------------------+     |
//! |  | Magic Byte Match    |     | Track Parsing               |     |
//! |  | - MP4: ftyp@4       |     | - Video/Audio/Subtitle      |     |
//! |  | - MKV: EBML@0       |     | - Codec Detection           |     |
//! |  +---------------------+     | - Sample Table              |     |
//! |                              +-----------------------------+     |
//! |                                         |                        |
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
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 tier selection per capsule, Q33 derive verification
//! - **Chaos**: 100% lockfree, cache-aligned (64B/128B/256B)
//! - **ASSUM**: All unsafe blocks documented with #ASSUME/#VERIFY tags
//! - **T28**: 5-tier testing (unit/property/integration/production/determinism)

// ============================================================================
// CONTAINER DETECTOR (always available with demux-mp4 OR demux-mkv)
// ============================================================================

#[cfg(any(feature = "demux-mp4", feature = "demux-mkv"))]
pub mod detector;

// ============================================================================
// MP4/ISO BMFF DEMUXER CAPSULES
// ============================================================================

#[cfg(feature = "demux-mp4")]
pub mod mp4;
#[cfg(feature = "demux-mp4")]
pub mod mp4_sample_table;
#[cfg(feature = "demux-mp4")]
pub mod mp4_track;

// ============================================================================
// MATROSKA/WEBM DEMUXER CAPSULES
// ============================================================================

#[cfg(feature = "demux-mkv")]
pub mod mkv;
#[cfg(feature = "demux-mkv")]
pub mod mkv_cluster;
#[cfg(feature = "demux-mkv")]
pub mod mkv_cues;
#[cfg(feature = "demux-mkv")]
pub mod mkv_track;
#[cfg(feature = "demux-mkv")]
pub mod webm_validator;

// ============================================================================
// CONTAINER DETECTOR RE-EXPORTS
// ============================================================================

#[cfg(any(feature = "demux-mp4", feature = "demux-mkv"))]
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

// ============================================================================
// MP4 RE-EXPORTS
// ============================================================================

#[cfg(feature = "demux-mp4")]
pub use mp4::{
    box_types, BoxInfo, DemuxError, DemuxerState, DemuxerStats, FileTypeBox, Mp4DemuxerCapsule,
    CONTAINER_BOXES, FULL_BOXES,
};

#[cfg(feature = "demux-mp4")]
pub use mp4_sample_table::{
    CttsEntry, Mp4SampleTableCapsule, SampleLocation, SampleTableError, StscEntry, SttsEntry,
    sample_table_flags,
};

#[cfg(feature = "demux-mp4")]
pub use mp4_track::{
    AudioCodec, Mp4TrackCapsule, TrackError, TrackSnapshot, TrackType, VideoCodec, track_flags,
};

// ============================================================================
// MKV/WEBM RE-EXPORTS
// ============================================================================

#[cfg(feature = "demux-mkv")]
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

#[cfg(feature = "demux-mkv")]
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

#[cfg(feature = "demux-mkv")]
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

#[cfg(feature = "demux-mkv")]
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

#[cfg(feature = "demux-mkv")]
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
