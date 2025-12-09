#!/usr/bin/env bash
#
# Cross-Platform Build Verification Script
# Verifies that installer is ready for GitHub Actions workflow
#
# Usage: ./verify_cross_platform.sh

set +e  # Don't exit on error, we want to show all results

echo "╔══════════════════════════════════════════════════╗"
echo "║  kindly-av1 Installer Cross-Platform Verification  ║"
echo "╚══════════════════════════════════════════════════╝"
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counters
PASS=0
FAIL=0

# Helper functions
pass() {
    echo -e "${GREEN}✓${NC} $1"
    ((PASS++))
}

fail() {
    echo -e "${RED}✗${NC} $1"
    ((FAIL++))
}

warn() {
    echo -e "${YELLOW}⚠${NC} $1"
}

# Check 1: Rust toolchain
echo "━━━ Toolchain Checks ━━━"
if command -v rustc &> /dev/null; then
    RUSTC_VERSION=$(rustc --version)
    pass "Rust installed: $RUSTC_VERSION"
else
    fail "Rust not installed"
fi

if command -v cargo &> /dev/null; then
    CARGO_VERSION=$(cargo --version)
    pass "Cargo installed: $CARGO_VERSION"
else
    fail "Cargo not installed"
fi

# Check 2: Required targets
echo ""
echo "━━━ Cross-Compilation Targets ━━━"
TARGETS=(
    "x86_64-unknown-linux-musl"
    "x86_64-pc-windows-msvc"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
)

for target in "${TARGETS[@]}"; do
    if rustup target list | grep -q "$target (installed)"; then
        pass "Target installed: $target"
    else
        warn "Target not installed: $target (GitHub Actions will install)"
    fi
done

# Check 3: Cargo.toml configuration
echo ""
echo "━━━ Cargo.toml Configuration ━━━"
if [ -f "Cargo.toml" ]; then
    pass "Cargo.toml exists"

    # Check for conditional dependencies
    if grep -q "cfg(unix)" Cargo.toml; then
        pass "Unix-specific dependencies configured"
    else
        fail "Unix-specific dependencies missing"
    fi

    if grep -q "cfg(windows)" Cargo.toml; then
        pass "Windows-specific dependencies configured"
    else
        fail "Windows-specific dependencies missing"
    fi

    # Check release profile
    if grep -q 'opt-level = "z"' Cargo.toml; then
        pass "Size optimization enabled (opt-level=z)"
    else
        warn "Size optimization not set (opt-level should be 'z')"
    fi

    if grep -q 'lto = true' Cargo.toml; then
        pass "Link-time optimization enabled"
    else
        warn "LTO not enabled"
    fi
else
    fail "Cargo.toml not found"
fi

# Check 4: Source files
echo ""
echo "━━━ Source Files ━━━"
REQUIRED_FILES=(
    "src/main.rs"
    "src/lib.rs"
    "src/platform.rs"
    "src/download.rs"
    "src/install.rs"
    "src/path_setup.rs"
)

for file in "${REQUIRED_FILES[@]}"; do
    if [ -f "$file" ]; then
        pass "Source file exists: $file"
    else
        fail "Source file missing: $file"
    fi
done

# Check 5: Compilation
echo ""
echo "━━━ Compilation Tests ━━━"
if cargo check 2>&1 | grep -q "Finished"; then
    pass "Native compilation successful"
else
    fail "Native compilation failed"
fi

# Check for warnings
WARNINGS=$(cargo clippy 2>&1 | grep -c "warning:" || true)
if [ "$WARNINGS" -eq 0 ]; then
    pass "No clippy warnings"
elif [ "$WARNINGS" -lt 10 ]; then
    warn "$WARNINGS clippy warnings (acceptable)"
else
    fail "$WARNINGS clippy warnings (should be <10)"
fi

# Check 6: Unit tests
echo ""
echo "━━━ Unit Tests ━━━"
if cargo test --lib 2>&1 | grep -q "test result: ok"; then
    TEST_COUNT=$(cargo test --lib 2>&1 | grep "test result: ok" | grep -oP '\d+(?= passed)' || echo "0")
    pass "$TEST_COUNT unit tests passing"
else
    fail "Unit tests failed"
fi

# Check 7: GitHub Actions workflow
echo ""
echo "━━━ GitHub Actions Workflow ━━━"
WORKFLOW_FILE="../.github/workflows/release.yml"
if [ -f "$WORKFLOW_FILE" ]; then
    pass "Workflow file exists"

    # Check for installer build step
    if grep -q "Build installer binary" "$WORKFLOW_FILE"; then
        pass "Installer build step configured"
    else
        fail "Installer build step missing in workflow"
    fi

    # Check for code signing steps
    if grep -q "Codesign binary" "$WORKFLOW_FILE"; then
        pass "macOS code signing configured"
    else
        warn "macOS code signing not configured"
    fi

    if grep -q "Sign binary (Windows)" "$WORKFLOW_FILE"; then
        pass "Windows code signing configured"
    else
        warn "Windows code signing not configured"
    fi

    # Check matrix includes installer
    if grep -q "INSTALLER_SRC" "$WORKFLOW_FILE"; then
        pass "Installer included in release archives"
    else
        fail "Installer not included in release archives"
    fi
else
    fail "Workflow file not found: $WORKFLOW_FILE"
fi

# Check 8: Install script
echo ""
echo "━━━ Bootstrap Install Script ━━━"
INSTALL_SCRIPT="../install.sh"
if [ -f "$INSTALL_SCRIPT" ]; then
    pass "install.sh exists"

    # Check executable
    if [ -x "$INSTALL_SCRIPT" ]; then
        pass "install.sh is executable"
    else
        warn "install.sh not executable (run: chmod +x install.sh)"
    fi

    # Check platform detection
    if grep -q "detect_platform()" "$INSTALL_SCRIPT"; then
        pass "Platform detection implemented"
    else
        fail "Platform detection missing"
    fi

    # Check for all supported platforms
    for target in "${TARGETS[@]}"; do
        if grep -q "$target" "$INSTALL_SCRIPT"; then
            pass "Platform supported in script: $target"
        else
            fail "Platform missing in script: $target"
        fi
    done
else
    fail "install.sh not found"
fi

# Check 9: Documentation
echo ""
echo "━━━ Documentation ━━━"
DOC_FILES=(
    "README.md"
    "CROSS_PLATFORM_BUILD.md"
)

for doc in "${DOC_FILES[@]}"; do
    if [ -f "$doc" ]; then
        pass "Documentation exists: $doc"
    else
        warn "Documentation missing: $doc"
    fi
done

# Summary
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Summary:"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [ $FAIL -eq 0 ]; then
    echo -e "${GREEN}✓ All critical checks passed!${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Commit changes: git add ."
    echo "  2. Create release tag: git tag v1.0.0"
    echo "  3. Push tag: git push origin v1.0.0"
    echo "  4. GitHub Actions will build for all platforms"
    exit 0
else
    echo -e "${RED}✗ $FAIL check(s) failed${NC}"
    echo ""
    echo "Please fix the failures above before proceeding."
    exit 1
fi
