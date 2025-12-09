//! Container Muxer Capsules - Phase 11A
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Production-ready muxers for common video container formats:
//!
//! ## Supported Formats
//!
//! - **MP4** (ISO Base Media File Format)
//!   - ftyp/moov/mdat atom structure
//!   - H.264/H.265/VP9/AV1 video tracks
//!   - AAC/Opus/FLAC/MP3/AC-3/E-AC-3 audio tracks
//!   - Edit lists, chapter markers
//!   - Fast-start support (moov before mdat)
//!   - Sample table management (stts, ctts, stsc, stsz, stco/co64, stss)
//!   - RLE compression for sample timing tables
//!   - O(1) streaming append operations
//!
//! - **MKV** (Matroska)
//!   - EBML element serialization
//!   - Multiple video/audio/subtitle tracks
//!   - Cues (seek index) generation
//!   - Chapters, tags, attachments
//!
//! - **WebM** (WebM subset of MKV)
//!   - VP9/AV1 video only
//!   - Vorbis/Opus audio only
//!   - Optimized for web streaming
//!
//! - **Fragmented MP4** (fMP4)
//!   - DASH/HLS compatible segments
//!   - moof/mdat fragment structure
//!   - Init segment + media segments
//!
//! ## Architecture
//!
//! Each format has dedicated capsules:
//! - **Writer Capsules** (T1 Atomic): Atom/EBML serialization
//! - **Muxer Capsules** (T5 Streaming): Format-specific muxing
//! - **Metacapsule** (T6 Mixed): Orchestration across formats
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T1/T5/T6 tiers, Q33 lockfree atomics
//! - **Chaos**: 100% lockfree, 256B/512B cache-aligned capsules
//! - **ASSUM**: All unsafe documented with #ASSUME/#VERIFY
//! - **T28**: 5-tier testing (unit/property/integration/production/determinism)
//! - **B32**: Criterion benchmarks with 95% CI

#![allow(dead_code)]

// MP4 muxing
#[cfg(feature = "mux-mp4")]
pub mod mp4_box_writer;
#[cfg(feature = "mux-mp4")]
pub mod fragmented_mp4;
#[cfg(feature = "mux-mp4")]
pub mod mp4_muxer;

// MKV/WebM muxing
#[cfg(feature = "mux-mkv")]
pub mod ebml_writer;
#[cfg(feature = "mux-mkv")]
pub mod mkv_muxer;

// WebM Muxer - T5 Streaming tier (Phase 11A complete)
// WebM-compliant subset of MKV: VP8/VP9/AV1 video + Vorbis/Opus audio only
#[cfg(feature = "mux-webm")]
pub mod webm_muxer;

// Shared utilities
pub mod timestamp;

// Muxer metacapsule (T6 Mixed orchestration) - always available
pub mod muxer_metacapsule;

// Re-exports
#[cfg(feature = "mux-mp4")]
pub use mp4_box_writer::*;
#[cfg(feature = "mux-mp4")]
pub use fragmented_mp4::*;
#[cfg(feature = "mux-mp4")]
pub use mp4_muxer::*;

#[cfg(feature = "mux-mkv")]
pub use ebml_writer::*;
#[cfg(feature = "mux-mkv")]
pub use mkv_muxer::*;

#[cfg(feature = "mux-webm")]
pub use webm_muxer::*;

pub use timestamp::*;
#[cfg(any(feature = "mux-mp4", feature = "mux-mkv", feature = "mux-webm"))]
pub use muxer_metacapsule::*;

/// Common codec identifiers for muxing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VideoCodec {
    /// H.264/AVC
    H264 = 0,
    /// H.265/HEVC
    H265 = 1,
    /// VP9
    Vp9 = 2,
    /// AV1
    Av1 = 3,
}

/// Common audio codec identifiers for muxing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AudioCodec {
    /// AAC (Advanced Audio Coding)
    Aac = 0,
    /// Opus
    Opus = 1,
    /// FLAC
    Flac = 2,
    /// Vorbis
    Vorbis = 3,
    /// MP3
    Mp3 = 4,
    /// AC-3
    Ac3 = 5,
    /// E-AC-3
    Eac3 = 6,
}

/// Track type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TrackType {
    /// Video track
    Video = 1,
    /// Audio track
    Audio = 2,
    /// Subtitle track
    Subtitle = 3,
    /// Chapter markers
    Chapters = 4,
}

/// Sample/frame flags
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct SampleFlags {
    /// Is keyframe (sync sample)
    pub is_keyframe: bool,
    /// Depends on other frames
    pub depends_on_others: bool,
    /// Other frames depend on this
    pub depended_on: bool,
    /// Has redundant coding
    pub has_redundancy: bool,
}

/// Common timescale values
pub const TIMESCALE_90KHZ: u32 = 90000;     // MPEG-TS compatible
pub const TIMESCALE_48KHZ: u32 = 48000;     // Common audio
pub const TIMESCALE_44100HZ: u32 = 44100;   // CD audio
pub const TIMESCALE_1000MS: u32 = 1000;     // Millisecond precision

/// MP4 brand codes
pub const BRAND_ISOM: [u8; 4] = *b"isom";
pub const BRAND_ISO2: [u8; 4] = *b"iso2";
pub const BRAND_AVC1: [u8; 4] = *b"avc1";
pub const BRAND_HVC1: [u8; 4] = *b"hvc1";
pub const BRAND_AV01: [u8; 4] = *b"av01";
pub const BRAND_MP41: [u8; 4] = *b"mp41";
pub const BRAND_DASH: [u8; 4] = *b"dash";

/// MKV/WebM DocType strings
pub const DOCTYPE_MATROSKA: &[u8] = b"matroska";
pub const DOCTYPE_WEBM: &[u8] = b"webm";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_codec() {
        assert_eq!(VideoCodec::H264 as u8, 0);
        assert_eq!(VideoCodec::Av1 as u8, 3);
    }

    #[test]
    fn test_audio_codec() {
        assert_eq!(AudioCodec::Aac as u8, 0);
        assert_eq!(AudioCodec::Opus as u8, 1);
    }

    #[test]
    fn test_track_type() {
        assert_eq!(TrackType::Video as u8, 1);
        assert_eq!(TrackType::Audio as u8, 2);
    }

    #[test]
    fn test_sample_flags_default() {
        let flags = SampleFlags::default();
        assert!(!flags.is_keyframe);
        assert!(!flags.depends_on_others);
    }

    #[test]
    fn test_timescales() {
        assert_eq!(TIMESCALE_90KHZ, 90000);
        assert_eq!(TIMESCALE_48KHZ, 48000);
    }

    #[test]
    fn test_brands() {
        assert_eq!(&BRAND_ISOM, b"isom");
        assert_eq!(&BRAND_AV01, b"av01");
    }
}
