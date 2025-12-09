# Release Workflow Guide

## Overview

Automated GitHub Actions workflow for creating cross-platform binary releases of kindly_dedup.

**Location**: `.github/workflows/release.yml`

**Tier**: T8 Network (distributed CI/CD coordination)

**Framework Compliance**: UCE34 Q1-Q34, Chaos 100% lockfree, T28 pre-release testing, Q34 audit trails

## Trigger

The workflow triggers automatically on git tag push matching pattern `v*`:

```bash
# Example: Release version 3.1.0
git tag v3.1.0
git push origin v3.1.0
```

## Build Targets

| Platform | Architecture | Target Triple | Runner |
|----------|--------------|---------------|--------|
| Linux | x86_64 | x86_64-unknown-linux-gnu | ubuntu-latest |
| macOS | x86_64 (Intel) | x86_64-apple-darwin | macos-latest |
| macOS | ARM64 (Apple Silicon) | aarch64-apple-darwin | macos-14 |

## Workflow Stages

### 1. Pre-Flight Validation (10 min timeout)

**Purpose**: Verify release readiness before building artifacts

**Checks**:
- ✅ Extract version from git tag
- ✅ Verify Cargo.toml version matches tag
- ✅ Run P0 unit tests
- ✅ Clippy lint check (zero warnings)
- ✅ Verify CHANGELOG.md updated for version

**Outputs**:
- `version`: Extracted version number (e.g., "3.1.0")
- `tag`: Full tag name (e.g., "v3.1.0")

### 2. Build Matrix (30 min timeout per target)

**Purpose**: Build release binaries for all platforms

**Configuration**:
- Rust toolchain: nightly
- Features enabled: `interactive` (CLI binary)
- Optimization: `--release` with symbol stripping
- Target-specific builds via `cargo build --target`

**Steps per target**:
1. Setup Rust nightly with target
2. Build release binary (`cargo build --bin kindly_dedup --release --target <target> --features interactive`)
3. Strip symbols (reduce binary size)
4. Test binary execution (`--version`, `--help`)
5. Create tarball with binary + docs (README.md, LICENSE, CHANGELOG.md)
6. Upload artifact for release

**Artifact naming**: `kindly_dedup-<tag>-<platform>-<arch>.tar.gz`

Examples:
- `kindly_dedup-v3.1.0-linux-x86_64.tar.gz`
- `kindly_dedup-v3.1.0-macos-x86_64.tar.gz`
- `kindly_dedup-v3.1.0-macos-arm64.tar.gz`

### 3. Create GitHub Release (15 min timeout)

**Purpose**: Publish release with changelog and artifacts

**Steps**:
1. Download all platform artifacts
2. Extract changelog section for version from CHANGELOG.md
3. Create GitHub Release with:
   - Tag name
   - Release title
   - Changelog body
   - All platform tarballs as assets
4. Generate release summary

**Permissions**: Requires `contents: write` for creating releases

### 4. Post-Release Checks (5 min timeout)

**Purpose**: Verify release success and generate summary

**Output**: GitHub Actions summary with:
- Platform targets
- Feature flags
- Optimization settings
- Download links
- Framework compliance checklist

## Usage

### Prerequisites

1. **Update version in Cargo.toml**:
```toml
[package]
version = "3.1.0"
```

2. **Update CHANGELOG.md**:
```markdown
## [3.1.0] - 2025-11-25

### Added
- Feature X

### Changed
- Improvement Y

### Fixed
- Bug Z
```

3. **Commit changes**:
```bash
git add Cargo.toml CHANGELOG.md
git commit -m "[RELEASE] v3.1.0: Description"
git push origin main
```

### Trigger Release

```bash
# Create and push tag
git tag v3.1.0
git push origin v3.1.0
```

**Note**: The workflow will FAIL if:
- Cargo.toml version doesn't match tag
- CHANGELOG.md missing version entry
- P0 tests fail
- Clippy warnings exist

### Monitor Progress

1. Go to GitHub Actions tab
2. Find "Release" workflow run
3. Monitor stages:
   - ✅ Pre-Flight Validation (2-5 min)
   - ✅ Build Matrix (10-20 min total, 3 parallel builds)
   - ✅ Create GitHub Release (1-2 min)
   - ✅ Post-Release Checks (<1 min)

### Download Binaries

After successful release, binaries available at:
```
https://github.com/{owner}/{repo}/releases/tag/v{version}
```

Example:
```
https://github.com/kindly-ai/kindly_dedup/releases/tag/v3.1.0
```

## Release Checklist

- [ ] Version bumped in Cargo.toml
- [ ] CHANGELOG.md updated with version section
- [ ] All tests passing locally (`cargo test --lib --release`)
- [ ] Clippy clean (`cargo clippy --lib --release -- -D warnings`)
- [ ] Changes committed to main branch
- [ ] Git tag created matching Cargo.toml version
- [ ] Tag pushed to GitHub
- [ ] Monitor workflow in GitHub Actions
- [ ] Verify release created with all 3 platform binaries
- [ ] Test download and execution of at least one binary

## Troubleshooting

### Version Mismatch Error

**Symptom**: Workflow fails with "Version mismatch" in validation

**Solution**: Ensure Cargo.toml version matches git tag:
```bash
# If tag is v3.1.0, Cargo.toml must have:
version = "3.1.0"
```

### Missing Changelog Entry

**Symptom**: Workflow fails with "CHANGELOG.md missing entry"

**Solution**: Add version section to CHANGELOG.md:
```markdown
## [3.1.0] - 2025-11-25
```

### Build Failures

**Symptom**: Build job fails for specific platform

**Solution**:
1. Check if code compiles for target locally:
   ```bash
   rustup target add x86_64-unknown-linux-gnu
   cargo build --target x86_64-unknown-linux-gnu --release
   ```
2. Review build logs in GitHub Actions
3. Fix platform-specific issues and push new tag

### Binary Not Stripped

**Symptom**: Binary size larger than expected

**Solution**: Workflow uses `RUSTFLAGS="-C strip=symbols"` and explicit `strip` command. Verify in artifact size (should be <10 MB for typical release).

## Framework Compliance

**UCE34**: Q1-Q34 systematic release validation
- Q1-Q9: Pre-flight validation (version, tests, lints)
- Q10-Q12: Build optimization (release mode, symbol stripping)
- Q30-Q34: Audit trails (changelog extraction, release notes)

**Chaos**: 100% lockfree binary verification
- Pre-release tests ensure no mutex violations
- Clippy lint enforcement

**T28**: 5-tier testing
- P0 unit tests run in pre-flight
- Full test suite should pass locally before release

**B32**: Fair performance claims
- Binaries built with same optimization flags
- Symbol stripping for consistent size

**Q34**: Audit trail compliance
- Changelog extraction from CHANGELOG.md
- Version tracking in release metadata
- GitHub release notes provide tamper-evident record

## Performance

**Total workflow time**: 15-30 minutes (typical)

**Breakdown**:
- Pre-flight validation: 2-5 min
- Build matrix (3 platforms parallel): 10-20 min
- Create release: 1-2 min
- Post-release checks: <1 min

**Optimization**:
- Parallel platform builds reduce total time 3×
- Rust cache (Swatinem/rust-cache@v2) reduces subsequent builds 50%
- Symbol stripping reduces artifact size 40-60%

## References

- **Workflow file**: `.github/workflows/release.yml` (339 lines)
- **Test workflow**: `.github/workflows/test.yml` (reference for patterns)
- **Framework docs**: `/home/samuel/CLAUDE.md` (UCE34, Chaos, T28, B32, Q34)
- **GitHub Actions**: https://docs.github.com/en/actions
