//! Recent files management
//!
//! Stores last 5 encoded files to `~/.kindly-av1/recent.json`
//!
//! ## JSON Format
//! ```json
//! {"files": [{"path": "/home/user/video.mp4", "size_bytes": 1234567890, "encoded_at": 1732712345}]}
//! ```
//!
//! ## Framework Compliance
//! - **Chaos**: Simple file I/O, no coordination needed
//! - **ASSUM**: File I/O assumptions documented below
//! - **Zero Dependencies**: Hand-written JSON parsing

use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_RECENT_FILES: usize = 5;

/// A recently encoded file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentFile {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub encoded_at: SystemTime,
}

/// Recent files list (max 5 entries)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecentFiles {
    files: Vec<RecentFile>,
}

#[derive(Debug)]
pub enum RecentFilesError {
    IoError(io::Error),
    ParseError(String),
}

impl From<io::Error> for RecentFilesError {
    fn from(err: io::Error) -> Self {
        RecentFilesError::IoError(err)
    }
}

impl RecentFiles {
    /// Load recent files from disk (returns empty if not found)
    ///
    /// #ASSUME: $HOME environment variable is set (verified via std::env::var)
    /// #VERIFY: Returns empty list if file doesn't exist (graceful fallback)
    pub fn load() -> Self {
        // #ASSUME: File read succeeds or doesn't exist
        // #VERIFY: Return empty list on any error (graceful degradation)
        match fs::read_to_string(Self::recent_path()) {
            Ok(contents) => Self::parse_json(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save recent files to disk
    ///
    /// #ASSUME: User has write permissions to home directory
    /// #VERIFY: Create config directory if it doesn't exist
    pub fn save(&self) -> Result<(), RecentFilesError> {
        // Ensure config directory exists
        let config_dir = Self::config_dir();
        fs::create_dir_all(&config_dir)?;

        // Serialize to JSON (hand-written)
        let json = self.to_json();
        fs::write(Self::recent_path(), json)?;
        Ok(())
    }

    /// Add a file to recent list (removes oldest if >5)
    ///
    /// #ASSUME: size_bytes is accurate
    /// #VERIFY: Maintains max 5 entries (removes oldest)
    pub fn add(&mut self, path: PathBuf, size_bytes: u64) {
        let recent_file = RecentFile {
            path,
            size_bytes,
            encoded_at: SystemTime::now(),
        };

        // Insert at front (newest first)
        self.files.insert(0, recent_file);

        // Keep only last MAX_RECENT_FILES entries
        self.files.truncate(MAX_RECENT_FILES);
    }

    /// Get all recent files (newest first)
    pub fn files(&self) -> &[RecentFile] {
        &self.files
    }

    /// Clear all recent files
    pub fn clear(&mut self) {
        self.files.clear();
    }

    /// Get config directory path (~/.kindly-av1/)
    ///
    /// #ASSUME: $HOME environment variable is set
    /// #VERIFY: Falls back to current directory if $HOME not set
    fn config_dir() -> PathBuf {
        match std::env::var("HOME") {
            Ok(home) => PathBuf::from(home).join(".kindly-av1"),
            Err(_) => PathBuf::from(".kindly-av1"),
        }
    }

    /// Get recent files path
    fn recent_path() -> PathBuf {
        Self::config_dir().join("recent.json")
    }

    /// Serialize to JSON (hand-written, no dependencies)
    fn to_json(&self) -> String {
        let mut entries = Vec::new();
        for file in &self.files {
            let path_str = file.path.to_string_lossy();
            let timestamp = file
                .encoded_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            entries.push(format!(
                r#"{{"path": "{}", "size_bytes": {}, "encoded_at": {}}}"#,
                path_str, file.size_bytes, timestamp
            ));
        }

        format!(r#"{{"files": [{}]}}"#, entries.join(", "))
    }

    /// Parse JSON (hand-written, no dependencies)
    ///
    /// #ASSUME: JSON is well-formed
    /// #VERIFY: Returns error on malformed JSON
    fn parse_json(json: &str) -> Result<Self, RecentFilesError> {
        let json = json.trim();

        // Find the "files" array
        let files_start = json
            .find(r#""files":"#)
            .ok_or_else(|| RecentFilesError::ParseError("Missing 'files' key".to_string()))?
            + r#""files":"#.len();

        let rest = &json[files_start..].trim_start();

        // Extract array contents
        let array_start = rest
            .find('[')
            .ok_or_else(|| RecentFilesError::ParseError("Missing array start".to_string()))?;
        let array_end = rest
            .rfind(']')
            .ok_or_else(|| RecentFilesError::ParseError("Missing array end".to_string()))?;

        let array_contents = &rest[array_start + 1..array_end];

        // Parse each file entry
        let mut files = Vec::new();
        if !array_contents.trim().is_empty() {
            // Split by objects (look for }{ pattern)
            let mut current = array_contents;
            while let Some(obj_start) = current.find('{') {
                let obj_end = current[obj_start..]
                    .find('}')
                    .ok_or_else(|| RecentFilesError::ParseError("Unclosed object".to_string()))?;

                let obj = &current[obj_start..obj_start + obj_end + 1];
                if let Some(file) = Self::parse_file_entry(obj) {
                    files.push(file);
                }

                // Move to next object
                current = &current[obj_start + obj_end + 1..];
            }
        }

        Ok(Self { files })
    }

    /// Parse a single file entry from JSON
    fn parse_file_entry(json: &str) -> Option<RecentFile> {
        // Extract path
        let path_pattern = r#""path": ""#;
        let path_start = json.find(path_pattern)? + path_pattern.len();
        let path_end = json[path_start..].find('"')?;
        let path = PathBuf::from(&json[path_start..path_start + path_end]);

        // Extract size_bytes
        let size_pattern = r#""size_bytes": "#;
        let size_start = json.find(size_pattern)? + size_pattern.len();
        let size_rest = &json[size_start..].trim_start();
        let size_end = size_rest.find(|c: char| !c.is_ascii_digit())?;
        let size_bytes = size_rest[..size_end].parse::<u64>().ok()?;

        // Extract encoded_at
        let timestamp_pattern = r#""encoded_at": "#;
        let timestamp_start = json.find(timestamp_pattern)? + timestamp_pattern.len();
        let timestamp_rest = &json[timestamp_start..].trim_start();
        let timestamp_end = timestamp_rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(timestamp_rest.len());
        let timestamp = timestamp_rest[..timestamp_end].parse::<u64>().ok()?;
        let encoded_at = UNIX_EPOCH + std::time::Duration::from_secs(timestamp);

        Some(RecentFile {
            path,
            size_bytes,
            encoded_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_recent_files() {
        let recent = RecentFiles::default();
        assert_eq!(recent.files().len(), 0);
    }

    #[test]
    fn test_add_recent_file() {
        let mut recent = RecentFiles::default();
        recent.add(PathBuf::from("/test/video.mp4"), 1024);
        assert_eq!(recent.files().len(), 1);
        assert_eq!(recent.files()[0].path, PathBuf::from("/test/video.mp4"));
        assert_eq!(recent.files()[0].size_bytes, 1024);
    }

    #[test]
    fn test_add_recent_file_max_limit() {
        let mut recent = RecentFiles::default();
        for i in 0..10 {
            recent.add(PathBuf::from(format!("/test/video{}.mp4", i)), i as u64);
        }
        // Should keep only last 5
        assert_eq!(recent.files().len(), MAX_RECENT_FILES);
        // Newest first
        assert_eq!(recent.files()[0].path, PathBuf::from("/test/video9.mp4"));
        assert_eq!(recent.files()[4].path, PathBuf::from("/test/video5.mp4"));
    }

    #[test]
    fn test_clear_recent_files() {
        let mut recent = RecentFiles::default();
        recent.add(PathBuf::from("/test/video.mp4"), 1024);
        recent.clear();
        assert_eq!(recent.files().len(), 0);
    }

    #[test]
    fn test_json_serialization() {
        let mut recent = RecentFiles::default();
        let file = RecentFile {
            path: PathBuf::from("/test/video.mp4"),
            size_bytes: 1234567890,
            encoded_at: UNIX_EPOCH + std::time::Duration::from_secs(1732712345),
        };
        recent.files.push(file);

        let json = recent.to_json();
        assert!(json.contains(r#""path": "/test/video.mp4""#));
        assert!(json.contains(r#""size_bytes": 1234567890"#));
        assert!(json.contains(r#""encoded_at": 1732712345"#));
    }

    #[test]
    fn test_json_deserialization() {
        let json = r#"{"files": [{"path": "/test/video.mp4", "size_bytes": 1234567890, "encoded_at": 1732712345}]}"#;
        let recent = RecentFiles::parse_json(json).unwrap();
        assert_eq!(recent.files().len(), 1);
        assert_eq!(recent.files()[0].path, PathBuf::from("/test/video.mp4"));
        assert_eq!(recent.files()[0].size_bytes, 1234567890);
        assert_eq!(
            recent.files()[0].encoded_at,
            UNIX_EPOCH + std::time::Duration::from_secs(1732712345)
        );
    }

    #[test]
    fn test_json_deserialization_empty() {
        let json = r#"{"files": []}"#;
        let recent = RecentFiles::parse_json(json).unwrap();
        assert_eq!(recent.files().len(), 0);
    }

    #[test]
    fn test_json_deserialization_multiple_files() {
        let json = r#"{"files": [{"path": "/test/video1.mp4", "size_bytes": 100, "encoded_at": 1000}, {"path": "/test/video2.mp4", "size_bytes": 200, "encoded_at": 2000}]}"#;
        let recent = RecentFiles::parse_json(json).unwrap();
        assert_eq!(recent.files().len(), 2);
        assert_eq!(recent.files()[0].path, PathBuf::from("/test/video1.mp4"));
        assert_eq!(recent.files()[1].path, PathBuf::from("/test/video2.mp4"));
    }

    #[test]
    fn test_json_roundtrip() {
        let mut original = RecentFiles::default();
        original.add(PathBuf::from("/test/video1.mp4"), 1000);
        original.add(PathBuf::from("/test/video2.mp4"), 2000);

        let json = original.to_json();
        let parsed = RecentFiles::parse_json(&json).unwrap();

        assert_eq!(parsed.files().len(), original.files().len());
        for (p, o) in parsed.files().iter().zip(original.files().iter()) {
            assert_eq!(p.path, o.path);
            assert_eq!(p.size_bytes, o.size_bytes);
        }
    }

    #[test]
    fn test_config_dir() {
        let config_dir = RecentFiles::config_dir();
        assert!(config_dir.ends_with(".kindly-av1"));
    }

    #[test]
    fn test_malformed_json() {
        let json = r#"{"files": [{"path": "/test/video.mp4""#; // Missing closing
        assert!(RecentFiles::parse_json(json).is_err());
    }
}
