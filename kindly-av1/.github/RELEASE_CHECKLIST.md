# Release Checklist for kindly-av1

Quick reference for creating multi-platform releases.

## Pre-Release Checklist

### Code & Testing

- [ ] All tests passing locally
  ```bash
  cargo test --all-features
  ```

- [ ] All tests passing on kindly-hub (T28 requirement)
  ```bash
  ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo test --all-features"
  ```

- [ ] Benchmarks validated (B32 requirement)
  ```bash
  ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench"
  ```

- [ ] Clippy checks clean
  ```bash
  cargo clippy --all-features -- -D warnings
  ```

- [ ] Code formatted
  ```bash
  cargo fmt --all --check
  ```

### Version & Documentation

- [ ] Version bumped in `Cargo.toml`
  ```toml
  [package]
  version = "1.0.0"  # Update this
  ```

- [ ] CHANGELOG.md updated with release notes
  - New features
  - Bug fixes
  - Breaking changes
  - Performance improvements

- [ ] README.md up to date
  - Installation instructions
  - Usage examples
  - System requirements

- [ ] LICENSE file present (required for Gumroad distribution)

### Security & Compliance

- [ ] No hardcoded secrets or API keys
  ```bash
  git grep -i "api_key\|secret\|password" src/
  ```

- [ ] TRADE_SECRET_NOTICE.md reviewed (if applicable)

- [ ] GitHub secrets configured (optional, see SECRETS_SETUP_GUIDE.md)
  - [ ] macOS signing secrets (7 secrets)
  - [ ] Windows signing secrets (2 secrets)

- [ ] SLSA provenance will be generated automatically

## Release Process

### 1. Create Git Tag

```bash
# Format: v{MAJOR}.{MINOR}.{PATCH}
VERSION="1.0.0"
git tag -a "v${VERSION}" -m "Release v${VERSION}"

# Verify tag
git tag -l "v${VERSION}"
git show "v${VERSION}"
```

### 2. Push Tag to GitHub

```bash
# This triggers the release workflow
git push origin "v${VERSION}"
```

### 3. Monitor Workflow

1. Go to: `https://github.com/YOUR_ORG/kindly-av1/actions`
2. Click on "Release Build" workflow
3. Monitor parallel builds:
   - ✅ Build Linux x86_64
   - ✅ Build Windows x86_64
   - ✅ Build macOS x86_64
   - ✅ Build macOS ARM64
4. Wait for "Create GitHub Release" job
5. Check "SLSA Provenance" job

**Expected Runtime**: 15-20 minutes total

### 4. Verify Draft Release

1. Go to: `https://github.com/YOUR_ORG/kindly-av1/releases`
2. Find draft release for `v{VERSION}`
3. Verify artifacts:
   - [ ] `kindly-av1-x86_64-unknown-linux-musl.tar.gz`
   - [ ] `kindly-av1-x86_64-pc-windows-msvc.zip`
   - [ ] `kindly-av1-x86_64-apple-darwin.tar.gz`
   - [ ] `kindly-av1-aarch64-apple-darwin.tar.gz`
   - [ ] Checksum files (`.sha256` for each)
   - [ ] `slsa-provenance.md`

### 5. Test Binaries

Download and test each platform binary:

**Linux**:
```bash
wget https://github.com/YOUR_ORG/kindly-av1/releases/download/v${VERSION}/kindly-av1-x86_64-unknown-linux-musl.tar.gz
tar -xzf kindly-av1-x86_64-unknown-linux-musl.tar.gz
cd kindly-av1-x86_64-unknown-linux-musl
./kindly-av1 --version
./kindly-av1 help
```

**Windows**:
```powershell
# Download ZIP from release page
Expand-Archive kindly-av1-x86_64-pc-windows-msvc.zip
cd kindly-av1-x86_64-pc-windows-msvc
.\kindly-av1.exe --version

# Verify signature (if signed)
Get-AuthenticodeSignature .\kindly-av1.exe
```

**macOS**:
```bash
# Download tar.gz for your architecture
curl -LO https://github.com/YOUR_ORG/kindly-av1/releases/download/v${VERSION}/kindly-av1-x86_64-apple-darwin.tar.gz
tar -xzf kindly-av1-x86_64-apple-darwin.tar.gz
cd kindly-av1-x86_64-apple-darwin

# First run may trigger Gatekeeper (if notarized)
./kindly-av1 --version

# Verify signature (if signed)
codesign -dvv kindly-av1
spctl -a -vv kindly-av1
```

### 6. Verify Checksums

```bash
# Linux/macOS
shasum -a 256 -c kindly-av1-x86_64-unknown-linux-musl.tar.gz.sha256

# Windows
certutil -hashfile kindly-av1-x86_64-pc-windows-msvc.zip SHA256
```

### 7. Edit Release Notes

1. Click "Edit" on draft release
2. Review auto-generated notes
3. Add highlights section:
   ```markdown
   ## Highlights

   - 🚀 GPU-accelerated AV1 encoding (10-100× speedup)
   - 💾 Checkpoint/resume for crash-safe encoding
   - 📊 Real-time TUI dashboard with encoding metrics
   - 🔒 Offline license verification

   ## Installation

   Download the appropriate archive for your platform, extract, and run.

   ### Quick Start

   ```bash
   # Encode video
   kindly-av1 encode input.mp4 -o output.av1 --preset medium --crf 28

   # Check license status
   kindly-av1 license status
   ```

   ## System Requirements

   - **Linux**: x86_64, glibc 2.31+ (or use musl static binary)
   - **Windows**: x86_64, Windows 10 1809+
   - **macOS**: x86_64 or ARM64, macOS 11.0+

   ## GPU Acceleration (Optional)

   - **AMD**: ROCm 6.0+ (Linux only)
   - **NVIDIA**: CUDA 12.0+ (Linux/Windows)
   - **Vulkan**: 1.3+ (all platforms)

   ## Breaking Changes

   None - initial release.

   ## Known Issues

   - GPU acceleration requires compatible hardware (see docs)
   - macOS Gatekeeper may require manual approval on first run
   ```

4. Add upgrade instructions (if applicable)
5. Add deprecation warnings (if applicable)

### 8. Publish Release

1. Review all details one final time
2. Click "Publish release"
3. Release is now public at: `https://github.com/YOUR_ORG/kindly-av1/releases/tag/v${VERSION}`

## Post-Release Checklist

### Gumroad Distribution

- [ ] Upload release archives to Gumroad
  - Linux, Windows, macOS (Intel), macOS (ARM)
- [ ] Update Gumroad product description
- [ ] Set price and licensing terms
- [ ] Test purchase flow
- [ ] Verify license key generation

### Documentation

- [ ] Update website download links
- [ ] Update installation guide
- [ ] Announce release on:
  - [ ] Twitter/X
  - [ ] Reddit (r/AV1)
  - [ ] Discord
  - [ ] Email list

### Monitoring

- [ ] Monitor GitHub issues for bug reports
- [ ] Monitor download statistics
- [ ] Monitor Gumroad sales/support tickets
- [ ] Check code signing verification (if applicable)

### Cleanup

- [ ] Delete test tags
  ```bash
  git tag -d v0.0.1-test
  git push origin :refs/tags/v0.0.1-test
  ```

- [ ] Archive old releases (keep last 3 major versions)

## Hotfix Workflow

For critical bugs requiring immediate patch:

1. Create hotfix branch from tag
   ```bash
   git checkout -b hotfix/v1.0.1 v1.0.0
   ```

2. Fix bug, commit, test thoroughly

3. Bump patch version in Cargo.toml

4. Create new tag
   ```bash
   git tag -a v1.0.1 -m "Hotfix: Critical bug fix"
   git push origin v1.0.1
   ```

5. Follow release process above (faster review)

## Rollback Procedure

If release has critical issues:

1. **Immediate**:
   - Delete release from GitHub
   - Remove Gumroad downloads
   - Post incident notice

2. **Investigation**:
   - Identify root cause
   - Create issue tracker ticket
   - Assign owner

3. **Fix**:
   - Follow hotfix workflow
   - Increment patch version
   - Thorough testing

4. **Communication**:
   - Email affected users
   - Post on social media
   - Update documentation

## Troubleshooting

### Build Failures

**"Rust toolchain not found"**
- Workflow uses nightly toolchain
- Check rust-toolchain.toml is committed

**"musl-tools not found" (Linux)**
- Workflow installs automatically
- Check ubuntu-latest runner has package manager access

**"Certificate import failed" (macOS)**
- Verify secrets are base64 encoded correctly
- Check password matches certificate export
- See SECRETS_SETUP_GUIDE.md

**"SignTool not found" (Windows)**
- Workflow uses Windows SDK 10.0.22621.0
- Update path if SDK version changes

### Notarization Issues

**"Invalid credentials"**
- Verify APPLE_ID is correct
- Use app-specific password (not Apple ID password)
- Check APPLE_TEAM_ID is 10 characters

**"Notarization rejected"**
- Binary must be signed first
- Check hardened runtime enabled
- Verify timestamp server accessible

### Checksum Mismatches

**"SHA256 doesn't match"**
- Re-download artifact
- Check file corruption during download
- Verify download from official GitHub release

## Emergency Contacts

- **GitHub Actions Issues**: https://github.com/actions/runner/issues
- **Apple Developer Support**: https://developer.apple.com/support/
- **Code Signing CA**: Check your certificate provider

## References

- [Workflow Architecture](.github/WORKFLOW_ARCHITECTURE.md)
- [Security Checklist](.github/SECURITY_CHECKLIST.md)
- [Secrets Setup](.github/SECRETS_SETUP_GUIDE.md)
- [Semantic Versioning](https://semver.org/)
- [SLSA Framework](https://slsa.dev/)

---

**Last Updated**: 2025-11-29
**Workflow Version**: v1.0
**Maintainer**: Kindly Team
