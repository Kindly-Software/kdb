#!/bin/bash
#
# build.sh - HIP Motion Estimation Kernel Build Script
#
# [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
#
# Quick build script for motion estimation GPU kernels.
# Must be run on system with ROCm 6.0+ installed (kindly-hub).
#
# Usage:
#   ./build.sh              # Build production kernel (.co)
#   ./build.sh debug        # Build debug kernel (with symbols)
#   ./build.sh asm          # Generate assembly output
#   ./build.sh clean        # Remove build artifacts
#   ./build.sh verify       # Verify build environment
#
# Remote Usage (from local machine):
#   ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1/kernels && ./build.sh"
#

set -euo pipefail

# Configuration
KERNEL_SOURCE="motion_estimation.hip"
KERNEL_NAME="motion_estimation_sad_kernel"
TARGET_ARCH="gfx1035"  # AMD Radeon 680M (RDNA2)
OUTPUT_DIR="."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Verify ROCm installation
verify_rocm() {
    log_info "Verifying ROCm installation..."

    if ! command -v hipcc &> /dev/null; then
        log_error "hipcc not found. Please install ROCm 6.0+ or add to PATH."
        echo "  export PATH=/opt/rocm/bin:\$PATH"
        exit 1
    fi

    if ! command -v rocm-smi &> /dev/null; then
        log_error "rocm-smi not found. ROCm may not be properly installed."
        exit 1
    fi

    # Check ROCm version
    ROCM_VERSION=$(rocm-smi --showversion 2>&1 | grep -oP 'ROCm version: \K[\d.]+' || echo "unknown")
    log_info "ROCm version: $ROCM_VERSION"

    # Check HIP version
    HIP_VERSION=$(hipcc --version 2>&1 | grep -oP 'HIP version: \K[\d.]+' || echo "unknown")
    log_info "HIP version: $HIP_VERSION"

    # Verify GPU is available
    if rocm-smi --showmeminfo &> /dev/null; then
        log_info "GPU detected: $(rocm-smi --showproductname 2>&1 | head -1)"
    else
        log_warn "No GPU detected. Build will succeed but kernel cannot be tested."
    fi

    log_info "ROCm verification complete."
}

# Build production kernel (code object)
build_production() {
    log_info "Building production kernel (.co)..."

    hipcc --genco \
        -O3 \
        --amdgpu-target=$TARGET_ARCH \
        -ffast-math \
        -o "$OUTPUT_DIR/motion_estimation.co" \
        "$KERNEL_SOURCE"

    if [ $? -eq 0 ]; then
        log_info "Production build complete: $OUTPUT_DIR/motion_estimation.co"
        ls -lh "$OUTPUT_DIR/motion_estimation.co"
    else
        log_error "Production build failed."
        exit 1
    fi
}

# Build debug kernel (with symbols)
build_debug() {
    log_info "Building debug kernel (.out)..."

    hipcc \
        -O0 \
        -g \
        --offload-arch=$TARGET_ARCH \
        -o "$OUTPUT_DIR/motion_estimation_debug.out" \
        "$KERNEL_SOURCE"

    if [ $? -eq 0 ]; then
        log_info "Debug build complete: $OUTPUT_DIR/motion_estimation_debug.out"
        ls -lh "$OUTPUT_DIR/motion_estimation_debug.out"
    else
        log_error "Debug build failed."
        exit 1
    fi
}

# Generate assembly output
build_assembly() {
    log_info "Generating assembly output (.s)..."

    hipcc -S \
        -O3 \
        --offload-arch=$TARGET_ARCH \
        -ffast-math \
        -o "$OUTPUT_DIR/motion_estimation.s" \
        "$KERNEL_SOURCE"

    if [ $? -eq 0 ]; then
        log_info "Assembly generation complete: $OUTPUT_DIR/motion_estimation.s"
        log_info "View with: less $OUTPUT_DIR/motion_estimation.s"
    else
        log_error "Assembly generation failed."
        exit 1
    fi
}

# Verify build output
verify_build() {
    log_info "Verifying kernel build..."

    if [ ! -f "$OUTPUT_DIR/motion_estimation.co" ]; then
        log_error "Kernel binary not found. Run './build.sh' first."
        exit 1
    fi

    # Check file format
    FILE_TYPE=$(file "$OUTPUT_DIR/motion_estimation.co" | grep -oP 'ELF.*' || echo "unknown")
    log_info "Binary format: $FILE_TYPE"

    # Check for kernel symbol
    if nm "$OUTPUT_DIR/motion_estimation.co" 2>/dev/null | grep -q "$KERNEL_NAME"; then
        log_info "Kernel symbol found: $KERNEL_NAME"
    else
        log_warn "Kernel symbol not found in binary. This may indicate a build issue."
    fi

    # Check file size (should be ~10-50KB)
    SIZE=$(stat -c%s "$OUTPUT_DIR/motion_estimation.co")
    SIZE_KB=$((SIZE / 1024))
    log_info "Binary size: ${SIZE_KB}KB"

    if [ $SIZE_KB -lt 5 ]; then
        log_warn "Binary unusually small (<5KB). May be incomplete."
    elif [ $SIZE_KB -gt 100 ]; then
        log_warn "Binary unusually large (>100KB). May include debug symbols."
    fi

    log_info "Build verification complete."
}

# Clean build artifacts
clean() {
    log_info "Cleaning build artifacts..."

    rm -f "$OUTPUT_DIR/motion_estimation.co"
    rm -f "$OUTPUT_DIR/motion_estimation.out"
    rm -f "$OUTPUT_DIR/motion_estimation_debug.out"
    rm -f "$OUTPUT_DIR/motion_estimation.s"
    rm -f "$OUTPUT_DIR/*.o"

    log_info "Clean complete."
}

# Main script logic
main() {
    case "${1:-production}" in
        production|prod|release)
            verify_rocm
            build_production
            verify_build
            ;;
        debug)
            verify_rocm
            build_debug
            ;;
        asm|assembly)
            verify_rocm
            build_assembly
            ;;
        verify)
            verify_rocm
            verify_build
            ;;
        clean)
            clean
            ;;
        help|--help|-h)
            echo "Usage: $0 [production|debug|asm|verify|clean|help]"
            echo ""
            echo "Commands:"
            echo "  production  Build production kernel (.co) [default]"
            echo "  debug       Build debug kernel with symbols (.out)"
            echo "  asm         Generate assembly output (.s)"
            echo "  verify      Verify ROCm installation and build"
            echo "  clean       Remove build artifacts"
            echo "  help        Show this help message"
            echo ""
            echo "Examples:"
            echo "  ./build.sh              # Build production kernel"
            echo "  ./build.sh debug        # Build debug kernel"
            echo "  ./build.sh verify       # Check environment"
            echo ""
            echo "Remote build:"
            echo "  ssh samuel@kindly-hub \"cd ~/Primitives/kindly-av1/kernels && ./build.sh\""
            ;;
        *)
            log_error "Unknown command: $1"
            echo "Run './build.sh help' for usage."
            exit 1
            ;;
    esac
}

# Run main with arguments
main "$@"
