//! # GMM Offline Training Tool
//!
//! Fits Gaussian Mixture Model components using Expectation-Maximization (EM) algorithm
//! and exports to GMMCapsule-compatible Q16.16 fixed-point binary format.
//!
//! ## UCE34 Framework Analysis
//!
//! - **Q10 (Tier Selection)**: T3 Fixed-Point for deterministic output
//! - **Q11 (Rust Transform)**: Q16.16 arithmetic matching atomic_capsule::protection::gmm_capsule
//! - **Q34 (Auditability)**: Reproducible training with deterministic output
//!
//! ## Usage
//!
//! ```bash
//! train_gmm --input samples.csv --num-components 8 --output gmm_8comp.bin
//! train_gmm --input samples.csv --num-components 4 --output gmm_4comp.bin --max-iter 200
//! ```
//!
//! ## Binary Output Format (GMMCapsule compatible)
//!
//! ```text
//! Header (16 bytes):
//!   magic: u32 = 0x474D4D43 ("GMMC")
//!   version: u32 = 1
//!   num_components: u32
//!   reserved: u32
//!
//! Components (32 bytes each):
//!   weight_q16_16: i64
//!   mean_q16_16: i64
//!   variance_q16_16: i64
//!   inv_variance_q16: i64
//! ```

use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

// ============================================================================
// Q16.16 FIXED-POINT ARITHMETIC (matches gmm_capsule.rs)
// ============================================================================

/// Q16.16 scale factor
const Q16_16_SCALE: f64 = 65536.0;

/// Convert f64 to Q16.16 fixed-point (i64)
#[inline]
fn f64_to_q16_16(value: f64) -> i64 {
    let scaled = value * Q16_16_SCALE;
    if scaled >= i64::MAX as f64 {
        i64::MAX
    } else if scaled <= i64::MIN as f64 {
        i64::MIN
    } else {
        scaled as i64
    }
}

/// Convert Q16.16 fixed-point (i64) to f64
#[inline]
#[cfg_attr(not(test), allow(dead_code))]
fn q16_16_to_f64(value: i64) -> f64 {
    value as f64 / Q16_16_SCALE
}

/// Q16.16 multiplication with proper scaling: (a * b) >> 16
#[inline]
#[cfg_attr(not(test), allow(dead_code))]
fn q16_16_mul(a: i64, b: i64) -> i64 {
    let product = (a as i128) * (b as i128);
    (product >> 16) as i64
}

/// Q16.16 division with proper scaling: (a << 16) / b
#[inline]
#[cfg_attr(not(test), allow(dead_code))]
fn q16_16_div(a: i64, b: i64) -> i64 {
    if b == 0 {
        return i64::MAX;
    }
    let numerator = (a as i128) << 16;
    (numerator / (b as i128)) as i64
}

// ============================================================================
// GAUSSIAN COMPONENT (Training State)
// ============================================================================

/// Single Gaussian component during training
#[derive(Debug, Clone)]
struct GaussianComponent {
    /// Component weight pi_k (probability of component k)
    weight: f64,
    /// Component mean mu_k
    mean: f64,
    /// Component variance sigma^2_k
    variance: f64,
}

impl GaussianComponent {
    /// Create new component with initial parameters
    fn new(weight: f64, mean: f64, variance: f64) -> Self {
        Self {
            weight,
            mean,
            variance: variance.max(1e-10), // Prevent zero variance
        }
    }

    /// Compute Gaussian probability density: N(x | mu, sigma^2)
    fn pdf(&self, x: f64) -> f64 {
        let diff = x - self.mean;
        let exponent = -0.5 * diff * diff / self.variance;
        let coefficient = 1.0 / (2.0 * std::f64::consts::PI * self.variance).sqrt();
        coefficient * exponent.exp()
    }
}

// ============================================================================
// GMM TRAINER (EM Algorithm)
// ============================================================================

/// GMM Trainer using Expectation-Maximization algorithm
struct GmmTrainer {
    /// Gaussian components
    components: Vec<GaussianComponent>,
    /// Training samples
    samples: Vec<f64>,
    /// Responsibility matrix: responsibilities[n][k] = P(component k | sample n)
    responsibilities: Vec<Vec<f64>>,
    /// Convergence threshold for log-likelihood change
    convergence_threshold: f64,
    /// Maximum iterations
    max_iterations: usize,
}

impl GmmTrainer {
    /// Create new trainer with specified number of components
    fn new(num_components: usize, samples: Vec<f64>) -> Self {
        let n_samples = samples.len();

        // Initialize responsibilities matrix
        let responsibilities = vec![vec![0.0; num_components]; n_samples];

        // Initialize components with k-means++ style initialization
        let components = Self::initialize_components(num_components, &samples);

        Self {
            components,
            samples,
            responsibilities,
            convergence_threshold: 1e-6,
            max_iterations: 100,
        }
    }

    /// K-means++ style initialization for better convergence
    fn initialize_components(num_components: usize, samples: &[f64]) -> Vec<GaussianComponent> {
        if samples.is_empty() {
            return vec![];
        }

        // Calculate overall statistics
        let mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;
        let variance: f64 = samples.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / samples.len() as f64;

        let variance = variance.max(1e-10);
        let stddev = variance.sqrt();

        // Initialize components spread across the data range
        let mut components = Vec::with_capacity(num_components);
        let uniform_weight = 1.0 / num_components as f64;

        for i in 0..num_components {
            // Spread means across the data range
            let offset = if num_components > 1 {
                (i as f64 - (num_components - 1) as f64 / 2.0) * stddev * 2.0 / num_components as f64
            } else {
                0.0
            };

            components.push(GaussianComponent::new(
                uniform_weight,
                mean + offset,
                variance,
            ));
        }

        components
    }

    /// Set convergence threshold
    fn with_convergence_threshold(mut self, threshold: f64) -> Self {
        self.convergence_threshold = threshold;
        self
    }

    /// Set maximum iterations
    fn with_max_iterations(mut self, max_iter: usize) -> Self {
        self.max_iterations = max_iter;
        self
    }

    /// E-step: Compute responsibilities
    ///
    /// For each sample n and component k:
    /// r_nk = (pi_k * N(x_n | mu_k, sigma^2_k)) / sum_j(pi_j * N(x_n | mu_j, sigma^2_j))
    fn e_step(&mut self) {
        let num_components = self.components.len();

        for (n, &sample) in self.samples.iter().enumerate() {
            // Compute weighted likelihoods for all components
            let weighted_likelihoods: Vec<f64> = self.components.iter()
                .map(|c| c.weight * c.pdf(sample))
                .collect();

            // Normalize to get responsibilities
            let total: f64 = weighted_likelihoods.iter().sum();

            if total > 1e-300 {
                for k in 0..num_components {
                    self.responsibilities[n][k] = weighted_likelihoods[k] / total;
                }
            } else {
                // If all likelihoods are ~0, assign uniform responsibility
                let uniform = 1.0 / num_components as f64;
                for k in 0..num_components {
                    self.responsibilities[n][k] = uniform;
                }
            }
        }
    }

    /// M-step: Update parameters
    ///
    /// For each component k:
    /// N_k = sum_n(r_nk)
    /// mu_k = (1/N_k) * sum_n(r_nk * x_n)
    /// sigma^2_k = (1/N_k) * sum_n(r_nk * (x_n - mu_k)^2)
    /// pi_k = N_k / N
    fn m_step(&mut self) {
        let n_samples = self.samples.len() as f64;
        let num_components = self.components.len();

        for k in 0..num_components {
            // Effective count for component k
            let n_k: f64 = self.responsibilities.iter()
                .map(|r| r[k])
                .sum();

            if n_k < 1e-10 {
                // Component has negligible responsibility, reset to avoid numerical issues
                continue;
            }

            // Update mean
            let new_mean: f64 = self.samples.iter()
                .zip(self.responsibilities.iter())
                .map(|(&x, r)| r[k] * x)
                .sum::<f64>() / n_k;

            // Update variance
            let new_variance: f64 = self.samples.iter()
                .zip(self.responsibilities.iter())
                .map(|(&x, r)| r[k] * (x - new_mean).powi(2))
                .sum::<f64>() / n_k;

            // Update weight
            let new_weight = n_k / n_samples;

            // Apply updates with minimum variance floor
            self.components[k].mean = new_mean;
            self.components[k].variance = new_variance.max(1e-10);
            self.components[k].weight = new_weight;
        }
    }

    /// Compute log-likelihood of data given current parameters
    fn log_likelihood(&self) -> f64 {
        self.samples.iter()
            .map(|&sample| {
                let mixture_prob: f64 = self.components.iter()
                    .map(|c| c.weight * c.pdf(sample))
                    .sum();

                if mixture_prob > 1e-300 {
                    mixture_prob.ln()
                } else {
                    -700.0 // Avoid -inf
                }
            })
            .sum()
    }

    /// Run EM algorithm until convergence
    fn fit(&mut self) -> usize {
        let mut prev_ll = f64::NEG_INFINITY;
        let mut iterations = 0;

        for iter in 0..self.max_iterations {
            // E-step: compute responsibilities
            self.e_step();

            // M-step: update parameters
            self.m_step();

            // Check convergence
            let current_ll = self.log_likelihood();
            let ll_change = (current_ll - prev_ll).abs();

            iterations = iter + 1;

            if ll_change < self.convergence_threshold {
                eprintln!("Converged at iteration {} (log-likelihood change: {:.2e})", iter + 1, ll_change);
                break;
            }

            prev_ll = current_ll;

            if (iter + 1) % 10 == 0 {
                eprintln!("Iteration {}: log-likelihood = {:.6}", iter + 1, current_ll);
            }
        }

        iterations
    }

    /// Get trained components
    fn components(&self) -> &[GaussianComponent] {
        &self.components
    }
}

// ============================================================================
// BINARY FORMAT EXPORT (GMMCapsule compatible)
// ============================================================================

/// Magic number for GMM binary file: "GMMC"
const GMM_MAGIC: u32 = 0x474D4D43;

/// Current binary format version
const GMM_VERSION: u32 = 1;

/// Export trained GMM to binary format
fn export_gmm_binary(
    components: &[GaussianComponent],
    output_path: &Path,
) -> std::io::Result<()> {
    let mut file = File::create(output_path)?;

    // Write header (16 bytes)
    file.write_all(&GMM_MAGIC.to_le_bytes())?;
    file.write_all(&GMM_VERSION.to_le_bytes())?;
    file.write_all(&(components.len() as u32).to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?; // reserved

    // Write components (32 bytes each)
    for component in components {
        // Convert to Q16.16 fixed-point
        let weight_q16 = f64_to_q16_16(component.weight);
        let mean_q16 = f64_to_q16_16(component.mean);
        let variance_q16 = f64_to_q16_16(component.variance);

        // Pre-compute inverse variance for fast Mahalanobis distance
        let inv_variance = if component.variance > 1e-10 {
            1.0 / component.variance
        } else {
            10000.0 // Cap at 10000 for near-zero variance
        };
        let inv_variance_q16 = f64_to_q16_16(inv_variance);

        file.write_all(&weight_q16.to_le_bytes())?;
        file.write_all(&mean_q16.to_le_bytes())?;
        file.write_all(&variance_q16.to_le_bytes())?;
        file.write_all(&inv_variance_q16.to_le_bytes())?;
    }

    Ok(())
}

// ============================================================================
// CSV LOADING
// ============================================================================

/// Load samples from CSV file
///
/// Expects single column of numeric values, or takes first column if multiple.
/// Skips header row if first cell is not numeric.
fn load_samples(input_path: &Path) -> std::io::Result<Vec<f64>> {
    let file = File::open(input_path)?;
    let reader = BufReader::new(file);
    let mut samples = Vec::new();
    let mut skipped_header = false;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        // Take first column (comma or space separated)
        let first_value = line
            .split(|c| c == ',' || c == ' ' || c == '\t')
            .next()
            .unwrap_or("");

        match first_value.parse::<f64>() {
            Ok(value) => {
                if value.is_finite() {
                    samples.push(value);
                }
            }
            Err(_) => {
                if !skipped_header {
                    skipped_header = true;
                    eprintln!("Skipping header row: {}", line);
                } else {
                    eprintln!("Warning: Skipping non-numeric line: {}", line);
                }
            }
        }
    }

    Ok(samples)
}

// ============================================================================
// CLI ARGUMENT PARSING (No external dependencies)
// ============================================================================

/// Parsed command-line arguments
struct Args {
    input: String,
    output: String,
    num_components: usize,
    max_iterations: usize,
    convergence_threshold: f64,
    verbose: bool,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let args: Vec<String> = env::args().collect();

        let mut input: Option<String> = None;
        let mut output: Option<String> = None;
        let mut num_components: usize = 8;
        let mut max_iterations: usize = 100;
        let mut convergence_threshold: f64 = 1e-6;
        let mut verbose = false;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--input" | "-i" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--input requires a value".to_string());
                    }
                    input = Some(args[i].clone());
                }
                "--output" | "-o" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--output requires a value".to_string());
                    }
                    output = Some(args[i].clone());
                }
                "--num-components" | "-n" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--num-components requires a value".to_string());
                    }
                    num_components = args[i].parse()
                        .map_err(|_| format!("Invalid num-components: {}", args[i]))?;
                }
                "--max-iter" | "-m" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--max-iter requires a value".to_string());
                    }
                    max_iterations = args[i].parse()
                        .map_err(|_| format!("Invalid max-iter: {}", args[i]))?;
                }
                "--threshold" | "-t" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--threshold requires a value".to_string());
                    }
                    convergence_threshold = args[i].parse()
                        .map_err(|_| format!("Invalid threshold: {}", args[i]))?;
                }
                "--verbose" | "-v" => {
                    verbose = true;
                }
                "--help" | "-h" => {
                    return Err(Self::help_message());
                }
                arg => {
                    return Err(format!("Unknown argument: {}", arg));
                }
            }
            i += 1;
        }

        let input = input.ok_or_else(|| "Missing required argument: --input".to_string())?;
        let output = output.ok_or_else(|| "Missing required argument: --output".to_string())?;

        if num_components < 1 || num_components > 8 {
            return Err("num-components must be between 1 and 8".to_string());
        }

        Ok(Self {
            input,
            output,
            num_components,
            max_iterations,
            convergence_threshold,
            verbose,
        })
    }

    fn help_message() -> String {
        r#"GMM Offline Training Tool

Fits Gaussian Mixture Model components using EM algorithm and exports
to GMMCapsule-compatible Q16.16 fixed-point binary format.

USAGE:
    train_gmm --input <FILE> --output <FILE> [OPTIONS]

REQUIRED:
    -i, --input <FILE>          Input CSV file with sample values
    -o, --output <FILE>         Output binary file (.bin)

OPTIONS:
    -n, --num-components <N>    Number of GMM components (1-8, default: 8)
    -m, --max-iter <N>          Maximum EM iterations (default: 100)
    -t, --threshold <F>         Convergence threshold (default: 1e-6)
    -v, --verbose               Enable verbose output
    -h, --help                  Print this help message

EXAMPLES:
    train_gmm --input samples.csv --output gmm_8comp.bin
    train_gmm -i data.csv -o model.bin -n 4 -m 200 -v

BINARY OUTPUT FORMAT:
    Header (16 bytes):
      magic: u32 = 0x474D4D43 ("GMMC")
      version: u32 = 1
      num_components: u32
      reserved: u32

    Components (32 bytes each):
      weight_q16_16: i64
      mean_q16_16: i64
      variance_q16_16: i64
      inv_variance_q16: i64

CSV INPUT FORMAT:
    Single column of numeric values, or first column if multiple.
    Header row is automatically skipped if first cell is non-numeric."#.to_string()
    }
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    // Parse arguments
    let args = match Args::parse() {
        Ok(args) => args,
        Err(msg) => {
            eprintln!("{}", msg);
            std::process::exit(1);
        }
    };

    eprintln!("GMM Offline Training Tool");
    eprintln!("========================");
    eprintln!("Input:          {}", args.input);
    eprintln!("Output:         {}", args.output);
    eprintln!("Components:     {}", args.num_components);
    eprintln!("Max iterations: {}", args.max_iterations);
    eprintln!("Threshold:      {:.2e}", args.convergence_threshold);
    eprintln!();

    // Load samples
    eprintln!("Loading samples from {}...", args.input);
    let samples = match load_samples(Path::new(&args.input)) {
        Ok(samples) => samples,
        Err(e) => {
            eprintln!("Error loading samples: {}", e);
            std::process::exit(1);
        }
    };

    if samples.len() < 10 {
        eprintln!("Error: Need at least 10 samples for GMM training (got {})", samples.len());
        std::process::exit(1);
    }

    eprintln!("Loaded {} samples", samples.len());

    // Calculate sample statistics
    let mean: f64 = samples.iter().sum::<f64>() / samples.len() as f64;
    let variance: f64 = samples.iter()
        .map(|&x| (x - mean).powi(2))
        .sum::<f64>() / samples.len() as f64;
    let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    eprintln!("Sample statistics:");
    eprintln!("  Mean:     {:.6}", mean);
    eprintln!("  Variance: {:.6}", variance);
    eprintln!("  Stddev:   {:.6}", variance.sqrt());
    eprintln!("  Min:      {:.6}", min);
    eprintln!("  Max:      {:.6}", max);
    eprintln!();

    // Train GMM
    eprintln!("Training GMM with {} components...", args.num_components);
    let mut trainer = GmmTrainer::new(args.num_components, samples)
        .with_max_iterations(args.max_iterations)
        .with_convergence_threshold(args.convergence_threshold);

    let iterations = trainer.fit();

    eprintln!();
    eprintln!("Training complete in {} iterations", iterations);
    eprintln!("Final log-likelihood: {:.6}", trainer.log_likelihood());
    eprintln!();

    // Print trained components
    eprintln!("Trained components:");
    eprintln!("{:>3} {:>10} {:>12} {:>12} {:>12}", "#", "Weight", "Mean", "Variance", "StdDev");
    eprintln!("{:-<55}", "");

    let components = trainer.components();
    for (i, c) in components.iter().enumerate() {
        eprintln!(
            "{:>3} {:>10.6} {:>12.6} {:>12.6} {:>12.6}",
            i,
            c.weight,
            c.mean,
            c.variance,
            c.variance.sqrt()
        );
    }
    eprintln!();

    // Export to binary
    eprintln!("Exporting to {}...", args.output);
    match export_gmm_binary(components, Path::new(&args.output)) {
        Ok(()) => {
            eprintln!("Export successful!");

            // Print Q16.16 values if verbose
            if args.verbose {
                eprintln!();
                eprintln!("Q16.16 fixed-point values:");
                eprintln!("{:>3} {:>18} {:>18} {:>18} {:>18}", "#", "Weight", "Mean", "Variance", "InvVariance");
                eprintln!("{:-<80}", "");

                for (i, c) in components.iter().enumerate() {
                    let weight_q16 = f64_to_q16_16(c.weight);
                    let mean_q16 = f64_to_q16_16(c.mean);
                    let variance_q16 = f64_to_q16_16(c.variance);
                    let inv_variance_q16 = f64_to_q16_16(1.0 / c.variance.max(1e-10));

                    eprintln!(
                        "{:>3} {:>18} {:>18} {:>18} {:>18}",
                        i,
                        weight_q16,
                        mean_q16,
                        variance_q16,
                        inv_variance_q16
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("Error exporting: {}", e);
            std::process::exit(1);
        }
    }

    // Calculate file size
    let header_size = 16;
    let component_size = 32;
    let total_size = header_size + component_size * args.num_components;
    eprintln!("Output file size: {} bytes", total_size);
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Q16.16 ARITHMETIC TESTS ====================

    #[test]
    fn test_q16_16_conversion_roundtrip() {
        let values = [0.0, 1.0, -1.0, 0.5, 100.0, -100.0, 0.001];
        for &v in &values {
            let q16 = f64_to_q16_16(v);
            let recovered = q16_16_to_f64(q16);
            assert!((v - recovered).abs() < 0.0001, "Conversion failed for {}", v);
        }
    }

    #[test]
    fn test_q16_16_multiplication() {
        let a = f64_to_q16_16(2.0);
        let b = f64_to_q16_16(3.0);
        let result = q16_16_mul(a, b);
        let expected = f64_to_q16_16(6.0);
        assert!((result - expected).abs() < 100, "Mul failed");
    }

    #[test]
    fn test_q16_16_division() {
        let a = f64_to_q16_16(6.0);
        let b = f64_to_q16_16(2.0);
        let result = q16_16_div(a, b);
        let expected = f64_to_q16_16(3.0);
        assert!((result - expected).abs() < 100, "Div failed");
    }

    // ==================== GAUSSIAN COMPONENT TESTS ====================

    #[test]
    fn test_gaussian_pdf() {
        let component = GaussianComponent::new(1.0, 0.0, 1.0);

        // PDF at mean should be maximum
        let pdf_at_mean = component.pdf(0.0);
        let pdf_away = component.pdf(1.0);
        assert!(pdf_at_mean > pdf_away);

        // Standard normal at mean: 1/sqrt(2*pi) ≈ 0.399
        assert!((pdf_at_mean - 0.3989).abs() < 0.01);
    }

    // ==================== GMM TRAINER TESTS ====================

    #[test]
    fn test_gmm_initialization() {
        let samples = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let trainer = GmmTrainer::new(2, samples);

        assert_eq!(trainer.components.len(), 2);
        assert!((trainer.components[0].weight - 0.5).abs() < 0.01);
        assert!((trainer.components[1].weight - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_gmm_e_step() {
        let samples = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let mut trainer = GmmTrainer::new(2, samples);

        trainer.e_step();

        // Check responsibilities sum to 1 for each sample
        for resp in &trainer.responsibilities {
            let sum: f64 = resp.iter().sum();
            assert!((sum - 1.0).abs() < 1e-6, "Responsibilities should sum to 1");
        }
    }

    #[test]
    fn test_gmm_fit_single_component() {
        let samples: Vec<f64> = (0..100).map(|i| i as f64 / 10.0).collect();
        let mean_expected = samples.iter().sum::<f64>() / samples.len() as f64;

        let mut trainer = GmmTrainer::new(1, samples);
        trainer.fit();

        // Single component should converge to sample mean
        let component = &trainer.components[0];
        assert!((component.mean - mean_expected).abs() < 0.1);
        assert!((component.weight - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_gmm_fit_bimodal() {
        // Create bimodal distribution: centered at 0 and 10
        let mut samples = Vec::new();
        for i in 0..50 {
            samples.push(i as f64 * 0.1 - 2.5); // Around 0
        }
        for i in 0..50 {
            samples.push(i as f64 * 0.1 + 7.5); // Around 10
        }

        let mut trainer = GmmTrainer::new(2, samples)
            .with_max_iterations(50);
        trainer.fit();

        // Should find two components near 0 and 10
        let means: Vec<f64> = trainer.components.iter().map(|c| c.mean).collect();
        let has_low = means.iter().any(|&m| m < 3.0);
        let has_high = means.iter().any(|&m| m > 7.0);

        assert!(has_low && has_high, "Should find bimodal structure");
    }

    // ==================== EXPORT TESTS ====================

    #[test]
    fn test_export_binary_size() {
        let components = vec![
            GaussianComponent::new(0.5, 0.0, 1.0),
            GaussianComponent::new(0.5, 10.0, 2.0),
        ];

        let temp_path = std::env::temp_dir().join("test_gmm.bin");
        export_gmm_binary(&components, &temp_path).unwrap();

        let metadata = std::fs::metadata(&temp_path).unwrap();
        let expected_size = 16 + 32 * 2; // header + 2 components
        assert_eq!(metadata.len() as usize, expected_size);

        // Cleanup
        std::fs::remove_file(temp_path).ok();
    }

    #[test]
    fn test_export_binary_magic() {
        let components = vec![GaussianComponent::new(1.0, 0.0, 1.0)];

        let temp_path = std::env::temp_dir().join("test_gmm_magic.bin");
        export_gmm_binary(&components, &temp_path).unwrap();

        let bytes = std::fs::read(&temp_path).unwrap();
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert_eq!(magic, GMM_MAGIC);

        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        assert_eq!(version, GMM_VERSION);

        // Cleanup
        std::fs::remove_file(temp_path).ok();
    }
}
