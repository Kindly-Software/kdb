# Release Security Checklist

This checklist ensures secure releases with proper code signing, verification, and supply chain security.

## Pre-Release Setup (One-Time)

### macOS Code Signing

- [ ] **Apple Developer Account**
  - [ ] Active $99/year subscription
  - [ ] Visit [developer.apple.com/account](https://developer.apple.com/account) and verify status
  - [ ] Accept latest developer agreement (required before each notarization)

- [ ] **Developer ID Application Certificate**
  - [ ] Created at [developer.apple.com/account/resources/certificates](https://developer.apple.com/account/resources/certificates)
  - [ ] Downloaded and installed in Keychain Access
  - [ ] Private key accessible (not grayed out in Keychain)
  - [ ] Not expired (check expiration date - typically 5 years)

- [ ] **Certificate Export**
  - [ ] Exported from Keychain Access as `.p12` with strong password
  - [ ] Converted to base64: `base64 -i cert.p12 -o cert.base64.txt`
  - [ ] Base64 file contains `-----BEGIN CERTIFICATE-----` header

- [ ] **App-Specific Password**
  - [ ] Generated at [appleid.apple.com/account/manage](https://appleid.apple.com/account/manage)
  - [ ] Named "GitHub Actions Notarization" for tracking
  - [ ] Saved securely (can't retrieve after initial generation)

- [ ] **GitHub Secrets Configured**
  - [ ] `MACOS_CERTIFICATE` = base64 certificate content
  - [ ] `MACOS_CERTIFICATE_PWD` = certificate export password
  - [ ] `MACOS_SIGNING_IDENTITY` = full identity from `security find-identity`
  - [ ] `APPLE_ID` = Apple ID email (exact match)
  - [ ] `APPLE_TEAM_ID` = 10-character team ID
  - [ ] `APPLE_APP_PASSWORD` = app-specific password (not main password)
  - [ ] `KEYCHAIN_PWD` = any strong random password

- [ ] **Test Build**
  - [ ] Push test tag: `git tag v0.0.1-test && git push origin v0.0.1-test`
  - [ ] Verify workflow completes without errors
  - [ ] Check macOS binary is signed: `codesign -dv kindly-av1`
  - [ ] Check notarization: `spctl -a -vv -t install kindly-av1`

### Windows Code Signing (Optional)

- [ ] **Code Signing Certificate**
  - [ ] Purchased from DigiCert/Sectigo/GlobalSign (~$100-400/year)
  - [ ] Standard certificate (NOT EV - requires hardware token)
  - [ ] Installed in Windows certificate store
  - [ ] Not expired (check expiration date)

- [ ] **Certificate Export**
  - [ ] Exported as `.pfx` with private key and strong password
  - [ ] Converted to base64: `certutil -encode cert.pfx cert.base64.txt`
  - [ ] Base64 file readable and complete

- [ ] **GitHub Secrets Configured**
  - [ ] `WINDOWS_CERTIFICATE` = base64 certificate content
  - [ ] `WINDOWS_CERTIFICATE_PWD` = certificate export password

- [ ] **Test Build**
  - [ ] Push test tag: `git tag v0.0.1-test && git push origin v0.0.1-test`
  - [ ] Verify workflow completes without errors
  - [ ] Check Windows binary is signed (right-click → Properties → Digital Signatures)

### Repository Configuration

- [ ] **Branch Protection**
  - [ ] Main branch protected (Settings → Branches → Branch protection rules)
  - [ ] Require pull request reviews before merging
  - [ ] Require status checks to pass (CI tests)
  - [ ] Enforce restrictions on pushing tags (prevent accidental releases)

- [ ] **Secret Security**
  - [ ] All secrets configured in repository settings (not hardcoded)
  - [ ] Secret values never logged or echoed in workflows
  - [ ] Secrets not shared across multiple repositories
  - [ ] Access to repository settings restricted to maintainers only

- [ ] **Workflow Permissions**
  - [ ] Workflow uses minimal permissions (`contents: write` only)
  - [ ] No `secrets: inherit` or overly broad permissions
  - [ ] Actions pinned to commit SHAs (not tags)

## Pre-Release Review (Every Release)

### Code Review

- [ ] **Changes Audited**
  - [ ] All commits since last release reviewed
  - [ ] No unexpected changes in dependencies (check `Cargo.lock` diff)
  - [ ] Security advisories checked: `cargo audit`
  - [ ] Clippy warnings resolved: `cargo clippy --all-features`

- [ ] **Version Bump**
  - [ ] `Cargo.toml` version updated (semantic versioning: MAJOR.MINOR.PATCH)
  - [ ] CHANGELOG.md updated with release notes
  - [ ] README.md reflects new version (if needed)
  - [ ] All version references consistent across files

- [ ] **Testing**
  - [ ] All tests pass locally: `cargo test --all-features`
  - [ ] Benchmarks run: `cargo bench` (verify no performance regressions)
  - [ ] Manual testing on target platforms (if available)
  - [ ] CI tests passed on GitHub Actions

### Build Preparation

- [ ] **Secrets Valid**
  - [ ] Certificates not expired (check Apple Developer account + Windows cert)
  - [ ] App-specific password still valid (test with `xcrun notarytool history`)
  - [ ] Apple Developer agreement current (visit appstoreconnect.apple.com)
  - [ ] No secret rotation needed (if yes, update GitHub Secrets)

- [ ] **Tag Ready**
  - [ ] Tag name follows format: `v[MAJOR].[MINOR].[PATCH]` (e.g., `v1.2.3`)
  - [ ] Tag annotation includes release summary
  - [ ] Tag signed with GPG key (optional but recommended): `git tag -s v1.2.3`

## Release Execution

### Trigger Build

- [ ] **Create Tag**
  ```bash
  git tag -a v1.2.3 -m "Release v1.2.3: [brief summary]"
  ```

- [ ] **Push Tag**
  ```bash
  git push origin v1.2.3
  ```

- [ ] **Monitor Workflow**
  - [ ] Navigate to Actions tab in GitHub
  - [ ] Click "Release Build" workflow
  - [ ] Watch all 4 platform builds (Linux, Windows, macOS x86, macOS ARM)

### Build Verification

- [ ] **Linux Build**
  - [ ] Workflow step "Build release binary" succeeded
  - [ ] Artifact uploaded: `kindly-av1-x86_64-unknown-linux-musl.tar.gz`
  - [ ] Checksum uploaded: `kindly-av1-x86_64-unknown-linux-musl.tar.gz.sha256`

- [ ] **Windows Build**
  - [ ] Workflow step "Build release binary" succeeded
  - [ ] (If signing configured) "Sign binary" step succeeded
  - [ ] Artifact uploaded: `kindly-av1-x86_64-pc-windows-msvc.zip`
  - [ ] Checksum uploaded: `kindly-av1-x86_64-pc-windows-msvc.zip.sha256`

- [ ] **macOS x86_64 Build**
  - [ ] Workflow step "Build release binary" succeeded
  - [ ] "Import Apple certificates" succeeded
  - [ ] "Codesign binary" succeeded
  - [ ] "Notarize binary" succeeded (may take 2-5 minutes)
  - [ ] Artifact uploaded: `kindly-av1-x86_64-apple-darwin.tar.gz`
  - [ ] Checksum uploaded: `kindly-av1-x86_64-apple-darwin.tar.gz.sha256`

- [ ] **macOS ARM64 Build**
  - [ ] Workflow step "Build release binary" succeeded
  - [ ] "Import Apple certificates" succeeded
  - [ ] "Codesign binary" succeeded
  - [ ] "Notarize binary" succeeded (may take 2-5 minutes)
  - [ ] Artifact uploaded: `kindly-av1-aarch64-apple-darwin.tar.gz`
  - [ ] Checksum uploaded: `kindly-av1-aarch64-apple-darwin.tar.gz.sha256`

- [ ] **Release Job**
  - [ ] Draft release created automatically
  - [ ] All 8 files attached (4 archives + 4 checksums)
  - [ ] Release notes auto-generated from commits

- [ ] **SLSA Provenance**
  - [ ] `slsa-provenance.md` generated
  - [ ] Contains workflow ID, commit SHA, actor
  - [ ] All SHA256 checksums included

## Post-Build Verification

### Artifact Integrity

- [ ] **Download Artifacts**
  - [ ] Download all 4 platform archives from draft release
  - [ ] Download all 4 checksum files

- [ ] **Verify Checksums**
  - **Linux:**
    ```bash
    shasum -a 256 -c kindly-av1-x86_64-unknown-linux-musl.tar.gz.sha256
    ```
  - **Windows:**
    ```powershell
    (Get-FileHash kindly-av1-x86_64-pc-windows-msvc.zip).Hash -eq (Get-Content kindly-av1-x86_64-pc-windows-msvc.zip.sha256 | Select-String "[a-fA-F0-9]{64}").Matches.Value
    ```
  - **macOS x86_64:**
    ```bash
    shasum -a 256 -c kindly-av1-x86_64-apple-darwin.tar.gz.sha256
    ```
  - **macOS ARM64:**
    ```bash
    shasum -a 256 -c kindly-av1-aarch64-apple-darwin.tar.gz.sha256
    ```

- [ ] **Extract Archives**
  - [ ] All archives extract without errors
  - [ ] Each contains: binary + README.md + LICENSE + CHANGELOG.md
  - [ ] Binary executable permissions set (Linux/macOS)

### Binary Testing

- [ ] **Linux Binary**
  - [ ] Runs on Ubuntu 20.04+ (or current LTS)
  - [ ] `ldd kindly-av1` shows no missing dependencies
  - [ ] `./kindly-av1 --version` shows correct version
  - [ ] Basic functionality test (encode small file)

- [ ] **Windows Binary**
  - [ ] Runs on Windows 10/11
  - [ ] (If signed) No SmartScreen warning
  - [ ] (If unsigned) SmartScreen bypass works (expected)
  - [ ] `kindly-av1.exe --version` shows correct version
  - [ ] Basic functionality test (encode small file)

- [ ] **macOS x86_64 Binary**
  - [ ] Runs on macOS 10.15+ (Intel Macs)
  - [ ] No "unidentified developer" warning (if notarized)
  - [ ] `./kindly-av1 --version` shows correct version
  - [ ] Verify signature: `codesign -dv kindly-av1`
  - [ ] Verify notarization: `spctl -a -vv -t install kindly-av1`
  - [ ] Basic functionality test (encode small file)

- [ ] **macOS ARM64 Binary**
  - [ ] Runs on macOS 11.0+ (Apple Silicon Macs)
  - [ ] No "unidentified developer" warning (if notarized)
  - [ ] `./kindly-av1 --version` shows correct version
  - [ ] Verify signature: `codesign -dv kindly-av1`
  - [ ] Verify notarization: `spctl -a -vv -t install kindly-av1`
  - [ ] Basic functionality test (encode small file)

### Code Signing Verification

- [ ] **macOS Signature Check**
  ```bash
  codesign -dv kindly-av1 2>&1 | grep "Authority=Developer ID Application"
  codesign -dv kindly-av1 2>&1 | grep "flags=0x10000(runtime)"
  ```
  - [ ] Shows "Developer ID Application: [Your Name]"
  - [ ] Shows hardened runtime enabled

- [ ] **macOS Notarization Check**
  ```bash
  spctl -a -vv -t install kindly-av1
  ```
  - [ ] Shows "source=Notarized Developer ID"
  - [ ] No errors or warnings

- [ ] **Windows Signature Check** (if configured)
  - [ ] Right-click binary → Properties → Digital Signatures
  - [ ] Shows your certificate authority (DigiCert/Sectigo/etc.)
  - [ ] Signature status: "This digital signature is OK"
  - [ ] Timestamp present (ensures validity after cert expiration)

## Release Publication

### Final Review

- [ ] **Release Notes**
  - [ ] Edit draft release in GitHub
  - [ ] Review auto-generated release notes
  - [ ] Add highlights and breaking changes (if any)
  - [ ] Include upgrade instructions (if breaking changes)
  - [ ] Mention new features and bug fixes

- [ ] **SLSA Provenance**
  - [ ] `slsa-provenance.md` attached to release
  - [ ] Commit SHA matches current repository state
  - [ ] Actor is authorized maintainer (not compromised account)

- [ ] **Security Review**
  - [ ] No sensitive information in release notes
  - [ ] No credentials or secrets in artifacts
  - [ ] All binaries from official workflow (check run ID in provenance)

### Publish Release

- [ ] **Publish**
  - [ ] Click "Publish release" button in GitHub
  - [ ] Release visible on repository homepage
  - [ ] Tag visible in repository tags

- [ ] **Announcement**
  - [ ] Tweet/blog post/changelog (if applicable)
  - [ ] Update documentation site (if applicable)
  - [ ] Notify users of breaking changes (if applicable)

## Post-Release Monitoring

### First 24 Hours

- [ ] **Download Metrics**
  - [ ] Monitor release download counts (GitHub Insights)
  - [ ] Check for unusual download patterns (potential supply chain attack)

- [ ] **User Reports**
  - [ ] Monitor GitHub issues for installation problems
  - [ ] Check social media for feedback
  - [ ] Respond to security reports immediately

- [ ] **Binary Verification**
  - [ ] Spot-check user downloads match official checksums
  - [ ] Verify notarization status on macOS (should not expire)

### First Week

- [ ] **Security Monitoring**
  - [ ] No reports of signature/notarization issues
  - [ ] No reports of binaries not running
  - [ ] No security vulnerabilities reported

- [ ] **Metrics Review**
  - [ ] Download counts reasonable for project size
  - [ ] No reports of malware/virus warnings (false positives)
  - [ ] Checksums verified by users (if they report)

## Incident Response

### If Build Fails

- [ ] **Investigate Failure**
  - [ ] Check workflow logs for error messages
  - [ ] Identify platform(s) affected
  - [ ] Determine if code issue or infrastructure issue

- [ ] **Fix and Retry**
  - [ ] Fix code issue (create commit + new tag)
  - [ ] Or fix workflow configuration (edit `.github/workflows/release.yml`)
  - [ ] Delete failed tag and release
  - [ ] Retry with new tag (increment patch version)

### If Notarization Fails

- [ ] **Check Apple Status**
  - [ ] Visit [developer.apple.com](https://developer.apple.com) and verify account active
  - [ ] Check if new agreement needs acceptance (appstoreconnect.apple.com)
  - [ ] Verify app-specific password not revoked

- [ ] **Retry Notarization**
  - [ ] If transient Apple server issue, wait 30 minutes and re-run workflow
  - [ ] If credentials issue, update GitHub Secrets and re-run
  - [ ] If code signing issue (hardened runtime), fix code and create new tag

### If Compromised Binary Reported

- [ ] **Immediate Actions**
  - [ ] Delete release immediately
  - [ ] Revoke compromised tag: `git tag -d vX.Y.Z && git push origin :refs/tags/vX.Y.Z`
  - [ ] Post security advisory on GitHub Security tab

- [ ] **Investigation**
  - [ ] Compare reported binary checksum with official SLSA provenance
  - [ ] Check workflow logs for tampering (unexpected commits/actors)
  - [ ] Audit GitHub Actions secrets (rotate if compromised)
  - [ ] Check for unauthorized repository access (Settings → Manage access)

- [ ] **Recovery**
  - [ ] Rotate all signing certificates/passwords
  - [ ] Update GitHub Secrets with new certificates
  - [ ] Audit all previous releases (check SLSA provenance)
  - [ ] Create clean release from verified source code

## Certificate Renewal

### 90 Days Before Expiration

- [ ] **Apple Developer ID Certificate**
  - [ ] Renew at [developer.apple.com/account/resources/certificates](https://developer.apple.com/account/resources/certificates)
  - [ ] Download new certificate
  - [ ] Export as `.p12` with new password
  - [ ] Convert to base64
  - [ ] Update GitHub Secrets (`MACOS_CERTIFICATE`, `MACOS_CERTIFICATE_PWD`)
  - [ ] Test with new certificate: push test tag

- [ ] **Windows Code Signing Certificate**
  - [ ] Renew with certificate authority (DigiCert/Sectigo/etc.)
  - [ ] Install new certificate in Windows
  - [ ] Export as `.pfx` with new password
  - [ ] Convert to base64
  - [ ] Update GitHub Secrets (`WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PWD`)
  - [ ] Test with new certificate: push test tag

### Annual Security Audit

- [ ] **Secret Rotation**
  - [ ] Generate new app-specific password for Apple notarization
  - [ ] Generate new keychain password
  - [ ] Update GitHub Secrets
  - [ ] Test workflow with new secrets

- [ ] **Workflow Review**
  - [ ] Update action versions (Dependabot PRs)
  - [ ] Review new security best practices (GitHub blog)
  - [ ] Check for new SLSA framework releases
  - [ ] Audit workflow permissions (still minimal?)

- [ ] **Repository Audit**
  - [ ] Review collaborator access (remove inactive maintainers)
  - [ ] Check secret access logs (Settings → Secrets → audit log)
  - [ ] Review branch protection rules (still enforced?)
  - [ ] Verify two-factor authentication enabled for all maintainers

## Checklist Version

**Version:** 1.0
**Last Updated:** 2025-11-29
**Next Review:** 2026-11-29 (annual)

## References

- **Workflow Guide:** [RELEASE_WORKFLOW_GUIDE.md](RELEASE_WORKFLOW_GUIDE.md)
- **GitHub Actions Security:** [docs.github.com/actions/security](https://docs.github.com/en/actions/security-guides)
- **Apple Notarization:** [developer.apple.com/documentation/security/notarizing_macos_software_before_distribution](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
- **SLSA Framework:** [slsa.dev](https://slsa.dev/)
