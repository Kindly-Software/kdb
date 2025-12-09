# GitHub Secrets Setup Guide for kindly-av1 Releases

Complete guide for configuring GitHub repository secrets required by the multi-platform release workflow.

## Overview

The release workflow (`release.yml`) supports optional code signing for macOS and Windows platforms. All secrets are optional - the workflow will build unsigned binaries if secrets are not configured.

## Required vs Optional Secrets

| Secret | Platform | Required | Purpose |
|--------|----------|----------|---------|
| MACOS_CERTIFICATE | macOS | Optional | Code signing certificate (Developer ID Application) |
| MACOS_CERTIFICATE_PWD | macOS | Optional | Certificate password |
| MACOS_SIGNING_IDENTITY | macOS | Optional | Signing identity name (e.g., "Developer ID Application: Name (Team ID)") |
| KEYCHAIN_PWD | macOS | Optional | Temporary keychain password (can be random) |
| APPLE_ID | macOS | Optional | Apple ID email for notarization |
| APPLE_TEAM_ID | macOS | Optional | Apple Developer Team ID (10-char alphanumeric) |
| APPLE_APP_PASSWORD | macOS | Optional | App-specific password for notarization |
| WINDOWS_CERTIFICATE | Windows | Optional | Code signing certificate (.pfx) |
| WINDOWS_CERTIFICATE_PWD | Windows | Optional | Certificate password |

## macOS Code Signing Setup

### Prerequisites

1. **Apple Developer Account** ($99/year)
2. **Developer ID Application Certificate**
3. **App-Specific Password** for notarization

### Step 1: Create Developer ID Certificate

1. Go to [Apple Developer Certificates](https://developer.apple.com/account/resources/certificates/list)
2. Click "+" to create new certificate
3. Select "Developer ID Application"
4. Follow instructions to generate Certificate Signing Request (CSR) in Keychain Access
5. Upload CSR and download certificate
6. Double-click to install in Keychain Access

### Step 2: Export Certificate

1. Open Keychain Access
2. Select "My Certificates"
3. Find your "Developer ID Application" certificate
4. Right-click → Export "Developer ID Application..."
5. Save as `.p12` file with a strong password
6. Convert to base64:

```bash
# macOS/Linux
base64 -i DeveloperIDApplication.p12 | pbcopy

# Windows (PowerShell)
[Convert]::ToBase64String([IO.File]::ReadAllBytes("DeveloperIDApplication.p12")) | Set-Clipboard
```

### Step 3: Get Signing Identity

Find your signing identity name:

```bash
security find-identity -v -p codesigning
```

Look for line like:
```
1) ABC123DEF456 "Developer ID Application: Your Name (TEAM12345)"
```

Copy the full quoted string: `Developer ID Application: Your Name (TEAM12345)`

### Step 4: Create App-Specific Password

1. Go to [Apple ID Account](https://appleid.apple.com/account/manage)
2. Sign in with your Apple ID
3. Navigate to "Security" → "App-Specific Passwords"
4. Click "+" to generate new password
5. Label it "kindly-av1 GitHub Actions"
6. Copy the generated password (format: `xxxx-xxxx-xxxx-xxxx`)

### Step 5: Add Secrets to GitHub

1. Go to your GitHub repository
2. Navigate to Settings → Secrets and variables → Actions
3. Click "New repository secret" for each:

| Secret Name | Value | Example |
|-------------|-------|---------|
| MACOS_CERTIFICATE | Base64 certificate from Step 2 | `MIIK...` (long string) |
| MACOS_CERTIFICATE_PWD | Password you set in Step 2 | `MySecurePassword123` |
| MACOS_SIGNING_IDENTITY | Full identity from Step 3 | `Developer ID Application: John Doe (ABC123DEF4)` |
| KEYCHAIN_PWD | Random password | `gh-actions-keychain-2024` |
| APPLE_ID | Your Apple ID email | `john.doe@example.com` |
| APPLE_TEAM_ID | 10-character Team ID | `ABC123DEF4` |
| APPLE_APP_PASSWORD | App-specific password from Step 4 | `xxxx-xxxx-xxxx-xxxx` |

### Step 6: Verify Setup

Create a test tag and push:

```bash
git tag v0.0.1-test
git push origin v0.0.1-test
```

Monitor workflow at: `https://github.com/YOUR_ORG/kindly-av1/actions`

Check for successful:
1. Certificate import
2. Codesigning
3. Notarization submission
4. Notarization approval (~5-10 minutes)

### Troubleshooting macOS Signing

**Error: "No identity found"**
- Verify MACOS_SIGNING_IDENTITY matches exactly (copy-paste from `security find-identity`)
- Check certificate is valid and not expired

**Error: "Invalid credentials"**
- Verify APPLE_ID is correct email
- Verify APPLE_APP_PASSWORD is app-specific (not your Apple ID password)
- Verify APPLE_TEAM_ID is 10 characters (check at developer.apple.com)

**Error: "Notarization failed"**
- Check binary is properly signed (workflow verifies before notarizing)
- Ensure Developer ID certificate is "Developer ID Application" (not "Developer ID Installer")
- Check Apple Developer account is in good standing

## Windows Code Signing Setup (Optional)

### Prerequisites

1. **Code Signing Certificate** from trusted CA (DigiCert, Sectigo, etc.)
2. Certificate must support "Code Signing" EKU

### Step 1: Export Certificate

If you have certificate in Windows Certificate Store:

```powershell
# Export certificate with private key
$cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -like "*Your Company*" }
$password = ConvertTo-SecureString -String "YourPassword" -Force -AsPlainText
Export-PfxCertificate -Cert $cert -FilePath "CodeSigningCert.pfx" -Password $password
```

### Step 2: Convert to Base64

```powershell
$bytes = [System.IO.File]::ReadAllBytes("CodeSigningCert.pfx")
$base64 = [System.Convert]::ToBase64String($bytes)
$base64 | Set-Clipboard
```

### Step 3: Add Secrets to GitHub

| Secret Name | Value | Example |
|-------------|-------|---------|
| WINDOWS_CERTIFICATE | Base64 certificate | `MIIK...` (long string) |
| WINDOWS_CERTIFICATE_PWD | Certificate password | `MySecurePassword123` |

### Step 4: Verify Setup

Push a tag and monitor Windows build job for:
1. Certificate import
2. Signing with SignTool
3. Signature verification

### Troubleshooting Windows Signing

**Error: "Certificate not found"**
- Verify base64 encoding is correct (no line breaks)
- Check password matches certificate export password

**Error: "SignTool not found"**
- Workflow uses Windows SDK 10.0.22621.0 SignTool
- If version mismatch, update workflow path

**Error: "Timestamp server unreachable"**
- DigiCert timestamp server may be down
- Try alternative: `http://timestamp.comodoca.com` or `http://timestamp.sectigo.com`

## Security Best Practices

### Secret Rotation

Rotate secrets annually or if compromised:

1. **macOS Certificates**: Expire after 5 years (renew 60 days before expiration)
2. **App-Specific Passwords**: Rotate annually
3. **Windows Certificates**: Expire after 1-3 years (check CA policy)

### Access Control

1. Limit repository access to trusted maintainers
2. Enable "Require administrator approval for workflow runs" for external contributors
3. Use environment secrets (not repository secrets) for production releases
4. Enable branch protection on release branches

### Secret Validation

Before adding secrets:

```bash
# Verify macOS certificate
openssl pkcs12 -in cert.p12 -nokeys -passin pass:PASSWORD | openssl x509 -noout -subject -dates

# Verify Windows certificate
certutil -dump cert.pfx
```

### Audit Trail

Monitor workflow runs for:
- Failed signing attempts
- Unauthorized tag pushes
- Secret access patterns

GitHub Actions logs show secret usage (values redacted).

## Testing Workflow Without Secrets

The workflow supports building unsigned binaries:

```yaml
# Workflow skips signing if secrets not set
if: startsWith(matrix.os, 'macos') && github.event_name == 'push'
```

To test workflow without signing:
1. Push tag from fork (no secrets access)
2. Check "workflow_dispatch" manual trigger
3. Verify unsigned artifacts are created

## Cost Considerations

| Item | Cost | Frequency |
|------|------|-----------|
| Apple Developer Account | $99/year | Annual |
| Code Signing Certificate (Windows) | $200-$500/year | Annual |
| Notarization | Free | Per submission |
| GitHub Actions Minutes | Free (public repos) | Per workflow run |
| GitHub Actions Minutes | ~$0.008/min (private repos) | Per workflow run |

**Workflow Runtime**: ~15-20 minutes total (4 parallel builds + release job)

**Private Repo Cost**: ~$0.12-$0.16 per release (20 min × $0.008/min)

## Next Steps

After configuring secrets:

1. **Test Release**: Create test tag (`v0.0.1-test`)
2. **Verify Artifacts**: Download and test signed binaries
3. **macOS Verification**:
   ```bash
   # Check signature
   codesign -dvv kindly-av1

   # Check notarization
   spctl -a -vv kindly-av1
   ```
4. **Windows Verification**:
   ```powershell
   # Check signature
   Get-AuthenticodeSignature kindly-av1.exe
   ```
5. **Delete Test Release**: Clean up test releases in GitHub UI

## Support

For issues:
1. Check workflow logs at `https://github.com/YOUR_ORG/kindly-av1/actions`
2. Verify secrets are correctly formatted (no extra whitespace)
3. Test certificates locally before uploading
4. Check Apple Developer account status
5. Verify timestamp servers are accessible

## References

- [Apple Notarization Guide](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
- [GitHub Actions Secrets](https://docs.github.com/en/actions/security-guides/encrypted-secrets)
- [Windows SignTool](https://learn.microsoft.com/en-us/dotnet/framework/tools/signtool-exe)
- [SLSA Framework](https://slsa.dev/)

---

**Last Updated**: 2025-11-29
**Workflow Version**: v1.0
**Maintainer**: Kindly Team
