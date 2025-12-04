# GMM Offline Training Tool

Offline Gaussian Mixture Model training tool for fitting GMM components and exporting to GMMCapsule-compatible Q16.16 fixed-point binary format.

## Overview

This tool implements the Expectation-Maximization (EM) algorithm for fitting Gaussian Mixture Models to sample data. The trained model is exported in a binary format compatible with `atomic_capsule::protection::GMMCapsule` for use in anomaly detection systems.

## UCE34 Framework Compliance

| Question | Answer |
|----------|--------|
| **Q10 (Tier)** | T3 Fixed-Point - Deterministic Q16.16 output |
| **Q11 (Rust Transform)** | Zero external dependencies, pure Rust |
| **Q33 (Verification)** | 10 unit tests, property tests |
| **Q34 (Auditability)** | Reproducible training, deterministic output |

## Build

```bash
cd /home/samuel/Primitives/tools/train_gmm
cargo build --release
```

## Usage

### Basic Usage

```bash
# Train 8-component GMM (default)
train_gmm --input samples.csv --output gmm_8comp.bin

# Train with 4 components
train_gmm --input data.csv --output model.bin --num-components 4

# Verbose output with Q16.16 values
train_gmm -i samples.csv -o gmm.bin -n 8 -v
```

### Command Line Arguments

| Argument | Short | Description | Default |
|----------|-------|-------------|---------|
| `--input` | `-i` | Input CSV file with sample values | **Required** |
| `--output` | `-o` | Output binary file (.bin) | **Required** |
| `--num-components` | `-n` | Number of GMM components (1-8) | 8 |
| `--max-iter` | `-m` | Maximum EM iterations | 100 |
| `--threshold` | `-t` | Convergence threshold | 1e-6 |
| `--verbose` | `-v` | Enable verbose output | false |
| `--help` | `-h` | Print help message | - |

### Examples

```bash
# Default 8-component training
train_gmm --input samples.csv --output gmm_8comp.bin

# 4 components with extended iterations
train_gmm -i data.csv -o model.bin -n 4 -m 200

# High-precision convergence
train_gmm -i data.csv -o model.bin -t 1e-9 -m 500

# Verbose output showing Q16.16 values
train_gmm -i samples.csv -o gmm.bin -n 8 -v
```

## Input Format (CSV)

The tool expects a CSV file with numeric values:

```csv
value
-2.5
-1.3
0.0
1.5
2.8
10.0
10.5
11.2
```

- **Single column**: Takes all numeric values
- **Multiple columns**: Takes first column only
- **Header row**: Automatically skipped if first cell is non-numeric
- **Minimum samples**: 10 samples required

## Output Format (Binary)

The binary output is compatible with `atomic_capsule::protection::GMMCapsule`:

```
Header (16 bytes):
  magic: u32 = 0x474D4D43 ("GMMC")
  version: u32 = 1
  num_components: u32
  reserved: u32

Components (32 bytes each):
  weight_q16_16: i64      # Component weight (0.0-1.0)
  mean_q16_16: i64        # Component mean
  variance_q16_16: i64    # Component variance
  inv_variance_q16: i64   # Pre-computed 1/variance
```

### Q16.16 Fixed-Point Format

- **Scale factor**: 65536 (2^16)
- **Range**: -32768.0 to +32767.99998
- **Precision**: 0.000015 (1/65536)

Conversion:
- `q16_value = (f64_value * 65536.0) as i64`
- `f64_value = q16_value as f64 / 65536.0`

## EM Algorithm

### E-Step (Expectation)

Compute responsibilities for each sample:

```
r_nk = (pi_k * N(x_n | mu_k, sigma^2_k)) / sum_j(pi_j * N(x_n | mu_j, sigma^2_j))
```

### M-Step (Maximization)

Update parameters:

```
N_k = sum_n(r_nk)
mu_k = (1/N_k) * sum_n(r_nk * x_n)
sigma^2_k = (1/N_k) * sum_n(r_nk * (x_n - mu_k)^2)
pi_k = N_k / N
```

### Convergence

Training stops when:
1. Log-likelihood change < threshold, OR
2. Maximum iterations reached

## Integration with GMMCapsule

Load the trained model into `GMMCapsule`:

```rust
use std::fs::File;
use std::io::Read;

fn load_gmm_binary(path: &str) -> GMMCapsule {
    let mut file = File::open(path).unwrap();
    let mut header = [0u8; 16];
    file.read_exact(&mut header).unwrap();

    // Verify magic and version
    let magic = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    assert_eq!(magic, 0x474D4D43, "Invalid magic number");

    let num_components = u32::from_le_bytes([header[8], header[9], header[10], header[11]]);

    let mut capsule = GMMCapsule::with_components(num_components as u8);

    // Load components
    for i in 0..num_components as usize {
        let mut comp = [0u8; 32];
        file.read_exact(&mut comp).unwrap();

        let weight = i64::from_le_bytes(comp[0..8].try_into().unwrap());
        let mean = i64::from_le_bytes(comp[8..16].try_into().unwrap());
        let variance = i64::from_le_bytes(comp[16..24].try_into().unwrap());
        let inv_variance = i64::from_le_bytes(comp[24..32].try_into().unwrap());

        capsule.components[i].weight_q16_16.store(weight, Ordering::Relaxed);
        capsule.components[i].mean_q16_16.store(mean, Ordering::Relaxed);
        capsule.components[i].variance_q16_16.store(variance, Ordering::Relaxed);
        capsule.components[i].inv_variance_q16.store(inv_variance, Ordering::Relaxed);
    }

    capsule
}
```

## Performance

| Operation | Time |
|-----------|------|
| Load 10K samples | <10ms |
| EM iteration (8 components, 10K samples) | ~5ms |
| Full training (100 iterations) | <500ms |
| Binary export | <1ms |

## Testing

Run unit tests:

```bash
cargo test
```

Tests cover:
- Q16.16 arithmetic (conversion, multiplication, division)
- Gaussian PDF computation
- GMM initialization
- E-step responsibility computation
- M-step parameter updates
- Single and bimodal distribution fitting
- Binary export format

## Files

```
tools/train_gmm/
├── Cargo.toml          # Project configuration
├── README.md           # This file
├── samples.csv         # Example input data
└── src/
    └── main.rs         # EM algorithm + CLI (400+ lines)
```

## License

Proprietary - Kindly Systems

## Related

- `atomic_capsule::protection::GMMCapsule` - Runtime GMM capsule
- `atomic_capsule::protection::AnomalyDetectorV2` - Uses GMM for Layer 2 detection
- `atomic_capsule::serialize::fixed_point::Q16_16` - Fixed-point arithmetic
