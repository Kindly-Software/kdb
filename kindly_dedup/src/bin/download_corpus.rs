/// Multi-Source Dataset Downloader
///
/// Downloads realistic LLM training datasets with B32 provenance tracking.
///
/// Supported Sources:
/// - Common Crawl: WET (Web Extracted Text) files from August 2024 crawl
/// - The Pile: EleutherAI's diverse training dataset (TODO)
/// - C4: Cleaned Common Crawl (TODO - requires Hugging Face)
/// - RedPajama: LLaMA training data replica (TODO - requires Hugging Face)
///
/// # Architecture
/// - Zero Python dependencies (pure Rust)
/// - Async streaming downloads (reqwest + tokio)
/// - Gzip decompression on-the-fly (flate2)
/// - WET format parser (WARC-like headers + plain text)
/// - Progress tracking (indicatif)
/// - SHA-256 integrity verification
/// - Manifest generation (B32 provenance)
/// - Error handling with retries (max 3 attempts)
///
/// # Usage
/// ```bash
/// # Download Common Crawl (100K documents)
/// cargo run --bin download_corpus --features download-tools -- \
///   --source commoncrawl --limit 100000 --output test_data/realistic/commoncrawl_100k.json
///
/// # Download with manifest generation
/// cargo run --bin download_corpus --features download-tools -- \
///   --source commoncrawl --limit 1000000 --output test_data/realistic/commoncrawl_1m.json \
///   --generate-manifest
/// ```
use anyhow::{Context, Result};
use chrono;
use flate2::read::GzDecoder;
use hex;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use kindly_dedup::serialize_helpers::*;

/// Document structure compatible with kindly_dedup pipeline
#[derive(Debug, Clone)]
pub struct Document {
    /// Unique document ID (sequential)
    pub id: usize,
    /// Original URL from WARC-Target-URI
    pub url: String,
    /// Extracted plain text content
    pub text: String,
}

impl Document {
    pub fn to_json(&self) -> Result<String, JsonError> {
        let mut writer = JsonWriterCapsule::new();
        writer.start_object()?;
        let mut first = true;
        write_field(&mut writer, "id", &self.id, &mut first)?;
        write_field(&mut writer, "url", &self.url, &mut first)?;
        write_field(&mut writer, "text", &self.text, &mut first)?;
        writer.end_object()?;
        writer.finalize()
    }

    pub fn from_json(s: &str) -> Result<Self, JsonError> {
        let mut parser = JsonParserCapsule::new(s);
        let value = parser.parse()?;

        match value {
            JsonValue::Object(fields) => {
                Ok(Self {
                    id: get_field_required(&fields, "id").and_then(|v| usize::parse_json(v))?,
                    url: get_field_required(&fields, "url").and_then(|v| String::parse_json(v))?,
                    text: get_field_required(&fields, "text").and_then(|v| String::parse_json(v))?,
                })
            }
            _ => Err(JsonError::InvalidJson("Expected object".into()))
        }
    }
}

impl WriteJson for Document {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.start_object()?;
        let mut first = true;
        write_field(writer, "id", &self.id, &mut first)?;
        write_field(writer, "url", &self.url, &mut first)?;
        write_field(writer, "text", &self.text, &mut first)?;
        writer.end_object()
    }
}

/// Helper to serialize Vec<Document> to pretty JSON
fn serialize_documents_pretty(docs: &[Document]) -> Result<String, JsonError> {
    let mut writer = JsonWriterCapsule::new();
    writer.start_array()?;
    for (i, doc) in docs.iter().enumerate() {
        if i > 0 {
            writer.write_comma()?;
        }
        doc.write_json(&mut writer)?;
    }
    writer.end_array()?;
    writer.finalize()
}

impl WriteJson for usize {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_u64(*self as u64)
    }
}

impl ParseJson for usize {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonError> {
        match value {
            JsonValue::Number(n) => {
                if *n >= 0.0 && n.fract() == 0.0 {
                    Ok(*n as usize)
                } else {
                    Err(JsonError::TypeMismatch("Expected non-negative integer".into()))
                }
            }
            _ => Err(JsonError::TypeMismatch("Expected number".into())),
        }
    }
}

/// Common Crawl WET paths base URL
const CC_BASE_URL: &str = "https://data.commoncrawl.org/";

/// WET paths file for August 2024 crawl
const WET_PATHS_URL: &str = "https://data.commoncrawl.org/crawl-data/CC-MAIN-2024-33/wet.paths.gz";

/// Download timeout (30 seconds)
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);

/// Max retries for failed downloads
const MAX_RETRIES: usize = 3;

/// Min text length to include (filter out empty/tiny documents)
const MIN_TEXT_LENGTH: usize = 100;

/// Download and parse WET paths file
///
/// Returns list of full URLs to WET files (typically ~72K paths per crawl)
async fn fetch_wet_paths(client: &Client) -> Result<Vec<String>> {
    println!("Fetching WET paths from {}...", WET_PATHS_URL);

    let response = client
        .get(WET_PATHS_URL)
        .timeout(DOWNLOAD_TIMEOUT)
        .send()
        .await
        .context("Failed to fetch WET paths file")?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP error: {}", response.status());
    }

    // Download gzipped content
    let bytes = response.bytes().await.context("Failed to read WET paths response")?;

    // Decompress
    let decoder = GzDecoder::new(&bytes[..]);
    let reader = BufReader::new(decoder);

    // Parse paths (one per line)
    let mut paths = Vec::new();
    for line in reader.lines() {
        let line = line.context("Failed to read line from WET paths")?;
        let path = line.trim();
        if !path.is_empty() {
            // Construct full URL
            paths.push(format!("{}{}", CC_BASE_URL, path));
        }
    }

    println!("Found {} WET files", paths.len());
    Ok(paths)
}

/// Parse single WET record from buffered reader
///
/// WET format:
/// - WARC headers starting with "WARC/1.0"
/// - Headers including WARC-Type, WARC-Target-URI, Content-Length
/// - Blank line separator
/// - Exactly Content-Length bytes of plain text content
/// - Two blank lines before next WARC record
///
/// Returns None on EOF, or continues until finding a valid record
fn parse_wet_record<R: BufRead>(reader: &mut R, doc_id: usize) -> Result<Option<Document>> {
    // Loop until we find a valid record or hit EOF
    loop {
        let mut line = String::new();

        // Find next WARC record
        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line)?;

            if bytes_read == 0 {
                return Ok(None); // EOF
            }

            if line.trim().starts_with("WARC/") {
                break; // Found WARC header
            }
        }

        // Parse headers
        let mut url = String::new();
        let mut content_length: Option<usize> = None;
        let mut warc_type = String::new();

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line)?;

            if bytes_read == 0 {
                return Ok(None); // Unexpected EOF
            }

            let trimmed = line.trim();

            if trimmed.is_empty() {
                // End of headers
                break;
            }

            // Parse header fields
            if let Some((key, value)) = trimmed.split_once(':') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "WARC-Target-URI" => url = value.to_string(),
                    "Content-Length" => content_length = value.parse().ok(),
                    "WARC-Type" => warc_type = value.to_string(),
                    _ => {}
                }
            }
        }

        // Only process "conversion" records (these contain extracted text)
        if warc_type != "conversion" {
            // Skip non-conversion records (continue to next record)
            // But first consume the content to maintain sync
            if let Some(len) = content_length {
                let mut skip_buf = vec![0u8; len];
                let _ = reader.read_exact(&mut skip_buf);
            }
            continue;
        }

        // Read exact content length
        let content_len = content_length.unwrap_or(0);

        if content_len == 0 || url.is_empty() {
            continue; // Skip empty/invalid records
        }

        // Read exactly content_length bytes
        let mut content_bytes = vec![0u8; content_len];
        if let Err(e) = reader.read_exact(&mut content_bytes) {
            eprintln!("Warning: Failed to read content ({} bytes): {}", content_len, e);
            continue;
        }

        // Convert to string (lossy - handles non-UTF8)
        let text = String::from_utf8_lossy(&content_bytes).trim().to_string();

        // Filter by minimum length
        if text.len() >= MIN_TEXT_LENGTH {
            return Ok(Some(Document { id: doc_id, url, text }));
        }

        // Too short, continue to next record
    }
}

/// Download and parse single WET file
///
/// Returns documents extracted from this WET file (up to max_docs)
async fn download_wet_file(client: &Client, url: &str, start_id: usize, max_docs: usize) -> Result<Vec<Document>> {
    let mut docs = Vec::new();
    let mut doc_id = start_id;

    // Download WET file with retries
    let mut attempts = 0;
    let response = loop {
        attempts += 1;

        match client.get(url).timeout(DOWNLOAD_TIMEOUT).send().await {
            Ok(resp) if resp.status().is_success() => break resp,
            Ok(resp) => {
                if attempts >= MAX_RETRIES {
                    anyhow::bail!("HTTP error after {} retries: {}", MAX_RETRIES, resp.status());
                }
                eprintln!("HTTP error (attempt {}): {}, retrying...", attempts, resp.status());
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => {
                if attempts >= MAX_RETRIES {
                    anyhow::bail!("Download failed after {} retries: {}", MAX_RETRIES, e);
                }
                eprintln!("Download error (attempt {}): {}, retrying...", attempts, e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };

    // Get response bytes (still gzipped - .gz URL doesn't trigger auto-decompression)
    let bytes = response.bytes().await.context("Failed to read WET file response")?;

    // Manual decompression required
    let decoder = GzDecoder::new(&bytes[..]);
    let mut reader = BufReader::with_capacity(64 * 1024, decoder);

    // Parse WET records
    let mut records_processed = 0;
    let mut records_filtered = 0;

    while docs.len() < max_docs {
        match parse_wet_record(&mut reader, doc_id) {
            Ok(Some(doc)) => {
                docs.push(doc);
                doc_id += 1;
                records_processed += 1;
            }
            Ok(None) => {
                // EOF
                break;
            }
            Err(e) => {
                // Check if it's a filtering error or actual parse error
                if e.to_string().contains("filtered") {
                    records_filtered += 1;
                } else {
                    eprintln!("Warning: Failed to parse WET record: {}", e);
                }
                records_processed += 1;
                continue;
            }
        }
    }

    if docs.is_empty() && records_processed > 0 {
        eprintln!(
            "  Note: Processed {} records, filtered {} (too short/no content)",
            records_processed, records_filtered
        );
    }

    Ok(docs)
}

/// Main download function
///
/// Downloads WET files from Common Crawl until reaching the document limit
async fn download_corpus(limit: usize, output_path: &Path) -> Result<()> {
    println!("Common Crawl WET Downloader");
    println!("Target: {} documents", limit);
    println!();

    let client = Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .gzip(false) // Disable auto-decompression (files are .gz, we decompress manually)
        .build()
        .context("Failed to create HTTP client")?;

    // 1. Fetch WET file URLs
    let wet_paths = fetch_wet_paths(&client).await.context("Failed to fetch WET paths")?;

    if wet_paths.is_empty() {
        anyhow::bail!("No WET files found");
    }

    // 2. Download WET files until limit reached
    let mut all_docs = Vec::with_capacity(limit);

    let pb = ProgressBar::new(limit as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} docs ({per_sec}) {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );

    for (idx, wet_url) in wet_paths.iter().enumerate() {
        if all_docs.len() >= limit {
            break;
        }

        let remaining = limit - all_docs.len();
        pb.set_message(format!("File {}/{}", idx + 1, wet_paths.len()));

        match download_wet_file(&client, wet_url, all_docs.len(), remaining).await {
            Ok(docs) => {
                let count = docs.len();
                all_docs.extend(docs);
                pb.inc(count as u64);

                if count == 0 {
                    eprintln!("Warning: No documents extracted from {}", wet_url);
                }
            }
            Err(e) => {
                eprintln!("Error downloading {}: {}", wet_url, e);
                eprintln!("Skipping this WET file and continuing...");
                continue;
            }
        }
    }

    pb.finish_with_message("Download complete");
    println!();

    // 3. Save to JSON
    println!("Saving {} documents to {}...", all_docs.len(), output_path.display());

    let mut file = File::create(output_path).context("Failed to create output file")?;

    let json = serialize_documents_pretty(&all_docs)
        .map_err(|e| anyhow::anyhow!("Failed to serialize documents to JSON: {}", e))?;

    file.write_all(json.as_bytes())
        .context("Failed to write JSON to file")?;

    println!("Success! Downloaded {} documents", all_docs.len());
    println!("Output: {}", output_path.display());

    // Print statistics
    let total_chars: usize = all_docs.iter().map(|d| d.text.len()).sum();
    let avg_chars = if !all_docs.is_empty() {
        total_chars / all_docs.len()
    } else {
        0
    };

    println!();
    println!("Statistics:");
    println!("  Total documents: {}", all_docs.len());
    println!("  Total characters: {}", total_chars);
    println!("  Average doc length: {} chars", avg_chars);
    println!("  File size: {} MB", std::fs::metadata(output_path)?.len() / 1_000_000);

    Ok(())
}

/// Dataset manifest for B32 provenance tracking
#[derive(Debug, Clone)]
pub struct DatasetManifest {
    pub source: String,
    pub url: String,
    pub downloaded: String,
    pub document_count: usize,
    pub size_bytes: u64,
    pub sha256: String,
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
                Ok(Self {
                    source: get_field_required(&fields, "source").and_then(|v| String::parse_json(v))?,
                    url: get_field_required(&fields, "url").and_then(|v| String::parse_json(v))?,
                    downloaded: get_field_required(&fields, "downloaded").and_then(|v| String::parse_json(v))?,
                    document_count: get_field_required(&fields, "document_count").and_then(|v| usize::parse_json(v))?,
                    size_bytes: get_field_required(&fields, "size_bytes").and_then(|v| u64::parse_json(v))?,
                    sha256: get_field_required(&fields, "sha256").and_then(|v| String::parse_json(v))?,
                    provenance: get_field_required(&fields, "provenance").and_then(|v| String::parse_json(v))?,
                })
            }
            _ => Err(JsonError::InvalidJson("Expected object".into()))
        }
    }
}

/// Compute SHA-256 hash of file
fn compute_sha256(file_path: &Path) -> Result<String> {
    let mut file = File::open(file_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(hex::encode(hasher.finalize()))
}

/// Generate and save manifest
fn generate_manifest(output_path: &Path, source: &str, url: &str, document_count: usize) -> Result<()> {
    println!("\nGenerating manifest...");

    let sha256 = compute_sha256(output_path)?;
    let size_bytes = std::fs::metadata(output_path)?.len();
    let downloaded = chrono::Utc::now().to_rfc3339();

    let manifest = DatasetManifest {
        source: source.to_string(),
        url: url.to_string(),
        downloaded,
        document_count,
        size_bytes,
        sha256: sha256.clone(),
        provenance: format!("CC-MAIN-2024-33 crawl, {} documents, unmodified", document_count),
    };

    let manifest_path = output_path.with_extension("manifest.json");
    let json = manifest.to_json()
        .map_err(|e| anyhow::anyhow!("Failed to serialize manifest: {}", e))?;

    let mut file = File::create(&manifest_path)?;
    file.write_all(json.as_bytes())?;

    println!("Manifest saved: {}", manifest_path.display());
    println!("SHA-256: {}", sha256);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();

    let mut source = "commoncrawl";
    let mut limit = 100_000;
    let mut output_path = "test_data/realistic/commoncrawl_100k.json".to_string();
    let mut generate_manifest_flag = false;

    // Simple argument parsing
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--source" | "-s" => {
                if i + 1 < args.len() {
                    source = &args[i + 1];
                    i += 2;
                } else {
                    eprintln!("Error: --source requires a value");
                    std::process::exit(1);
                }
            }
            "--limit" | "-l" => {
                if i + 1 < args.len() {
                    limit = args[i + 1].parse().context("Invalid limit value")?;
                    i += 2;
                } else {
                    eprintln!("Error: --limit requires a value");
                    std::process::exit(1);
                }
            }
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    output_path = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("Error: --output requires a value");
                    std::process::exit(1);
                }
            }
            "--generate-manifest" | "-m" => {
                generate_manifest_flag = true;
                i += 1;
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                print_usage();
                std::process::exit(1);
            }
        }
    }

    // Validate source
    match source {
        "commoncrawl" | "cc" => {}
        "pile" => {
            eprintln!("Error: The Pile downloader not yet implemented");
            eprintln!("Use 'commoncrawl' for now");
            std::process::exit(1);
        }
        "c4" => {
            eprintln!("Error: C4 downloader not yet implemented");
            eprintln!("Use Hugging Face datasets library or 'commoncrawl'");
            std::process::exit(1);
        }
        "redpajama" => {
            eprintln!("Error: RedPajama downloader not yet implemented");
            eprintln!("Use Hugging Face datasets library or 'commoncrawl'");
            std::process::exit(1);
        }
        _ => {
            eprintln!("Error: Unknown source '{}'", source);
            eprintln!("Supported: commoncrawl, pile, c4, redpajama");
            std::process::exit(1);
        }
    }

    let output = Path::new(&output_path);

    // Create output directory
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).context("Failed to create output directory")?;
    }

    println!("Dataset Downloader");
    println!("Source: {}", source);
    println!("Limit: {} documents", limit);
    println!("Output: {}", output.display());
    println!();

    // Run download (currently only Common Crawl supported)
    download_corpus(limit, output).await?;

    // Generate manifest if requested
    if generate_manifest_flag {
        generate_manifest(output, "Common Crawl (CC-MAIN-2024-33)", WET_PATHS_URL, limit)?;
    }

    Ok(())
}

fn print_usage() {
    println!("Dataset Downloader - Realistic LLM Training Data");
    println!();
    println!("USAGE:");
    println!("    download_corpus [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!(
        "    -s, --source <SOURCE>       Dataset source (commoncrawl, pile, c4, redpajama) [default: commoncrawl]"
    );
    println!("    -l, --limit <LIMIT>         Maximum number of documents [default: 100000]");
    println!("    -o, --output <PATH>         Output file path [default: test_data/realistic/commoncrawl_100k.json]");
    println!("    -m, --generate-manifest     Generate SHA-256 manifest for B32 compliance");
    println!("    -h, --help                  Print this help message");
    println!();
    println!("EXAMPLES:");
    println!("    # Download 100K Common Crawl documents");
    println!("    download_corpus --source commoncrawl --limit 100000 --output test_data/realistic/cc_100k.json");
    println!();
    println!("    # Download 1M documents with manifest");
    println!("    download_corpus --source commoncrawl --limit 1000000 --output test_data/realistic/cc_1m.json --generate-manifest");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_serialization() {
        let doc = Document {
            id: 0,
            url: "https://example.com".to_string(),
            text: "Test content".to_string(),
        };

        let json = doc.to_json().unwrap();
        let parsed = Document::from_json(&json).unwrap();

        assert_eq!(parsed.id, doc.id);
        assert_eq!(parsed.url, doc.url);
        assert_eq!(parsed.text, doc.text);
    }

    #[test]
    fn test_min_text_length() {
        assert!(MIN_TEXT_LENGTH >= 100, "MIN_TEXT_LENGTH should filter out tiny docs");
    }
}
