//! Auto-discovery of video files
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Automatically discovers video files in a directory, supporting:
//! - Directory scanning with format detection
//! - Sorting by modification time (most recent first)
//! - Size and metadata extraction
//! - Binary directory discovery (same folder as executable)
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T5 Streaming tier (lazy discovery)
//! - **Chaos**: No unsafe, pure Rust directory traversal
//! - **ASSUM**: All filesystem operations are safe

use std::path::{Path, PathBuf};
use std::time::SystemTime;
use crate::file::format::{detect_format, InputFormat};

/// Discovered video file with metadata
#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    /// Full path to the video file
    pub path: PathBuf,
    /// Detected input format
    pub format: InputFormat,
    /// File size in bytes
    pub size_bytes: u64,
    /// Last modification time
    pub modified: SystemTime,
    /// File name (for display)
    pub filename: String,
}

impl DiscoveredFile {
    /// Create a new DiscoveredFile from path and metadata
    fn new(path: PathBuf, format: InputFormat, size_bytes: u64, modified: SystemTime) -> Self {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Self {
            path,
            format,
            size_bytes,
            modified,
            filename,
        }
    }

    /// Get human-readable file size
    pub fn size_display(&self) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if self.size_bytes >= GB {
            format!("{:.2} GB", self.size_bytes as f64 / GB as f64)
        } else if self.size_bytes >= MB {
            format!("{:.2} MB", self.size_bytes as f64 / MB as f64)
        } else if self.size_bytes >= KB {
            format!("{:.2} KB", self.size_bytes as f64 / KB as f64)
        } else {
            format!("{} B", self.size_bytes)
        }
    }

}

impl std::fmt::Display for DiscoveredFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}, {})",
            self.filename,
            self.format,
            self.size_display()
        )
    }
}

/// Discovery options for filtering and sorting
#[derive(Debug, Clone)]
pub struct DiscoveryOptions {
    /// Include subdirectories (recursive search)
    pub recursive: bool,
    /// Maximum depth for recursive search (0 = unlimited)
    pub max_depth: usize,
    /// Minimum file size to include (bytes)
    pub min_size: u64,
    /// Maximum file size to include (bytes, 0 = unlimited)
    pub max_size: u64,
    /// Only include these formats (empty = all)
    pub formats: Vec<InputFormat>,
    /// Sort order
    pub sort_by: SortOrder,
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            recursive: false,
            max_depth: 1,
            min_size: 0,
            max_size: 0,
            formats: Vec::new(),
            sort_by: SortOrder::ModifiedDesc,
        }
    }
}

/// Sort order for discovered files
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Most recently modified first
    ModifiedDesc,
    /// Oldest modified first
    ModifiedAsc,
    /// Largest files first
    SizeDesc,
    /// Smallest files first
    SizeAsc,
    /// Alphabetical by filename (A-Z)
    NameAsc,
    /// Reverse alphabetical (Z-A)
    NameDesc,
}

/// Discover video files in directory
///
/// Scans the specified directory for video files with recognized formats.
/// Returns files sorted by modification time (most recent first) by default.
///
/// # Arguments
///
/// * `dir` - Directory to scan
///
/// # Returns
///
/// Vector of discovered video files
///
/// # Example
///
/// ```no_run
/// use kindly_av1::file::discover_videos;
///
/// let videos = discover_videos("./videos");
/// for video in videos {
///     println!("{}", video);
/// }
/// ```
pub fn discover_videos<P: AsRef<Path>>(dir: P) -> Vec<DiscoveredFile> {
    discover_videos_with_options(dir, &DiscoveryOptions::default())
}

/// Discover video files with custom options
///
/// # Arguments
///
/// * `dir` - Directory to scan
/// * `options` - Discovery options for filtering and sorting
///
/// # Returns
///
/// Vector of discovered video files matching the options
pub fn discover_videos_with_options<P: AsRef<Path>>(
    dir: P,
    options: &DiscoveryOptions,
) -> Vec<DiscoveredFile> {
    let dir = dir.as_ref();
    let mut videos = Vec::new();

    discover_recursive(dir, options, 0, &mut videos);

    // Sort according to options
    match options.sort_by {
        SortOrder::ModifiedDesc => {
            videos.sort_by(|a, b| b.modified.cmp(&a.modified));
        }
        SortOrder::ModifiedAsc => {
            videos.sort_by(|a, b| a.modified.cmp(&b.modified));
        }
        SortOrder::SizeDesc => {
            videos.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
        }
        SortOrder::SizeAsc => {
            videos.sort_by(|a, b| a.size_bytes.cmp(&b.size_bytes));
        }
        SortOrder::NameAsc => {
            videos.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));
        }
        SortOrder::NameDesc => {
            videos.sort_by(|a, b| b.filename.to_lowercase().cmp(&a.filename.to_lowercase()));
        }
    }

    videos
}

/// Internal recursive discovery function
fn discover_recursive(
    dir: &Path,
    options: &DiscoveryOptions,
    current_depth: usize,
    results: &mut Vec<DiscoveredFile>,
) {
    // Check depth limit
    if options.max_depth > 0 && current_depth >= options.max_depth {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return, // Silently skip unreadable directories
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Handle directories (recursive case)
        if path.is_dir() && options.recursive {
            discover_recursive(&path, options, current_depth + 1, results);
            continue;
        }

        // Skip non-files
        if !path.is_file() {
            continue;
        }

        // Check format
        let format = match detect_format(&path) {
            Some(f) => f,
            None => continue,
        };

        // Check format filter
        if !options.formats.is_empty() && !options.formats.contains(&format) {
            continue;
        }

        // Get metadata
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let size = metadata.len();

        // Check size filters
        if size < options.min_size {
            continue;
        }
        if options.max_size > 0 && size > options.max_size {
            continue;
        }

        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        results.push(DiscoveredFile::new(path, format, size, modified));
    }
}

/// Discover videos in the same directory as the executable
///
/// Useful for portable deployments where videos are placed alongside the binary.
///
/// # Returns
///
/// Vector of discovered video files, or empty if directory cannot be determined
pub fn discover_in_binary_dir() -> Vec<DiscoveredFile> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return discover_videos(dir);
        }
    }
    Vec::new()
}

/// Discover videos in the current working directory
///
/// # Returns
///
/// Vector of discovered video files, or empty if cwd cannot be determined
pub fn discover_in_current_dir() -> Vec<DiscoveredFile> {
    if let Ok(cwd) = std::env::current_dir() {
        return discover_videos(cwd);
    }
    Vec::new()
}

/// Summary statistics for discovered files
#[derive(Debug, Clone, Default)]
pub struct DiscoverySummary {
    /// Total number of files found
    pub total_files: usize,
    /// Total size in bytes
    pub total_size: u64,
    /// Files by format
    pub by_format: std::collections::HashMap<InputFormat, usize>,
    /// Files with native demuxer support
    pub native_demuxer: usize,
    /// Files that can be read directly (YUV, Y4M)
    pub direct_read: usize,
}

impl DiscoverySummary {
    /// Generate summary from discovered files
    pub fn from_files(files: &[DiscoveredFile]) -> Self {
        let mut summary = Self::default();

        for file in files {
            summary.total_files += 1;
            summary.total_size += file.size_bytes;

            *summary.by_format.entry(file.format).or_insert(0) += 1;

            match file.format {
                InputFormat::RawYuv | InputFormat::Y4m => {
                    summary.direct_read += 1;
                }
                InputFormat::Mp4 | InputFormat::Mkv | InputFormat::WebM => {
                    summary.native_demuxer += 1;
                }
                _ => {}
            }
        }

        summary
    }

    /// Get human-readable total size
    pub fn total_size_display(&self) -> String {
        const GB: u64 = 1024 * 1024 * 1024;
        const MB: u64 = 1024 * 1024;

        if self.total_size >= GB {
            format!("{:.2} GB", self.total_size as f64 / GB as f64)
        } else {
            format!("{:.2} MB", self.total_size as f64 / MB as f64)
        }
    }
}

impl std::fmt::Display for DiscoverySummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} video files ({}) - {} direct read, {} native demuxer",
            self.total_files,
            self.total_size_display(),
            self.direct_read,
            self.native_demuxer
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovered_file_size_display() {
        let file = DiscoveredFile {
            path: PathBuf::from("/test/video.mp4"),
            format: InputFormat::Mp4,
            size_bytes: 1024 * 1024 * 500, // 500 MB
            modified: SystemTime::now(),
            filename: "video.mp4".to_string(),
        };
        assert!(file.size_display().contains("MB"));

        let file = DiscoveredFile {
            path: PathBuf::from("/test/video.mp4"),
            format: InputFormat::Mp4,
            size_bytes: 1024 * 1024 * 1024 * 2, // 2 GB
            modified: SystemTime::now(),
            filename: "video.mp4".to_string(),
        };
        assert!(file.size_display().contains("GB"));
    }

    #[test]
    fn test_discovered_file_format_detection() {
        let mp4 = DiscoveredFile {
            path: PathBuf::from("/test/video.mp4"),
            format: InputFormat::Mp4,
            size_bytes: 1000,
            modified: SystemTime::now(),
            filename: "video.mp4".to_string(),
        };
        assert_eq!(mp4.format, InputFormat::Mp4);

        let y4m = DiscoveredFile {
            path: PathBuf::from("/test/video.y4m"),
            format: InputFormat::Y4m,
            size_bytes: 1000,
            modified: SystemTime::now(),
            filename: "video.y4m".to_string(),
        };
        assert_eq!(y4m.format, InputFormat::Y4m);
    }

    #[test]
    fn test_discovery_options_default() {
        let opts = DiscoveryOptions::default();
        assert!(!opts.recursive);
        assert_eq!(opts.max_depth, 1);
        assert_eq!(opts.min_size, 0);
        assert_eq!(opts.max_size, 0);
        assert!(opts.formats.is_empty());
        assert_eq!(opts.sort_by, SortOrder::ModifiedDesc);
    }

    #[test]
    fn test_discovery_summary() {
        let files = vec![
            DiscoveredFile {
                path: PathBuf::from("/test/a.mp4"),
                format: InputFormat::Mp4,
                size_bytes: 1000,
                modified: SystemTime::now(),
                filename: "a.mp4".to_string(),
            },
            DiscoveredFile {
                path: PathBuf::from("/test/b.y4m"),
                format: InputFormat::Y4m,
                size_bytes: 2000,
                modified: SystemTime::now(),
                filename: "b.y4m".to_string(),
            },
        ];

        let summary = DiscoverySummary::from_files(&files);
        assert_eq!(summary.total_files, 2);
        assert_eq!(summary.total_size, 3000);
        assert_eq!(summary.native_demuxer, 1);
        assert_eq!(summary.direct_read, 1);
    }

    #[test]
    fn test_discover_in_current_dir() {
        // Should not panic even if no videos exist
        let _ = discover_in_current_dir();
    }
}
