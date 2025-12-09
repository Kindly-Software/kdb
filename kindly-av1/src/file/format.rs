//! Format detection and video information
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Auto-detection of video formats from file extension and content probing.
//!
//! ## Supported Formats
//!
//! - **Direct**: YUV (raw), Y4M (self-describing)
//! - **Native demuxers**: MP4, MKV, WebM (H.264, VP9, AV1)
//! - **Planned**: MOV, AVI
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T5 Streaming tier
//! - **Chaos**: Zero-copy header parsing
//! - **ASSUM**: Format detection is safe (no unsafe)

use std::path::Path;
use crate::file::error::FileError;

/// Supported input formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputFormat {
    /// Raw YUV file (.yuv) - requires explicit width/height
    RawYuv,
    /// YUV4MPEG2 file (.y4m) - self-describing with header
    Y4m,
    /// MPEG-4 container (.mp4, .m4v) - native demuxer
    Mp4,
    /// Matroska container (.mkv) - native demuxer
    Mkv,
    /// WebM container (.webm) - native demuxer
    WebM,
    /// QuickTime container (.mov) - planned
    Mov,
    /// Audio Video Interleave (.avi) - planned
    Avi,
}

impl InputFormat {
    /// Check if format is self-describing (has header with metadata)
    #[inline]
    pub const fn is_self_describing(&self) -> bool {
        !matches!(self, Self::RawYuv)
    }

    /// Get common file extensions for this format
    pub const fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::RawYuv => &["yuv"],
            Self::Y4m => &["y4m"],
            Self::Mp4 => &["mp4", "m4v"],
            Self::Mkv => &["mkv"],
            Self::WebM => &["webm"],
            Self::Mov => &["mov", "qt"],
            Self::Avi => &["avi"],
        }
    }

    /// Get MIME type for the format
    pub const fn mime_type(&self) -> &'static str {
        match self {
            Self::RawYuv => "video/raw",
            Self::Y4m => "video/x-yuv4mpeg",
            Self::Mp4 => "video/mp4",
            Self::Mkv => "video/x-matroska",
            Self::WebM => "video/webm",
            Self::Mov => "video/quicktime",
            Self::Avi => "video/x-msvideo",
        }
    }

    /// Get display name for the format
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::RawYuv => "Raw YUV",
            Self::Y4m => "YUV4MPEG2",
            Self::Mp4 => "MPEG-4",
            Self::Mkv => "Matroska",
            Self::WebM => "WebM",
            Self::Mov => "QuickTime",
            Self::Avi => "AVI",
        }
    }
}

impl std::fmt::Display for InputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Detect input format from file extension
///
/// # Arguments
///
/// * `path` - Path to the video file
///
/// # Returns
///
/// * `Some(InputFormat)` if extension is recognized
/// * `None` if extension is unknown or missing
pub fn detect_format<P: AsRef<Path>>(path: P) -> Option<InputFormat> {
    let path = path.as_ref();
    let ext = path.extension()?.to_str()?.to_lowercase();

    match ext.as_str() {
        "yuv" => Some(InputFormat::RawYuv),
        "y4m" => Some(InputFormat::Y4m),
        "mp4" | "m4v" => Some(InputFormat::Mp4),
        "mkv" => Some(InputFormat::Mkv),
        "webm" => Some(InputFormat::WebM),
        "mov" | "qt" => Some(InputFormat::Mov),
        "avi" => Some(InputFormat::Avi),
        _ => None,
    }
}

/// Supported pixel formats for encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// YUV 4:2:0 planar, 8-bit
    Yuv420p,
    /// YUV 4:2:2 planar, 8-bit
    Yuv422p,
    /// YUV 4:4:4 planar, 8-bit
    Yuv444p,
    /// YUV 4:2:0 planar, 10-bit little-endian
    Yuv420p10le,
    /// YUV 4:2:2 planar, 10-bit little-endian
    Yuv422p10le,
    /// YUV 4:4:4 planar, 10-bit little-endian
    Yuv444p10le,
    /// NV12 (Y plane + interleaved UV)
    Nv12,
}

impl PixelFormat {
    /// Get standard pixel format name
    pub const fn standard_name(&self) -> &'static str {
        match self {
            Self::Yuv420p => "yuv420p",
            Self::Yuv422p => "yuv422p",
            Self::Yuv444p => "yuv444p",
            Self::Yuv420p10le => "yuv420p10le",
            Self::Yuv422p10le => "yuv422p10le",
            Self::Yuv444p10le => "yuv444p10le",
            Self::Nv12 => "nv12",
        }
    }

    /// Get bits per component
    pub const fn bit_depth(&self) -> u8 {
        match self {
            Self::Yuv420p | Self::Yuv422p | Self::Yuv444p | Self::Nv12 => 8,
            Self::Yuv420p10le | Self::Yuv422p10le | Self::Yuv444p10le => 10,
        }
    }

    /// Get chroma subsampling (horizontal, vertical)
    pub const fn chroma_subsampling(&self) -> (u8, u8) {
        match self {
            Self::Yuv420p | Self::Yuv420p10le | Self::Nv12 => (2, 2),
            Self::Yuv422p | Self::Yuv422p10le => (2, 1),
            Self::Yuv444p | Self::Yuv444p10le => (1, 1),
        }
    }

    /// Calculate frame size in bytes
    pub const fn frame_size(&self, width: u32, height: u32) -> usize {
        let (h_sub, v_sub) = self.chroma_subsampling();
        let bytes_per_component = if self.bit_depth() > 8 { 2 } else { 1 };

        let y_size = (width * height) as usize * bytes_per_component;
        let chroma_width = (width as usize + h_sub as usize - 1) / h_sub as usize;
        let chroma_height = (height as usize + v_sub as usize - 1) / v_sub as usize;
        let uv_size = chroma_width * chroma_height * bytes_per_component;

        y_size + 2 * uv_size
    }

    /// Parse from standard pixel format string (e.g., from demuxer metadata)
    pub fn from_standard_name(name: &str) -> Option<Self> {
        match name {
            "yuv420p" => Some(Self::Yuv420p),
            "yuv422p" => Some(Self::Yuv422p),
            "yuv444p" => Some(Self::Yuv444p),
            "yuv420p10le" | "yuv420p10" => Some(Self::Yuv420p10le),
            "yuv422p10le" | "yuv422p10" => Some(Self::Yuv422p10le),
            "yuv444p10le" | "yuv444p10" => Some(Self::Yuv444p10le),
            "nv12" => Some(Self::Nv12),
            _ => None,
        }
    }
}

impl Default for PixelFormat {
    fn default() -> Self {
        Self::Yuv420p
    }
}

impl std::fmt::Display for PixelFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.standard_name())
    }
}

/// Color space specification (re-exported from metadata module)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ColorSpace {
    /// BT.709 (HD standard)
    #[default]
    Bt709 = 0,
    /// BT.601 (SD standard)
    Bt601 = 1,
    /// BT.2020 (UHD/HDR standard)
    Bt2020 = 2,
    /// sRGB
    Srgb = 3,
    /// Unknown/unspecified
    Unspecified = 255,
}

impl ColorSpace {
    /// Get standard name
    pub const fn standard_name(&self) -> &'static str {
        match self {
            ColorSpace::Bt709 => "BT.709",
            ColorSpace::Bt601 => "BT.601",
            ColorSpace::Bt2020 => "BT.2020",
            ColorSpace::Srgb => "sRGB",
            ColorSpace::Unspecified => "Unspecified",
        }
    }
}

impl std::fmt::Display for ColorSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.standard_name())
    }
}

/// Video file information
#[derive(Debug, Clone)]
pub struct VideoInfo {
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Total frame count (may be estimated for some formats)
    pub frame_count: u64,
    /// Frame rate (frames per second)
    pub frame_rate: f64,
    /// Pixel format
    pub pixel_format: PixelFormat,
    /// Color space
    pub color_space: ColorSpace,
    /// Duration in seconds
    pub duration_secs: f64,
    /// Bit rate in bits per second (if available)
    pub bit_rate: Option<u64>,
    /// Codec name (for container formats)
    pub codec: Option<String>,
}

impl VideoInfo {
    /// Create VideoInfo with basic parameters
    pub fn new(width: u32, height: u32, frame_rate: f64) -> Self {
        Self {
            width,
            height,
            frame_count: 0,
            frame_rate,
            pixel_format: PixelFormat::default(),
            color_space: ColorSpace::default(),
            duration_secs: 0.0,
            bit_rate: None,
            codec: None,
        }
    }

    /// Calculate frame size in bytes
    pub fn frame_size(&self) -> usize {
        self.pixel_format.frame_size(self.width, self.height)
    }

    /// Calculate total file size estimate for raw data
    pub fn raw_size_estimate(&self) -> u64 {
        self.frame_size() as u64 * self.frame_count
    }

    /// Validate dimensions against maximum limits
    pub fn validate(&self) -> Result<(), FileError> {
        // Check maximum resolution (8K)
        if self.width > crate::MAX_WIDTH || self.height > crate::MAX_HEIGHT {
            return Err(FileError::ResolutionTooLarge {
                width: self.width,
                height: self.height,
                max_width: crate::MAX_WIDTH,
                max_height: crate::MAX_HEIGHT,
            });
        }

        // Check for zero dimensions
        if self.width == 0 || self.height == 0 {
            return Err(FileError::InvalidDimensions {
                width: self.width,
                height: self.height,
                reason: "dimensions cannot be zero",
            });
        }

        // Check for odd dimensions with 4:2:0 subsampling
        let (h_sub, v_sub) = self.pixel_format.chroma_subsampling();
        if h_sub > 1 && self.width % 2 != 0 {
            return Err(FileError::InvalidDimensions {
                width: self.width,
                height: self.height,
                reason: "width must be even for 4:2:0/4:2:2 pixel formats",
            });
        }
        if v_sub > 1 && self.height % 2 != 0 {
            return Err(FileError::InvalidDimensions {
                width: self.width,
                height: self.height,
                reason: "height must be even for 4:2:0 pixel format",
            });
        }

        Ok(())
    }
}

impl std::fmt::Display for VideoInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}x{} @ {:.3} fps, {} frames, {:.2}s, {}, {}",
            self.width,
            self.height,
            self.frame_rate,
            self.frame_count,
            self.duration_secs,
            self.pixel_format,
            self.color_space
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_format() {
        assert_eq!(detect_format("video.mp4"), Some(InputFormat::Mp4));
        assert_eq!(detect_format("video.MP4"), Some(InputFormat::Mp4));
        assert_eq!(detect_format("video.y4m"), Some(InputFormat::Y4m));
        assert_eq!(detect_format("video.yuv"), Some(InputFormat::RawYuv));
        assert_eq!(detect_format("video.mkv"), Some(InputFormat::Mkv));
        assert_eq!(detect_format("video.webm"), Some(InputFormat::WebM));
        assert_eq!(detect_format("video.mov"), Some(InputFormat::Mov));
        assert_eq!(detect_format("video.avi"), Some(InputFormat::Avi));
        assert_eq!(detect_format("video.xyz"), None);
        assert_eq!(detect_format("video"), None);
    }

    #[test]
    fn test_format_native_support() {
        // All formats are self-describing except raw YUV
        assert!(!InputFormat::RawYuv.is_self_describing());
        assert!(InputFormat::Y4m.is_self_describing());
        assert!(InputFormat::Mp4.is_self_describing());
        assert!(InputFormat::Mkv.is_self_describing());
        assert!(InputFormat::WebM.is_self_describing());
    }

    #[test]
    fn test_format_is_self_describing() {
        assert!(!InputFormat::RawYuv.is_self_describing());
        assert!(InputFormat::Y4m.is_self_describing());
        assert!(InputFormat::Mp4.is_self_describing());
    }

    #[test]
    fn test_pixel_format_frame_size() {
        // 1920x1080 YUV420p = 1920*1080 + 2*(960*540) = 2073600 + 1036800 = 3110400
        assert_eq!(PixelFormat::Yuv420p.frame_size(1920, 1080), 3110400);

        // 1920x1080 YUV444p = 1920*1080*3 = 6220800
        assert_eq!(PixelFormat::Yuv444p.frame_size(1920, 1080), 6220800);

        // 1920x1080 YUV420p10le = 3110400 * 2 = 6220800
        assert_eq!(PixelFormat::Yuv420p10le.frame_size(1920, 1080), 6220800);
    }

    #[test]
    fn test_pixel_format_from_standard_name() {
        assert_eq!(PixelFormat::from_standard_name("yuv420p"), Some(PixelFormat::Yuv420p));
        assert_eq!(PixelFormat::from_standard_name("yuv420p10le"), Some(PixelFormat::Yuv420p10le));
        assert_eq!(PixelFormat::from_standard_name("nv12"), Some(PixelFormat::Nv12));
        assert_eq!(PixelFormat::from_standard_name("rgb24"), None);
    }

    #[test]
    fn test_video_info_validation() {
        // Valid 1080p
        let info = VideoInfo::new(1920, 1080, 30.0);
        assert!(info.validate().is_ok());

        // Zero dimensions
        let info = VideoInfo::new(0, 1080, 30.0);
        assert!(info.validate().is_err());

        // Odd width with 4:2:0
        let mut info = VideoInfo::new(1921, 1080, 30.0);
        info.pixel_format = PixelFormat::Yuv420p;
        assert!(info.validate().is_err());

        // 8K+ (too large)
        let info = VideoInfo::new(8000, 5000, 30.0);
        assert!(info.validate().is_err());
    }

    #[test]
    fn test_video_info_display() {
        let info = VideoInfo {
            width: 1920,
            height: 1080,
            frame_count: 1800,
            frame_rate: 30.0,
            pixel_format: PixelFormat::Yuv420p,
            color_space: ColorSpace::Bt709,
            duration_secs: 60.0,
            bit_rate: None,
            codec: None,
        };
        let display = format!("{}", info);
        assert!(display.contains("1920x1080"));
        assert!(display.contains("30.000 fps"));
        assert!(display.contains("1800 frames"));
    }
}
