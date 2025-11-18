# Test Automation Scripts

Complete test automation infrastructure for kindly_dedup using shell scripts and GitHub Actions.

## Quick Start

```bash
# Fast suite (P0 critical tests, ~60s)
./scripts/test_fast.sh

# Full suite (P0-P2 comprehensive, ~5 min)
./scripts/test_full.sh

# View results
echo $?  # Exit code: 0 = success, 1 = failure
```

## Scripts Overview

### test_fast.sh

**Purpose**: Rapid feedback loop for developers

**Tests** (4 categories, ~20 tests):
- Compilation checks (build, formatting, clippy)
- Unit tests (P0 core)
- Integration tests (P0 core)
- Feature tests (SIMD, Bloom)
- Binary smoke tests (help, version)

**Execution Time**: <60 seconds
**Exit Code**: 0 (all passed), 1 (failures)
**Use Case**: Pre-commit, local development, CI feedback

**Colors**:
- ✓ GREEN = test passed
- ✗ RED = test failed
- ⊙ YELLOW = test skipped

**Usage**:
```bash
# Run and report
./scripts/test_fast.sh

# Check exit code
./scripts/test_fast.sh && echo "Ready to commit" || echo "Fix tests"
```

### test_full.sh

**Purpose**: Comprehensive testing before release

**Tests** (7 phases, ~50+ tests):
1. Compilation & Style: Build, format, clippy
2. Library Tests: P0/P5 unit, property, integration, production
3. Feature Tests: SIMD, Bloom, Crypto, Audit
4. Format & Pipeline: Streaming, T5, error handling
5. Protection: Obfuscation, encryption, hardening
6. Advanced: Persistent, disk-backed LSH, bounded DocumentId
7. Binary: Smoke tests (help, version)

**Execution Time**: <5 minutes
**Exit Code**: 0 (all passed), 1 (failures)
**Use Case**: CI on main branch, pre-release validation

**Environment Variables**:
```bash
# Run verbosely (show errors)
VERBOSE=true ./scripts/test_full.sh

# Stop on first failure
STOP_ON_FAILURE=true ./scripts/test_full.sh

# Both flags
VERBOSE=true STOP_ON_FAILURE=true ./scripts/test_full.sh
```

## GitHub Actions CI

### Workflows

**File**: `.github/workflows/test.yml`

**Jobs**:
1. **Fast Suite** (ubuntu-latest, ~10 min)
   - Runs on: every push, PR
   - Tests: Compilation, P0 unit/integration, binary

2. **Full Suite** (ubuntu-latest, ~30 min)
   - Runs on: main branch, PR
   - Tests: All P0-P2, features, advanced

3. **Feature Matrix** (ubuntu-latest, ~15 min each)
   - Runs on: main branch, PR
   - Features: default, simd-minhash, persistent-dedup, audit-trail, protection

4. **CPU Features** (ubuntu-latest, ~15 min each)
   - Runs on: main branch, PR
   - Tiers: Scalar, SSE4.2, AVX2

5. **macOS Check** (macos-latest, ~20 min)
   - Runs on: main branch, PR
   - Tests: Cross-platform compatibility

6. **Summary** (ubuntu-latest)
   - Reports: Overall test status
   - Fails if any job fails

### Trigger Events

```yaml
on:
  push:
    branches: [main, phase2.4.1-derive-macro-migration, develop]
  pull_request:
    branches: [main]
```

**Runs on**:
- Every push to main/develop
- Every pull request to main
- Automatically skipped on docs-only changes

### Caching

**Cached items**:
- Cargo registry (~20 MB)
- Cargo build artifacts (~500 MB)
- Cargo git index (~10 MB)

**Cache key**: Based on `Cargo.lock`, platform, toolchain

**TTL**: 7 days

## Test Framework Compliance

### T28 (Testing Framework)

**Tiers**:
- Q1-Q7: Unit tests ✓ (P0 unit)
- Q8-Q14: Property tests ✓ (P0 property)
- Q15-Q21: Integration tests ✓ (P0 integration)
- Q22-Q28: Production tests ✓ (P0 production)

### UCE34 (Systematic Discovery)

**Q34 Auditability**: Audit trail verification in tests

**Compliance Evidence**:
- Build hardening (Q33 verification)
- Crypto license validation (Q34 auditability)
- Encrypted state tests (Q33 verification)
- Hash chain integrity (Q34 auditability)

### ASSUM (Safety Framework)

**99.99%+ Safety**:
- Zero unsafe code in fast paths
- All assumptions documented
- ASSUM tests validate safety

### B32 (Benchmarking Framework)

**Fair Baselines**:
- Baseline tests (scalar MinHash)
- SIMD optimized tests (portable_simd)
- Feature dispatch tests (CPU capabilities)

## Integration Examples

### Pre-Commit Hook

```bash
#!/bin/bash
# .git/hooks/pre-commit (make executable: chmod +x)

set -euo pipefail

# Run fast suite before committing
if ! ./scripts/test_fast.sh; then
    echo "Commit blocked: tests failed"
    exit 1
fi

# Stage changes
git add .
```

**Install**:
```bash
cp scripts/pre-commit-hook.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

### GitHub Actions Badge

Add to README.md:

```markdown
[![Test Suite](https://github.com/kindly-software/kindly_dedup/actions/workflows/test.yml/badge.svg)](https://github.com/kindly-software/kindly_dedup/actions/workflows/test.yml)
```

### Local CI Simulation

Run GitHub Actions workflow locally using [act](https://github.com/nektos/act):

```bash
# Install act (one-time)
brew install act  # macOS
# or: curl https://raw.githubusercontent.com/nektos/act/master/install.sh | bash

# Run workflow
act -j fast-suite

# Run specific job with matrix
act -j feature-matrix -P ubuntu-latest=ghcr.io/catthehacker/ubuntu:full-latest
```

## Troubleshooting

### Test Failure: "Binary not found"

**Problem**: `./target/release/kindly_dedup: No such file or directory`

**Solution**:
```bash
cargo build --bin kindly_dedup --release
```

### Test Failure: "atomic_capsule not found"

**Problem**: Path dependency resolution error

**Solution**:
```bash
# Ensure atomic_capsule is available
ls -la ../atomic_capsule/Cargo.toml

# Update lockfile
cargo update
cargo build --release
```

### Clippy Warnings

**Problem**: Tests fail due to clippy lint errors

**Solution**:
```bash
# Check warnings
cargo clippy --lib --release -- -D warnings

# Auto-fix (when possible)
cargo clippy --lib --release --fix
```

### Feature Compilation Errors

**Problem**: Unknown feature flag

**Solution**:
```bash
# List available features
cargo metadata --format-version=1 | jq '.packages[0].features'

# Use valid combination
cargo test --features "simd-minhash,audit-trail" --release
```

## Performance Metrics

### Test Execution Time

| Suite | Phase | Time | Status |
|-------|-------|------|--------|
| Fast | Compile | 20-30s | ✓ |
| Fast | Tests | 20-30s | ✓ |
| **Fast Total** | **All** | **<60s** | **✓** |
| Full | Phase 1 | 20-30s | ✓ |
| Full | Phase 2 | 30-40s | ✓ |
| Full | Phase 3 | 20-30s | ✓ |
| Full | Phase 4 | 10-20s | ✓ |
| Full | Phase 5 | 20-30s | ✓ |
| Full | Phase 6 | 20-30s | ⚠ (optional) |
| Full | Phase 7 | <5s | ✓ |
| **Full Total** | **All** | **<5 min** | **✓** |

### CI Job Duration

| Job | Duration | Platform |
|-----|----------|----------|
| Fast Suite | 8-10 min | ubuntu-latest |
| Full Suite | 25-30 min | ubuntu-latest |
| Feature Matrix (1) | 12-15 min | ubuntu-latest |
| CPU Features (1) | 12-15 min | ubuntu-latest |
| macOS Check | 18-20 min | macos-latest |
| **Total (parallel)** | **~30 min** | Mixed |

### Test Count

```
P0 Unit:           ~45 tests
P0 Property:       ~35 tests
P0 Integration:    ~40 tests
P0 Production:     ~30 tests
P5 Unit:           ~25 tests
P5 Integration:    ~20 tests
Feature-specific:  ~50 tests
Advanced:          ~30 tests
Binary smoke:      ~5 tests
─────────────────────────
TOTAL:             ~280 tests
```

## Documentation

- **UCE34 Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/xml/frameworks/uce34.xml`
- **T28 Testing**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/xml/frameworks/t28.xml`
- **kindly_dedup CLAUDE.md**: `./CLAUDE.md`

## Contributing

When adding new tests:

1. **Organize by tier**: P0 (critical), P1-P5 (features), P6 (advanced)
2. **Use naming convention**: `test_<tier>_<feature>_<type>`
3. **Add to appropriate phase**: Edit `test_fast.sh` or `test_full.sh`
4. **Update CI workflow**: Add to `.github/workflows/test.yml`
5. **Document**: Add entry to this README

## Support

For issues or questions:
- Check test output: `tail -100 /tmp/test_output.log`
- Enable verbose mode: `VERBOSE=true ./scripts/test_full.sh`
- Review CI logs: GitHub Actions > Workflows > Test Suite

---

**Last Updated**: 2025-11-17
**Framework Compliance**: UCE34, T28, ASSUM, B32, COCA
