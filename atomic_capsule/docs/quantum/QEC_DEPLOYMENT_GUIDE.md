# QEC Deployment Guide

**Version**: 1.0
**Date**: 2025-11-21
**Status**: Production-Ready
**Platform Support**: Linux, macOS, Windows (with MSVC)

---

## Table of Contents

1. [Hardware Requirements](#hardware-requirements)
2. [Software Requirements](#software-requirements)
3. [Build Configuration](#build-configuration)
4. [Installation](#installation)
5. [Platform-Specific Setup](#platform-specific-setup)
6. [Integration with QPU Frameworks](#integration-with-qpu-frameworks)
7. [Performance Tuning](#performance-tuning)
8. [Monitoring & Metrics](#monitoring--metrics)
9. [Troubleshooting](#troubleshooting)
10. [Production Checklist](#production-checklist)

---

## Hardware Requirements

### Minimum Specifications

```
CPU:    x86_64 with SSE4.2 (Intel Core 2008+, AMD Phenom II+)
        OR aarch64 with NEON (ARM Cortex-A53+)
        OR WebAssembly (wasm32-unknown-unknown)

RAM:    1 GB minimum
        4 GB recommended for production (distance-7)
        16 GB for research/large simulations (distance-9+)

Storage: 10 MB for binary
         100 MB for data/logs
         500 MB for benchmarks

Network: Optional (for distributed QEC)
```

### Recommended Specifications (Production)

```
CPU:    Intel Xeon (Skylake+) or AMD EPYC (Naples+)
        8+ cores recommended for parallel decoding
        AVX2 required for SIMD syndrome extraction

RAM:    32 GB (supports distance-9, multiple QEC instances)

Storage: SSD (NVMe preferred) for logging
         1 TB for persistent dedup benchmarks

Network: 1 Gbps+ for distributed coordination
         <50ms latency between nodes
```

### Hardware Acceleration (Optional)

```
FPGA:   Xilinx Virtex UltraScale+ or higher
        Altera Stratix 10 or higher
        Budget: $5K-$10K
        Speedup: 8-21× (Phase Q3.7)

GPU:    NVIDIA A100 (40GB PCIe)
        OR AMD MI200 (120GB HBM)
        Budget: $10K-$40K
        Speedup: 100-1000× (Phase Q3.7)
```

---

## Software Requirements

### Rust Toolchain

```bash
# Minimum version
rustc 1.70+ (stable)
rustup update

# Recommended for best performance
rustc 1.80+ (stable with latest LLVM)
rust nightly 2025-11-21+ (for portable_simd)
```

### Feature Dependencies

#### Core (No Dependencies)

- `std` library (always included)
- `atomic` intrinsics (CPU ISA)

#### SIMD Acceleration (Nightly)

```bash
rustup install nightly
rustup component add rust-src --toolchain nightly

# Install portable_simd (when stabilized in core)
# Or use feature flag: features = ["portable_simd"]
```

#### Platform-Specific

```
Linux:   glibc 2.29+ (Ubuntu 20.04+, Fedora 31+)
macOS:   macOS 11+ (Big Sur)
Windows: Windows 10 21H2+ with MSVC (not MinGW)
WASM:    Emscripten SDK 3.1.0+ or Wasm-ld (LLVM 15+)
```

---

## Build Configuration

### Cargo.toml

```toml
[package]
name = "my-qec-app"
version = "0.1.0"
edition = "2021"

[dependencies]
# Core QEC
atomic_capsule = { version = "0.7.0", features = [
    "quantum-pure",      # Pure Rust baseline
    "quantum-qec",       # QEC-specific capsules
] }

# SIMD acceleration (optional)
# Uncomment for 3-4× speedup
# atomic_capsule = { version = "0.7.0", features = [
#     "quantum-simd",     # SIMD gates
#     "portable_simd",    # nightly feature
# ] }

# Production features
[features]
default = ["release-optimized"]
release-optimized = []
dev = []

# Platform-specific
[target.'cfg(target_family = "wasm")'.dependencies]
# WASM-specific deps (if any)

# Performance settings
[profile.release]
opt-level = 3           # Maximum optimization
lto = "fat"             # Link-time optimization
codegen-units = 1       # Single codegen unit for better optimization
strip = true            # Strip symbols for smaller binary
panic = "abort"         # Faster panic handling

# Nightly-only optimizations (if supported)
# [profile.release]
# cargo-features = ["build-std"]
```

### Feature Matrix

```toml
[features]
# Core variants
quantum-pure = []                  # Pure Rust (no SIMD)
quantum-simd = ["portable_simd"]   # SIMD acceleration
quantum-qec = []                   # QEC-specific capsules

# Decoder selection (usually auto-selected)
decoder-union-find = []
decoder-mwpm = []
decoder-greedy = []

# Platform support
platform-x86-64 = []
platform-aarch64 = []
platform-wasm = []

# Production features
audit-trail = []                   # Q34 audit logging
metrics = []                       # Prometheus metrics
distributed = []                   # Multi-node QEC

# Presets (convenient combinations)
preset-dev = ["quantum-pure", "decoder-union-find"]
preset-prod = ["quantum-simd", "decoder-mwpm", "audit-trail", "metrics"]
preset-wasm = ["quantum-pure", "decoder-union-find"]  # WASM-compatible only
```

---

## Installation

### Option 1: From crates.io (Recommended)

```bash
# Create new project
cargo new my-qec-project
cd my-qec-project

# Add dependency
cargo add atomic_capsule --features quantum-pure,quantum-qec

# Build and run
cargo build --release
cargo run --release
```

### Option 2: From Git (Latest Features)

```bash
# Use git dependency in Cargo.toml
[dependencies]
atomic_capsule = { git = "https://github.com/anthropics/atomic_capsule.git",
                   rev = "a44b712",
                   features = ["quantum-pure", "quantum-qec"] }

cargo build --release
```

### Option 3: Local Development

```bash
# Clone repository
git clone https://github.com/anthropics/atomic_capsule.git
cd atomic_capsule

# Link as path dependency
# In your Cargo.toml:
# [dependencies]
# atomic_capsule = { path = "../atomic_capsule", features = ["quantum-pure", "quantum-qec"] }

cargo build --release
```

### Verification

```bash
# Check build succeeds
cargo test --lib --features quantum-pure,quantum-qec

# Verify no warnings
cargo clippy --all-targets --features quantum-pure,quantum-qec

# Generate documentation
cargo doc --lib --features quantum-pure,quantum-qec --open
```

---

## Platform-Specific Setup

### Linux (Ubuntu 22.04+)

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install build essentials
sudo apt install -y build-essential pkg-config

# (Optional) Install LLVM for better codegen
sudo apt install -y llvm clang

# Install for SIMD support (nightly)
rustup install nightly
rustup component add rust-src --toolchain nightly

# Build and test
cargo build --release --features quantum-simd,portable_simd
cargo test --release --features quantum-simd
```

### macOS (12+)

```bash
# Install Xcode Command Line Tools (if needed)
xcode-select --install

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install Homebrew dependencies
brew install llvm pkg-config

# Build with optimizations
RUSTFLAGS="-C target-cpu=native" cargo build --release \
    --features quantum-simd,portable_simd

# Test
cargo test --release --features quantum-simd
```

### Windows 10/11 (MSVC)

```powershell
# Install Rust (use rustup)
# https://rustup.rs/ (recommended: MSVC toolchain)

# Install Visual Studio 2019+ or Build Tools for Visual Studio 2022
# https://visualstudio.microsoft.com/downloads/

# Open PowerShell as Administrator
# Add to PATH (if needed):
$env:Path += ";$env:RUSTUP_HOME\toolchains\stable-x86_64-pc-windows-msvc\bin"

# Build
cargo build --release --features quantum-simd,portable_simd

# Test
cargo test --release --features quantum-simd
```

### WebAssembly (wasm32)

```bash
# Install WASM target
rustup target add wasm32-unknown-unknown

# Build WASM binary
cargo build --target wasm32-unknown-unknown --release \
    --features quantum-pure

# Verify binary
ls -lh target/wasm32-unknown-unknown/release/*.wasm

# Size should be <500KB
file target/wasm32-unknown-unknown/release/my_qec.wasm

# (Optional) Optimize with wasm-opt
cargo install wasm-opt
wasm-opt -Oz -o optimized.wasm target/wasm32-unknown-unknown/release/my_qec.wasm
```

### Docker (Recommended for Production)

```dockerfile
# Dockerfile
FROM rust:latest as builder
WORKDIR /app
COPY . .
RUN cargo build --release --features quantum-simd

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /app/target/release/qec-server /usr/local/bin/

EXPOSE 8080
ENTRYPOINT ["qec-server"]
```

```bash
# Build and run
docker build -t my-qec-app .
docker run --rm -p 8080:8080 my-qec-app
```

---

## Integration with QPU Frameworks

### IBM Qiskit

```python
# Python side: send QEC syndrome to Rust backend
import subprocess
import json

syndrome = [0, 1, 0, 1, 0, 0, 1, 0]  # From quantum circuit

# Call Rust QEC decoder
result = subprocess.run(
    ['/usr/local/bin/qec-decoder', json.dumps(syndrome)],
    capture_output=True,
    text=True
)

corrections = json.loads(result.stdout)
print(f"Corrections: {corrections}")

# Apply corrections to quantum circuit
from qiskit import QuantumCircuit
qc = QuantumCircuit(25)
for qubit in corrections:
    qc.z(qubit)
```

```rust
// Rust side: QEC decoder server
use atomic_capsule::quantum::qec_integration::*;
use std::io::{self, BufRead};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let syndrome: Vec<u8> = serde_json::from_str(&line?)?;

        let decoder = UnionFindDecoderCapsule::new(5);
        let corrections = decoder.decode(&syndrome)?;

        let output = serde_json::to_string(&corrections)?;
        println!("{}", output);
    }
    Ok(())
}
```

### Google Cirq

```python
# Cirq integration: use Rust for classical QEC
import cirq
import subprocess
import json

def qec_correction(circuit: cirq.Circuit, syndrome: list) -> list:
    """Decode syndrome using Rust backend"""
    result = subprocess.run(
        ['/usr/local/bin/qec-decoder'],
        input=json.dumps(syndrome),
        capture_output=True,
        text=True
    )
    return json.loads(result.stdout)

# Example: Surface code QEC
q = cirq.GridQubit.rect(5, 5)  # 5×5 surface code
circuit = cirq.Circuit()

# Syndrome extraction (Cirq)
syndrome = [...]  # Measurement results

# Decoding (Rust backend)
corrections = qec_correction(circuit, syndrome)
print(f"Corrections from Rust: {corrections}")
```

### Rigetti PyQuil

```python
# PyQuil integration with Rust QEC
from pyquil import Program
from pyquil.gates import H, CNOT, MEASURE
import json
import subprocess

def run_qec_with_rigetti(qpu_name: str = "Aspen-M-2"):
    """Run surface code QEC on Rigetti QPU"""
    # Prepare surface code circuit (simplified)
    p = Program()

    # Syndrome extraction
    ro = p.declare('ro', 'BIT', 24)
    for i in range(24):
        p += MEASURE(i, ro[i])

    # Get syndrome
    results = qpu.run(p, [ro])
    syndrome = list(results[0])

    # Decode with Rust
    result = subprocess.run(
        ['/usr/local/bin/qec-decoder'],
        input=json.dumps(syndrome),
        capture_output=True,
        text=True
    )
    corrections = json.loads(result.stdout)

    # Apply corrections (classical post-processing)
    print(f"Corrections: {corrections}")
    return corrections
```

---

## Performance Tuning

### Compiler Flags

```bash
# Maximum performance (Release mode)
RUSTFLAGS="-C target-cpu=native -C opt-level=3" \
cargo build --release

# With LTO and link-time optimization
RUSTFLAGS="-C target-cpu=native -C opt-level=3 -C lto=fat -C codegen-units=1" \
cargo build --release

# For benchmarking (measure real performance)
cargo bench --features quantum-simd -- --ignored

# Profile with flamegraph (Linux)
cargo install flamegraph
cargo flamegraph --release --features quantum-simd -- --bench

# Windows: Use tracy profiler
# https://github.com/wolfpld/tracy
```

### Runtime Parameters

```rust
// Tune for your hardware
let config = QECConfig {
    distance: 5,
    num_qubits: 25,
    physical_error_rate: 0.003,

    // Decoder selection
    decoder_mode: DecoderMode::Auto,

    // Parallelization
    mwpm_num_workers: num_cpus::get(),  // Use all cores

    // SIMD acceleration
    enable_simd_syndrome: cfg!(any(
        target_arch = "x86_64",
        target_arch = "aarch64"
    )),

    // Hardware-specific
    #[cfg(target_arch = "x86_64")]
    use_avx2: true,
};
```

### Memory Optimization

```rust
// Reduce memory footprint
let config = QECConfig {
    distance: 3,           // Smaller code (9 qubits vs 25)

    // Disable syndrome history if not needed
    enable_syndrome_history: false,

    // Use greedy decoder (faster, lower accuracy)
    decoder_mode: DecoderMode::Greedy,
};

// Monitor memory usage
let capsule = QECIntegrationBuilder::new()
    .build()?;

let memory_kb = capsule.memory_footprint() / 1024;
println!("Memory: {} KB", memory_kb);
```

---

## Monitoring & Metrics

### Built-in Monitoring

```rust
use atomic_capsule::quantum::qec_integration::*;

let mut qec = QECIntegrationBuilder::new()
    .stabilizer_state(&state)
    .union_find_decoder(&decoder)
    .build()?;

for round in 0..10_000 {
    let result = qec.run_qec_cycle()?;

    // Log metrics every 100 cycles
    if round % 100 == 0 {
        let stats = qec.get_statistics();
        println!("Round {}: latency={}μs, errors={:.2e}",
                 round,
                 stats.avg_latency_ns / 1000,
                 stats.avg_logical_error_rate);
    }
}
```

### Prometheus Integration (Optional)

```rust
// Export metrics for Prometheus
use prometheus::{Counter, Gauge, Histogram};

lazy_static::lazy_static! {
    static ref QEC_CYCLES: Counter = Counter::new("qec_cycles_total", "Total QEC cycles").unwrap();
    static ref QEC_LATENCY: Histogram = Histogram::new("qec_latency_ns", "QEC cycle latency (ns)").unwrap();
    static ref LOGICAL_ERRORS: Counter = Counter::new("qec_logical_errors_total", "Total logical errors").unwrap();
}

// In QEC loop
let result = qec.run_qec_cycle()?;
QEC_CYCLES.inc();
QEC_LATENCY.observe(result.total_latency_ns as f64);
if result.suppression_rate < 0.5 {
    LOGICAL_ERRORS.inc();
}
```

### Distributed Tracing

```rust
use tracing::{info, warn, span, Level};

let span = span!(Level::DEBUG, "qec_cycle");
let _enter = span.enter();

info!("Starting QEC cycle");
let result = qec.run_qec_cycle()?;

if result.total_latency_ns > 100_000 {
    warn!("QEC cycle exceeded latency budget: {}μs",
          result.total_latency_ns / 1000);
}

info!("QEC cycle completed");
```

---

## Troubleshooting

### Build Issues

#### Error: "feature `portable_simd` not found"

**Cause**: Nightly Rust feature not available in stable

**Solution**:
```bash
# Use stable without SIMD
cargo build --release --features quantum-pure

# Or enable nightly
rustup install nightly
cargo +nightly build --release --features quantum-simd,portable_simd
```

#### Error: "LLVM ERROR: Could not compile with PIC relocation model"

**Cause**: Incompatible LLVM version

**Solution**:
```bash
# Update LLVM
rustup update nightly

# Or disable LTO
RUSTFLAGS="-C lto=off" cargo build --release
```

### Runtime Issues

#### Error: "Memory allocation failed"

**Cause**: Insufficient RAM for stabilizer state

**Solution**:
```bash
# Reduce code distance
let state = StabilizerStateCapsule::new(9)?;  // distance-3 instead of distance-5

# Or increase available memory
ulimit -m unlimited  # Linux
# Or use pagefile (Windows)
```

#### Error: "Decoder timeout (MWPM > 1000 iterations)"

**Cause**: Dense syndrome or high error rate

**Solution**:
```rust
// Fall back to Union-Find
.decoder_mode(DecoderMode::UnionFind)

// Or reduce error rate
state.set_error_rate(0.001);  // 0.1% instead of 0.3%
```

---

## Production Checklist

Before deploying to production, verify:

### Code Quality

- [ ] `cargo test --all --features quantum-simd` passes
- [ ] `cargo clippy` has no warnings
- [ ] `cargo doc` builds without errors
- [ ] `cargo audit` shows no security vulnerabilities

### Performance

- [ ] Latency SLA validated: <100μs per cycle
- [ ] Throughput target met: >10,000 cycles/sec
- [ ] Memory footprint acceptable: <100 MB
- [ ] CPU usage reasonable: <50% on target hardware

### Functionality

- [ ] QEC suppression rate > 90%
- [ ] Decoder accuracy > 95%
- [ ] No logical errors in 1,000 cycles (stress test)
- [ ] Error recovery working (graceful degradation)

### Monitoring

- [ ] Metrics exported (Prometheus/StatsD)
- [ ] Logs configurable (debug, info, warn, error)
- [ ] Alerts configured for error thresholds
- [ ] Dashboard available (Grafana)

### Documentation

- [ ] README with quick start
- [ ] API documentation complete
- [ ] Deployment guide updated
- [ ] Troubleshooting guide tested

### Security

- [ ] No hardcoded secrets
- [ ] Input validation on syndrome vectors
- [ ] Error handling doesn't leak information
- [ ] Audit trails enabled (Q34)

### Integration

- [ ] QPU framework integration tested (Qiskit/Cirq/PyQuil)
- [ ] Classical-quantum boundary validated
- [ ] Correction application verified
- [ ] End-to-end workflow tested

### Deployment

- [ ] Docker image builds and runs
- [ ] Kubernetes manifests ready (if applicable)
- [ ] Health checks passing
- [ ] Graceful shutdown implemented

---

## Additional Resources

### Documentation
- [QEC_USER_GUIDE.md](QEC_USER_GUIDE.md) - Usage examples and patterns
- [QEC_API_REFERENCE.md](QEC_API_REFERENCE.md) - Complete API documentation
- [STABILIZER_ALGORITHM.md](STABILIZER_ALGORITHM.md) - Mathematical foundations

### Repositories
- [atomic_capsule](https://github.com/anthropics/atomic_capsule) - Main library
- [Qiskit](https://github.com/Qiskit/qiskit) - IBM quantum framework
- [Cirq](https://github.com/quantumlib/Cirq) - Google quantum framework
- [PyQuil](https://github.com/rigetti/pyquil) - Rigetti quantum framework

### Communities
- Rust Quantum: https://github.com/rust-quantum
- Qiskit Community: https://qiskit.org/community
- Quantum Computing Stack Exchange: https://quantumcomputing.stackexchange.com

---

**Version**: 1.0
**Last Updated**: 2025-11-21
**Status**: Production-Ready ✓
**Framework Compliance**: UCE34, Chaos, B32, T28, ASSUM, I20 ✓
