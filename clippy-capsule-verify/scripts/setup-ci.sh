#!/bin/bash
# Interactive CI/CD Setup Script for clippy-capsule-verify
# Usage: ./scripts/setup-ci.sh
# Framework: UCE34 Q30-Q34 (Validation + Auditability)

set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script version
VERSION="1.0.0"

# Helper functions
print_header() {
    echo -e "${BLUE}============================================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}============================================================${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

# Banner
clear
print_header "Clippy Capsule Verify - CI/CD Setup v${VERSION}"
echo ""
echo "This script will set up automated CI/CD for COCA compliance enforcement."
echo ""

# Check prerequisites
print_info "Checking prerequisites..."

# Check for git
if ! command -v git &> /dev/null; then
    print_error "Git is not installed. Please install git first."
    exit 1
fi

# Check if we're in a git repository
if [ ! -d ".git" ]; then
    print_error "Not in a git repository. Please run this script from your project root."
    exit 1
fi

# Check for cargo
if ! command -v cargo &> /dev/null; then
    print_error "Cargo is not installed. Please install Rust first."
    exit 1
fi

print_success "All prerequisites met"
echo ""

# Interactive platform selection
print_header "Platform Selection"
echo ""
echo "Select your CI/CD platform(s):"
echo "  1) GitHub Actions only"
echo "  2) GitLab CI only"
echo "  3) Local git hooks only (no CI/CD)"
echo "  4) GitHub Actions + Local hooks"
echo "  5) GitLab CI + Local hooks"
echo "  6) All of the above (GitHub + GitLab + Local)"
echo ""
read -p "Selection [1-6]: " PLATFORM_CHOICE
echo ""

# Feature selection
print_header "Feature Configuration"
echo ""
echo "Additional features:"
echo "  1) VSCode integration"
echo "  2) .clippy.toml configuration"
echo "  3) .cargo/config.toml optimization"
echo ""
read -p "Enable all features? [Y/n]: " ENABLE_FEATURES
ENABLE_FEATURES=${ENABLE_FEATURES:-Y}
echo ""

# Confirmation
print_header "Configuration Summary"
echo ""
case $PLATFORM_CHOICE in
    1) echo "Platform: GitHub Actions" ;;
    2) echo "Platform: GitLab CI" ;;
    3) echo "Platform: Local git hooks" ;;
    4) echo "Platform: GitHub Actions + Local hooks" ;;
    5) echo "Platform: GitLab CI + Local hooks" ;;
    6) echo "Platform: All (GitHub + GitLab + Local)" ;;
    *) print_error "Invalid selection"; exit 1 ;;
esac
echo "Features: $([ "$ENABLE_FEATURES" = "Y" ] || [ "$ENABLE_FEATURES" = "y" ] && echo "Enabled" || echo "Disabled")"
echo ""
read -p "Proceed with installation? [Y/n]: " CONFIRM
CONFIRM=${CONFIRM:-Y}
if [ "$CONFIRM" != "Y" ] && [ "$CONFIRM" != "y" ]; then
    print_warning "Installation cancelled"
    exit 0
fi
echo ""

# Start installation
print_header "Installation Progress"
echo ""

INSTALLED_COMPONENTS=()

# Function to install GitHub Actions
install_github_actions() {
    print_info "Installing GitHub Actions workflow..."
    mkdir -p .github/workflows

    cat > .github/workflows/clippy-capsule-verify.yml << 'WORKFLOW_EOF'
name: Clippy Capsule Verify

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main, develop ]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  clippy-p0-critical:
    name: P0 Critical Lints (Fast Fail)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust nightly
        uses: dtolnay/rust-toolchain@nightly
        with:
          components: clippy, rustfmt

      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-registry-

      - name: Cache cargo index
        uses: actions/cache@v3
        with:
          path: ~/.cargo/git
          key: ${{ runner.os }}-cargo-git-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-git-

      - name: Cache cargo build
        uses: actions/cache@v3
        with:
          path: target
          key: ${{ runner.os }}-cargo-build-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-build-

      - name: Run P0 critical lints
        run: |
          cargo clippy --all-targets -- \
            -D clippy::capsule_mutex_violation \
            -D clippy::capsule_unaligned_violation \
            -D clippy::capsule_missing_generation \
            -D clippy::capsule_non_atomic_field

  clippy-full:
    name: Full Lint Suite (P0 + P1)
    runs-on: ubuntu-latest
    needs: clippy-p0-critical
    strategy:
      matrix:
        rust: [stable, nightly]
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust ${{ matrix.rust }}
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ matrix.rust }}
          components: clippy, rustfmt

      - name: Cache dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ matrix.rust }}-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-${{ matrix.rust }}-

      - name: Run full clippy suite
        run: |
          cargo clippy --all-targets --all-features -- \
            -D clippy::capsule_mutex_violation \
            -D clippy::capsule_unaligned_violation \
            -D clippy::capsule_missing_generation \
            -D clippy::capsule_non_atomic_field \
            -W clippy::missing_capsule_verification

      - name: Run tests
        run: cargo test --all-features

      - name: Check formatting
        run: cargo fmt --all -- --check

  upload-artifacts:
    name: Upload Lint Results
    runs-on: ubuntu-latest
    needs: clippy-full
    if: always()
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust nightly
        uses: dtolnay/rust-toolchain@nightly
        with:
          components: clippy

      - name: Generate lint report
        run: |
          cargo clippy --all-targets --all-features --message-format=json 2>&1 | tee clippy-report.json || true

      - name: Upload lint results
        uses: actions/upload-artifact@v3
        with:
          name: clippy-capsule-verify-report
          path: clippy-report.json
          retention-days: 30
WORKFLOW_EOF

    print_success "Created .github/workflows/clippy-capsule-verify.yml"
    INSTALLED_COMPONENTS+=("GitHub Actions workflow")
}

# Function to install GitLab CI
install_gitlab_ci() {
    print_info "Installing GitLab CI configuration..."

    cat > .gitlab-ci.yml << 'GITLAB_EOF'
# GitLab CI Configuration for clippy-capsule-verify
# Framework: UCE34 Q30-Q34 (Validation + Auditability)

stages:
  - build
  - test
  - lint
  - report

variables:
  CARGO_HOME: ${CI_PROJECT_DIR}/.cargo
  RUST_BACKTRACE: "1"
  CARGO_TERM_COLOR: always

cache:
  key: ${CI_COMMIT_REF_SLUG}
  paths:
    - .cargo/
    - target/

before_script:
  - rustc --version
  - cargo --version

# P0 Critical Lints (Fast Fail)
clippy-p0-critical:
  stage: lint
  image: rustlang/rust:nightly
  script:
    - cargo clippy --all-targets --
        -D clippy::capsule_mutex_violation
        -D clippy::capsule_unaligned_violation
        -D clippy::capsule_missing_generation
        -D clippy::capsule_non_atomic_field
  allow_failure: false
  only:
    - main
    - develop
    - merge_requests

# Full Lint Suite (P0 + P1)
clippy-full:
  stage: lint
  image: rustlang/rust:nightly
  needs: ["clippy-p0-critical"]
  script:
    - cargo clippy --all-targets --all-features --
        -D clippy::capsule_mutex_violation
        -D clippy::capsule_unaligned_violation
        -D clippy::capsule_missing_generation
        -D clippy::capsule_non_atomic_field
        -W clippy::missing_capsule_verification
  allow_failure: false
  only:
    - main
    - develop
    - merge_requests

# Run tests
test:
  stage: test
  image: rustlang/rust:nightly
  script:
    - cargo test --all-features
  only:
    - main
    - develop
    - merge_requests

# Check formatting
fmt-check:
  stage: lint
  image: rustlang/rust:nightly
  script:
    - cargo fmt --all -- --check
  allow_failure: false
  only:
    - main
    - develop
    - merge_requests

# Generate lint report
lint-report:
  stage: report
  image: rustlang/rust:nightly
  script:
    - cargo clippy --all-targets --all-features --message-format=json 2>&1 | tee clippy-report.json || true
  artifacts:
    paths:
      - clippy-report.json
    expire_in: 30 days
  when: always
  only:
    - main
    - develop
    - merge_requests
GITLAB_EOF

    print_success "Created .gitlab-ci.yml"
    INSTALLED_COMPONENTS+=("GitLab CI configuration")
}

# Function to install git hooks
install_git_hooks() {
    print_info "Installing git hooks..."

    HOOKS_DIR=".git/hooks"
    mkdir -p "$HOOKS_DIR"

    # Pre-commit hook
    cat > "$HOOKS_DIR/pre-commit" << 'HOOK_EOF'
#!/bin/bash
# Pre-Commit Hook: Fast P0 Critical Checks
# Framework: UCE34 Q30 (Validation)

set -e

# Add cargo to PATH
export PATH="$HOME/.cargo/bin:$PATH"

echo "🔍 [Pre-Commit] Running P0 critical lint checks..."

# Start timer
START_TIME=$(date +%s)

# P0 critical lints only (fast check)
if cargo clippy --all-targets --quiet -- \
  -D clippy::capsule_mutex_violation \
  -D clippy::capsule_unaligned_violation \
  -D clippy::capsule_missing_generation \
  -D clippy::capsule_non_atomic_field 2>&1 | grep -q "error:"; then

    END_TIME=$(date +%s)
    DURATION=$((END_TIME - START_TIME))

    echo "❌ [Pre-Commit] P0 critical violations detected! (${DURATION}s)"
    echo ""
    echo "Fix the following before committing:"
    echo "  - Remove Mutex/RwLock from capsules (use AtomicU64)"
    echo "  - Add padding to align size to alignment boundary"
    echo "  - Add generation counters to T1 Atomic capsules"
    echo "  - Replace non-atomic fields with atomic types"
    echo ""
    echo "To bypass (NOT RECOMMENDED):"
    echo "  git commit --no-verify"
    exit 1
else
    END_TIME=$(date +%s)
    DURATION=$((END_TIME - START_TIME))
    echo "✅ [Pre-Commit] P0 critical checks passed! (${DURATION}s)"
    exit 0
fi
HOOK_EOF

    # Pre-push hook
    cat > "$HOOKS_DIR/pre-push" << 'HOOK_EOF'
#!/bin/bash
# Pre-Push Hook: Comprehensive Validation
# Framework: UCE34 Q30-Q34 (Validation + Auditability)

set -e

# Add cargo to PATH
export PATH="$HOME/.cargo/bin:$PATH"

echo "🔍 [Pre-Push] Running comprehensive validation..."

# Start timer
START_TIME=$(date +%s)

# Step 1: All clippy lints (P0 + P1)
echo "📋 Step 1/3: Clippy lints (P0 + P1)..."
cargo clippy --all-targets --all-features --quiet -- \
  -D clippy::capsule_mutex_violation \
  -D clippy::capsule_unaligned_violation \
  -D clippy::capsule_missing_generation \
  -D clippy::capsule_non_atomic_field \
  -W clippy::missing_capsule_verification

# Step 2: Run tests
echo "🧪 Step 2/3: Running tests..."
cargo test --all-features --quiet

# Step 3: Check formatting
echo "🎨 Step 3/3: Checking code formatting..."
cargo fmt --all -- --check

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo "✅ [Pre-Push] All checks passed! (${DURATION}s)"
exit 0
HOOK_EOF

    # Commit-msg hook
    cat > "$HOOKS_DIR/commit-msg" << 'HOOK_EOF'
#!/bin/bash
# Commit-Msg Hook: Enforce commit message format
# Framework: UCE34 Q34 (Auditability)

COMMIT_MSG_FILE=$1
COMMIT_MSG=$(cat "$COMMIT_MSG_FILE")

# Check for required tags
if echo "$COMMIT_MSG" | grep -qE '^\[(TRADE SECRET|P0 FIX|P1 FIX|P2 FIX|FEAT|FIX|REFACTOR|DOCS|TEST|CI)\]'; then
    exit 0
else
    echo "❌ [Commit-Msg] Invalid commit message format!"
    echo ""
    echo "Required format: [TAG] Description"
    echo ""
    echo "Valid tags:"
    echo "  [TRADE SECRET] - Trade secret code (local commits only)"
    echo "  [P0 FIX]       - P0 critical lint fix"
    echo "  [P1 FIX]       - P1 high lint fix"
    echo "  [P2 FIX]       - P2 medium lint fix"
    echo "  [FEAT]         - New feature"
    echo "  [FIX]          - Bug fix"
    echo "  [REFACTOR]     - Code refactoring"
    echo "  [DOCS]         - Documentation update"
    echo "  [TEST]         - Test addition/update"
    echo "  [CI]           - CI/CD configuration"
    echo ""
    echo "Example: [P0 FIX] Replace Mutex with AtomicU64 in CircuitBreakerCapsule"
    exit 1
fi
HOOK_EOF

    # Make hooks executable
    chmod +x "$HOOKS_DIR/pre-commit"
    chmod +x "$HOOKS_DIR/pre-push"
    chmod +x "$HOOKS_DIR/commit-msg"

    print_success "Installed git hooks (pre-commit, pre-push, commit-msg)"
    INSTALLED_COMPONENTS+=("Git hooks")
}

# Function to install VSCode integration
install_vscode() {
    print_info "Installing VSCode integration..."

    mkdir -p .vscode

    cat > .vscode/settings.json << 'VSCODE_EOF'
{
  "rust-analyzer.check.command": "clippy",
  "rust-analyzer.check.extraArgs": [
    "--all-targets",
    "--all-features",
    "--",
    "-D", "clippy::capsule_mutex_violation",
    "-D", "clippy::capsule_unaligned_violation",
    "-D", "clippy::capsule_missing_generation",
    "-D", "clippy::capsule_non_atomic_field",
    "-W", "clippy::missing_capsule_verification"
  ],
  "rust-analyzer.diagnostics.enable": true,
  "rust-analyzer.diagnostics.experimental.enable": true,
  "files.autoSave": "afterDelay",
  "files.autoSaveDelay": 1000,
  "editor.formatOnSave": true,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  }
}
VSCODE_EOF

    cat > .vscode/tasks.json << 'TASKS_EOF'
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "Clippy P0 Check",
      "type": "shell",
      "command": "cargo",
      "args": [
        "clippy",
        "--all-targets",
        "--",
        "-D", "clippy::capsule_mutex_violation",
        "-D", "clippy::capsule_unaligned_violation",
        "-D", "clippy::capsule_missing_generation",
        "-D", "clippy::capsule_non_atomic_field"
      ],
      "problemMatcher": "$rustc",
      "group": {
        "kind": "build",
        "isDefault": true
      },
      "presentation": {
        "reveal": "always"
      }
    },
    {
      "label": "Clippy Full Check",
      "type": "shell",
      "command": "cargo",
      "args": [
        "clippy",
        "--all-targets",
        "--all-features",
        "--",
        "-D", "warnings"
      ],
      "problemMatcher": "$rustc",
      "group": "build"
    }
  ]
}
TASKS_EOF

    print_success "Created .vscode/settings.json and .vscode/tasks.json"
    INSTALLED_COMPONENTS+=("VSCode integration")
}

# Function to install clippy.toml
install_clippy_config() {
    print_info "Installing .clippy.toml configuration..."

    if [ -f ".clippy.toml.example" ]; then
        cp .clippy.toml.example .clippy.toml
        print_success "Created .clippy.toml from template"
    else
        cat > .clippy.toml << 'CLIPPY_EOF'
# Clippy Capsule Verification Configuration
# Framework: UCE34 Q30-Q34 (Validation + Auditability)

# P0 CRITICAL LINTS (DENY)
capsule-mutex-violation = "deny"
capsule-unaligned-violation = "deny"
capsule-missing-generation = "deny"
capsule-non-atomic-field = "deny"

# P1 HIGH LINTS (WARN)
missing-capsule-verification = "warn"

# Standard clippy configuration
all-lints = "warn"
pedantic = { level = "warn", priority = -1 }
perf = { level = "warn", priority = 1 }

# Disable noisy lints
module-name-repetitions = "allow"
struct-excessive-bools = "allow"
similar-names = "allow"

# MSRV
msrv = "1.77.0"

# Thresholds
cognitive-complexity-threshold = 25
type-complexity-threshold = 500
too-many-arguments-threshold = 8
too-many-lines-threshold = 150
CLIPPY_EOF
        print_success "Created .clippy.toml"
    fi

    INSTALLED_COMPONENTS+=(".clippy.toml configuration")
}

# Function to install cargo config
install_cargo_config() {
    print_info "Installing .cargo/config.toml optimization..."

    mkdir -p .cargo

    cat > .cargo/config.toml << 'CARGO_EOF'
# Cargo Configuration for clippy-capsule-verify
# Performance optimizations for faster builds

[build]
# Parallel compilation
jobs = 8

# Use LLD linker (30% faster)
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

# Incremental compilation
incremental = true

[target.x86_64-unknown-linux-gnu]
linker = "clang"

[registries.crates-io]
# Sparse registry protocol (faster dependency fetching)
protocol = "sparse"

[profile.dev]
# Optimize dependencies in dev mode
opt-level = 0
debug = true
incremental = true

[profile.release]
# Maximum optimization
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
CARGO_EOF

    print_success "Created .cargo/config.toml"
    INSTALLED_COMPONENTS+=(".cargo/config.toml optimization")
}

# Execute installation based on platform choice
case $PLATFORM_CHOICE in
    1)  # GitHub Actions only
        install_github_actions
        ;;
    2)  # GitLab CI only
        install_gitlab_ci
        ;;
    3)  # Local git hooks only
        install_git_hooks
        ;;
    4)  # GitHub + hooks
        install_github_actions
        install_git_hooks
        ;;
    5)  # GitLab + hooks
        install_gitlab_ci
        install_git_hooks
        ;;
    6)  # All platforms
        install_github_actions
        install_gitlab_ci
        install_git_hooks
        ;;
esac

# Install additional features if requested
if [ "$ENABLE_FEATURES" = "Y" ] || [ "$ENABLE_FEATURES" = "y" ]; then
    install_vscode
    install_clippy_config
    install_cargo_config
fi

# Summary
echo ""
print_header "Installation Complete!"
echo ""
echo "Installed components:"
for component in "${INSTALLED_COMPONENTS[@]}"; do
    echo "  ✅ $component"
done
echo ""

# Next steps
print_header "Next Steps"
echo ""
echo "1. Review generated files:"
[ -f ".github/workflows/clippy-capsule-verify.yml" ] && echo "   - .github/workflows/clippy-capsule-verify.yml"
[ -f ".gitlab-ci.yml" ] && echo "   - .gitlab-ci.yml"
[ -f ".git/hooks/pre-commit" ] && echo "   - .git/hooks/pre-commit"
[ -f ".git/hooks/pre-push" ] && echo "   - .git/hooks/pre-push"
[ -f ".git/hooks/commit-msg" ] && echo "   - .git/hooks/commit-msg"
[ -f ".vscode/settings.json" ] && echo "   - .vscode/settings.json"
[ -f ".clippy.toml" ] && echo "   - .clippy.toml"
[ -f ".cargo/config.toml" ] && echo "   - .cargo/config.toml"
echo ""
echo "2. Test git hooks:"
echo "   .git/hooks/pre-commit"
echo ""
echo "3. Commit changes:"
echo "   git add ."
echo "   git commit -m '[CI] Add clippy-capsule-verify automation'"
echo ""
echo "4. Push to trigger CI/CD:"
echo "   git push origin main"
echo ""

print_success "Setup complete! 🎉"
