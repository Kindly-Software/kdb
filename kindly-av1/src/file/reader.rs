//! Frame reader abstraction and implementations
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Provides a unified FrameReader trait for reading video frames from
//! different source formats. Supports:
//!
//! - Raw YUV files (requires explicit dimensions)
//! - Y4M files (self-describing header)
//! - Container formats via NativeReaderCapsule (MP4, MKV, WebM)
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T5 Streaming tier (lazy frame loading)
//! - **Chaos**: Zero-copy where possible, mmap for direct formats
//! - **ASSUM**: All unsafe documented

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::file::error::FileError;
use crate::file::format::{InputFormat, PixelFormat, VideoInfo};

/// YUV frame data with planar layout
#[derive(Debug, Clone)]
pub struct Frame {
    /// Y (luma) plane data
    pub y: Vec<u8>,
    /// U (Cb chroma) plane data
    pub u: Vec<u8>,
    /// V (Cr chroma) plane data
    pub v: Vec<u8>,
    /// Frame number (0-indexed)
    pub frame_num: u64,
    /// Frame width
    pub width: u32,
    /// Frame height
    pub height: u32,
}

impl Frame {
    /// Create a new frame with allocated but uninitialized planes
    pub fn new_uninit(width: u32, height: u32, pixel_format: PixelFormat, frame_num: u64) -> Self {
        let (h_sub, v_sub) = pixel_format.chroma_subsampling();
        let bytes_per = if pixel_format.bit_depth() > 8 { 2 } else { 1 };

        let y_size = (width * height) as usize * bytes_per;
        let chroma_width = ((width as usize) + (h_sub as usize) - 1) / (h_sub as usize);
        let chroma_height = ((height as usize) + (v_sub as usize) - 1) / (v_sub as usize);
        let uv_size = chroma_width * chroma_height * bytes_per;

        Self {
            y: vec![0u8; y_size],
            u: vec![0u8; uv_size],
            v: vec![0u8; uv_size],
            frame_num,
            width,
            height,
        }
    }

    /// Get total frame size in bytes
    pub fn size(&self) -> usize {
        self.y.len() + self.u.len() + self.v.len()
    }
}

/// Frame reader trait (T5 Streaming pattern)
///
/// Provides a unified interface for reading video frames from different
/// source formats. Implementations should be lazy (read frames on demand)
/// and support seeking for resume capability.
pub trait FrameReader: Send {
    /// Read the next frame
    ///
    /// # Returns
    ///
    /// * `Ok(Some(frame))` - Next frame was read successfully
    /// * `Ok(None)` - End of stream reached
    /// * `Err(e)` - Error reading frame
    fn read_frame(&mut self) -> Result<Option<Frame>, FileError>;

    /// Get video information
    fn info(&self) -> &VideoInfo;

    /// Seek to a specific frame number
    ///
    /// # Arguments
    ///
    /// * `frame` - Target frame number (0-indexed)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Seek succeeded
    /// * `Err(e)` - Seek failed
    fn seek(&mut self, frame: u64) -> Result<(), FileError>;

    /// Get current frame number (0-indexed)
    fn current_frame(&self) -> u64;

    /// Check if more frames are available
    fn has_more_frames(&self) -> bool {
        self.current_frame() < self.info().frame_count
    }

    /// Get remaining frame count
    fn remaining_frames(&self) -> u64 {
        self.info().frame_count.saturating_sub(self.current_frame())
    }
}

/// Y4M (YUV4MPEG2) file reader
///
/// Reads Y4M files which include a header with video information.
/// Supports YUV420p, YUV422p, and YUV444p color spaces.
pub struct Y4mReader {
    reader: BufReader<File>,
    info: VideoInfo,
    current_frame: u64,
    header_size: u64,
    frame_header_size: usize,
}

impl Y4mReader {
    /// Open a Y4M file for reading
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the Y4M file
    ///
    /// # Returns
    ///
    /// * `Ok(reader)` - Successfully opened and parsed Y4M header
    /// * `Err(e)` - Failed to open or parse
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, FileError> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FileError::NotFound(path.to_path_buf())
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                FileError::PermissionDenied(path.to_path_buf())
            } else {
                FileError::Io(e)
            }
        })?;

        let file_size = file.metadata()?.len();
        let mut reader = BufReader::new(file);

        // Parse Y4M header
        let mut header = String::new();
        let header_size = reader.read_line(&mut header)? as u64;

        let info = Self::parse_header(&header, path, file_size, header_size)?;

        // Frame header is "FRAME\n" = 6 bytes (may have parameters after FRAME)
        let frame_header_size = 6;

        Ok(Self {
            reader,
            info,
            current_frame: 0,
            header_size,
            frame_header_size,
        })
    }

    /// Parse Y4M header line
    fn parse_header(header: &str, path: &Path, file_size: u64, header_size: u64) -> Result<VideoInfo, FileError> {
        let header = header.trim();

        // Must start with "YUV4MPEG2"
        if !header.starts_with("YUV4MPEG2") {
            return Err(FileError::InvalidY4mHeader {
                path: path.to_path_buf(),
                details: "Header must start with YUV4MPEG2".to_string(),
            });
        }

        let mut width: Option<u32> = None;
        let mut height: Option<u32> = None;
        let mut frame_rate = 30.0_f64;
        let mut pixel_format = PixelFormat::Yuv420p;

        // Parse parameters (space-separated, single-char prefix)
        for param in header.split_whitespace().skip(1) {
            if param.is_empty() {
                continue;
            }

            let (prefix, value) = param.split_at(1);
            match prefix {
                "W" => {
                    width = value.parse().ok();
                }
                "H" => {
                    height = value.parse().ok();
                }
                "F" => {
                    // Frame rate as N:D (numerator:denominator)
                    if let Some((num, den)) = value.split_once(':') {
                        if let (Ok(n), Ok(d)) = (num.parse::<f64>(), den.parse::<f64>()) {
                            if d > 0.0 {
                                frame_rate = n / d;
                            }
                        }
                    }
                }
                "C" => {
                    // Color space
                    pixel_format = match value {
                        "420" | "420jpeg" | "420mpeg2" | "420paldv" => PixelFormat::Yuv420p,
                        "422" => PixelFormat::Yuv422p,
                        "444" => PixelFormat::Yuv444p,
                        "420p10" => PixelFormat::Yuv420p10le,
                        "422p10" => PixelFormat::Yuv422p10le,
                        "444p10" => PixelFormat::Yuv444p10le,
                        _ => PixelFormat::Yuv420p, // Default
                    };
                }
                _ => {} // Ignore unknown parameters (I, A, X, etc.)
            }
        }

        let width = width.ok_or_else(|| FileError::InvalidY4mHeader {
            path: path.to_path_buf(),
            details: "Width (W) not specified".to_string(),
        })?;

        let height = height.ok_or_else(|| FileError::InvalidY4mHeader {
            path: path.to_path_buf(),
            details: "Height (H) not specified".to_string(),
        })?;

        // Calculate frame count from file size
        // Frame size = header(6) + Y + U + V planes
        let frame_payload = pixel_format.frame_size(width, height) as u64;
        let frame_total = 6 + frame_payload; // "FRAME\n" + payload
        let data_size = file_size - header_size;
        let frame_count = data_size / frame_total;
        let duration_secs = frame_count as f64 / frame_rate;

        let mut info = VideoInfo::new(width, height, frame_rate);
        info.pixel_format = pixel_format;
        info.frame_count = frame_count;
        info.duration_secs = duration_secs;

        info.validate()?;

        Ok(info)
    }
}

impl FrameReader for Y4mReader {
    fn read_frame(&mut self) -> Result<Option<Frame>, FileError> {
        // Read frame header
        let mut frame_header = vec![0u8; self.frame_header_size];

        match self.reader.read_exact(&mut frame_header) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(None);
            }
            Err(e) => return Err(FileError::Io(e)),
        }

        // Verify "FRAME" magic (may have parameters, ends with \n)
        if !frame_header.starts_with(b"FRAME") {
            return Err(FileError::InvalidY4mHeader {
                path: std::path::PathBuf::from("<y4m>"),
                details: format!("Expected FRAME header, got {:?}", &frame_header[..5]),
            });
        }

        // If frame header has parameters (FRAME Xparam...\n), read until newline
        if frame_header[5] != b'\n' {
            // Read until newline
            let mut extra = Vec::new();
            loop {
                let mut byte = [0u8; 1];
                self.reader.read_exact(&mut byte)?;
                if byte[0] == b'\n' {
                    break;
                }
                extra.push(byte[0]);
            }
        }

        // Read frame data
        let mut frame = Frame::new_uninit(
            self.info.width,
            self.info.height,
            self.info.pixel_format,
            self.current_frame,
        );

        self.reader.read_exact(&mut frame.y)?;
        self.reader.read_exact(&mut frame.u)?;
        self.reader.read_exact(&mut frame.v)?;

        self.current_frame += 1;
        Ok(Some(frame))
    }

    fn info(&self) -> &VideoInfo {
        &self.info
    }

    fn seek(&mut self, frame: u64) -> Result<(), FileError> {
        if frame >= self.info.frame_count {
            return Err(FileError::SeekError {
                frame,
                reason: format!("Frame {} exceeds total frames {}", frame, self.info.frame_count),
            });
        }

        // Calculate offset: header + (frame_header + frame_data) * frame
        let frame_size = self.frame_header_size as u64 +
                         self.info.pixel_format.frame_size(self.info.width, self.info.height) as u64;
        let offset = self.header_size + frame_size * frame;

        self.reader.seek(SeekFrom::Start(offset))?;
        self.current_frame = frame;

        Ok(())
    }

    fn current_frame(&self) -> u64 {
        self.current_frame
    }
}

/// Raw YUV file reader
///
/// Reads raw YUV files without headers. Requires explicit dimensions.
pub struct RawYuvReader {
    reader: BufReader<File>,
    info: VideoInfo,
    current_frame: u64,
}

impl RawYuvReader {
    /// Open a raw YUV file with explicit dimensions
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the YUV file
    /// * `width` - Frame width in pixels
    /// * `height` - Frame height in pixels
    /// * `pixel_format` - Pixel format (default: YUV420p)
    /// * `frame_rate` - Frame rate (default: 30.0)
    pub fn open<P: AsRef<Path>>(
        path: P,
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        frame_rate: f64,
    ) -> Result<Self, FileError> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FileError::NotFound(path.to_path_buf())
            } else {
                FileError::Io(e)
            }
        })?;

        let file_size = file.metadata()?.len();
        let frame_size = pixel_format.frame_size(width, height) as u64;
        let frame_count = file_size / frame_size;
        let duration_secs = frame_count as f64 / frame_rate;

        let mut info = VideoInfo::new(width, height, frame_rate);
        info.pixel_format = pixel_format;
        info.frame_count = frame_count;
        info.duration_secs = duration_secs;

        info.validate()?;

        Ok(Self {
            reader: BufReader::new(file),
            info,
            current_frame: 0,
        })
    }
}

impl FrameReader for RawYuvReader {
    fn read_frame(&mut self) -> Result<Option<Frame>, FileError> {
        let mut frame = Frame::new_uninit(
            self.info.width,
            self.info.height,
            self.info.pixel_format,
            self.current_frame,
        );

        match self.reader.read_exact(&mut frame.y) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(None);
            }
            Err(e) => return Err(FileError::Io(e)),
        }

        self.reader.read_exact(&mut frame.u)?;
        self.reader.read_exact(&mut frame.v)?;

        self.current_frame += 1;
        Ok(Some(frame))
    }

    fn info(&self) -> &VideoInfo {
        &self.info
    }

    fn seek(&mut self, frame: u64) -> Result<(), FileError> {
        if frame >= self.info.frame_count {
            return Err(FileError::SeekError {
                frame,
                reason: format!("Frame {} exceeds total frames {}", frame, self.info.frame_count),
            });
        }

        let frame_size = self.info.pixel_format.frame_size(self.info.width, self.info.height) as u64;
        let offset = frame_size * frame;

        self.reader.seek(SeekFrom::Start(offset))?;
        self.current_frame = frame;

        Ok(())
    }

    fn current_frame(&self) -> u64 {
        self.current_frame
    }
}

/// Create an appropriate reader for the given format
///
/// # Arguments
///
/// * `path` - Path to the video file
/// * `format` - Detected input format
/// * `raw_config` - Optional configuration for raw YUV files (width, height, format, fps)
///
/// # Returns
///
/// * `Ok(reader)` - Appropriate reader for the format
/// * `Err(e)` - Failed to create reader
///
/// ## Zero-Dependency Native Architecture (GPL-Free)
///
/// kindly-av1 uses 100% native Rust demuxers and decoders. No FFmpeg dependency.
/// This ensures:
///
/// - **Proprietary license** - No GPL/LGPL contamination
/// - **Zero external dependencies** - Pure Rust stack
/// - **Faster startup** - No process spawning
/// - **Better error messages** - Container-specific validation
/// - **Full control** - No external binary versioning issues
///
/// ## Supported Formats
///
/// - **Native demux+decode**: MP4, MKV, WebM (H.264, VP9, AV1 codecs)
/// - **Direct read**: Y4M, Raw YUV
/// - **Not yet supported**: AVI, MOV (planned for future release)
pub fn create_reader<P: AsRef<Path>>(
    path: P,
    format: InputFormat,
    raw_config: Option<(u32, u32, PixelFormat, f64)>,
) -> Result<Box<dyn FrameReader>, FileError> {
    let path = path.as_ref();

    match format {
        InputFormat::RawYuv => {
            let (width, height, pixel_format, frame_rate) = raw_config.ok_or_else(|| {
                FileError::RequiresDimensions {
                    path: path.to_path_buf(),
                    message: "Use --width and --height for raw YUV files",
                }
            })?;
            Ok(Box::new(RawYuvReader::open(path, width, height, pixel_format, frame_rate)?))
        }
        InputFormat::Y4m => {
            Ok(Box::new(Y4mReader::open(path)?))
        }

        // Native reader ONLY - zero FFmpeg dependency (GPL-free)
        // MOV files use the same ISO BMFF structure as MP4, handled by Mp4DemuxerCapsule
        InputFormat::Mp4 | InputFormat::Mov | InputFormat::Mkv | InputFormat::WebM => {
            use crate::file::native_reader::NativeReaderCapsule;
            Ok(Box::new(NativeReaderCapsule::open(path)?))
        }

        // AVI not yet supported natively (planned for future release)
        InputFormat::Avi => {
            Err(FileError::FormatNotYetSupported {
                path: path.to_path_buf(),
                format: "AVI".to_string(),
                reason: "AVI native demuxer not yet implemented. Use MP4, MKV, or WebM containers, or convert with: ffmpeg -i input.avi -c copy output.mp4".to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_new_uninit_420p() {
        let frame = Frame::new_uninit(1920, 1080, PixelFormat::Yuv420p, 0);
        assert_eq!(frame.y.len(), 1920 * 1080);
        assert_eq!(frame.u.len(), 960 * 540);
        assert_eq!(frame.v.len(), 960 * 540);
        assert_eq!(frame.frame_num, 0);
    }

    #[test]
    fn test_frame_new_uninit_444p() {
        let frame = Frame::new_uninit(1920, 1080, PixelFormat::Yuv444p, 42);
        assert_eq!(frame.y.len(), 1920 * 1080);
        assert_eq!(frame.u.len(), 1920 * 1080);
        assert_eq!(frame.v.len(), 1920 * 1080);
        assert_eq!(frame.frame_num, 42);
    }

    #[test]
    fn test_frame_size() {
        let frame = Frame::new_uninit(1920, 1080, PixelFormat::Yuv420p, 0);
        // Y + U + V = 1920*1080 + 960*540 + 960*540 = 3,110,400
        assert_eq!(frame.size(), 3110400);
    }

    #[test]
    fn test_create_reader_raw_requires_dimensions() {
        let result = create_reader("/nonexistent.yuv", InputFormat::RawYuv, None);
        assert!(matches!(result, Err(FileError::RequiresDimensions { .. })));
    }
}
