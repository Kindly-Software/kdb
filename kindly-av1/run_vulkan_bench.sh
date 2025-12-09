#!/usr/bin/env bash
# Run Vulkan GPU benchmarks on kindly-hub
# Tests both AMD Radeon 680M (GPU0) and NVIDIA RTX 3080 Mobile (GPU1)

set -euo pipefail

# Source Cargo environment
source ~/.cargo/env

echo "========================================================================"
echo "  Vulkan GPU Benchmark Runner"
echo "  kindly-hub: AMD Radeon 680M + NVIDIA RTX 3080 Mobile"
echo "========================================================================"
echo ""

# Check Vulkan availability
echo "=== Vulkan Devices ==="
vulkaninfo --summary 2>&1 | grep -A 5 "GPU0:" || true
vulkaninfo --summary 2>&1 | grep -A 5 "GPU1:" || true
echo ""

# Build with Vulkan features
echo "=== Building with gpu-vulkan feature ==="
cd ~/Primitives/kindly-av1
cargo build --release --features gpu-vulkan 2>&1 | tail -20
echo ""

# Run GPU motion benchmarks (CPU baseline only, GPU not available yet)
echo "=== Running GPU Motion Benchmarks ==="
cargo bench --bench gpu_motion_bench --no-fail-fast 2>&1 | tail -50
echo ""

echo "========================================================================"
echo "  Benchmark complete!"
echo "  Results saved to: target/criterion/"
echo "========================================================================"
