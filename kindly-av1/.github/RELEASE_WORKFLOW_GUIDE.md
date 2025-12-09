# GitHub Actions Release Workflow Guide

## Overview

This workflow automates secure multi-platform builds for `kindly-av1` with code signing, checksums, and SLSA provenance tracking.

**Workflow File:** `.github/workflows/release.yml`

## Features

1. **Multi-Platform Builds** - Linux x86_64, Windows x86_64, macOS x86_64, macOS ARM64
2. **Code Signing** - macOS notarization (mandatory), Windows signtool (optional)
3. **Security** - SHA256 checksums, SLSA provenance, pinned action versions
4. **Release Automation** - Automatic GitHub Releases with draft approval

## Supported Platforms

| Platform | Target Triple | Runner | Archive Format |
|----------|---------------|--------|----------------|
| **Linux x86_64** | `x86_64-unknown-linux-musl` | `ubuntu-latest` | `.tar.gz` |
| **Windows x86_64** | `x86_64-pc-windows-msvc` | `windows-latest` | `.zip` |
| **macOS Intel** | `x86_64-apple-darwin` | `macos-13` | `.tar.gz` |
| **macOS Apple Silicon** | `aarch64-apple-darwin` | `macos-14` | `.tar.gz` |

**Why musl for Linux?** Static linking ensures binary runs on any Linux distro without glibc version conflicts.

**Why different macOS runners?** macOS-13 has Intel hardware (x86_64), macOS-14 has Apple Silicon (ARM64) for native builds.

## Triggering the Workflow

### Automatic (Recommended)

Create and push a version tag:

```bash
git tag v1.0.0
git push origin v1.0.0
```

**Tag Format:** `v[MAJOR].[MINOR].[PATCH]` (e.g., `v1.0.0`, `v2.3.1`)

### Manual (Testing)

Navigate to **Actions** → **Release Build** → **Run workflow** → Select branch

## macOS Code Signing Setup

### Prerequisites

1. **Apple Developer Account** - $99/year subscription required
2. **Developer ID Application Certificate** - For signing binaries outside the App Store

### Step 1: Create Certificate

1. Visit [Apple Developer Certificates](https://developer.apple.com/account/resources/certificates/list)
2. Click **+** to create a new certificate
3. Select **Developer ID Application** (not "Developer ID Installer" - that's for .pkg files)
4. Follow CSR generation instructions using Keychain Access
5. Download certificate and install in Keychain Access

### Step 2: Export Certificate

1. Open **Keychain Access** on macOS
2. Find your **Developer ID Application** certificate
3. Right-click → **Export "Developer ID Application: Your Name"**
4. Save as `.p12` file with a strong password
5. Convert to base64:

```bash
base64 -i certificate.p12 -o certificate.base64.txt
```

### Step 3: Create App-Specific Password

1. Visit [Apple ID Account](https://appleid.apple.com/account/manage)
2. Sign in with your Apple ID
3. Navigate to **Security** → **App-Specific Passwords**
4. Click **+** to generate a new password
5. Name it "GitHub Actions Notarization"
6. Save the generated password (you won't see it again)

### Step 4: Configure GitHub Secrets

Add these secrets to your repository (**Settings** → **Secrets and variables** → **Actions** → **New repository secret**):

| Secret Name | Value | Description |
|-------------|-------|-------------|
| `MACOS_CERTIFICATE` | Contents of `certificate.base64.txt` | Base64-encoded .p12 certificate |
| `MACOS_CERTIFICATE_PWD` | Password used when exporting .p12 | Certificate private key password |
| `MACOS_SIGNING_IDENTITY` | `Developer ID Application: Your Name (TEAM_ID)` | Full identity string from certificate |
| `APPLE_ID` | `your.email@example.com` | Your Apple ID email |
| `APPLE_TEAM_ID` | `AB12CD34EF` | 10-character team ID from developer.apple.com |
| `APPLE_APP_PASSWORD` | App-specific password from Step 3 | For notarytool authentication |
| `KEYCHAIN_PWD` | Any strong random password | Temporary keychain password (not stored) |

**Finding Your Team ID:**
1. Visit [Apple Developer Membership](https://developer.apple.com/account/#!/membership/)
2. Your Team ID is shown in the membership details (10 characters, e.g., `ABC123XYZ9`)

**Finding Your Signing Identity:**
```bash
security find-identity -v -p codesigning
```

Look for the line with "Developer ID Application" - copy the entire string in quotes.

### Step 5: Verify Configuration

Push a test tag and check the workflow logs:

```bash
git tag v0.0.1-test
git push origin v0.0.1-test
```

**Expected macOS Steps:**
1. ✅ Import Apple certificates - Creates temporary keychain
2. ✅ Codesign binary - Signs with Developer ID, adds hardened runtime
3. ✅ Notarize binary - Submits to Apple, waits for approval (~2-5 minutes)

**Common Issues:**
- **"User interaction is not allowed"** - `KEYCHAIN_PWD` incorrect or keychain locked
- **"No identity found"** - `MACOS_SIGNING_IDENTITY` doesn't match certificate
- **"Invalid credentials"** - Check `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD`
- **"Agreement update required"** - Visit [App Store Connect](https://appstoreconnect.apple.com/) and accept new terms

## Windows Code Signing Setup (Optional)

**Note:** Windows code signing is **optional**. Unsigned binaries work but trigger SmartScreen warnings.

### Prerequisites

1. **Code Signing Certificate** - From DigiCert, Sectigo, GlobalSign, etc. (~$100-400/year)
2. **Certificate Type** - Standard code signing (EV certificates require hardware token)

### Step 1: Export Certificate

1. Export your code signing certificate as `.pfx` file with private key
2. Convert to base64:

```powershell
certutil -encode certificate.pfx certificate.base64.txt
```

Or on Linux/macOS:
```bash
base64 -i certificate.pfx -o certificate.base64.txt
```

### Step 2: Configure GitHub Secrets

Add these secrets:

| Secret Name | Value | Description |
|-------------|-------|-------------|
| `WINDOWS_CERTIFICATE` | Contents of `certificate.base64.txt` | Base64-encoded .pfx certificate |
| `WINDOWS_CERTIFICATE_PWD` | Password used when exporting .pfx | Certificate private key password |

### Step 3: Verify Configuration

The workflow automatically checks if `WINDOWS_CERTIFICATE` exists. If present, it signs the binary. If absent, it skips signing (no error).

**Expected Windows Steps (if configured):**
1. ✅ Sign binary - Uses signtool.exe with SHA256 + timestamp
2. ✅ Verify signature - Confirms signature validity

**EV Certificates:** If you have an Extended Validation (EV) certificate requiring a hardware USB token, you'll need to use [self-hosted runners](https://docs.github.com/en/actions/hosting-your-own-runners) with the token plugged in. Cloud runners cannot access physical tokens.

## Linux Build Details

**Target:** `x86_64-unknown-linux-musl` (static binary)

**Why musl instead of glibc?**
- **Portability:** Works on any Linux distro (Ubuntu, Fedora, Alpine, etc.)
- **No dependencies:** Fully static binary, no runtime library requirements
- **Smaller size:** Typically 10-20% smaller than glibc builds
- **Reproducibility:** Consistent behavior across environments

**Installation:** Workflow automatically installs `musl-tools` on Ubuntu runners.

## Build Optimization

The workflow uses aggressive optimization flags:

```bash
RUSTFLAGS="-C target-cpu=native -C opt-level=3 -C lto=fat -C codegen-units=1"
```

**Flag Breakdown:**
- `target-cpu=native` - Optimizes for GitHub Actions runner CPUs (may not work on older CPUs)
- `opt-level=3` - Maximum runtime speed (slower compilation)
- `lto=fat` - Full link-time optimization across all dependencies
- `codegen-units=1` - Single codegen unit for maximum optimization (slower build)

**Trade-offs:**
- ✅ **30-50% faster binaries** (typical)
- ❌ **2-5× slower compilation** (acceptable for releases)
- ⚠️ **Binaries may not work on CPUs older than 2018** (due to `target-cpu=native`)

**For maximum compatibility,** remove `target-cpu=native` and use `target-cpu=x86-64-v2` (2008+ CPUs).

## Release Process

1. **Build Job** - Compiles binaries for all platforms in parallel
2. **Release Job** - Creates GitHub Release (draft mode)
3. **SLSA Provenance Job** - Generates build attestation

### Generated Artifacts

Each platform produces:

```
kindly-av1-{target}.{tar.gz|zip}       # Binary + docs
kindly-av1-{target}.{tar.gz|zip}.sha256  # SHA256 checksum
```

**Example:**
```
kindly-av1-x86_64-unknown-linux-musl.tar.gz
kindly-av1-x86_64-unknown-linux-musl.tar.gz.sha256
kindly-av1-x86_64-apple-darwin.tar.gz
kindly-av1-x86_64-apple-darwin.tar.gz.sha256
kindly-av1-aarch64-apple-darwin.tar.gz
kindly-av1-aarch64-apple-darwin.tar.gz.sha256
kindly-av1-x86_64-pc-windows-msvc.zip
kindly-av1-x86_64-pc-windows-msvc.zip.sha256
```

### Archive Contents

```
kindly-av1-{target}/
├── kindly-av1 (or kindly-av1.exe)
├── README.md
├── LICENSE
└── CHANGELOG.md
```

### Checksum Verification

Users can verify downloads:

**Linux/macOS:**
```bash
shasum -a 256 -c kindly-av1-x86_64-unknown-linux-musl.tar.gz.sha256
```

**Windows PowerShell:**
```powershell
(Get-FileHash kindly-av1-x86_64-pc-windows-msvc.zip -Algorithm SHA256).Hash -eq (Get-Content kindly-av1-x86_64-pc-windows-msvc.zip.sha256 | Select-String -Pattern "[a-fA-F0-9]{64}").Matches.Value
```

## SLSA Provenance

The workflow generates [SLSA Level 1](https://slsa.dev/) provenance metadata:

- **Workflow name and ID** - Traceable to specific GitHub Actions run
- **Commit SHA** - Exact source code version
- **Actor** - Who triggered the build
- **SHA256 checksums** - All artifact hashes

**Provenance File:** `slsa-provenance.md` (attached to release)

**Why SLSA?**
- **Supply chain security** - Proves binaries built from official source
- **Tamper detection** - Verifies no modifications after build
- **Audit trail** - Full transparency of build process

**SLSA Level 1 Limitations:**
- Does not prevent build system tampering (Level 2+ required)
- Manual verification required (not automated)

**Future:** Migrate to [slsa-framework/slsa-github-generator](https://github.com/slsa-framework/slsa-github-generator) for Level 3 provenance (automated verification).

## Security Best Practices

### 1. Pinned Action Versions

All actions use **commit SHA** pinning (most secure):

```yaml
uses: actions/checkout@692973e3d937129bcbf40652eb9f2f61becf3332 # v4.1.7
```

**Why?**
- **Immutability** - Tag `v4.1.7` can be changed by attacker; SHA cannot
- **Supply chain security** - Prevents malicious updates to dependencies
- **Compliance** - Required for [SLSA Level 2+](https://slsa.dev/)

**Maintenance:**
- Dependabot automatically updates SHA pins when new versions released
- Comments (`# v4.1.7`) preserve semantic version for readability

### 2. Minimal Permissions

```yaml
permissions:
  contents: write  # Only permission granted
```

**Principle of least privilege:**
- Workflow only needs `contents: write` to create releases
- No access to secrets, workflows, packages, etc.
- Reduces blast radius if workflow compromised

### 3. Secret Management

**DO:**
- ✅ Use GitHub Secrets for all sensitive values
- ✅ Never log secret values (GitHub auto-redacts in logs)
- ✅ Use separate secrets for different certificates/environments
- ✅ Rotate secrets periodically (at least annually)

**DON'T:**
- ❌ Hardcode certificates/passwords in workflow files
- ❌ Echo secrets to logs (e.g., `echo $MACOS_CERTIFICATE`)
- ❌ Store secrets in repository code/commits
- ❌ Share secrets across multiple repositories

### 4. Artifact Retention

```yaml
retention-days: 7  # Build artifacts deleted after 7 days
retention-days: 90 # SLSA provenance kept for 90 days
```

**Why limited retention?**
- **Cost** - GitHub charges for artifact storage
- **Security** - Reduces attack surface (fewer old artifacts to compromise)
- **Compliance** - Release assets stored permanently in GitHub Releases

### 5. Draft Releases

```yaml
draft: true  # Releases created as drafts
```

**Manual approval required:**
1. Workflow creates draft release with all artifacts
2. Maintainer reviews artifacts, checksums, SLSA provenance
3. Maintainer publishes release (or deletes if issues found)

**Benefits:**
- Prevents accidental releases from broken builds
- Allows testing downloads before public announcement
- Time to write release notes and changelog

### 6. Fail-Fast Strategy

```yaml
fail-fast: false  # Continue building other platforms if one fails
```

**Why disabled?**
- **Complete feedback** - See all platform failures at once
- **Partial releases** - Can release working platforms while fixing failures
- **CI efficiency** - Don't waste runner time stopping mid-build

## Troubleshooting

### Build Failures

**Error:** `cargo build failed with exit code 101`

**Solution:** Check if code compiles locally with same target:
```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

**Error:** `musl-gcc: command not found`

**Solution:** Workflow should install `musl-tools` automatically. If missing, add to workflow:
```yaml
- name: Install musl tools
  run: sudo apt-get update && sudo apt-get install -y musl-tools
```

### macOS Signing Failures

**Error:** `errSecInternalComponent` or `User interaction is not allowed`

**Solution:** Keychain password incorrect. Verify `KEYCHAIN_PWD` secret matches workflow.

**Error:** `No identity found for signing`

**Solution:** `MACOS_SIGNING_IDENTITY` doesn't match certificate. Run locally:
```bash
security find-identity -v -p codesigning
```

Copy the **exact** string from the output (including team ID in parentheses).

**Error:** `Invalid credentials` during notarization

**Solution:** One of these is wrong:
- `APPLE_ID` - Must be your Apple ID email (exact match)
- `APPLE_TEAM_ID` - 10-character team ID from developer.apple.com
- `APPLE_APP_PASSWORD` - App-specific password (not your Apple ID password)

**Error:** `Could not find the RequestUUID` or `Agreement update required`

**Solution:** Apple changed developer agreement. Visit [App Store Connect](https://appstoreconnect.apple.com/) and accept new terms.

### Windows Signing Failures

**Error:** `SignTool Error: No certificates were found that met all the given criteria`

**Solution:** Certificate thumbprint incorrect or certificate expired. Verify:
```powershell
certutil -dump certificate.pfx
```

**Error:** `The specified timestamp server either could not be reached or returned an invalid response`

**Solution:** DigiCert timestamp server temporarily down. Retry workflow or change timestamp URL:
```yaml
& $signtool sign /tr http://timestamp.sectigo.com /td SHA256 $binary
```

### Release Creation Failures

**Error:** `Resource not accessible by integration`

**Solution:** Missing `contents: write` permission. Check workflow:
```yaml
permissions:
  contents: write
```

**Error:** `Release already exists for tag v1.0.0`

**Solution:** Delete existing release/tag before re-running:
```bash
git tag -d v1.0.0
git push origin :refs/tags/v1.0.0
```

Then delete the GitHub Release via web UI.

## Performance

**Typical Workflow Times:**

| Job | Duration | Parallel? |
|-----|----------|-----------|
| Linux build | 8-12 minutes | ✅ |
| Windows build | 10-15 minutes | ✅ |
| macOS x86_64 build | 12-18 minutes | ✅ |
| macOS ARM64 build | 12-18 minutes | ✅ |
| Release creation | 1-2 minutes | After builds |
| SLSA provenance | <1 minute | After builds |

**Total:** ~15-20 minutes (parallelized)

**Optimization Tips:**
1. **Rust cache** - Swatinem/rust-cache saves 5-10 minutes per build
2. **Remove `lto=fat`** - Cuts build time in half (costs 10-15% performance)
3. **Use `cargo build` instead of `cargo build --release`** - 5× faster (for testing only)

## Migration from Other CI Systems

### From Travis CI

**Travis CI:**
```yaml
os:
  - linux
  - osx
  - windows
```

**GitHub Actions:**
```yaml
matrix:
  include:
    - os: ubuntu-latest
    - os: macos-latest
    - os: windows-latest
```

### From CircleCI

**CircleCI:**
```yaml
executors:
  linux: ubuntu-2004
  macos: macos-13
```

**GitHub Actions:**
```yaml
runs-on: ${{ matrix.os }}
matrix:
  os: [ubuntu-latest, macos-13]
```

### From GitLab CI

**GitLab CI:**
```yaml
build:linux:
  image: rust:latest
  script:
    - cargo build --release
```

**GitHub Actions:**
```yaml
- uses: dtolnay/rust-toolchain@stable
- run: cargo build --release
```

## References

### Documentation
- [GitHub Actions: Building and testing Rust](https://docs.github.com/en/actions/use-cases-and-examples/building-and-testing/building-and-testing-rust)
- [GitHub Actions: Security best practices](https://docs.github.com/en/actions/reference/security/secure-use)
- [Apple Notarization Guide](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
- [SLSA Framework](https://slsa.dev/)

### Research Sources
- [Cross Compiling Rust Projects in GitHub Actions](https://blog.urth.org/2023/03/05/cross-compiling-rust-projects-in-github-actions/)
- [Automatic Code-signing and Notarization for macOS apps using GitHub Actions](https://federicoterzi.com/blog/automatic-code-signing-and-notarization-for-macos-apps-using-github-actions/)
- [Automatic Code-signing on Windows using GitHub Actions](https://federicoterzi.com/blog/automatic-codesigning-on-windows-using-github-actions/)
- [GitHub Actions Security Best Practices](https://medium.com/@amareswer/github-actions-security-best-practices-1d3f33cdf705)
- [Achieving SLSA 3 Compliance with GitHub Actions](https://github.blog/security/supply-chain-security/slsa-3-compliance-with-github-actions/)

### Tools
- [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain) - Rust toolchain installation
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache) - Cargo caching action
- [actions/upload-artifact](https://github.com/actions/upload-artifact) - Artifact upload
- [softprops/action-gh-release](https://github.com/softprops/action-gh-release) - GitHub Releases

## License

This workflow configuration is part of the kindly-av1 project. See LICENSE for details.
