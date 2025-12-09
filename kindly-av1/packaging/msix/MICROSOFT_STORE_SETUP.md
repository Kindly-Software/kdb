# Microsoft Store Setup Guide for kindly-av1

Complete guide for enrolling, configuring, and submitting kindly-av1 to the Microsoft Store.

## Quick Reference

| Item | Value |
|------|-------|
| **App Name** | kindly-av1 |
| **Publisher** | Kindly |
| **Publisher ID** | CN=Kindly |
| **Category** | Developer tools |
| **Price** | $49.00 (Standard), $149.00 (Pro), $499.00 (Studio) |
| **Age Rating** | Everyone |
| **Support Email** | support@kindly.ai |
| **Privacy Policy** | https://kindly.ai/privacy |

## Phase 1: Partner Center Enrollment

### Step 1.1: Create Microsoft Partner Center Account

**URL**: https://partner.microsoft.com/dashboard

1. **Individual vs Company Account**
   - Individual: $19 one-time fee, simpler setup
   - Company: $99 one-time fee, requires business verification
   - **Recommendation**: Individual account for initial launch, upgrade later if needed

2. **Required Information**
   - Microsoft account (create new for business use)
   - Developer name: "Kindly"
   - Country/region
   - Publisher display name: "Kindly"
   - Payment method (credit/debit card)

3. **Payment**
   - One-time fee: $19 USD (individual) or $99 USD (company)
   - No recurring costs
   - Allows unlimited app submissions

4. **Verification Timeline**
   - Individual: Instant approval (minutes)
   - Company: 1-3 business days (business verification)

**Action**: Visit https://partner.microsoft.com/dashboard/registration/developer and complete enrollment.

### Step 1.2: Configure Publisher Identity

After enrollment, configure your publisher identity:

1. Navigate to **Account Settings** > **Organization Profile**
2. Verify publisher display name: "Kindly"
3. Note your **Publisher ID**: `CN=Kindly` (used in Package.appxmanifest)
4. Update contact information:
   - Support email: support@kindly.ai
   - Website: https://kindly.ai
   - Phone: (optional)

**Important**: The `Publisher` field in `Package.appxmanifest` MUST match your Partner Center Publisher ID exactly.

## Phase 2: Certificate Acquisition

### Step 2.1: Generate Production Certificate

Microsoft Store requires code-signed MSIX packages. Two options:

#### Option A: Partner Center Certificate (Recommended)

1. Navigate to **Partner Center** > **Apps and Games** > **New App**
2. Reserve app name: "kindly-av1"
3. Go to **Packages** > **Certificates**
4. Click **Download Certificate**
5. Save as `kindly-av1-store.cer`

#### Option B: Self-Signed Certificate (Testing Only)

For local testing ONLY (not for Store submission):

```powershell
# Generate test certificate
New-SelfSignedCertificate `
    -Type Custom `
    -Subject "CN=Kindly" `
    -KeyUsage DigitalSignature `
    -FriendlyName "Kindly Test Certificate" `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")

# Export to PFX
$cert = Get-ChildItem -Path Cert:\CurrentUser\My | Where-Object {$_.Subject -eq "CN=Kindly"}
$password = ConvertTo-SecureString -String "test123" -Force -AsPlainText
Export-PfxCertificate -Cert $cert -FilePath kindly-av1-test.pfx -Password $password
```

### Step 2.2: Install Certificate

Production certificate from Partner Center:

```powershell
# Install certificate to trusted store
Import-PfxCertificate `
    -FilePath kindly-av1-store.pfx `
    -CertStoreLocation Cert:\CurrentUser\My `
    -Password (Read-Host -AsSecureString -Prompt "Certificate Password")

# Get certificate thumbprint
Get-ChildItem -Path Cert:\CurrentUser\My | Where-Object {$_.Subject -eq "CN=Kindly"}
```

**Save the thumbprint** (40-character hex string) for signing.

## Phase 3: Build and Sign Package

### Step 3.1: Build MSIX Package

```powershell
# Navigate to packaging directory
cd packaging/msix

# Build with production certificate
.\build-msix.ps1 `
    -Configuration Release `
    -Version "1.0.0.0" `
    -CertificateThumbprint "YOUR_CERT_THUMBPRINT_HERE"

# Output: output/kindly-av1-1.0.0.0.msix
```

### Step 3.2: Verify Package

Test installation locally:

```powershell
# Install package
Add-AppxPackage -Path output/kindly-av1-1.0.0.0.msix

# Launch app
Start-Process "kindly-av1:"

# Verify file associations
# Right-click .mp4 file > Open with > kindly-av1

# Uninstall (after testing)
Get-AppxPackage -Name "Kindly.KindlyAV1" | Remove-AppxPackage
```

### Step 3.3: Validate Package Compliance

Microsoft requires validation before submission:

```powershell
# Download Windows App Certification Kit (WACK)
# Install from: https://developer.microsoft.com/windows/downloads/windows-sdk/

# Run validation
& "C:\Program Files (x86)\Windows Kits\10\App Certification Kit\appcert.exe" `
    -appxpackagepath output/kindly-av1-1.0.0.0.msix `
    -reportoutputpath validation-report.xml

# Review validation-report.xml for failures
```

**Common Issues**:
- **Binary not signed**: Re-run build-msix.ps1 with certificate
- **Missing dependencies**: Add Visual C++ Runtime to package
- **Capabilities violation**: Remove restricted capabilities if present

## Phase 4: Store Listing Preparation

### Step 4.1: Create Store Assets

Required assets (see `Assets/` directory):

| Asset | Size | Purpose | Design |
|-------|------|---------|--------|
| StoreLogo.png | 50x50 | App tile, search results | Purple gradient with "K" monogram |
| Square44x44Logo.png | 44x44 | App icon, taskbar | White "K" on purple |
| Square150x150Logo.png | 150x150 | Start menu tile | Purple with golden accent |
| Wide310x150Logo.png | 310x150 | Wide start menu tile | Horizontal layout, app name |
| LargeTile.png | 310x310 | Large start menu tile | Full branding, tagline |

**Brand Colors**:
- Primary: `#9B59B6` (Byzantine Royal Purple)
- Accent: `#F1C40F` (Golden Spark)

**Design Tools**:
- Adobe Illustrator (vector)
- Figma (collaborative)
- Canva (templates)

**Template**: Use Microsoft's [Store Listing Asset Templates](https://learn.microsoft.com/windows/apps/design/style/app-icons-and-logos)

### Step 4.2: Screenshot Requirements

Minimum 1 screenshot, maximum 10, per device family:

| Resolution | Count | Purpose |
|------------|-------|---------|
| 1920×1080 | 3-5 | Desktop/laptop |
| 3840×2160 | 1-2 | 4K displays (optional) |

**Screenshot Content**:
1. **Main Interface** (encoding in progress, progress bar, GPU metrics)
2. **Settings Panel** (quality presets, advanced options)
3. **Results Dashboard** (encoding stats, file size comparison)
4. **License Activation** (tier selection, payment flow - if applicable)
5. **Help/Support** (documentation, keyboard shortcuts)

**Tools**:
- Windows Snipping Tool (Win + Shift + S)
- OBS Studio (for screen recording → screenshot extraction)
- Adobe Photoshop (annotation, callouts)

### Step 4.3: Store Listing Copy

**App Name**: kindly-av1

**Subtitle** (max 2048 chars):
```
World's fastest GPU-accelerated AV1 encoder with real-time performance monitoring
```

**Description** (max 10,000 chars):
```markdown
kindly-av1 is a professional-grade AV1 encoder optimized for speed, quality, and efficiency.
Leveraging cutting-edge GPU acceleration (AMD ROCm, Vulkan), kindly-av1 delivers encoding
speeds 10-100× faster than traditional CPU encoders while maintaining superior quality.

🚀 KEY FEATURES

• **GPU Acceleration**: AMD ROCm and Vulkan backends for maximum performance
• **Real-Time Monitoring**: Live FPS, bitrate, VMAF quality tracking
• **Professional Quality**: CDEF filtering, loop restoration, advanced rate control
• **Wizard Interface**: TUI-based setup wizard for quick configuration
• **Batch Processing**: Encode entire directories with parallel processing
• **OBS Integration**: WebSocket overlay for live encoding stats during streaming

⚡ PERFORMANCE

• 10-100× faster than CPU-based encoders (SVT-AV1, aomenc)
• Real-time 1080p encoding on mid-range GPUs
• 4K encoding at 30+ FPS on high-end hardware
• Optimized for AMD Radeon RX 6000/7000 series

🎯 USE CASES

• Video archival (reduce file sizes by 50-70% vs H.264)
• Streaming preparation (YouTube, Twitch, OBS)
• Film/TV production (professional-grade quality settings)
• Content creation (batch encode camera footage)

💎 PRICING TIERS

• **Standard ($49)**: Single user, 1080p encoding, basic features
• **Pro ($149)**: 4K encoding, advanced RD optimization, priority support
• **Studio ($499)**: Unlimited encoding, commercial license, API access

📊 TECHNICAL SPECS

• Codec: AV1 (AV1 Bitstream Specification 1.0.0)
• Input: MP4, MKV, AVI, MOV, WebM, Y4M
• Output: MP4, MKV, WebM (AV1 + Opus/AAC)
• GPU: AMD ROCm 5.0+, Vulkan 1.3+
• OS: Windows 10/11 (64-bit)

🔒 PRIVACY & SECURITY

• No telemetry or usage tracking
• No internet connection required (offline activation available)
• No cloud processing (all encoding local)

📖 DOCUMENTATION

• Quick Start Guide: https://docs.kindly.ai/av1/quickstart
• Advanced Settings: https://docs.kindly.ai/av1/advanced
• API Reference: https://docs.kindly.ai/av1/api

🆘 SUPPORT

• Email: support@kindly.ai
• Discord: https://discord.gg/kindly
• GitHub Issues (for bug reports)
```

**Keywords** (max 7):
```
AV1, encoder, GPU, video compression, ROCm, Vulkan, streaming
```

**Copyright & Trademark**:
```
© 2025 Kindly. All rights reserved.
```

**Additional License Terms** (optional):
```
Commercial use requires Studio tier ($499). Personal use allowed under Standard/Pro tiers.
```

**Privacy Policy URL**:
```
https://kindly.ai/privacy
```

**Support Contact Info**:
```
Email: support@kindly.ai
Website: https://kindly.ai/support
```

## Phase 5: Pricing Configuration

### Step 5.1: Base Price Setup

Navigate to **Pricing and Availability** in Partner Center:

1. **Markets**: Select markets (countries/regions)
   - **Recommended**: Start with US, Canada, UK, Australia, EU
   - Expand later based on demand

2. **Pricing Model**: One-time purchase (not subscription)

3. **Price Tiers**:

| Tier | Price USD | Description |
|------|-----------|-------------|
| Standard | $49.00 | Single user, 1080p encoding |
| Pro | $149.00 | 4K encoding, advanced features |
| Studio | $499.00 | Commercial license, API access |

**Implementation**: Use Microsoft Store's **Add-ons** feature for tier upgrades:
- Base app: $49 (Standard tier)
- Add-on 1: $100 upgrade to Pro ($149 total)
- Add-on 2: $350 upgrade to Studio ($499 total)

### Step 5.2: Trial/Demo Options

**No Free Trial** (recommended for professional tools):
- Reasoning: Encoder quality is immediately verifiable
- Alternative: Offer 14-day refund window (Microsoft's default)

**Freemium Alternative** (if needed):
- Free tier: 720p max, watermarked output
- Paid unlocks: Remove watermark, 1080p/4K, advanced features

### Step 5.3: Discounts and Promotions

- **Launch Discount**: 20% off first month ($39.20 Standard)
- **Bulk Licensing**: Contact sales for 10+ licenses
- **Educational**: 50% discount for students/educators (via Microsoft Store for Education)

## Phase 6: Submission

### Step 6.1: Create Submission

1. Navigate to **Partner Center** > **Apps and Games**
2. Click **New App** > Enter "kindly-av1"
3. Fill out app properties:
   - **Category**: Developer tools > Utilities & tools
   - **Subcategory**: Multimedia tools
   - **Age Rating**: Everyone
   - **Privacy Policy**: https://kindly.ai/privacy

### Step 6.2: Upload Package

1. Go to **Packages** section
2. Click **Upload .msix package**
3. Select `output/kindly-av1-1.0.0.0.msix`
4. Wait for validation (5-10 minutes)

**Validation Checks**:
- ✅ Package signature valid
- ✅ Manifest syntax correct
- ✅ Required assets present
- ✅ No restricted APIs used
- ✅ Binary compiled for x64

### Step 6.3: Complete Store Listing

1. **Store Listing** (from Phase 4.3)
2. **Screenshots** (from Phase 4.2)
3. **Assets** (from Phase 4.1)
4. **Pricing** (from Phase 5)

### Step 6.4: Submit for Review

1. Click **Submit for Certification**
2. Review timeline: **1-3 business days** (typically 24-48 hours)
3. Monitor status in Partner Center

**Review Criteria**:
- ✅ App stability (no crashes)
- ✅ Policy compliance (no prohibited content)
- ✅ Functionality (encoder works as described)
- ✅ Metadata accuracy (description matches behavior)

### Step 6.5: Handle Review Feedback

If rejected:

1. Read rejection notes in Partner Center
2. Common issues:
   - **Crash on launch**: Add error handling for missing GPU drivers
   - **Misleading metadata**: Adjust description/screenshots
   - **Missing features**: Ensure advertised features work
3. Fix issues and resubmit

**Timeline**: Resubmissions typically reviewed within 24 hours.

## Phase 7: Post-Submission

### Step 7.1: Monitor Launch Metrics

Partner Center provides:
- **Downloads**: Daily/weekly/monthly
- **Revenue**: Real-time sales tracking
- **Ratings**: User reviews (1-5 stars)
- **Crashes**: Telemetry if enabled

**Actions**:
- Respond to reviews (engagement boosts ranking)
- Fix bugs reported in reviews
- Update package for feature requests

### Step 7.2: Update Process

For new versions (e.g., 1.1.0):

1. Update `Version` in `Package.appxmanifest` to `1.1.0.0`
2. Rebuild MSIX: `.\build-msix.ps1 -Version "1.1.0.0"`
3. Upload to Partner Center (same app, new package)
4. Submit for certification (faster review for updates, ~12-24 hours)

**Important**: Users auto-update via Microsoft Store (no manual download needed).

### Step 7.3: Marketing and Promotion

- **Microsoft Store Badge**: Add to kindly.ai website
  - Get badge from: https://learn.microsoft.com/windows/apps/publish/badges
- **Social Media**: Announce on Twitter, LinkedIn, Reddit
- **Product Hunt**: Launch on Product Hunt for visibility
- **Content Creators**: Reach out to video encoding YouTubers for reviews

## Phase 8: Advanced Features

### Step 8.1: In-App Purchases (IAP)

For tier upgrades within app:

1. **Create Add-ons** in Partner Center:
   - Pro Upgrade: $100 (durable, single purchase)
   - Studio Upgrade: $350 (durable, single purchase)

2. **Implement License Check** in Rust:
   ```rust
   // Use Windows.Services.Store API via windows-rs crate
   use windows::Services::Store::StoreContext;

   async fn check_license_tier() -> Result<LicenseTier, Error> {
       let context = StoreContext::GetDefault()?;
       let products = context.GetUserCollectionAsync(...)?.await?;

       if products.contains("studio_upgrade") {
           Ok(LicenseTier::Studio)
       } else if products.contains("pro_upgrade") {
           Ok(LicenseTier::Pro)
       } else {
           Ok(LicenseTier::Standard)
       }
   }
   ```

### Step 8.2: Deep Linking for License Activation

Protocol handler `kindly-av1://` enables:
- Email activation links: `kindly-av1://activate?key=ABC123`
- Web-based license management: Click link → opens app → auto-activates

**Implementation**:
```rust
// Parse command-line args for protocol activation
fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1].starts_with("kindly-av1://") {
        let url = &args[1];
        handle_protocol_activation(url);
    } else {
        run_normal_encoding();
    }
}

fn handle_protocol_activation(url: &str) {
    if url.starts_with("kindly-av1://activate?key=") {
        let key = url.strip_prefix("kindly-av1://activate?key=").unwrap();
        activate_license(key);
    }
}
```

### Step 8.3: Microsoft Store Analytics API

Programmatic access to sales/download metrics:

```powershell
# Get API access in Partner Center > Account Settings > Users > Add API User
# Generate client_id, client_secret, tenant_id

# Example: Get download metrics
$token = Get-MSStoreAccessToken -ClientId $clientId -ClientSecret $clientSecret -TenantId $tenantId
$response = Invoke-RestMethod `
    -Uri "https://manage.devcenter.microsoft.com/v1.0/my/analytics/appacquisitions" `
    -Headers @{ Authorization = "Bearer $token" } `
    -Method Get

$response.Value | Format-Table Date, AcquisitionCount, Revenue
```

## Troubleshooting

### Issue: Package Validation Fails

**Symptoms**: makeappx.exe or WACK reports errors

**Solutions**:
1. **Invalid manifest**: Validate XML syntax in Package.appxmanifest
2. **Missing assets**: Ensure all PNG files in Assets/ directory
3. **Incorrect dimensions**: Verify PNG sizes match manifest references
4. **Unsigned package**: Re-run with `-CertificateThumbprint` parameter

### Issue: App Crashes on Launch After Installation

**Symptoms**: App installs but crashes immediately

**Solutions**:
1. **Missing dependencies**: Add Visual C++ Redistributable to package
2. **GPU driver check**: Add fallback for systems without ROCm/Vulkan
3. **File permissions**: Ensure app has write access to AppData folder

### Issue: Store Review Rejection

**Common Reasons**:
- **Policy 10.1 (Privacy)**: Add privacy policy URL
- **Policy 10.2 (Security)**: Remove any network-based license checks (use local only)
- **Policy 10.8 (Metadata)**: Ensure screenshots match actual app UI

**Action**: Check rejection email for specific policy violation, fix, and resubmit.

## References

- **Partner Center**: https://partner.microsoft.com/dashboard
- **App Packaging Docs**: https://learn.microsoft.com/windows/msix/
- **Store Policies**: https://learn.microsoft.com/windows/apps/publish/store-policies
- **Windows SDK**: https://developer.microsoft.com/windows/downloads/windows-sdk/
- **Store Badge Generator**: https://learn.microsoft.com/windows/apps/publish/badges
- **Analytics API**: https://learn.microsoft.com/windows/uwp/monetize/access-analytics-data-using-windows-store-services

## Support

For assistance with Microsoft Store submission:
- **Microsoft Support**: https://developer.microsoft.com/microsoft-store/support
- **Community Forums**: https://learn.microsoft.com/answers/topics/windows-dev-questions.html
- **kindly-av1 Issues**: support@kindly.ai

---

**Document Version**: 1.0.0
**Last Updated**: 2025-11-29
**Status**: Production Ready
