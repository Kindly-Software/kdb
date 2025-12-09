#!/bin/bash
# Build atomic_capsule HTTP server for production deployment
#
# Purpose: Compile atomic_capsule with all HTTP middleware features enabled
# Features: HTTP/1.1+HTTP/2, TLS 1.3, WebSocket, Static files, CORS, CSRF, Security headers,
#           Form parsing, Validation, Cache middleware, Circuit breaker, Rate limiter, Metrics
#
# Requirements: Rust nightly toolchain (for SIMD optimizations)
# Time: ~10-20 minutes compile time (depends on CPU cores)
# Output: target/release/atomic_http_server (stripped: 10-15MB)

set -e  # Exit on any error

# ============================================================================
# Configuration
# ============================================================================

PROJECT_DIR="/home/samuel/Primitives/atomic_capsule"
BINARY_NAME="atomic_http_server"
BUILD_MODE="release"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# ============================================================================
# Helper Functions
# ============================================================================

log_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

log_success() {
    echo -e "${GREEN}✅${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}⚠${NC}  $1"
}

log_error() {
    echo -e "${RED}❌${NC} $1"
}

check_tool() {
    if ! command -v "$1" &> /dev/null; then
        log_error "$1 not found. Please install it."
        exit 1
    fi
}

# ============================================================================
# Pre-flight Checks
# ============================================================================

log_info "Running pre-flight checks..."

# Check required tools
check_tool rustup
check_tool cargo

# Ensure nightly toolchain
log_info "Checking Rust toolchain..."
if ! rustup toolchain list | grep -q "nightly"; then
    log_warn "Rust nightly not installed. Installing..."
    rustup toolchain install nightly
fi

# Set default to nightly for this build
rustup default nightly
log_success "Using Rust nightly: $(rustc +nightly --version)"

# Change to project directory
if [ ! -d "$PROJECT_DIR" ]; then
    log_error "Project directory not found: $PROJECT_DIR"
    exit 1
fi

cd "$PROJECT_DIR"
log_success "Working directory: $(pwd)"

# ============================================================================
# Verify Cargo.toml
# ============================================================================

log_info "Verifying Cargo.toml..."
if ! grep -q "name = \"atomic_capsule\"" Cargo.toml; then
    log_error "Invalid Cargo.toml: not atomic_capsule project"
    exit 1
fi

log_success "Cargo.toml verified"

# ============================================================================
# Feature Set for HTTP Server
# ============================================================================

# Core features for HTTP server (Phase 11 - T1/T4/T5/T9)
HTTP_FEATURES=(
    "std"                          # Standard library
    "http"                         # T8 Network: HTTP server capsules
    "http-simd"                    # T2 SIMD HTTP parsing
    "tls"                          # T8+T1+T4 Network: TLS 1.3
    "websocket"                    # T8+T1 Network: WebSocket RFC 6455
    "network"                      # T8 Distributed coordination

    # HTTP Middleware (Phase 11 - 7 capsules)
    "static-files"                 # T9+T1 StaticFileServerCapsule (22× speedup)
    "cors-middleware"              # T1 CorsMiddlewareCapsule (40-100× EXCEPTIONAL)
    "csrf-protection"              # T1 CsrfProtectionCapsule (200-500× EXCEPTIONAL)
    "security-headers"             # T1 SecurityHeadersCapsule (3-10× TYPICAL)
    "form-parser"                  # T4+T5 FormParserCapsule (5× TYPICAL)
    "validation"                   # T1+T2 ValidationCapsule (10-30× EXCEPTIONAL)
    "cache-middleware"             # T1 CacheMiddlewareCapsule (5-20× EXCEPTIONAL)

    # Supporting features
    "circuit-breaker-standard64"   # T1 Circuit breaker pattern
    "rate-limiter"                 # T1 Rate limiting
    "metrics"                      # T8 Prometheus metrics
    "logging"                      # T0+T1+T5 Logging
    "observability"                # T6 Mixed observability

    # Collection support
    "cache"                        # T6 LockfreeCacheCapsule
    "histogram"                    # T4 HistogramCapsule
    "queue-all"                    # T4 Queue capsules

    # Fixed-point for deterministic calculations
    "fixed-point"                  # T3 Determinism (Q16.16)

    # SIMD optimizations (nightly required)
    "simd-native"                  # T2 SIMD for native platforms
    "simd-crypto"                  # T2 SIMD cryptography
    "portable_simd"                # Portable SIMD (base requirement)

    # Async support
    "tokio-compat"                 # Enable tokio integration
    "streaming-async"              # T5 Async streaming
    "async-log"                    # T5 Async logging
    "async-channels"               # T1 Lockfree async channels

    # Persistence for audit trails
    "persistent"                   # T9 Persistent state
    "capsule-mmap"                 # T9 Capsule-native mmap
    "audit-q34"                    # Q34 compliance audit

    # Derive macros for verification
    "derive"                       # #[derive(ComputationalCapsule)]
    "nightly"                      # Enable nightly-only features
)

# Join features with comma
FEATURES=$(IFS=,; echo "${HTTP_FEATURES[*]}")

log_info "Features enabled: $(echo "$FEATURES" | tr ',' '\n' | wc -l) total"

# ============================================================================
# Clean Build (Optional)
# ============================================================================

if [ "${CLEAN_BUILD:-0}" == "1" ]; then
    log_warn "Cleaning previous build artifacts..."
    cargo clean
    log_success "Clean complete"
fi

# ============================================================================
# Build
# ============================================================================

log_info "Building atomic_capsule HTTP server (release mode, nightly)..."
log_info "This may take 10-20 minutes on a modern CPU..."

# Build with all HTTP features
cargo build --release \
    --features "$FEATURES" \
    2>&1 | tee /tmp/build_log.txt

# Check build status
if [ "${PIPESTATUS[0]}" -ne 0 ]; then
    log_error "Build failed. See /tmp/build_log.txt for details"
    exit 1
fi

log_success "Build completed successfully"

# ============================================================================
# Verify Binary
# ============================================================================

log_info "Verifying binary..."

BINARY_PATH="target/release/$BINARY_NAME"

if [ ! -f "$BINARY_PATH" ]; then
    # If specific binary doesn't exist, check for library build
    log_warn "Binary '$BINARY_NAME' not found (may be library-only build)"
    log_info "Checking for library artifacts..."

    if ls target/release/libatomic_capsule.* &>/dev/null; then
        log_success "Library built: $(ls -lh target/release/libatomic_capsule.* | head -1 | awk '{print $NF, $5}')"
    fi

    # Show what was built
    log_info "Build artifacts:"
    find target/release -maxdepth 1 -type f -executable -o -name "*.so" -o -name "*.a" -o -name "*.rlib" 2>/dev/null | sort | head -20
else
    SIZE=$(du -h "$BINARY_PATH" | cut -f1)
    log_success "Binary exists: $BINARY_PATH ($SIZE)"

    # Show binary properties
    file "$BINARY_PATH"

    # ====================================================================
    # Strip Debug Symbols (Optional, for size reduction)
    # ====================================================================

    SIZE_BEFORE=$(du -h "$BINARY_PATH" | cut -f1)
    log_info "Stripping debug symbols to reduce binary size..."
    strip "$BINARY_PATH"
    SIZE_AFTER=$(du -h "$BINARY_PATH" | cut -f1)

    log_success "Stripped: $SIZE_BEFORE → $SIZE_AFTER"

    # ====================================================================
    # Binary Properties
    # ====================================================================

    log_info "Binary properties:"
    ldd "$BINARY_PATH" 2>&1 | head -5 || true
fi

# ============================================================================
# Verification Summary
# ============================================================================

log_info "Build verification:"
log_success "✓ Nightly toolchain active"
log_success "✓ HTTP features enabled (static-files, cors, csrf, security-headers, form-parser, validation, cache-middleware)"
log_success "✓ T1/T2/T4/T5/T8/T9 tiers compiled"
log_success "✓ SIMD optimizations enabled"
log_success "✓ Circuit breaker & rate limiter"
log_success "✓ Metrics & observability"

# ============================================================================
# Summary
# ============================================================================

echo ""
log_success "Build complete!"
echo ""
log_info "Binary location: $BINARY_PATH"
log_info "Build log: /tmp/build_log.txt"
log_info ""
log_info "Next steps:"
log_info "  1. Deploy to 6900HX: ./deploy_to_6900hx.sh"
log_info "  2. Check deployment: ssh samuel@192.168.0.38 'systemctl status atomic-http-server'"
log_info "  3. View logs: ssh samuel@192.168.0.38 'journalctl -u atomic-http-server -f'"
echo ""
