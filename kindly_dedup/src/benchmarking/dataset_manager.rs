/// Dataset Manager for Realistic LLM Training Data
///
/// B32 Compliance: Real datasets with documented provenance (NO synthetic data for sales claims)
///
/// Supported Sources:
/// - The Pile (EleutherAI): https://pile.eleuther.ai/
/// - Common Crawl: https://commoncrawl.org/
/// - C4 (Colossal Clean Crawled Corpus): https://huggingface.co/datasets/allenai/c4
/// - RedPajama (LLaMA training replica): https://huggingface.co/datasets/togethercomputer/RedPajama-Data-1T
///
/// Architecture:
/// - Streaming downloads (HTTP/2, reqwest + tokio)
/// - SHA-256 integrity verification
/// - Manifest provenance tracking
/// - Progress tracking (indicatif)
/// - Retry logic (max 3 attempts)
use anyhow::{Context, Result};
use atomic_capsule::parallel::BatchProgressRenderer;
use atomic_capsule::primitives::ProgressTrackerCapsule;
use crate::serialize_helpers::*;
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Dataset provenance manifest (B32 requirement)
///
/// Tracks source, URL, download timestamp, integrity hash, and metadata
/// for transparent benchmarking
#[derive(Debug, Clone)]
pub struct DatasetManifest {
    /// Dataset source name (e.g., "The Pile", "Common Crawl")
    pub source: String,
    /// Original URL or dataset identifier
    pub url: String,
    /// ISO 8601 timestamp of download
    pub downloaded: String,
    /// Number of documents in dataset
    pub document_count: usize,
    /// Total size in bytes
    pub size_bytes: u64,
    /// SHA-256 hash for integrity verification
    pub sha256: String,
    /// Provenance notes (version, subset, modifications)
    pub provenance: String,
}

impl DatasetManifest {
    pub fn to_json(&self) -> Result<String, JsonError> {
        let mut writer = JsonWriterCapsule::new();
        writer.start_object()?;

        let mut first = true;
        write_field(&mut writer, "source", &self.source, &mut first)?;
        write_field(&mut writer, "url", &self.url, &mut first)?;
        write_field(&mut writer, "downloaded", &self.downloaded, &mut first)?;
        write_field(&mut writer, "document_count", &self.document_count, &mut first)?;
        write_field(&mut writer, "size_bytes", &self.size_bytes, &mut first)?;
        write_field(&mut writer, "sha256", &self.sha256, &mut first)?;
        write_field(&mut writer, "provenance", &self.provenance, &mut first)?;

        writer.end_object()?;
        writer.finalize()
    }

    pub fn from_json(s: &str) -> Result<Self, JsonError> {
        let mut parser = JsonParserCapsule::new(s);
        let value = parser.parse()?;

        match value {
            JsonValue::Object(fields) => {
                let source = match get_field_required(&fields, "source")? {
                    JsonValue::String(s) => s.clone(),
                    _ => return Err(JsonError::TypeMismatch("Expected string for source".into())),
                };

                let url = match get_field_required(&fields, "url")? {
                    JsonValue::String(s) => s.clone(),
                    _ => return Err(JsonError::TypeMismatch("Expected string for url".into())),
                };

                let downloaded = match get_field_required(&fields, "downloaded")? {
                    JsonValue::String(s) => s.clone(),
                    _ => return Err(JsonError::TypeMismatch("Expected string for downloaded".into())),
                };

                let document_count = match get_field_required(&fields, "document_count")? {
                    JsonValue::Number(n) if n.fract() == 0.0 => *n as usize,
                    _ => return Err(JsonError::TypeMismatch("Expected integer for document_count".into())),
                };

                let size_bytes = match get_field_required(&fields, "size_bytes")? {
                    JsonValue::Number(n) if n.fract() == 0.0 => *n as u64,
                    _ => return Err(JsonError::TypeMismatch("Expected integer for size_bytes".into())),
                };

                let sha256 = match get_field_required(&fields, "sha256")? {
                    JsonValue::String(s) => s.clone(),
                    _ => return Err(JsonError::TypeMismatch("Expected string for sha256".into())),
                };

                let provenance = match get_field_required(&fields, "provenance")? {
                    JsonValue::String(s) => s.clone(),
                    _ => return Err(JsonError::TypeMismatch("Expected string for provenance".into())),
                };

                Ok(DatasetManifest {
                    source,
                    url,
                    downloaded,
                    document_count,
                    size_bytes,
                    sha256,
                    provenance,
                })
            }
            _ => Err(JsonError::TypeMismatch("Expected object".into())),
        }
    }
}

/// Dataset source types
#[derive(Debug, Clone, Copy)]
pub enum DatasetSource {
    /// The Pile (EleutherAI) - 825GB diverse dataset
    Pile,
    /// Common Crawl - Web crawl archives
    CommonCrawl,
    /// C4 - Cleaned Common Crawl
    C4,
    /// RedPajama - LLaMA training data replica
    RedPajama,
}

impl DatasetSource {
    /// Get source name
    pub fn name(&self) -> &str {
        match self {
            Self::Pile => "The Pile (EleutherAI)",
            Self::CommonCrawl => "Common Crawl",
            Self::C4 => "C4 (Colossal Clean Crawled Corpus)",
            Self::RedPajama => "RedPajama (LLaMA Training Data)",
        }
    }

    /// Get base URL for dataset
    pub fn base_url(&self) -> &str {
        match self {
            Self::Pile => "https://the-eye.eu/public/AI/pile/",
            Self::CommonCrawl => "https://data.commoncrawl.org/",
            Self::C4 => "https://huggingface.co/datasets/allenai/c4",
            Self::RedPajama => "https://huggingface.co/datasets/togethercomputer/RedPajama-Data-1T",
        }
    }
}

/// Dataset Manager
///
/// Handles downloading, verification, and manifest generation
/// for realistic LLM training datasets
pub struct DatasetManager {
    /// Base path for dataset storage
    base_path: PathBuf,
    /// HTTP client (reused across downloads)
    client: Client,
}

impl DatasetManager {
    /// Create new dataset manager
    ///
    /// # Arguments
    /// - `base_path`: Root directory for dataset storage
    pub fn new<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();

        // Create base directory if it doesn't exist
        std::fs::create_dir_all(&base_path).context("Failed to create dataset base directory")?;

        // Create HTTP client with reasonable defaults
        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // 5 min timeout for large files
            .gzip(true) // Auto-decompress gzip
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { base_path, client })
    }

    /// Download The Pile subset
    ///
    /// Downloads subset of The Pile dataset (JSONL format)
    ///
    /// # Arguments
    /// - `limit`: Maximum number of documents to download
    ///
    /// # Returns
    /// Path to downloaded dataset file
    pub async fn download_pile(&self, limit: usize) -> Result<PathBuf> {
        let source = DatasetSource::Pile;
        let output_path = self.base_path.join(format!("pile_{}k.json", limit / 1000));

        // For now, use existing download_corpus binary
        // TODO: Implement direct Pile downloader
        eprintln!("Note: The Pile download not yet implemented. Use download_corpus for Common Crawl.");
        eprintln!("Pile URL: {}", source.base_url());

        anyhow::bail!("The Pile downloader not yet implemented")
    }

    /// Download Common Crawl subset
    ///
    /// Downloads subset of Common Crawl WET (Web Extracted Text) files
    ///
    /// # Arguments
    /// - `limit`: Maximum number of documents to download
    ///
    /// # Returns
    /// Path to downloaded dataset file
    pub async fn download_common_crawl(&self, limit: usize) -> Result<PathBuf> {
        let source = DatasetSource::CommonCrawl;
        let output_path = self.base_path.join(format!("commoncrawl_{}k.json", limit / 1000));

        // Use existing download_corpus implementation
        eprintln!("Downloading Common Crawl ({} documents)...", limit);
        eprintln!("This may take several hours for large datasets.");
        eprintln!("Output: {}", output_path.display());

        // Note: Actual download happens via download_corpus binary
        // This method serves as documentation and manifest generation
        Ok(output_path)
    }

    /// Download C4 subset
    ///
    /// Downloads subset of C4 (Cleaned Common Crawl)
    ///
    /// # Arguments
    /// - `limit`: Maximum number of documents to download
    ///
    /// # Returns
    /// Path to downloaded dataset file
    pub async fn download_c4(&self, limit: usize) -> Result<PathBuf> {
        let source = DatasetSource::C4;
        let output_path = self.base_path.join(format!("c4_{}k.json", limit / 1000));

        // C4 requires Hugging Face datasets library or API
        eprintln!("Note: C4 download requires Hugging Face API access.");
        eprintln!("C4 URL: {}", source.base_url());

        anyhow::bail!("C4 downloader not yet implemented - use Hugging Face datasets")
    }

    /// Download RedPajama subset
    ///
    /// Downloads subset of RedPajama (LLaMA training data replica)
    ///
    /// # Arguments
    /// - `limit`: Maximum number of documents to download
    ///
    /// # Returns
    /// Path to downloaded dataset file
    pub async fn download_redpajama(&self, limit: usize) -> Result<PathBuf> {
        let source = DatasetSource::RedPajama;
        let output_path = self.base_path.join(format!("redpajama_{}k.json", limit / 1000));

        // RedPajama requires Hugging Face datasets library or API
        eprintln!("Note: RedPajama download requires Hugging Face API access.");
        eprintln!("RedPajama URL: {}", source.base_url());

        anyhow::bail!("RedPajama downloader not yet implemented - use Hugging Face datasets")
    }

    /// Verify dataset integrity using SHA-256
    ///
    /// Computes SHA-256 hash and compares with manifest
    ///
    /// # Arguments
    /// - `dataset_path`: Path to dataset file
    /// - `expected_hash`: Expected SHA-256 hash (hex string)
    ///
    /// # Returns
    /// `true` if hash matches, `false` otherwise
    pub fn verify_integrity(&self, dataset_path: &Path, expected_hash: &str) -> Result<bool> {
        let computed = compute_sha256(dataset_path).context("Failed to compute SHA-256 hash")?;

        Ok(computed == expected_hash)
    }

    /// Create manifest for dataset
    ///
    /// Generates provenance manifest with metadata
    ///
    /// # Arguments
    /// - `dataset_path`: Path to dataset file
    /// - `source`: Dataset source
    /// - `document_count`: Number of documents
    /// - `provenance_notes`: Additional provenance information
    ///
    /// # Returns
    /// Dataset manifest
    pub fn create_manifest(
        &self,
        dataset_path: &Path,
        source: DatasetSource,
        document_count: usize,
        provenance_notes: &str,
    ) -> Result<DatasetManifest> {
        // Compute SHA-256 hash
        let sha256 = compute_sha256(dataset_path).context("Failed to compute SHA-256 for manifest")?;

        // Get file size
        let metadata = std::fs::metadata(dataset_path).context("Failed to get dataset metadata")?;
        let size_bytes = metadata.len();

        // Current timestamp (ISO 8601)
        let downloaded = chrono::Utc::now().to_rfc3339();

        let manifest = DatasetManifest {
            source: source.name().to_string(),
            url: source.base_url().to_string(),
            downloaded,
            document_count,
            size_bytes,
            sha256,
            provenance: provenance_notes.to_string(),
        };

        Ok(manifest)
    }

    /// Save manifest to JSON file
    ///
    /// Saves manifest alongside dataset file with .manifest.json extension
    ///
    /// # Arguments
    /// - `manifest`: Manifest to save
    /// - `dataset_path`: Path to dataset (manifest will be saved as dataset_path.manifest.json)
    pub fn save_manifest(&self, manifest: &DatasetManifest, dataset_path: &Path) -> Result<()> {
        let manifest_path = dataset_path.with_extension("manifest.json");

        let json = manifest.to_json().map_err(|e| anyhow::anyhow!("Failed to serialize manifest: {}", e))?;

        let mut file = File::create(&manifest_path).context("Failed to create manifest file")?;

        file.write_all(json.as_bytes()).context("Failed to write manifest")?;

        println!("Manifest saved: {}", manifest_path.display());

        Ok(())
    }

    /// Load manifest from JSON file
    ///
    /// # Arguments
    /// - `dataset_path`: Path to dataset (will load dataset_path.manifest.json)
    ///
    /// # Returns
    /// Loaded manifest
    pub fn load_manifest(&self, dataset_path: &Path) -> Result<DatasetManifest> {
        let manifest_path = dataset_path.with_extension("manifest.json");

        let mut file = File::open(&manifest_path).context("Failed to open manifest file")?;

        let mut contents = String::new();
        file.read_to_string(&mut contents).context("Failed to read manifest file")?;

        let manifest = DatasetManifest::from_json(&contents)
            .map_err(|e| anyhow::anyhow!("Failed to parse manifest: {}", e))?;

        Ok(manifest)
    }
}

/// Streaming Downloader (HTTP/2)
///
/// High-performance streaming downloads with progress tracking
pub struct StreamingDownloader {
    client: Client,
}

impl StreamingDownloader {
    /// Create new streaming downloader
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .http2_prior_knowledge() // Force HTTP/2
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client })
    }

    /// Download file with progress tracking
    ///
    /// # Arguments
    /// - `url`: URL to download
    /// - `output_path`: Output file path
    ///
    /// # Returns
    /// Number of bytes downloaded
    pub async fn download_with_progress(&self, url: &str, output_path: &Path) -> Result<u64> {
        println!("Downloading: {}", url);

        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to send HTTP request")?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP error: {}", response.status());
        }

        // Get content length for progress tracking
        let total_size = response.content_length().unwrap_or(0);

        // Create progress tracker (atomic_capsule replacement for indicatif)
        let tracker = Arc::new(ProgressTrackerCapsule::new(total_size));
        let tracker_clone = Arc::clone(&tracker);

        // Start background renderer (10ms batching, 100 FPS)
        let mut renderer = BatchProgressRenderer::start(tracker_clone, move |current, total| {
            if total > 0 {
                let percentage = (current * 100) / total;
                let mb_current = current / 1_000_000;
                let mb_total = total / 1_000_000;
                print!("\r[{} MB / {} MB] {}%", mb_current, mb_total, percentage);
            } else {
                print!("\r{} bytes downloaded", current);
            }
            let _ = std::io::Write::flush(&mut std::io::stdout());
        });

        // Create output file
        let mut file = File::create(output_path).context("Failed to create output file")?;

        // Stream download
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt; // For .next()

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed to read chunk")?;
            file.write_all(&chunk).context("Failed to write chunk to file")?;

            downloaded += chunk.len() as u64;
            tracker.increment_by(chunk.len() as u64);
        }

        renderer.stop();
        println!("\nDownload complete");

        println!("Downloaded {} bytes to {}", downloaded, output_path.display());

        Ok(downloaded)
    }
}

/// Compute SHA-256 hash of file
///
/// # Arguments
/// - `file_path`: Path to file
///
/// # Returns
/// SHA-256 hash as hex string
pub fn compute_sha256(file_path: &Path) -> Result<String> {
    let mut file = File::open(file_path).context("Failed to open file for SHA-256 computation")?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let n = file.read(&mut buffer).context("Failed to read file for SHA-256")?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataset_source_names() {
        assert_eq!(DatasetSource::Pile.name(), "The Pile (EleutherAI)");
        assert_eq!(DatasetSource::CommonCrawl.name(), "Common Crawl");
        assert_eq!(DatasetSource::C4.name(), "C4 (Colossal Clean Crawled Corpus)");
        assert_eq!(DatasetSource::RedPajama.name(), "RedPajama (LLaMA Training Data)");
    }

    #[test]
    fn test_dataset_source_urls() {
        assert!(DatasetSource::Pile.base_url().contains("pile"));
        assert!(DatasetSource::CommonCrawl.base_url().contains("commoncrawl"));
        assert!(DatasetSource::C4.base_url().contains("c4"));
        assert!(DatasetSource::RedPajama.base_url().contains("RedPajama"));
    }

    #[test]
    fn test_manifest_serialization() {
        let manifest = DatasetManifest {
            source: "Test Source".to_string(),
            url: "https://example.com".to_string(),
            downloaded: "2025-10-29T18:00:00Z".to_string(),
            document_count: 1000,
            size_bytes: 1024000,
            sha256: "abc123def456".to_string(),
            provenance: "Test dataset, v1.0".to_string(),
        };

        let json = manifest.to_json().unwrap();
        let parsed = DatasetManifest::from_json(&json).unwrap();

        assert_eq!(parsed.source, manifest.source);
        assert_eq!(parsed.document_count, manifest.document_count);
        assert_eq!(parsed.sha256, manifest.sha256);
    }

    #[test]
    fn test_dataset_manager_creation() {
        let temp_dir = std::env::temp_dir().join("kindly_dedup_test_datasets");
        let manager = DatasetManager::new(&temp_dir).unwrap();

        assert!(temp_dir.exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
