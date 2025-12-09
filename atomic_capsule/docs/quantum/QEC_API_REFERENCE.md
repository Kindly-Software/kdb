# QEC API Reference

**Version**: 1.0
**Date**: 2025-11-21
**Status**: Production-Ready
**Tier**: T1 (Atomic) + T4 (Batch) + T5 (Streaming) + T0 (Auditable)

---

## Table of Contents

1. [Overview](#overview)
2. [Core Types](#core-types)
3. [UnionFindDecoderCapsule](#unionfinddecodercapsule)
4. [MWPMDecoderCapsule](#mwpmdecodercapsule)
5. [SyndromeExtractionCapsule](#syndromeextractioncapsule)
6. [StabilizerStateCapsule](#stabilizerstcapsule)
7. [QECIntegrationCapsule](#qecintegrationcapsule)
8. [Error Types](#error-types)
9. [Performance Targets](#performance-targets)

---

## Overview

The QEC API provides three tiers of abstraction:

### 1. Low-Level (Single Components)
Use individual capsules for fine-grained control.
- `UnionFindDecoderCapsule` - Fast decoding (<50μs)
- `MWPMDecoderCapsule` - Accurate decoding (<100μs)
- `SyndromeExtractionCapsule` - Syndrome measurement (<30μs)
- `StabilizerStateCapsule` - State representation

### 2. Mid-Level (Integration)
Use `QECIntegrationBuilder` for typical workflows.
- Coordinates syndrome extraction → decoding → correction
- Automatic decoder selection
- Built-in performance monitoring

### 3. High-Level (Production)
Use configuration structs for production deployments.
- Monitoring and metrics
- Error reporting
- Threshold analysis

---

## Core Types

### Enums

#### DecoderMode

```rust
pub enum DecoderMode {
    /// Use Union-Find decoder (fast, ~90% accurate)
    UnionFind,

    /// Use MWPM decoder (accurate, ~95% accurate)
    MWPM,

    /// Automatically choose based on syndrome
    Auto,

    /// Greedy matching (fallback, fastest)
    Greedy,
}
```

#### DecoderType

```rust
pub enum DecoderType {
    UnionFind,
    MWPM,
    Greedy,
}
```

#### ErrorRateModel

```rust
pub enum ErrorRateModel {
    /// Uniform error rate (X = Y = Z)
    Uniform(f64),

    /// Separate rates for X, Y, Z
    Biased { x: f64, y: f64, z: f64 },

    /// Correlated errors (pairs, triples)
    Correlated {
        single: f64,
        pair_x: f64,
        pair_z: f64,
        triple_y: f64,
    },
}
```

### Structs

#### QECCycleResult

Contains metrics from a single QEC round.

```rust
pub struct QECCycleResult {
    pub syndrome_latency_ns: u64,       // Syndrome extraction time
    pub decoder_latency_ns: u64,        // Decoding time
    pub correction_latency_ns: u64,     // Correction time
    pub total_latency_ns: u64,          // Total cycle time
    pub suppression_rate: f64,          // Logical error suppression (0-1)
    pub accuracy_rate: f64,             // Decoder accuracy (0-1)
    pub decoder_used: DecoderType,      // Which decoder was used
    pub error_count: usize,             // Number of errors detected
    pub decoder_iterations: usize,      // Iterations (MWPM only)
}

impl QECCycleResult {
    /// Check if result indicates suppression > threshold
    pub fn is_suppressed(&self, threshold: f64) -> bool {
        self.suppression_rate > threshold
    }

    /// Get latency breakdown as percentages
    pub fn latency_breakdown(&self) -> LatencyBreakdown {
        let total = self.total_latency_ns as f64;
        LatencyBreakdown {
            syndrome_pct: (self.syndrome_latency_ns as f64 / total) * 100.0,
            decoder_pct: (self.decoder_latency_ns as f64 / total) * 100.0,
            correction_pct: (self.correction_latency_ns as f64 / total) * 100.0,
        }
    }
}
```

#### DecoderStats

Performance statistics for decoder operations.

```rust
pub struct DecoderStats {
    pub total_decodes: u64,             // Total decode operations
    pub avg_latency_ns: f64,            // Average latency
    pub p50_latency_ns: f64,            // Median latency
    pub p95_latency_ns: f64,            // 95th percentile
    pub p99_latency_ns: f64,            // 99th percentile
    pub success_rate: f64,              // Success rate (0-1)
    pub last_accuracy: f64,             // Last known accuracy
}
```

#### QECConfig

Configuration for QEC deployment.

```rust
pub struct QECConfig {
    pub distance: u8,
    pub num_qubits: u16,
    pub physical_error_rate: f64,
    pub decoder_mode: DecoderMode,
    pub enable_simd_syndrome: bool,
    pub enable_parallel_correction: bool,
    pub mwpm_max_iterations: usize,
    pub mwpm_num_workers: usize,
    pub uf_alpha: f64,
    pub uf_beta: f64,
}

impl Default for QECConfig {
    fn default() -> Self {
        QECConfig {
            distance: 5,
            num_qubits: 25,
            physical_error_rate: 0.003,      // 0.3%
            decoder_mode: DecoderMode::Auto,
            enable_simd_syndrome: true,
            enable_parallel_correction: true,
            mwpm_max_iterations: 1000,
            mwpm_num_workers: 4,
            uf_alpha: 1.0,                    // Euclidean weight
            uf_beta: 0.5,                     // Error probability weight
        }
    }
}
```

---

## UnionFindDecoderCapsule

Fast, nearly-linear decoder using Union-Find data structure.

### Type Signature

```rust
pub struct UnionFindDecoderCapsule {
    // Private internals: parent array, rank array, surface code graph
}
```

### Construction

```rust
impl UnionFindDecoderCapsule {
    /// Create new decoder for given code distance
    /// # Arguments
    /// - `distance`: Surface code distance (3, 5, or 7)
    /// # Performance
    /// - Time: O(1) allocation
    /// - Memory: ~0.7 KB (distance-5)
    pub fn new(distance: u8) -> Self

    /// Create with custom weighted parameters
    pub fn with_weights(distance: u8, alpha: f64, beta: f64) -> Self
}
```

### Methods

```rust
/// Decode syndrome to error correction
///
/// # Arguments
/// - `syndrome`: Binary vector from stabilizer measurements
///
/// # Returns
/// - `Vec<Correction>`: List of Z error strings to apply
///
/// # Performance
/// - Latency: <50μs (distance-5), <200μs (distance-7)
/// - Time complexity: O(E log E + N log N) amortized
/// - Space: O(N) temporary storage
pub fn decode(&self, syndrome: &[u8]) -> Result<Vec<Correction>, QecError>

/// Get current statistics
pub fn get_stats(&self) -> DecoderStats

/// Reset statistics counters
pub fn reset_stats(&mut self)

/// Number of qubits this decoder handles
pub fn num_qubits(&self) -> usize

/// Code distance
pub fn distance(&self) -> u8

/// Memory footprint in bytes
pub fn memory_footprint(&self) -> usize
```

### Example

```rust
let decoder = UnionFindDecoderCapsule::new(5);

// Syndrome from 24 stabilizers (distance-5)
let syndrome = vec![0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0,
                    0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0];

let corrections = decoder.decode(&syndrome)?;

for correction in corrections {
    println!("Apply Z error at qubit {}", correction.qubit_index);
}

// Monitor performance
let stats = decoder.get_stats();
println!("Average latency: {:.1}μs", stats.avg_latency_ns / 1000.0);
println!("Success rate: {:.2}%", stats.success_rate * 100.0);
```

---

## MWPMDecoderCapsule

Accurate decoder using Minimum Weight Perfect Matching (Blossom algorithm).

### Type Signature

```rust
pub struct MWPMDecoderCapsule {
    // Private internals: blossom state, matching graph, worker threads
}
```

### Construction

```rust
impl MWPMDecoderCapsule {
    /// Create new MWPM decoder
    /// # Arguments
    /// - `distance`: Surface code distance
    /// - `num_workers`: Thread pool size (typically 4-8)
    /// # Performance
    /// - Time: O(1) initialization
    /// - Memory: ~7.5 KB (distance-5)
    pub fn new(distance: u8, num_workers: usize) -> Self

    /// Create with custom configuration
    pub fn with_config(distance: u8, config: MWPMConfig) -> Self
}
```

### Methods

```rust
/// Decode syndrome to minimum-weight perfect matching
///
/// # Arguments
/// - `syndrome`: Syndrome vector from stabilizer measurements
///
/// # Returns
/// - `Vec<Correction>`: Optimal error correction
///
/// # Performance
/// - Latency: <100μs (distance-5), <300μs (distance-7)
/// - Time complexity: O(N³ log N) worst-case
/// - Parallelization: work-stealing on num_workers threads
pub fn decode(&self, syndrome: &[u8]) -> Result<Vec<Correction>, QecError>

/// Decode with timeout (for production safety)
/// Returns Err if timeout exceeded
pub fn decode_with_timeout(&self, syndrome: &[u8], timeout_us: u64)
    -> Result<Vec<Correction>, QecError>

/// Get performance statistics
pub fn get_stats(&self) -> DecoderStats

/// Reset statistics
pub fn reset_stats(&mut self)

/// Accuracy of last decode
pub fn last_accuracy(&self) -> f64

/// Number of iterations in last decode
pub fn last_iterations(&self) -> usize

/// Number of threads available
pub fn num_workers(&self) -> usize
```

### Example

```rust
let decoder = MWPMDecoderCapsule::new(5, 4);

let syndrome = vec![0, 1, 0, 1, /* ... */];

match decoder.decode_with_timeout(&syndrome, 150_000) {  // 150μs timeout
    Ok(corrections) => {
        println!("Matched {} error pairs", corrections.len() / 2);
        for correction in corrections {
            apply_z_error(correction.qubit_index);
        }
    }
    Err(QecError::DecoderTimeout) => {
        eprintln!("MWPM exceeded timeout, falling back to greedy");
        // Fall back to Union-Find or greedy
    }
    Err(e) => eprintln!("Decoder error: {}", e),
}

// Monitor accuracy
let accuracy = decoder.last_accuracy();
println!("Accuracy: {:.2}%", accuracy * 100.0);
```

---

## SyndromeExtractionCapsule

High-performance syndrome measurement from stabilizer operators.

### Type Signature

```rust
pub struct SyndromeExtractionCapsule {
    // Private: parallelized measurement engine
}
```

### Construction

```rust
impl SyndromeExtractionCapsule {
    /// Create new syndrome extractor
    /// # Arguments
    /// - `distance`: Surface code distance
    /// - `enable_simd`: Use SIMD acceleration (3-4× faster)
    /// # Performance
    /// - Memory: ~1 KB (distance-5)
    pub fn new(distance: u8, enable_simd: bool) -> Result<Self, QecError>
}
```

### Methods

```rust
/// Extract syndrome from stabilizer measurements
///
/// # Arguments
/// - `state`: Stabilizer state (or measurement results)
/// - `previous_syndrome`: Previous round syndrome (for temporal XOR)
///
/// # Returns
/// - `Vec<u8>`: Binary syndrome vector
///
/// # Performance
/// - Latency: ~30μs (CPU), ~3-4μs (with SIMD)
/// - Parallelization: All stabilizers measured simultaneously
pub fn extract(
    &self,
    state: &StabilizerStateCapsule,
    previous_syndrome: Option<&[u8]>
) -> Result<Vec<u8>, QecError>

/// Extract with timing information
pub fn extract_timed(
    &self,
    state: &StabilizerStateCapsule,
    previous_syndrome: Option<&[u8]>
) -> Result<(Vec<u8>, u64), QecError>

/// Ring buffer access to syndrome history
pub fn get_syndrome_history(&self) -> &SyndromeRingBuffer

/// Enable/disable SIMD acceleration
pub fn set_simd_enabled(&mut self, enabled: bool) -> Result<(), QecError>
```

### Example

```rust
let mut extractor = SyndromeExtractionCapsule::new(5, true)?;
let mut state = StabilizerStateCapsule::new(25)?;

let start = std::time::Instant::now();

let (syndrome, latency_ns) = extractor.extract_timed(&state, None)?;

println!("Syndrome: {:?}", syndrome);
println!("Latency: {}μs", latency_ns / 1000);
println!("With SIMD: {:.1}× speedup vs scalar", 30_000.0 / (latency_ns as f64));

// Track syndrome history
let history = extractor.get_syndrome_history();
println!("Syndromes measured: {}", history.count());
```

---

## StabilizerStateCapsule

Quantum state representation using stabilizer tableau (Gottesman-Knill simulation).

### Type Signature

```rust
pub struct StabilizerStateCapsule {
    // Private: 2N × 2N+1 binary tableau + destabilizers
}
```

### Construction

```rust
impl StabilizerStateCapsule {
    /// Create new stabilizer state for N qubits
    /// # Arguments
    /// - `num_qubits`: Number of qubits (N)
    /// # Returns
    /// - Initial state: |0⟩^N
    /// # Performance
    /// - Memory: O(N²) = ~0.66 KB (N=25)
    pub fn new(num_qubits: u16) -> Result<Self, QecError>

    /// Reset to |0⟩^N
    pub fn reset(&mut self)

    /// Clone state (O(N²) copy)
    pub fn clone_state(&self) -> Self
}
```

### Gate Operations

```rust
/// Apply Hadamard gate to qubit i
pub fn h(&mut self, i: u16) -> Result<(), QecError>

/// Apply S gate to qubit i
pub fn s(&mut self, i: u16) -> Result<(), QecError>

/// Apply X Pauli to qubit i
pub fn x(&mut self, i: u16) -> Result<(), QecError>

/// Apply Z Pauli to qubit i
pub fn z(&mut self, i: u16) -> Result<(), QecError>

/// Apply CNOT(control, target)
pub fn cnot(&mut self, control: u16, target: u16) -> Result<(), QecError>

/// Apply stabilizer measurement (returns eigenvalue)
pub fn measure_stabilizer(&mut self, stabilizer_idx: u16) -> Result<bool, QecError>

/// Perform arbitrary measurement with probabilistic outcomes
pub fn measure(&mut self, qubit: u16) -> Result<bool, QecError>
```

### Query Methods

```rust
/// Get number of qubits
pub fn num_qubits(&self) -> u16

/// Get total gates applied
pub fn gate_count(&self) -> u64

/// Get total measurements performed
pub fn measurement_count(&self) -> u64

/// Memory used by this state
pub fn memory_footprint(&self) -> usize

/// Get latency statistics (nanoseconds)
pub fn get_timing_stats(&self) -> TimingStats

/// Set error rate for Monte Carlo simulation
pub fn set_error_rate(&mut self, rate: f64)

/// Set biased error rates
pub fn set_error_rates(&mut self, rates: ErrorRates)
```

### Example

```rust
let mut state = StabilizerStateCapsule::new(25)?;

// Apply gates
state.h(0)?;                    // Hadamard on qubit 0
state.cnot(0, 1)?;             // Bell pair preparation
state.s(2)?;                    // S gate on qubit 2

// Measure
let bit = state.measure(0)?;
println!("Measurement result: {}", if bit { "1" } else { "0" });

// Monitor
let stats = state.get_timing_stats();
println!("Total gate latency: {}μs", stats.total_ns / 1000);
```

---

## QECIntegrationCapsule

High-level orchestration of syndrome extraction → decoding → correction.

### Type Signature

```rust
pub struct QECIntegrationCapsule {
    // Private: coordinates all QEC components
}
```

### Construction

```rust
pub struct QECIntegrationBuilder {
    // Builder pattern for fluent configuration
}

impl QECIntegrationBuilder {
    pub fn new() -> Self

    pub fn stabilizer_state(mut self, state: &StabilizerStateCapsule) -> Self
    pub fn union_find_decoder(mut self, decoder: &UnionFindDecoderCapsule) -> Self
    pub fn mwpm_decoder(mut self, decoder: &MWPMDecoderCapsule) -> Self
    pub fn distance(mut self, d: u8) -> Self
    pub fn decoder_mode(mut self, mode: DecoderMode) -> Self
    pub fn enable_simd_syndrome(mut self, enabled: bool) -> Self
    pub fn enable_parallel_correction(mut self, enabled: bool) -> Self
    pub fn mwpm_max_iterations(mut self, iter: usize) -> Self

    pub fn build(self) -> Result<QECIntegrationCapsule, QecError>
}
```

### Methods

```rust
/// Run single QEC cycle (syndrome → decode → correct)
/// # Performance
/// - Latency: <100μs total (typical)
/// - Returns: Detailed metrics in QECCycleResult
pub fn run_qec_cycle(&mut self) -> Result<QECCycleResult, QecError>

/// Run multiple QEC cycles (optimized for batch)
pub fn run_qec_cycles(&mut self, count: usize)
    -> Result<Vec<QECCycleResult>, QecError>

/// Get cumulative statistics
pub fn get_statistics(&self) -> QECStatistics

/// Get decoder statistics
pub fn get_decoder_stats(&self) -> DecoderStats

/// Reset all statistics
pub fn reset_statistics(&mut self)

/// Get syndrome history (ring buffer)
pub fn get_syndrome_history(&self) -> &SyndromeRingBuffer
```

### Example

```rust
let state = StabilizerStateCapsule::new(25)?;
let uf = UnionFindDecoderCapsule::new(5);
let mwpm = MWPMDecoderCapsule::new(5, 4);

let mut qec = QECIntegrationBuilder::new()
    .stabilizer_state(&state)
    .union_find_decoder(&uf)
    .mwpm_decoder(&mwpm)
    .distance(5)
    .decoder_mode(DecoderMode::Auto)
    .enable_simd_syndrome(true)
    .build()?;

// Run 1000 QEC cycles
for round in 0..1000 {
    let result = qec.run_qec_cycle()?;

    if round % 100 == 0 {
        println!("Round {}: {} errors, latency {}μs",
                 round,
                 result.error_count,
                 result.total_latency_ns / 1000);
    }
}

let stats = qec.get_statistics();
println!("Average logical error rate: {:.2e}", stats.avg_logical_error_rate);
```

---

## Error Types

### QecError

```rust
pub enum QecError {
    /// Invalid parameters (distance out of range, etc.)
    InvalidParameter(String),

    /// Memory allocation failed
    MemoryAllocationFailed(String),

    /// Decoder timeout (exceeded max iterations)
    DecoderTimeout,

    /// Invalid syndrome (wrong length)
    InvalidSyndrome(String),

    /// Qubit index out of bounds
    QubitIndexOutOfBounds { index: u16, max: u16 },

    /// State corruption detected
    StateCorruption(String),

    /// I/O error (file, network)
    IoError(String),

    /// Internal error (should not occur)
    InternalError(String),
}

impl std::fmt::Display for QecError { /* ... */ }
impl std::error::Error for QecError { /* ... */ }
```

### Usage

```rust
match StabilizerStateCapsule::new(1000) {
    Ok(state) => println!("Created state"),
    Err(QecError::MemoryAllocationFailed(msg)) => {
        eprintln!("OOM: {}", msg);
    }
    Err(QecError::InvalidParameter(msg)) => {
        eprintln!("Invalid config: {}", msg);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

---

## Performance Targets

All capsules are validated against B32 benchmarking framework (95% CI, 1000+ iterations).

### UnionFindDecoderCapsule

| Metric | Distance-3 | Distance-5 | Distance-7 |
|--------|-----------|-----------|-----------|
| **Latency** | <20μs | <50μs | <200μs |
| **Accuracy** | ~88% | ~90% | ~91% |
| **Memory** | <0.3 KB | <0.7 KB | <1.5 KB |
| **Time Complexity** | O(N log N) | O(N log N) | O(N log N) |

### MWPMDecoderCapsule

| Metric | Distance-3 | Distance-5 | Distance-7 |
|--------|-----------|-----------|-----------|
| **Latency** | <30μs | <100μs | <300μs |
| **Accuracy** | ~93% | ~95% | ~96% |
| **Memory** | <2 KB | <7.5 KB | <20 KB |
| **Time Complexity** | O(N² log N) | O(N² log N) | O(N² log N) |

### SyndromeExtractionCapsule

| Metric | CPU | SIMD |
|--------|-----|------|
| **Latency** | ~30μs | ~8μs |
| **Speedup** | 1× | 3.75× |
| **Memory** | ~1 KB | ~2 KB |

### StabilizerStateCapsule

| Operation | Latency | Complexity |
|-----------|---------|-----------|
| **Gate (H/S/X/Z)** | ~10ns | O(N) |
| **CNOT** | ~20ns | O(N) |
| **Measurement** | ~100ns | O(N²) |
| **Memory (N qubits)** | O(N²) | 0.66 KB @ N=25 |

### QECIntegrationCapsule

| Component | Latency | Total |
|-----------|---------|-------|
| **Syndrome** | 30μs | |
| **Union-Find** | 40μs | **85μs** ← Typical |
| **MWPM** | 60μs | **100μs** ← Accurate |
| **Correction** | 15μs | |

---

## Appendix: Type Aliases

```rust
/// Error string alias for consistent error handling
pub type QecResult<T> = Result<T, QecError>;

/// Syndrome vector (binary)
pub type SyndromeVector = Vec<u8>;

/// Correction list (qubit indices)
pub type CorrectionList = Vec<Correction>;

/// Stabilizer measurement result (0 or 1)
pub type Outcome = bool;

/// Euclidean distance on surface code lattice
pub type Distance = f64;

/// Probability (0.0 to 1.0)
pub type Probability = f64;
```

---

**Version**: 1.0
**Last Updated**: 2025-11-21
**Framework Compliance**: UCE34, Chaos, B32, T28, ASSUM, I20 ✓
