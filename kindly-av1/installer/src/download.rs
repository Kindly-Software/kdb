// download.rs - T5 Streaming Download Capsule
// UCE34 Q10 T5 Streaming tier - Download with progress, retry, checksum verification

use sha2::{Sha256, Digest};
use std::io::{self, Read, Write};
use std::path::Path;
use std::fs::File;

/// Download errors with clear user-facing messages
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("Could not connect. Check your internet connection.")]
    Network(#[source] Box<ureq::Error>),

    #[error("Download corrupted. Please try again.")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("Not enough disk space.")]
    DiskFull(#[source] io::Error),

    #[error("Cannot write to install directory.")]
    PermissionDenied(#[source] io::Error),

    #[error("Download failed after {0} attempts")]
    MaxRetriesExceeded(u32),
}

/// Download capsule state (T5 Streaming)
pub struct DownloadCapsule {
    base_url: String,
    retry_attempts: u32,
    retry_delay_ms: u64,
}

impl DownloadCapsule {
    /// Create new download capsule
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            retry_attempts: 3,
            retry_delay_ms: 1000, // Start with 1s
        }
    }

    /// Download file with progress indicator and retry logic
    pub fn download_with_progress(
        &self,
        filename: &str,
        output_path: &Path,
    ) -> Result<(), DownloadError> {
        let url = format!("{}/{}", self.base_url, filename);
        let checksum_url = format!("{}.sha256", url);

        // Download checksum file first
        let expected_checksum = self.download_checksum(&checksum_url)?;

        // Download file with retries
        for attempt in 1..=self.retry_attempts {
            match self.try_download(&url, output_path, &expected_checksum) {
                Ok(()) => return Ok(()),
                Err(e) if attempt < self.retry_attempts => {
                    eprintln!("[kindly-av1] Attempt {}/{} failed, retrying...", attempt, self.retry_attempts);
                    std::thread::sleep(std::time::Duration::from_millis(
                        self.retry_delay_ms * (1 << (attempt - 1)) // Exponential backoff
                    ));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(DownloadError::MaxRetriesExceeded(self.retry_attempts))
    }

    /// Download checksum file
    fn download_checksum(&self, url: &str) -> Result<String, DownloadError> {
        let response = ureq::get(url)
            .call()
            .map_err(|e| DownloadError::Network(Box::new(e)))?;

        let mut checksum = String::new();
        response.into_reader()
            .read_to_string(&mut checksum)
            .map_err(Self::classify_io_error)?;

        Ok(checksum.trim().to_string())
    }

    /// Try single download attempt with progress
    fn try_download(
        &self,
        url: &str,
        output_path: &Path,
        expected_checksum: &str,
    ) -> Result<(), DownloadError> {
        // Make request
        let response = ureq::get(url)
            .call()
            .map_err(|e| DownloadError::Network(Box::new(e)))?;

        // Get total size if available
        let total_size = response.header("Content-Length")
            .and_then(|s| s.parse::<u64>().ok());

        // Stream download with progress (T5 Streaming - no full file in memory)
        let mut reader = response.into_reader();
        let mut file = File::create(output_path)
            .map_err(Self::classify_io_error)?;
        let mut hasher = Sha256::new();

        let mut buffer = [0u8; 8192];
        let mut downloaded = 0u64;
        let mut last_print = 0u64;

        loop {
            let n = reader.read(&mut buffer)
                .map_err(Self::classify_io_error)?;
            if n == 0 { break; }

            file.write_all(&buffer[..n])
                .map_err(Self::classify_io_error)?;
            hasher.update(&buffer[..n]);
            downloaded += n as u64;

            // Print progress every 1MB
            if downloaded - last_print >= 1_048_576 {
                Self::print_progress(downloaded, total_size);
                last_print = downloaded;
            }
        }

        // Final progress
        Self::print_progress(downloaded, total_size);
        println!(); // Newline after progress

        // Verify checksum
        let actual_checksum = format!("{:x}", hasher.finalize());
        if actual_checksum != expected_checksum {
            std::fs::remove_file(output_path).ok(); // Clean up corrupted file
            return Err(DownloadError::ChecksumMismatch {
                expected: expected_checksum.to_string(),
                actual: actual_checksum,
            });
        }

        Ok(())
    }

    /// Print progress indicator
    fn print_progress(downloaded: u64, total_size: Option<u64>) {
        let mb = downloaded as f64 / 1_048_576.0;

        if let Some(total) = total_size {
            let total_mb = total as f64 / 1_048_576.0;
            let percent = (downloaded as f64 / total as f64 * 100.0) as u32;
            print!("\r[kindly-av1] Downloading... {}% ({:.1}MB / {:.1}MB)", percent, mb, total_mb);
        } else {
            print!("\r[kindly-av1] Downloading... {:.1}MB", mb);
        }

        io::stdout().flush().ok();
    }

    /// Classify I/O errors into user-friendly categories
    fn classify_io_error(err: io::Error) -> DownloadError {
        match err.kind() {
            io::ErrorKind::PermissionDenied => DownloadError::PermissionDenied(err),
            io::ErrorKind::OutOfMemory => DownloadError::DiskFull(err),
            _ => {
                // Check for ENOSPC (disk full) via raw_os_error
                #[cfg(unix)]
                if err.raw_os_error() == Some(28) { // ENOSPC
                    return DownloadError::DiskFull(err);
                }

                DownloadError::PermissionDenied(err) // Default fallback
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_capsule_creation() {
        let capsule = DownloadCapsule::new("https://example.com");
        assert_eq!(capsule.base_url, "https://example.com");
        assert_eq!(capsule.retry_attempts, 3);
    }
}
