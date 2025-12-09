# Apple Notarization Setup Guide

**Version**: 1.0.0
**Date**: 2025-11-29
**Status**: Complete Workflow Ready

## Overview

This guide covers the complete Apple Developer enrollment, certificate generation, and GitHub Actions integration process for notarizing kindly-av1 macOS binaries. Notarization is **required** for Gatekeeper bypass on macOS 10.15+ (Catalina and later).

## Cost Summary

| Item | Cost | Frequency | Notes |
|------|------|-----------|-------|
| Apple Developer Program | $99 USD (≈$135 CAD) | Annual | Required for notarization |
| Certificate Renewal | $0 (included) | Annual | Auto-renewed with program |
| Notarization Submissions | $0 (unlimited) | Per-build | No per-submission fees |

**Total Annual Cost**: $99 USD (≈$135 CAD)

## Prerequisites

- macOS machine with Xcode Command Line Tools installed
- GitHub repository with admin access (for secrets)
- Valid payment method (credit/debit card)

## Phase 1: Apple Developer Enrollment

### 1.1 Individual vs Organization

| Type | Requirements | Approval Time | Use Case |
|------|--------------|---------------|----------|
| **Individual** | Personal Apple ID, government-issued ID | 24-48 hours | Solo developer, freelancer |
| **Organization** | D-U-N-S Number, legal entity verification | 1-2 weeks | Company, LLC, corporation |

**Recommendation for kindly-av1**: Individual account (faster approval, simpler verification).

### 1.2 Enrollment Process

1. **Visit**: https://developer.apple.com/programs/enroll/
2. **Sign in** with your Apple ID (create if needed)
3. **Select enrollment type**: Individual or Organization
4. **Provide information**:
   - Legal name (must match government ID)
   - Address
   - Phone number
5. **Payment**: $99 USD annual fee (credit/debit card)
6. **Wait for approval**: Check email for enrollment confirmation

**Timeline**:
- Individual: 24-48 hours (typically <24h)
- Organization: 1-2 weeks (D-U-N-S lookup + verification)

### 1.3 Verification Checklist

After enrollment approval:

- [ ] Apple ID email confirmed
- [ ] Apple Developer account active (check https://developer.apple.com/account)
- [ ] Team ID visible (10-character alphanumeric, e.g., `A1B2C3D4E5`)
- [ ] Certificates section accessible

## Phase 2: Certificate Generation

### 2.1 Certificate Types Required

| Certificate | Purpose | Platforms | Format |
|-------------|---------|-----------|--------|
| **Developer ID Application** | Code signing for CLI binaries | macOS | .cer → .p12 |
| **Developer ID Installer** | Signing .pkg installers (optional) | macOS | .cer → .p12 |

**For kindly-av1**: Developer ID Application is sufficient (CLI binary only, no .pkg).

### 2.2 Generate Certificate Signing Request (CSR)

**On macOS**:

1. Open **Keychain Access** (Applications → Utilities)
2. Menu: **Keychain Access → Certificate Assistant → Request a Certificate From a Certificate Authority**
3. Fill form:
   - **User Email Address**: Your Apple ID email
   - **Common Name**: `kindly-av1 Code Signing` (descriptive name)
   - **CA Email Address**: Leave empty
   - **Request is**: Select **Saved to disk**
4. Click **Continue**
5. Save as: `CertificateSigningRequest.certSigningRequest`

### 2.3 Request Certificate from Apple

1. **Visit**: https://developer.apple.com/account/resources/certificates/list
2. Click **+** (Add Certificate)
3. Select **Developer ID Application** (under "Production")
4. Click **Continue**
5. Upload `CertificateSigningRequest.certSigningRequest`
6. Click **Continue**
7. Download certificate: `developerID_application.cer`

### 2.4 Install and Export Certificate

**Install**:

1. Double-click `developerID_application.cer`
2. Keychain Access opens → Certificate added to **login** keychain
3. Expand certificate in list → See private key underneath

**Export as .p12**:

1. In Keychain Access, select the **certificate** (not the private key)
2. Right-click → **Export "Developer ID Application: Your Name (TEAM_ID)"**
3. Save as: `kindly-av1-certificate.p12`
4. **Set password**: Choose strong password (e.g., generate with `openssl rand -base64 32`)
5. Save password securely (needed for GitHub secrets)

**Critical**: Backup `kindly-av1-certificate.p12` and password to secure location (1Password, encrypted disk).

### 2.5 Verify Certificate

```bash
# Check certificate details
security find-identity -v -p codesigning

# Expected output:
# 1) ABCDEF1234567890ABCDEF1234567890ABCDEF12 "Developer ID Application: Your Name (TEAM_ID)"
#    1 valid identities found
```

**Identity String** (needed for GitHub secrets):
- Format: `Developer ID Application: Your Name (TEAM_ID)`
- Example: `Developer ID Application: Samuel Kindly (A1B2C3D4E5)`

## Phase 3: GitHub Secrets Configuration

### 3.1 Required Secrets

Navigate to: `https://github.com/your-username/kindly-av1/settings/secrets/actions`

Click **New repository secret** for each:

| Secret Name | Value | Source |
|-------------|-------|--------|
| `MACOS_CERTIFICATE` | Base64-encoded .p12 certificate | See § 3.2 |
| `MACOS_CERTIFICATE_PWD` | Certificate .p12 password | From § 2.4 export step |
| `KEYCHAIN_PWD` | Temporary keychain password (random) | Generate new (e.g., `openssl rand -base64 32`) |
| `MACOS_SIGNING_IDENTITY` | Certificate identity string | From § 2.5 verify step |
| `APPLE_ID` | Apple ID email | Your developer account email |
| `APPLE_TEAM_ID` | 10-character Team ID | From https://developer.apple.com/account (top-right) |
| `APPLE_APP_PASSWORD` | App-specific password | See § 3.3 |

### 3.2 Encode Certificate to Base64

**On macOS**:

```bash
# Encode .p12 to base64 and copy to clipboard
base64 -i kindly-av1-certificate.p12 | pbcopy

# Paste into GitHub secret MACOS_CERTIFICATE
# (Output is multi-line, paste entire block)
```

**On Linux**:

```bash
# Encode .p12 to base64
base64 -w 0 kindly-av1-certificate.p12 > certificate.b64

# Copy contents to GitHub secret MACOS_CERTIFICATE
cat certificate.b64
```

### 3.3 Generate App-Specific Password

Apple requires app-specific passwords for notarization (not your main Apple ID password).

**Steps**:

1. **Visit**: https://appleid.apple.com/account/manage
2. **Sign in** with Apple ID
3. Navigate to **Security** section
4. Under **App-Specific Passwords**, click **Generate**
5. Label: `kindly-av1 GitHub Actions Notarization`
6. Click **Create**
7. **Copy password** (format: `xxxx-xxxx-xxxx-xxxx`, all lowercase)
8. Paste into GitHub secret `APPLE_APP_PASSWORD`

**Note**: Password shown only once. Store securely if needed later.

### 3.4 Find Team ID

**Method 1** (Apple Developer Portal):

1. Visit: https://developer.apple.com/account
2. Team ID shown in top-right corner (10 characters)
3. Example: `A1B2C3D4E5`

**Method 2** (Certificate):

```bash
# Extract Team ID from certificate
security find-identity -v -p codesigning | grep "Developer ID Application"

# Output: ... "Developer ID Application: Your Name (A1B2C3D4E5)"
# Team ID is in parentheses: A1B2C3D4E5
```

### 3.5 Secrets Verification Checklist

- [ ] `MACOS_CERTIFICATE`: Multi-line base64 string (starts with `MIIJ...`)
- [ ] `MACOS_CERTIFICATE_PWD`: Password from § 2.4 (.p12 export)
- [ ] `KEYCHAIN_PWD`: Random 32-char password (generated fresh)
- [ ] `MACOS_SIGNING_IDENTITY`: `Developer ID Application: Your Name (TEAM_ID)`
- [ ] `APPLE_ID`: Email address (your Apple ID)
- [ ] `APPLE_TEAM_ID`: 10-character alphanumeric (e.g., `A1B2C3D4E5`)
- [ ] `APPLE_APP_PASSWORD`: 16-char app-specific password (`xxxx-xxxx-xxxx-xxxx`)

## Phase 4: Local Testing (Optional but Recommended)

Test notarization workflow locally before GitHub Actions run.

### 4.1 Sign Binary

```bash
# Build release binary
cd /home/samuel/Primitives/kindly-av1
cargo build --release --target x86_64-apple-darwin

# Sign with hardened runtime
codesign --force \
  --sign "Developer ID Application: Your Name (TEAM_ID)" \
  --options runtime \
  --timestamp \
  --verbose \
  target/x86_64-apple-darwin/release/kindly-av1

# Verify signature
codesign --verify --verbose target/x86_64-apple-darwin/release/kindly-av1

# Check notarization requirements
spctl --assess --type execute --verbose target/x86_64-apple-darwin/release/kindly-av1
```

**Expected output**:
```
target/.../kindly-av1: accepted
source=Notarized Developer ID
```

### 4.2 Create Notarization Archive

```bash
# Create directory for notarization
mkdir -p /tmp/notarize-test
cp target/x86_64-apple-darwin/release/kindly-av1 /tmp/notarize-test/

# Create zip archive
ditto -c -k --keepParent /tmp/notarize-test/kindly-av1 /tmp/kindly-av1.zip
```

### 4.3 Submit for Notarization

```bash
# Submit to Apple (wait for completion)
xcrun notarytool submit /tmp/kindly-av1.zip \
  --apple-id "your@email.com" \
  --team-id "TEAM_ID" \
  --password "xxxx-xxxx-xxxx-xxxx" \
  --wait

# Alternative: Store password in keychain
xcrun notarytool store-credentials "kindly-av1" \
  --apple-id "your@email.com" \
  --team-id "TEAM_ID" \
  --password "xxxx-xxxx-xxxx-xxxx"

# Then submit with keychain reference
xcrun notarytool submit /tmp/kindly-av1.zip \
  --keychain-profile "kindly-av1" \
  --wait
```

**Expected output**:
```
Submission ID: 12345678-1234-1234-1234-123456789012
Successfully uploaded file
  id: 12345678-1234-1234-1234-123456789012
  path: /tmp/kindly-av1.zip
Waiting for processing to complete...
Current status: Accepted
```

### 4.4 Check Notarization Status

```bash
# Get submission history
xcrun notarytool history \
  --apple-id "your@email.com" \
  --team-id "TEAM_ID" \
  --password "xxxx-xxxx-xxxx-xxxx"

# Get detailed log for specific submission
xcrun notarytool log 12345678-1234-1234-1234-123456789012 \
  --apple-id "your@email.com" \
  --team-id "TEAM_ID" \
  --password "xxxx-xxxx-xxxx-xxxx"
```

### 4.5 Verify Notarized Binary

```bash
# Check Gatekeeper acceptance
spctl --assess --type execute --verbose /tmp/notarize-test/kindly-av1

# Expected output:
# /tmp/notarize-test/kindly-av1: accepted
# source=Notarized Developer ID
# origin=Developer ID Application: Your Name (TEAM_ID)
```

**Note**: Stapling not required for CLI binaries (only .app bundles). Notarization stored on Apple's servers, verified at first run.

## Phase 5: GitHub Actions Integration

### 5.1 Workflow Overview

The existing workflow at `.github/workflows/release.yml` includes:

1. **Import Certificates** (lines 78-97): Creates temporary keychain, imports .p12
2. **Codesign Binary** (lines 99-123): Signs with hardened runtime + timestamp
3. **Notarize Binary** (lines 125-165): Submits to Apple, waits for approval

### 5.2 Workflow Triggers

**Automatic**:
```bash
# Tag-based release (recommended)
git tag v1.0.0
git push origin v1.0.0

# GitHub Actions detects tag matching v[0-9]+.[0-9]+.[0-9]+
# Builds, signs, notarizes, and creates GitHub Release
```

**Manual**:
```bash
# Go to: https://github.com/your-username/kindly-av1/actions
# Select "Release Build" workflow
# Click "Run workflow" → Select branch → Run
```

### 5.3 Verify Workflow Run

1. Navigate to: `https://github.com/your-username/kindly-av1/actions`
2. Select latest "Release Build" run
3. Check macOS jobs: `Build x86_64-apple-darwin` and `Build aarch64-apple-darwin`
4. Expand steps:
   - **Import Apple certificates**: Should show "1 identity imported"
   - **Codesign binary**: Should show "valid on disk" and "satisfies its Designated Requirement"
   - **Notarize binary**: Should show "Submission ID" and "Current status: Accepted"

### 5.4 Download and Test Release

```bash
# After workflow completes, download from GitHub Releases
# Example for x86_64:
wget https://github.com/your-username/kindly-av1/releases/download/v1.0.0/kindly-av1-x86_64-apple-darwin.tar.gz

# Extract
tar xzf kindly-av1-x86_64-apple-darwin.tar.gz

# Test Gatekeeper acceptance
./kindly-av1-x86_64-apple-darwin/kindly-av1 --version

# On first run, macOS verifies notarization (requires internet)
# Subsequent runs use cached verification (offline-capable)
```

## Phase 6: Troubleshooting

### 6.1 Certificate Import Failures

**Error**: `security: SecKeychainItemImport: MAC verification failed`

**Cause**: Wrong certificate password in `MACOS_CERTIFICATE_PWD`

**Fix**:
1. Re-export .p12 from Keychain Access with known password
2. Update `MACOS_CERTIFICATE_PWD` secret in GitHub

---

**Error**: `security: SecPolicySetValue: One or more parameters passed to a function were not valid`

**Cause**: Certificate expired or revoked

**Fix**:
1. Check certificate validity: `security find-identity -v -p codesigning`
2. If expired, generate new certificate (§ 2.2-2.4)
3. Update `MACOS_CERTIFICATE` secret

### 6.2 Code Signing Failures

**Error**: `errSecInternalComponent` or `User interaction is not allowed`

**Cause**: Keychain not unlocked or missing `set-key-partition-list`

**Fix**: Ensure workflow includes (already in release.yml line 97):
```bash
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KEYCHAIN_PWD" $KEYCHAIN_PATH
```

---

**Error**: `the codesign_allocate helper tool cannot be found or used`

**Cause**: Xcode Command Line Tools not installed in runner

**Fix**: Already handled by `macos-13` and `macos-14` runners (Xcode pre-installed).

### 6.3 Notarization Failures

**Error**: `Error: HTTP status code: 401. Invalid credentials.`

**Cause**: Wrong `APPLE_ID`, `APPLE_TEAM_ID`, or `APPLE_APP_PASSWORD`

**Fix**:
1. Verify Apple ID email: https://appleid.apple.com
2. Verify Team ID: https://developer.apple.com/account (top-right)
3. Generate new app-specific password (§ 3.3)
4. Update GitHub secrets

---

**Error**: `The binary is not signed with a valid Developer ID certificate.`

**Cause**: Binary not signed before notarization submission

**Fix**: Ensure codesign step (lines 99-123) runs before notarize step (lines 125-165).

---

**Error**: `The binary uses the Get Task Allow entitlement.`

**Cause**: Debug entitlement enabled (disallowed for distribution)

**Fix**: Build with `--release` (already configured in workflow line 66).

---

**Error**: `The signature does not include a secure timestamp.`

**Cause**: Missing `--timestamp` flag in codesign

**Fix**: Already included in workflow (line 110): `--timestamp \`

---

**Error**: `The executable does not have the hardened runtime enabled.`

**Cause**: Missing `--options runtime` flag in codesign

**Fix**: Already included in workflow (line 109): `--options runtime \`

### 6.4 Notarization Log Analysis

```bash
# Locally, fetch detailed notarization log
xcrun notarytool log <submission-id> \
  --apple-id "your@email.com" \
  --team-id "TEAM_ID" \
  --password "xxxx-xxxx-xxxx-xxxx" \
  notarization-log.json

# Inspect JSON for specific errors
jq '.issues' notarization-log.json
```

**Common issues**:
- `"severity": "error"` → Binary rejected (must fix)
- `"severity": "warning"` → Binary accepted but fixable (optional)

### 6.5 Hardened Runtime Requirements

**Required for notarization**:
- Hardened runtime enabled (`--options runtime`)
- Secure timestamp (`--timestamp`)
- No Get Task Allow entitlement (release builds only)

**Optional entitlements** (if needed):

Create `entitlements.plist`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- Allow JIT compilation (for Rust with certain features) -->
    <key>com.apple.security.cs.allow-jit</key>
    <true/>

    <!-- Allow unsigned executable memory (rarely needed) -->
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>

    <!-- Disable library validation (allow loading unsigned dylibs) -->
    <key>com.apple.security.cs.disable-library-validation</key>
    <true/>
</dict>
</plist>
```

Sign with entitlements:
```bash
codesign --force \
  --sign "Developer ID Application: Your Name (TEAM_ID)" \
  --options runtime \
  --timestamp \
  --entitlements entitlements.plist \
  target/x86_64-apple-darwin/release/kindly-av1
```

**Note**: kindly-av1 does not require entitlements (static binary, no dylibs, no JIT).

### 6.6 Gatekeeper Bypass Verification

**On end-user macOS**:

```bash
# First run (internet required for verification)
./kindly-av1 --version

# If quarantine attribute present, macOS shows dialog:
# "kindly-av1" is from an identified developer. Are you sure you want to open it?

# If notarized, dialog shows:
# Apple verified this app for malicious software and none was detected.

# If NOT notarized, macOS blocks execution:
# "kindly-av1" cannot be opened because the developer cannot be verified.
```

**Manual quarantine removal** (for testing):
```bash
# Remove quarantine attribute
xattr -d com.apple.quarantine kindly-av1

# Now runs without Gatekeeper check
./kindly-av1 --version
```

## Phase 7: Maintenance and Renewal

### 7.1 Certificate Expiration

**Developer ID certificates**: Valid for **5 years** from issue date.

**Renewal process** (before expiration):
1. Generate new CSR (§ 2.2)
2. Request new certificate (§ 2.3)
3. Export new .p12 (§ 2.4)
4. Update GitHub secrets (§ 3.2)

**Auto-renewal**: Not available. Manual renewal required 1-2 months before expiration.

### 7.2 Apple Developer Program Renewal

**Automatic renewal**: Yes, if payment method on file

**Cost**: $99 USD/year (charged to card annually)

**Expiration warning**: Apple sends email 30 days before expiration

**Renewal steps**:
1. Visit: https://developer.apple.com/account
2. Click **Renew Membership**
3. Confirm payment method
4. Pay $99 USD

**If membership expires**:
- Certificates remain valid until certificate expiration (up to 5 years)
- Cannot request new certificates
- Cannot submit new notarizations (old notarizations cached by macOS still work)

### 7.3 Revoking Compromised Certificates

**If .p12 file leaked or password compromised**:

1. Visit: https://developer.apple.com/account/resources/certificates/list
2. Select compromised certificate
3. Click **Revoke**
4. Confirm revocation

**Impact**:
- Previously signed binaries still work (signature valid)
- Previously notarized binaries still work (notarization cached)
- New builds require new certificate

**Recovery**:
1. Generate new CSR (§ 2.2)
2. Request new certificate (§ 2.3)
3. Export new .p12 (§ 2.4)
4. Update GitHub secrets (§ 3.2)

### 7.4 Team ID Changes

**If switching Apple Developer accounts** (e.g., individual → organization):

1. Enroll new account (§ 1.1-1.3)
2. Generate certificates for new Team ID (§ 2.2-2.4)
3. Update GitHub secrets with new values:
   - `MACOS_CERTIFICATE` (new .p12)
   - `MACOS_CERTIFICATE_PWD` (new password)
   - `MACOS_SIGNING_IDENTITY` (new Team ID in identity string)
   - `APPLE_ID` (new account email if changed)
   - `APPLE_TEAM_ID` (new Team ID)
   - `APPLE_APP_PASSWORD` (new app-specific password)

**Note**: Old binaries signed with old Team ID remain valid. Only new builds use new Team ID.

## Phase 8: Advanced Topics

### 8.1 Multiple Certificates (Installer + Application)

**If creating .pkg installers** (future feature):

1. Request **Developer ID Installer** certificate (§ 2.3, select "Developer ID Installer" type)
2. Export second .p12: `kindly-av1-installer-certificate.p12`
3. Add GitHub secrets:
   - `MACOS_INSTALLER_CERTIFICATE` (base64-encoded .p12)
   - `MACOS_INSTALLER_CERTIFICATE_PWD` (password)
   - `MACOS_INSTALLER_SIGNING_IDENTITY` (identity string)

**Update workflow** to sign .pkg:
```yaml
- name: Sign installer (macOS)
  run: |
    productsign --sign "$MACOS_INSTALLER_SIGNING_IDENTITY" \
      --timestamp \
      kindly-av1.pkg \
      kindly-av1-signed.pkg
```

### 8.2 Universal Binaries (x86_64 + ARM64)

**Create universal binary** (single binary for both architectures):

```bash
# Build both targets
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# Combine with lipo
lipo -create \
  target/x86_64-apple-darwin/release/kindly-av1 \
  target/aarch64-apple-darwin/release/kindly-av1 \
  -output kindly-av1-universal

# Sign universal binary
codesign --force \
  --sign "Developer ID Application: Your Name (TEAM_ID)" \
  --options runtime \
  --timestamp \
  kindly-av1-universal

# Verify architectures
lipo -archs kindly-av1-universal
# Output: x86_64 arm64
```

**Note**: Current workflow builds separate binaries for x86_64 and ARM64 (recommended for size optimization).

### 8.3 Notarization with CI/CD (GitLab, Bitbucket)

**Principle**: Same secrets, different syntax

**GitLab CI** example:
```yaml
notarize:
  stage: deploy
  script:
    - echo "$MACOS_CERTIFICATE" | base64 --decode > certificate.p12
    - security create-keychain -p "$KEYCHAIN_PWD" build.keychain
    - security import certificate.p12 -P "$MACOS_CERTIFICATE_PWD" -k build.keychain
    - codesign --sign "$MACOS_SIGNING_IDENTITY" --options runtime --timestamp kindly-av1
    - xcrun notarytool submit kindly-av1.zip --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
  only:
    - tags
```

**Bitbucket Pipelines** example:
```yaml
pipelines:
  tags:
    'v*':
      - step:
          name: Notarize
          image: macos-monterey-xcode:13
          script:
            - export MACOS_CERTIFICATE=$(echo $MACOS_CERTIFICATE | base64 --decode)
            - security create-keychain -p "$KEYCHAIN_PWD" build.keychain
            - security import certificate.p12 -P "$MACOS_CERTIFICATE_PWD" -k build.keychain
            - codesign --sign "$MACOS_SIGNING_IDENTITY" --options runtime --timestamp kindly-av1
            - xcrun notarytool submit kindly-av1.zip --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
```

### 8.4 Offline Notarization Workflow

**For air-gapped builds** (high-security environments):

1. **Build on air-gapped machine**:
   ```bash
   cargo build --release --target x86_64-apple-darwin
   ```

2. **Transfer unsigned binary** to internet-connected macOS machine

3. **Sign on internet-connected machine**:
   ```bash
   codesign --force \
     --sign "Developer ID Application: Your Name (TEAM_ID)" \
     --options runtime \
     --timestamp \
     kindly-av1
   ```

4. **Notarize**:
   ```bash
   ditto -c -k --keepParent kindly-av1 kindly-av1.zip
   xcrun notarytool submit kindly-av1.zip \
     --keychain-profile "kindly-av1" \
     --wait
   ```

5. **Transfer notarized binary** back to air-gapped environment

6. **Distribute** (notarization cached on Apple's servers)

## Phase 9: Cost-Benefit Analysis

### 9.1 With Notarization

| Benefit | Impact |
|---------|--------|
| Gatekeeper bypass | Users can double-click to run (no right-click "Open" workaround) |
| Professional trust | macOS shows "Apple verified this app for malicious software" |
| Reduced support burden | Fewer "How do I run this?" support tickets |
| Enterprise adoption | Corporate IT policies often require notarization |
| App Store readiness | Prerequisite for future Mac App Store submission |

**Cost**: $99 USD/year

### 9.2 Without Notarization

| Drawback | Impact |
|----------|--------|
| Gatekeeper blocks execution | Users must right-click → Open → Confirm (confusing for non-technical users) |
| Security warnings | macOS shows "cannot be verified" (scary for users) |
| Higher support burden | Users email asking if binary is malware |
| Corporate firewalls block | Enterprise networks may block unsigned binaries |
| No App Store option | Cannot distribute via Mac App Store |

**Cost**: $0 USD/year

### 9.3 Recommendation

**For kindly-av1** (commercial product at $49-$499):

✅ **Notarization is justified**

**Reasoning**:
- $99/year cost amortized across sales (2-10 sales covers entire year)
- Professional appearance critical for paid software
- Reduced support burden saves time (worth ≫$99/year)
- Enterprise tier ($499) customers expect notarization
- Future App Store distribution option

**Alternative** (if budget constrained):
- Launch without notarization (document workaround in README)
- Add notarization after first 10-20 sales (use revenue to fund enrollment)

## Phase 10: Checklist Summary

### Pre-Enrollment

- [ ] Decide individual vs organization enrollment
- [ ] Prepare payment method ($99 USD)
- [ ] Have government-issued ID ready (individual) or D-U-N-S number (organization)

### Enrollment

- [ ] Enroll at https://developer.apple.com/programs/enroll/
- [ ] Pay $99 USD annual fee
- [ ] Wait for approval (24-48 hours individual, 1-2 weeks org)
- [ ] Note Team ID (10 characters, shown in account page)

### Certificate Generation

- [ ] Generate CSR in Keychain Access (§ 2.2)
- [ ] Request Developer ID Application certificate (§ 2.3)
- [ ] Download and install .cer file
- [ ] Export as .p12 with strong password (§ 2.4)
- [ ] Backup .p12 and password securely
- [ ] Note signing identity string (§ 2.5)

### GitHub Secrets

- [ ] `MACOS_CERTIFICATE`: Base64-encoded .p12 (§ 3.2)
- [ ] `MACOS_CERTIFICATE_PWD`: .p12 password
- [ ] `KEYCHAIN_PWD`: Random 32-char password
- [ ] `MACOS_SIGNING_IDENTITY`: Identity string from § 2.5
- [ ] `APPLE_ID`: Apple ID email
- [ ] `APPLE_TEAM_ID`: 10-character Team ID
- [ ] `APPLE_APP_PASSWORD`: App-specific password (§ 3.3)

### Local Testing (Optional)

- [ ] Sign binary locally (§ 4.1)
- [ ] Create notarization archive (§ 4.2)
- [ ] Submit for notarization (§ 4.3)
- [ ] Verify notarization status (§ 4.4)
- [ ] Test Gatekeeper acceptance (§ 4.5)

### GitHub Actions

- [ ] Verify workflow at `.github/workflows/release.yml`
- [ ] Push tag (e.g., `git tag v1.0.0 && git push origin v1.0.0`)
- [ ] Monitor workflow run (§ 5.3)
- [ ] Download release from GitHub Releases
- [ ] Test on clean macOS machine (§ 5.4)

### Maintenance

- [ ] Calendar reminder for certificate expiration (5 years)
- [ ] Calendar reminder for program renewal (annual)
- [ ] Document recovery plan for compromised certificate

## References

- **Apple Developer Program**: https://developer.apple.com/programs/
- **Notarization Documentation**: https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution
- **Code Signing Guide**: https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/
- **notarytool Documentation**: https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution/customizing_the_notarization_workflow
- **Hardened Runtime**: https://developer.apple.com/documentation/security/hardened_runtime

## Support

For kindly-av1 specific notarization issues:
- Email: support@kindly.dev
- GitHub Issues: https://github.com/your-username/kindly-av1/issues

For Apple Developer enrollment issues:
- Apple Developer Support: https://developer.apple.com/support/
- Phone: 1-800-633-2152 (US/Canada)

---

**Document Version**: 1.0.0
**Last Updated**: 2025-11-29
**Maintainer**: Samuel (Kindly)
