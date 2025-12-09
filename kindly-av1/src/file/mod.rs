//! File I/O Module for kindly-av1
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! This module provides comprehensive file I/O capabilities for video encoding:
//!
//! - **Auto-discovery**: Scan directories for video files
//! - **Format detection**: Detect format from extension (YUV, Y4M, MP4, MKV, WebM, MOV, AVI)
//! - **Unified reading**: FrameReader trait for all formats
//! - **FFmpeg integration**: Pipe-based decoding for container formats
//!
//! ## Supported Formats
//!
//! | Format | Extension | Direct Read | FFmpeg Required |
//! |--------|-----------|-------------|-----------------|
//! | Raw YUV | .yuv | Yes (needs dimensions) | No |
//! | YUV4MPEG2 | .y4m | Yes (self-describing) | No |
//! | MPEG-4 | .mp4, .m4v | No | Yes |
//! | Matroska | .mkv | No | Yes |
//! | WebM | .webm | No | Yes |
//! | QuickTime | .mov, .qt | No | Yes |
//! | AVI | .avi | No | Yes |
//!
//! ## Architecture
//!
//! ```text
//! +-----------+     +--------------+     +--------------+
//! | Discovery | --> | FormatDetect | --> | FrameReader  |
//! +-----------+     +--------------+     +--------------+
//!                          |                    |
//!                          v                    v
//!                   +-------------+      +------------------+
//!                   | Y4M/YUV     |      | NativeReader     |
//!                   | Direct Read |      | (MP4/MKV/WebM)   |
//!                   +-------------+      +------------------+
//! ```
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T5 Streaming tier (lazy frame reading)
//! - **Chaos**: 256B cache-aligned NativeReaderCapsule, atomic state, generation counters
//! - **ASSUM**: All unsafe documented with #ASSUME/#VERIFY tags
//! - **B32**: Benchmark-ready frame reading (zero-copy where possible)
//! - **T28**: Comprehensive tests for all formats and edge cases
//!
//! ## Quick Start
//!
//! ```no_run
//! use kindly_av1::file::{discover_videos, create_reader, detect_format};
//!
//! // Auto-discover videos in current directory
//! let videos = discover_videos(".");
//! for video in &videos {
//!     println!("Found: {} ({}, {})", video.filename, video.format, video.size_display());
//! }
//!
//! // Open and read frames
//! if let Some(video) = videos.first() {
//!     let mut reader = create_reader(&video.path, video.format, None)?;
//!     println!("Video info: {}", reader.info());
//!
//!     while let Some(frame) = reader.read_frame()? {
//!         println!("Frame {}: {} bytes", frame.frame_num, frame.size());
//!     }
//! }
//! # Ok::<(), kindly_av1::file::FileError>(())
//! ```

mod error;
mod format;
mod discovery;
mod reader;
mod native_reader;
mod yuv_frame;
mod metadata;

// Re-export public types from error module
pub use error::FileError;
pub use error::SUPPORTED_PIXEL_FORMATS;

// Re-export public types from format module
pub use format::{
    InputFormat,
    PixelFormat,
    VideoInfo,
    ColorSpace,
    detect_format,
};

// Re-export public types from metadata module
pub use metadata::{
    VideoMetadataCapsule,
    MetadataSnapshot,
    parse_y4m_header,
    parse_mp4_metadata,
    parse_mkv_metadata,
};

// Re-export public types from discovery module
pub use discovery::{
    DiscoveredFile,
    DiscoveryOptions,
    DiscoverySummary,
    SortOrder,
    discover_videos,
    discover_videos_with_options,
    discover_in_binary_dir,
    discover_in_current_dir,
};

// Re-export public types from reader module
pub use reader::{
    Frame,
    FrameReader,
    Y4mReader,
    RawYuvReader,
    create_reader,
};

// Re-export public types from native_reader module
pub use native_reader::NativeReaderCapsule;

// Re-export public types from yuv_frame module
pub use yuv_frame::YuvFrameCapsule;

/// Check system readiness for video file processing
///
/// Returns a summary of available capabilities:
/// - Native demuxers support MP4, MKV, WebM (H.264, VP9, AV1)
/// - Direct formats always supported (YUV, Y4M)
pub fn check_system_capabilities() -> SystemCapabilities {
    SystemCapabilities {
        direct_formats_supported: true,
        native_demuxers_supported: true,
    }
}

/// System capabilities for video processing
#[derive(Debug, Clone)]
pub struct SystemCapabilities {
    /// Direct format reading supported (always true)
    pub direct_formats_supported: bool,
    /// Native demuxers supported (MP4, MKV, WebM with H.264/VP9/AV1)
    pub native_demuxers_supported: bool,
}

impl SystemCapabilities {
    /// Get list of supported formats
    pub fn supported_formats(&self) -> Vec<InputFormat> {
        let mut formats = vec![
            InputFormat::RawYuv,
            InputFormat::Y4m,
            InputFormat::Mp4,
            InputFormat::Mkv,
            InputFormat::WebM,
        ];
        formats
    }
}

impl std::fmt::Display for SystemCapabilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "System Capabilities:")?;
        writeln!(f, "  Direct formats: supported (YUV, Y4M)")?;
        writeln!(f, "  Native demuxers: supported (MP4, MKV, WebM)")?;
        writeln!(f, "  Codecs: H.264, VP9, AV1")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all public types are accessible
        let _: Option<FileError> = None;
        let _: Option<InputFormat> = None;
        let _: Option<PixelFormat> = None;
        let _: Option<VideoInfo> = None;
        let _: Option<DiscoveredFile> = None;
        let _: Option<Frame> = None;
    }

    #[test]
    fn test_detect_format_integration() {
        assert_eq!(detect_format("video.mp4"), Some(InputFormat::Mp4));
        assert_eq!(detect_format("video.y4m"), Some(InputFormat::Y4m));
        assert_eq!(detect_format("video.yuv"), Some(InputFormat::RawYuv));
    }

    #[test]
    fn test_system_capabilities() {
        let caps = check_system_capabilities();

        // Direct formats always supported
        assert!(caps.direct_formats_supported);

        // Supported formats includes at least direct formats
        let formats = caps.supported_formats();
        assert!(formats.contains(&InputFormat::RawYuv));
        assert!(formats.contains(&InputFormat::Y4m));
    }

    #[test]
    fn test_system_capabilities_display() {
        let caps = check_system_capabilities();
        let display = format!("{}", caps);
        assert!(display.contains("System Capabilities"));
        assert!(display.contains("Direct formats"));
    }

    #[test]
    fn test_discover_empty_dir() {
        // Create a temporary directory (or use one that exists but has no videos)
        let videos = discover_videos("/tmp");
        // Should not panic, may or may not find videos
        let _ = videos;
    }

    #[test]
    fn test_pixel_format_frame_size_via_module() {
        let size = PixelFormat::Yuv420p.frame_size(1920, 1080);
        assert_eq!(size, 3110400);
    }
}
