# .github Documentation Index

Quick navigation for GitHub Actions release workflow documentation.

## Quick Start

**Want to trigger a release?** See [README.md § Quick Reference](README.md#quick-reference)

**First time setup?** See [RELEASE_WORKFLOW_GUIDE.md § macOS Code Signing Setup](RELEASE_WORKFLOW_GUIDE.md#macos-code-signing-setup)

**Troubleshooting?** See [RELEASE_WORKFLOW_GUIDE.md § Troubleshooting](RELEASE_WORKFLOW_GUIDE.md#troubleshooting)

## Files Overview

| File | Lines | Purpose | Audience |
|------|-------|---------|----------|
| **[workflows/release.yml](workflows/release.yml)** | 290 | Main workflow file | Maintainers (edit) |
| **[README.md](README.md)** | 147 | Quick reference | Developers (quick start) |
| **[RELEASE_WORKFLOW_GUIDE.md](RELEASE_WORKFLOW_GUIDE.md)** | 745 | Comprehensive guide | New maintainers (setup) |
| **[SECRETS_SETUP_GUIDE.md](SECRETS_SETUP_GUIDE.md)** | 456 | Secret configuration | New maintainers (first-time setup) |
| **[RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md)** | 389 | Release checklist | Release managers (every release) |
| **[SECURITY_CHECKLIST.md](SECURITY_CHECKLIST.md)** | 458 | Security checklist | Release managers (verification) |
| **[WORKFLOW_ARCHITECTURE.md](WORKFLOW_ARCHITECTURE.md)** | 412 | Architecture diagrams | Technical leads (understanding) |
| **[DELIVERY_SUMMARY.md](DELIVERY_SUMMARY.md)** | 581 | Implementation summary | Project managers (overview) |
| **[INDEX.md](INDEX.md)** | 239 | This file | Everyone (navigation) |

**Total:** 3,717 lines of documentation

## By Role

### Developer (Contributing Code)

**Triggering a release:**
1. Read: [README.md § Quick Reference](README.md#quick-reference)
2. Command: `git tag -a v1.2.3 -m "Release v1.2.3" && git push origin v1.2.3`
3. Monitor: GitHub Actions tab → Release Build workflow
4. Wait: ~15-20 minutes for draft release

**Verifying a release:**
1. Download: Release artifacts from GitHub Releases
2. Verify: [README.md § Verify Download](README.md#verify-download)

### Maintainer (First-Time Setup)

**macOS code signing setup:**
1. Prerequisites: [SECRETS_SETUP_GUIDE.md § Prerequisites](SECRETS_SETUP_GUIDE.md#prerequisites)
2. Step-by-step: [SECRETS_SETUP_GUIDE.md § macOS Code Signing Setup](SECRETS_SETUP_GUIDE.md#macos-code-signing-setup)
3. Verification: [SECURITY_CHECKLIST.md § Pre-Release Setup](SECURITY_CHECKLIST.md#pre-release-setup-one-time)

**Windows code signing setup (optional):**
1. Prerequisites: [SECRETS_SETUP_GUIDE.md § Prerequisites](SECRETS_SETUP_GUIDE.md#prerequisites-1)
2. Step-by-step: [SECRETS_SETUP_GUIDE.md § Windows Code Signing Setup](SECRETS_SETUP_GUIDE.md#windows-code-signing-setup-optional)
3. Verification: [SECURITY_CHECKLIST.md § Pre-Release Setup](SECURITY_CHECKLIST.md#pre-release-setup-one-time)

### Release Manager (Every Release)

**Complete workflow:**
1. Pre-release: [RELEASE_CHECKLIST.md § Pre-Release Checklist](RELEASE_CHECKLIST.md#pre-release-checklist)
2. Release process: [RELEASE_CHECKLIST.md § Release Process](RELEASE_CHECKLIST.md#release-process)
3. Post-release: [RELEASE_CHECKLIST.md § Post-Release Checklist](RELEASE_CHECKLIST.md#post-release-checklist)

**Alternative (detailed):**
1. Code review: [SECURITY_CHECKLIST.md § Pre-Release Review](SECURITY_CHECKLIST.md#pre-release-review-every-release)
2. Trigger build: [README.md § Quick Reference](README.md#quick-reference)
3. Monitor: [RELEASE_WORKFLOW_GUIDE.md § Release Process](RELEASE_WORKFLOW_GUIDE.md#release-process)
4. Artifact integrity: [SECURITY_CHECKLIST.md § Post-Build Verification](SECURITY_CHECKLIST.md#post-build-verification)
5. Binary testing: [SECURITY_CHECKLIST.md § Binary Testing](SECURITY_CHECKLIST.md#binary-testing)
6. Publish: [SECURITY_CHECKLIST.md § Release Publication](SECURITY_CHECKLIST.md#release-publication)

**Post-release monitoring:**
1. First 24 hours: [SECURITY_CHECKLIST.md § First 24 Hours](SECURITY_CHECKLIST.md#first-24-hours)
2. First week: [SECURITY_CHECKLIST.md § First Week](SECURITY_CHECKLIST.md#first-week)

### Technical Lead (Architecture Understanding)

**Workflow architecture:**
1. Execution flow: [WORKFLOW_ARCHITECTURE.md § Workflow Execution Flow](WORKFLOW_ARCHITECTURE.md#workflow-execution-flow)
2. Security layers: [WORKFLOW_ARCHITECTURE.md § Security Architecture](WORKFLOW_ARCHITECTURE.md#security-architecture)
3. Platform details: [WORKFLOW_ARCHITECTURE.md § Platform Architecture](WORKFLOW_ARCHITECTURE.md#platform-architecture)
4. Performance: [WORKFLOW_ARCHITECTURE.md § Performance Architecture](WORKFLOW_ARCHITECTURE.md#performance-architecture)

**Implementation details:**
1. Design decisions: [DELIVERY_SUMMARY.md § Technical Highlights](DELIVERY_SUMMARY.md#technical-highlights)
2. Research sources: [DELIVERY_SUMMARY.md § Research Sources](DELIVERY_SUMMARY.md#research-sources)
3. Framework compliance: [DELIVERY_SUMMARY.md § UCE34 Framework Compliance](DELIVERY_SUMMARY.md#uce34-framework-compliance)

### Security Auditor (Compliance)

**Security architecture:**
1. SLSA compliance: [WORKFLOW_ARCHITECTURE.md § Security Architecture](WORKFLOW_ARCHITECTURE.md#security-architecture)
2. Action pinning: [RELEASE_WORKFLOW_GUIDE.md § Security Best Practices](RELEASE_WORKFLOW_GUIDE.md#security-best-practices)
3. Secret management: [RELEASE_WORKFLOW_GUIDE.md § Secret Management](RELEASE_WORKFLOW_GUIDE.md#3-secret-management)

**Security checklist:**
1. Pre-release: [SECURITY_CHECKLIST.md § Pre-Release Setup](SECURITY_CHECKLIST.md#pre-release-setup-one-time)
2. Code signing: [SECURITY_CHECKLIST.md § Code Signing Verification](SECURITY_CHECKLIST.md#code-signing-verification)
3. Incident response: [SECURITY_CHECKLIST.md § Incident Response](SECURITY_CHECKLIST.md#incident-response)

### Project Manager (Overview)

**Executive summary:**
1. Delivered features: [DELIVERY_SUMMARY.md § Delivered Files](DELIVERY_SUMMARY.md#delivered-files)
2. Platform support: [DELIVERY_SUMMARY.md § Supported Platforms](DELIVERY_SUMMARY.md#supported-platforms)
3. Cost estimates: [WORKFLOW_ARCHITECTURE.md § Cost Architecture](WORKFLOW_ARCHITECTURE.md#cost-architecture)

**Next steps:**
1. Immediate: [DELIVERY_SUMMARY.md § Immediate](DELIVERY_SUMMARY.md#immediate-before-first-release)
2. Short-term: [DELIVERY_SUMMARY.md § Short-Term](DELIVERY_SUMMARY.md#short-term-first-3-releases)
3. Long-term: [DELIVERY_SUMMARY.md § Long-Term](DELIVERY_SUMMARY.md#long-term-after-10-releases)

## By Task

### Setup

| Task | Document | Section |
|------|----------|---------|
| Configure macOS signing | [SECRETS_SETUP_GUIDE.md](SECRETS_SETUP_GUIDE.md) | § macOS Code Signing Setup |
| Configure Windows signing | [SECRETS_SETUP_GUIDE.md](SECRETS_SETUP_GUIDE.md) | § Windows Code Signing Setup |
| Verify secret configuration | [SECRETS_SETUP_GUIDE.md](SECRETS_SETUP_GUIDE.md) | § Verify Setup |
| First-time verification | [SECURITY_CHECKLIST.md](SECURITY_CHECKLIST.md) | § Pre-Release Setup |
| Test workflow | [README.md](README.md) | § Quick Reference |

### Release

| Task | Document | Section |
|------|----------|---------|
| **Quick workflow** | [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) | § Release Process (all steps) |
| Pre-release checks | [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) | § Pre-Release Checklist |
| Trigger release | [README.md](README.md) | § Quick Reference |
| Monitor build | [RELEASE_WORKFLOW_GUIDE.md](RELEASE_WORKFLOW_GUIDE.md) | § Release Process |
| Verify artifacts | [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) | § Verify Draft Release |
| Test binaries | [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) | § Test Binaries |
| Publish release | [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) | § Publish Release |
| Post-release tasks | [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) | § Post-Release Checklist |

### Troubleshooting

| Issue | Document | Section |
|-------|----------|---------|
| Build failures | [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) | § Build Failures |
| macOS signing failures | [SECRETS_SETUP_GUIDE.md](SECRETS_SETUP_GUIDE.md) | § Troubleshooting macOS Signing |
| macOS notarization failures | [SECRETS_SETUP_GUIDE.md](SECRETS_SETUP_GUIDE.md) | § Troubleshooting macOS Signing |
| Windows signing failures | [SECRETS_SETUP_GUIDE.md](SECRETS_SETUP_GUIDE.md) | § Troubleshooting Windows Signing |
| Checksum mismatches | [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) | § Checksum Mismatches |
| Release rollback | [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) | § Rollback Procedure |
| Incident response | [SECURITY_CHECKLIST.md](SECURITY_CHECKLIST.md) | § Incident Response |

### Understanding

| Topic | Document | Section |
|-------|----------|---------|
| Workflow execution | [WORKFLOW_ARCHITECTURE.md](WORKFLOW_ARCHITECTURE.md) | § Workflow Execution Flow |
| Security layers | [WORKFLOW_ARCHITECTURE.md](WORKFLOW_ARCHITECTURE.md) | § Security Architecture |
| Platform differences | [WORKFLOW_ARCHITECTURE.md](WORKFLOW_ARCHITECTURE.md) | § Platform Architecture |
| Performance optimization | [WORKFLOW_ARCHITECTURE.md](WORKFLOW_ARCHITECTURE.md) | § Performance Architecture |
| Cost analysis | [WORKFLOW_ARCHITECTURE.md](WORKFLOW_ARCHITECTURE.md) | § Cost Architecture |

## By Platform

### Linux (x86_64-unknown-linux-musl)

- **Setup:** No additional setup required
- **Details:** [WORKFLOW_ARCHITECTURE.md § Linux x86_64](WORKFLOW_ARCHITECTURE.md#linux-x86_64-ubuntu-latest)
- **Benefits:** Static binary, runs on any distro, no dependencies
- **Trade-offs:** 5-10% slower than glibc (acceptable for CLI tools)

### Windows (x86_64-pc-windows-msvc)

- **Setup:** [RELEASE_WORKFLOW_GUIDE.md § Windows Code Signing Setup](RELEASE_WORKFLOW_GUIDE.md#windows-code-signing-setup-optional) (optional)
- **Details:** [WORKFLOW_ARCHITECTURE.md § Windows x86_64](WORKFLOW_ARCHITECTURE.md#windows-x86_64-windows-latest)
- **Benefits:** Native Microsoft toolchain, signing optional
- **Trade-offs:** SmartScreen warning if unsigned (users can bypass)

### macOS Intel (x86_64-apple-darwin)

- **Setup:** [RELEASE_WORKFLOW_GUIDE.md § macOS Code Signing Setup](RELEASE_WORKFLOW_GUIDE.md#macos-code-signing-setup) (mandatory for good UX)
- **Details:** [WORKFLOW_ARCHITECTURE.md § macOS x86_64](WORKFLOW_ARCHITECTURE.md#macos-x86_64-macos-13-intel-hardware)
- **Benefits:** Native Intel build, notarized binaries trusted
- **Trade-offs:** Apple Developer account required ($99/year), notarization adds 2-5 min

### macOS Apple Silicon (aarch64-apple-darwin)

- **Setup:** [RELEASE_WORKFLOW_GUIDE.md § macOS Code Signing Setup](RELEASE_WORKFLOW_GUIDE.md#macos-code-signing-setup) (mandatory for good UX)
- **Details:** [WORKFLOW_ARCHITECTURE.md § macOS ARM64](WORKFLOW_ARCHITECTURE.md#macos-arm64-macos-14-apple-silicon-hardware)
- **Benefits:** Native Apple Silicon, 20-50% faster than x86_64, notarized binaries trusted
- **Trade-offs:** Apple Developer account required ($99/year), notarization adds 2-5 min

## Quick Links

### External Documentation

- [GitHub Actions: Building and testing Rust](https://docs.github.com/en/actions/use-cases-and-examples/building-and-testing/building-and-testing-rust)
- [GitHub Actions: Security best practices](https://docs.github.com/en/actions/reference/security/secure-use)
- [Apple Notarization Guide](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
- [SLSA Framework](https://slsa.dev/)
- [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain)
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache)

### Internal Documentation

- [Main README](../README.md) - Project overview
- [CHANGELOG](../CHANGELOG.md) - Version history
- [LICENSE](../LICENSE) - License terms

## Metrics

**Documentation Coverage:**
- Setup guides: 3 (Secrets, Release Workflow, Security)
- Checklists: 2 (Release, Security)
- Troubleshooting scenarios: 25+
- Security checklist items: 158
- Total lines: 3,717
- Total words: ~65,000

**Platform Coverage:**
- Linux: ✅ x86_64 (musl)
- Windows: ✅ x86_64 (MSVC)
- macOS: ✅ x86_64 (Intel) + ARM64 (Apple Silicon)

**Security Coverage:**
- Action pinning: ✅ All actions SHA-pinned
- Code signing: ✅ macOS (mandatory), Windows (optional)
- Checksums: ✅ SHA256 for all platforms
- SLSA provenance: ✅ Level 1
- Secret management: ✅ GitHub Secrets
- Manual approval: ✅ Draft releases

## Version History

- **v1.0 (2025-11-29):** Initial release
  - 4-platform support (Linux, Windows, macOS x86, macOS ARM)
  - macOS notarization (mandatory)
  - Windows signing (optional)
  - SHA256 checksums
  - SLSA Level 1 provenance
  - 158-item security checklist
  - 2,763 lines of documentation

## Feedback

**Found an issue?** Open a GitHub issue with:
- Workflow run URL (Actions tab → failed run)
- Error messages from logs
- Platform(s) affected
- Whether code signing configured

**Suggestions?** Open a GitHub issue or pull request.

## License

This documentation is part of the kindly-av1 project. See [LICENSE](../LICENSE) for details.
