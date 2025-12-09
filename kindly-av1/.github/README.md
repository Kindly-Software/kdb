# GitHub Actions Configuration

This directory contains GitHub Actions workflows for the kindly-av1 project.

## Workflows

### Release Build (`workflows/release.yml`)

Automated multi-platform release builds with code signing and checksums.

**Quick Start:**
```bash
git tag v1.0.0
git push origin v1.0.0
```

**Platforms:** Linux x86_64 (musl), Windows x86_64 (MSVC), macOS x86_64 (Intel), macOS ARM64 (Apple Silicon)

**Security:** macOS notarization, optional Windows signing, SHA256 checksums, SLSA provenance

**Full Documentation:** See [RELEASE_WORKFLOW_GUIDE.md](RELEASE_WORKFLOW_GUIDE.md)

## Setup Requirements

### Required (All Platforms)

No setup needed for basic builds. Workflow runs automatically on tag push.

### Optional: macOS Code Signing

For notarized macOS binaries, configure these secrets:

| Secret | How to Get |
|--------|------------|
| `MACOS_CERTIFICATE` | Export Developer ID cert as .p12, convert to base64 |
| `MACOS_CERTIFICATE_PWD` | Password used when exporting certificate |
| `MACOS_SIGNING_IDENTITY` | Full identity from `security find-identity` |
| `APPLE_ID` | Your Apple ID email |
| `APPLE_TEAM_ID` | 10-char team ID from developer.apple.com |
| `APPLE_APP_PASSWORD` | App-specific password from appleid.apple.com |
| `KEYCHAIN_PWD` | Any strong random password |

**Detailed Setup:** [RELEASE_WORKFLOW_GUIDE.md § macOS Code Signing Setup](RELEASE_WORKFLOW_GUIDE.md#macos-code-signing-setup)

### Optional: Windows Code Signing

For signed Windows binaries, configure these secrets:

| Secret | How to Get |
|--------|------------|
| `WINDOWS_CERTIFICATE` | Export code signing cert as .pfx, convert to base64 |
| `WINDOWS_CERTIFICATE_PWD` | Password used when exporting certificate |

**Detailed Setup:** [RELEASE_WORKFLOW_GUIDE.md § Windows Code Signing Setup](RELEASE_WORKFLOW_GUIDE.md#windows-code-signing-setup-optional)

## Quick Reference

### Trigger Release

```bash
# Create annotated tag
git tag -a v1.2.3 -m "Release v1.2.3"

# Push tag to GitHub
git push origin v1.2.3
```

### Monitor Build

1. Go to **Actions** tab in GitHub repository
2. Click on **Release Build** workflow
3. Watch build progress for all 4 platforms

### Publish Release

1. Build completes → Draft release created automatically
2. Review artifacts and checksums
3. Edit release notes if needed
4. Click **Publish release**

### Verify Download

**Linux/macOS:**
```bash
shasum -a 256 -c kindly-av1-x86_64-unknown-linux-musl.tar.gz.sha256
```

**Windows:**
```powershell
(Get-FileHash kindly-av1-x86_64-pc-windows-msvc.zip).Hash -eq (Get-Content kindly-av1-x86_64-pc-windows-msvc.zip.sha256 | Select-String -Pattern "[a-fA-F0-9]{64}").Matches.Value
```

## Files

```
.github/
├── workflows/
│   └── release.yml                # Main release workflow
├── RELEASE_WORKFLOW_GUIDE.md      # Comprehensive setup guide
└── README.md                      # This file
```

## Troubleshooting

### Build fails on one platform

- **Fail-fast disabled:** Other platforms continue building
- **Partial releases:** Can release working platforms while fixing failures
- **Logs:** Click failed job in Actions tab for detailed error messages

### macOS notarization fails

**Common causes:**
- Expired Apple Developer agreement (visit appstoreconnect.apple.com)
- Incorrect `APPLE_APP_PASSWORD` (must be app-specific, not main password)
- Wrong `APPLE_TEAM_ID` (10 characters from developer.apple.com/account)

**Solution:** Check [RELEASE_WORKFLOW_GUIDE.md § Troubleshooting](RELEASE_WORKFLOW_GUIDE.md#troubleshooting)

### Windows signing fails

**If secrets configured:**
- Check certificate expiration: `certutil -dump certificate.pfx`
- Verify timestamp server accessible (retry if DigiCert down)

**If secrets not configured:**
- Windows signing is **optional** - workflow skips it automatically
- Binaries work but trigger SmartScreen warnings on first run

## Security

### SLSA Compliance

This workflow provides **SLSA Level 1** provenance:
- ✅ Automated builds (GitHub Actions)
- ✅ Traceable source (commit SHA)
- ✅ Signed checksums (SHA256)
- ⚠️ No tamper protection (Level 2+ required)

### Pinned Action Versions

All actions use **commit SHA** pinning for supply chain security:

```yaml
uses: actions/checkout@692973e3d937129bcbf40652eb9f2f61becf3332 # v4.1.7
```

**Why?** Tags can be changed by attackers; commit SHAs cannot.

### Secret Safety

- ✅ All sensitive values stored as GitHub Secrets
- ✅ Auto-redacted in workflow logs
- ✅ Minimal permissions (`contents: write` only)
- ✅ Temporary keychains/certificates (cleaned after build)

## Performance

**Typical workflow times:**
- **Parallel builds:** 15-20 minutes (4 platforms simultaneously)
- **Caching:** Rust dependencies cached (saves 5-10 min/build)
- **Artifacts:** Uploaded to GitHub Releases automatically

## Documentation

- **Comprehensive Guide:** [RELEASE_WORKFLOW_GUIDE.md](RELEASE_WORKFLOW_GUIDE.md) (14,000+ words)
  - Setup instructions for macOS/Windows code signing
  - Troubleshooting common issues
  - Security best practices
  - SLSA provenance details
  - Migration from Travis/CircleCI/GitLab

- **GitHub Actions Docs:** [Building and testing Rust](https://docs.github.com/en/actions/use-cases-and-examples/building-and-testing/building-and-testing-rust)

## Support

**Issues?** Open a GitHub issue with:
- Workflow run URL (Actions tab → failed run)
- Error messages from logs
- Platform(s) affected
- Whether code signing configured

**Questions?** See [RELEASE_WORKFLOW_GUIDE.md](RELEASE_WORKFLOW_GUIDE.md) first - covers 95% of common scenarios.
