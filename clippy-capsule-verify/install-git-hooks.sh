#!/bin/bash
# Install All Git Hooks for clippy-capsule-verify
# Usage: ./install-git-hooks.sh
# Framework: UCE34 Q30 (Validation)

set -e

HOOKS_DIR=".git/hooks"

echo "📦 Installing git hooks for clippy-capsule-verify..."

# Create hooks directory if it doesn't exist
mkdir -p "$HOOKS_DIR"

# Install pre-commit hook (P0 critical checks)
cat > "$HOOKS_DIR/pre-commit" << 'EOF'
#!/bin/bash
# Pre-Commit Hook: Fast P0 Critical Checks
# Location: .git/hooks/pre-commit
# Framework: UCE34 Q30 (Validation)

set -e

# Add cargo to PATH (git hooks don't inherit shell environment)
export PATH="$HOME/.cargo/bin:$PATH"

echo "🔍 [Pre-Commit] Running P0 critical lint checks..."

# P0 critical lints only (fast check)
cargo clippy --all-targets --quiet -- \
  -D clippy::capsule_mutex_violation \
  -D clippy::capsule_unaligned_violation \
  -D clippy::capsule_missing_generation \
  -D clippy::capsule_non_atomic_field

if [ $? -eq 0 ]; then
    echo "✅ [Pre-Commit] P0 critical checks passed!"
    exit 0
else
    echo "❌ [Pre-Commit] P0 critical violations detected!"
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
fi
EOF

# Install pre-push hook (comprehensive validation)
cat > "$HOOKS_DIR/pre-push" << 'EOF'
#!/bin/bash
# Pre-Push Hook: Comprehensive Validation
# Location: .git/hooks/pre-push
# Framework: UCE34 Q30-Q34 (Validation + Auditability)

set -e

# Add cargo to PATH
export PATH="$HOME/.cargo/bin:$PATH"

echo "🔍 [Pre-Push] Running comprehensive validation..."

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

if [ $? -eq 0 ]; then
    echo "✅ [Pre-Push] All checks passed! Safe to push."
    exit 0
else
    echo "❌ [Pre-Push] Validation failed!"
    echo ""
    echo "Fix the following before pushing:"
    echo "  1. Fix all clippy warnings (cargo clippy)"
    echo "  2. Fix failing tests (cargo test)"
    echo "  3. Format code (cargo fmt)"
    echo ""
    echo "To bypass (NOT RECOMMENDED FOR PRODUCTION):"
    echo "  git push --no-verify"
    exit 1
fi
EOF

# Install commit-msg hook (enforce message format)
cat > "$HOOKS_DIR/commit-msg" << 'EOF'
#!/bin/bash
# Commit-Msg Hook: Enforce commit message format
# Location: .git/hooks/commit-msg
# Framework: UCE34 Q34 (Auditability)

COMMIT_MSG_FILE=$1
COMMIT_MSG=$(cat "$COMMIT_MSG_FILE")

# Check for required tags
if echo "$COMMIT_MSG" | grep -qE '^\[(TRADE SECRET|P0 FIX|P1 FIX|FEAT|FIX|REFACTOR|DOCS|TEST)\]'; then
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
    echo "  [FEAT]         - New feature"
    echo "  [FIX]          - Bug fix"
    echo "  [REFACTOR]     - Code refactoring"
    echo "  [DOCS]         - Documentation update"
    echo "  [TEST]         - Test addition/update"
    echo ""
    echo "Example: [P0 FIX] Replace Mutex with AtomicU64 in CircuitBreakerCapsule"
    exit 1
fi
EOF

# Make hooks executable
chmod +x "$HOOKS_DIR/pre-commit"
chmod +x "$HOOKS_DIR/pre-push"
chmod +x "$HOOKS_DIR/commit-msg"

echo "✅ Git hooks installed successfully!"
echo ""
echo "Installed hooks:"
echo "  - pre-commit  (P0 critical checks, 5-15s)"
echo "  - pre-push    (P0+P1 checks + tests, 30-60s)"
echo "  - commit-msg  (enforce commit message format)"
echo ""
echo "Test hooks:"
echo "  .git/hooks/pre-commit"
echo "  .git/hooks/pre-push"
echo ""
echo "To bypass hooks (NOT RECOMMENDED):"
echo "  git commit --no-verify"
echo "  git push --no-verify"
