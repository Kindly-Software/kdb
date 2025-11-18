// Python datasketch baseline wrapper
// B32 Compliance: Industry-standard baseline for fair comparison
// NO STRAWMAN - datasketch is the standard Python MinHash/LSH library

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Python datasketch benchmark wrapper
///
/// Executes Python datasketch script and parses JSON results.
/// This provides an industry-standard baseline for B32 compliance.
pub struct PythonDatasketch {
    script_path: PathBuf,
}

/// Baseline benchmark results
///
/// B32 Requirements:
/// - Throughput (docs/sec)
/// - Latency per document (μs)
/// - Total time (seconds)
/// - Duplicates found (count)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineResult {
    /// Documents processed per second
    pub throughput_docs_per_sec: f64,

    /// Latency per document in microseconds
    pub latency_per_doc_us: f64,

    /// Total execution time in seconds
    pub total_time_sec: f64,

    /// Number of duplicate pairs found
    pub duplicates_found: usize,

    /// Error message if benchmark failed
    #[serde(default)]
    pub error: Option<String>,
}

impl PythonDatasketch {
    /// Create new Python datasketch wrapper
    ///
    /// # Arguments
    /// * `script_path` - Path to Python script (datasketch_baseline.py)
    pub fn new(script_path: impl Into<PathBuf>) -> Self {
        Self {
            script_path: script_path.into(),
        }
    }

    /// Run datasketch benchmark on corpus
    ///
    /// # Arguments
    /// * `corpus_path` - Path to JSON corpus file (JSONL format)
    /// * `num_perm` - Number of permutations (default 128)
    /// * `threshold` - Jaccard similarity threshold (default 0.85)
    ///
    /// # Returns
    /// Result with BaselineResult or error
    ///
    /// # B32 Compliance
    /// - Same hardware as Rust benchmarks
    /// - Same dataset
    /// - Standard datasketch library (NOT optimized)
    /// - Fair comparison baseline
    pub fn run_benchmark(&self, corpus_path: &Path, num_perm: usize, threshold: f64) -> anyhow::Result<BaselineResult> {
        // Verify Python script exists
        if !self.script_path.exists() {
            anyhow::bail!("Python script not found: {}", self.script_path.display());
        }

        // Verify corpus exists
        if !corpus_path.exists() {
            anyhow::bail!("Corpus file not found: {}", corpus_path.display());
        }

        // Execute Python script
        let output = Command::new("python3")
            .arg(&self.script_path)
            .arg(corpus_path)
            .arg(num_perm.to_string())
            .arg(threshold.to_string())
            .output()?;

        // Check if execution succeeded
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "Python benchmark failed with status: {}\nError: {}",
                output.status,
                stderr
            );
        }

        // Parse JSON output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let result: BaselineResult = serde_json::from_str(&stdout)
            .map_err(|e| anyhow::anyhow!("Failed to parse JSON output: {}\nOutput: {}", e, stdout))?;

        Ok(result)
    }

    /// Run benchmark with default parameters
    ///
    /// Uses standard MinHash configuration:
    /// - 128 permutations
    /// - 0.85 Jaccard threshold
    pub fn run_benchmark_default(&self, corpus_path: &Path) -> anyhow::Result<BaselineResult> {
        self.run_benchmark(corpus_path, 128, 0.85)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires Python + datasketch installed
    fn test_python_datasketch_small() {
        let script = PathBuf::from("benches/baselines/datasketch_baseline.py");
        let wrapper = PythonDatasketch::new(script);

        // Create small test corpus
        let corpus = PathBuf::from("test_data/small_corpus.json");

        if corpus.exists() {
            let result = wrapper.run_benchmark_default(&corpus).unwrap();

            // Validate results
            assert!(result.throughput_docs_per_sec > 0.0);
            assert!(result.latency_per_doc_us > 0.0);
            assert!(result.total_time_sec > 0.0);
        }
    }
}
