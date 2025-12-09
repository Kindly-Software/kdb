//! File I/O error types for kindly-av1
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Comprehensive error handling for file operations, format detection,
//! and native demuxer/decoder integration.
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T5 Streaming tier error handling
//! - **Chaos**: Rich error context for debugging
//! - **ASSUM**: Error paths documented

use std::io;
use std::path::PathBuf;

/// File I/O errors
#[derive(Debug)]
pub enum FileError {
    /// File not found at specified path
    NotFound(PathBuf),

    /// Permission denied accessing file
    PermissionDenied(PathBuf),

    /// Unknown or unsupported format
    UnsupportedFormat {
        path: PathBuf,
        extension: Option<String>,
    },

    /// Format not yet implemented (native demuxer in progress)
    FormatNotYetSupported {
        path: PathBuf,
        format: String,
        reason: String,
    },

    /// Raw YUV requires explicit dimensions
    RequiresDimensions {
        path: PathBuf,
        message: &'static str,
    },

    /// Invalid Y4M header
    InvalidY4mHeader {
        path: PathBuf,
        details: String,
    },

    /// Video stream not found in container
    NoVideoStream(PathBuf),

    /// Unsupported pixel format
    UnsupportedPixelFormat {
        format: String,
        supported: &'static [&'static str],
    },

    /// Frame read error
    FrameReadError {
        frame_num: u64,
        expected_bytes: usize,
        actual_bytes: usize,
    },

    /// Seek error
    SeekError {
        frame: u64,
        reason: String,
    },

    /// End of stream reached
    EndOfStream {
        frames_read: u64,
    },

    /// IO error wrapper
    Io(io::Error),

    /// Resolution exceeds maximum (8K)
    ResolutionTooLarge {
        width: u32,
        height: u32,
        max_width: u32,
        max_height: u32,
    },

    /// Invalid video dimensions
    InvalidDimensions {
        width: u32,
        height: u32,
        reason: &'static str,
    },
}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(path) => {
                write!(f, "File not found: {}", path.display())
            }
            Self::PermissionDenied(path) => {
                write!(f, "Permission denied: {}", path.display())
            }
            Self::UnsupportedFormat { path, extension } => {
                if let Some(ext) = extension {
                    write!(f, "Unsupported format '.{}': {}", ext, path.display())
                } else {
                    write!(f, "Unknown format (no extension): {}", path.display())
                }
            }
            Self::FormatNotYetSupported { path, format, reason } => {
                write!(f, "Format '{}' not yet supported for {}: {}", format, path.display(), reason)
            }
            Self::RequiresDimensions { path, message } => {
                write!(f, "Raw YUV file requires dimensions: {} ({})", path.display(), message)
            }
            Self::InvalidY4mHeader { path, details } => {
                write!(f, "Invalid Y4M header in {}: {}", path.display(), details)
            }
            Self::NoVideoStream(path) => {
                write!(f, "No video stream found in {}", path.display())
            }
            Self::UnsupportedPixelFormat { format, supported } => {
                write!(f, "Unsupported pixel format '{}'. Supported: {:?}", format, supported)
            }
            Self::FrameReadError { frame_num, expected_bytes, actual_bytes } => {
                write!(f, "Frame {} read error: expected {} bytes, got {}",
                       frame_num, expected_bytes, actual_bytes)
            }
            Self::SeekError { frame, reason } => {
                write!(f, "Failed to seek to frame {}: {}", frame, reason)
            }
            Self::EndOfStream { frames_read } => {
                write!(f, "End of stream reached after {} frames", frames_read)
            }
            Self::Io(e) => {
                write!(f, "IO error: {}", e)
            }
            Self::ResolutionTooLarge { width, height, max_width, max_height } => {
                write!(f, "Resolution {}x{} exceeds maximum {}x{}",
                       width, height, max_width, max_height)
            }
            Self::InvalidDimensions { width, height, reason } => {
                write!(f, "Invalid dimensions {}x{}: {}", width, height, reason)
            }
        }
    }
}

impl std::error::Error for FileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for FileError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// Supported pixel formats for conversion
pub const SUPPORTED_PIXEL_FORMATS: &[&str] = &[
    "yuv420p",
    "yuv422p",
    "yuv444p",
    "yuv420p10le",
    "yuv422p10le",
    "yuv444p10le",
    "nv12",
    "nv21",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = FileError::NotFound(PathBuf::from("/test/video.mp4"));
        assert!(err.to_string().contains("File not found"));

        let err = FileError::UnsupportedFormat {
            path: PathBuf::from("/test/video.xyz"),
            extension: Some("xyz".to_string()),
        };
        assert!(err.to_string().contains("Unsupported format"));

        let err = FileError::NoVideoStream(PathBuf::from("/test/video.mp4"));
        assert!(err.to_string().contains("No video stream found"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
        let file_err: FileError = io_err.into();
        assert!(matches!(file_err, FileError::Io(_)));
    }

    #[test]
    fn test_supported_pixel_formats() {
        assert!(SUPPORTED_PIXEL_FORMATS.contains(&"yuv420p"));
        assert!(SUPPORTED_PIXEL_FORMATS.contains(&"yuv420p10le"));
    }
}
