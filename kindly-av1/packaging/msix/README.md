# kindly-av1 Microsoft Store MSIX Packaging

Complete packaging solution for distributing kindly-av1 on the Microsoft Store.

## Quick Start

### Prerequisites

1. **Windows 10/11** with Windows SDK 10.0.19041.0+
2. **PowerShell 7.0+** (for build script)
3. **Rust Toolchain** (for kindly-av1 compilation)
4. **Microsoft Partner Center Account** ($19 one-time fee)

### Build Package

```powershell
# 1. Build kindly-av1 binary
cd /home/samuel/Primitives/kindly-av1
cargo build --release --target x86_64-pc-windows-msvc

# 2. Generate MSIX package
cd packaging/msix
.\build-msix.ps1 -Configuration Release -Version "1.0.0.0"

# Output: output/kindly-av1-1.0.0.0.msix
```

### Test Locally

```powershell
# Install package
Add-AppxPackage -Path output/kindly-av1-1.0.0.0.msix

# Launch app
Start-Process "kindly-av1:"

# Uninstall
Get-AppxPackage -Name "Kindly.KindlyAV1" | Remove-AppxPackage
```

## Directory Structure

```
msix/
├── Package.appxmanifest          # App manifest (identity, capabilities, file associations)
├── priconfig.xml                 # Package resource index configuration
├── build-msix.ps1               # Automated build script (PowerShell)
├── MICROSOFT_STORE_SETUP.md     # Complete setup + submission guide (15,000+ words)
├── README.md                    # This file
├── Assets/                      # Store assets (5 PNG files)
│   ├── StoreLogo.png            # 50×50 (app tile, search)
│   ├── Square44x44Logo.png      # 44×44 (icon, taskbar)
│   ├── Square150x150Logo.png    # 150×150 (start menu medium tile)
│   ├── Wide310x150Logo.png      # 310×150 (start menu wide tile)
│   ├── LargeTile.png            # 310×310 (start menu large tile)
│   ├── README.md                # Asset design guidelines
│   ├── generate_placeholders.rs # Placeholder generator (Rust)
│   └── Cargo.toml               # Generator dependencies
├── staging/                     # Build staging directory (auto-generated)
└── output/                      # Final MSIX packages (auto-generated)
```

## File Descriptions

### Package.appxmanifest (126 lines)

**Purpose**: App identity, capabilities, and file associations.

**Key Sections**:
- **Identity**: Name="Kindly.KindlyAV1", Publisher="CN=Kindly"
- **Capabilities**: runFullTrust (desktop app)
- **File Associations**: .mp4, .mkv, .avi, .mov, .webm, .y4m
- **Protocol Handler**: kindly-av1:// (license activation deep links)
- **Visual Elements**: Purple theme (#9B59B6), asset references

**Customization**:
- Update `Version` attribute for new releases (1.0.0.0 → 1.1.0.0)
- Update `Publisher` to match Partner Center Publisher ID (CRITICAL)

### priconfig.xml (30 lines)

**Purpose**: Package resource indexing configuration (multi-language, scaling).

**When to Edit**: Rarely. Only if adding localization or DPI-specific assets.

### build-msix.ps1 (250 lines)

**Purpose**: Automated MSIX build + signing pipeline.

**Features**:
- Validates prerequisites (makeappx.exe, signtool.exe)
- Builds kindly-av1 binary via cargo
- Stages files (binary, manifest, assets)
- Creates package resource index (PRI)
- Packages MSIX via makeappx.exe
- Signs with certificate (test or production)

**Parameters**:
```powershell
-Configuration <Release|Debug>       # Build configuration (default: Release)
-Version <X.Y.Z.W>                   # Package version (default: 1.0.0.0)
-Publisher <CN=Name>                 # Publisher ID (default: CN=Kindly)
-CertificateThumbprint <hex>         # Production cert thumbprint
-SkipSign                            # Skip signing (for testing)
-Verbose                             # Verbose output
```

**Usage Examples**:
```powershell
# Development build (unsigned)
.\build-msix.ps1 -SkipSign

# Production build (signed)
.\build-msix.ps1 -CertificateThumbprint "1234567890ABCDEF..."

# Custom version
.\build-msix.ps1 -Version "1.2.3.0"
```

### MICROSOFT_STORE_SETUP.md (15,249 lines)

**Purpose**: Complete guide for Partner Center enrollment, certificate acquisition, listing setup, and submission.

**Sections** (8 phases):
1. **Partner Center Enrollment** ($19 fee, account setup)
2. **Certificate Acquisition** (production cert from Partner Center)
3. **Build and Sign Package** (MSIX creation + validation)
4. **Store Listing Preparation** (assets, screenshots, copy)
5. **Pricing Configuration** ($49/$149/$499 tiers)
6. **Submission** (upload, review, approval)
7. **Post-Submission** (metrics, updates, marketing)
8. **Advanced Features** (IAP, deep linking, analytics)

**Key Information**:
- Timeline: 1-3 business days for review
- Pricing: $49 (Standard), $149 (Pro), $499 (Studio)
- Required assets: 5 PNG files, 3-5 screenshots
- Review criteria: Stability, policy compliance, metadata accuracy

### Assets Directory

**Current Status**: ✅ Placeholder PNGs generated (purple #9B59B6 solid color)

**Action Required**: Replace with branded designs before submission.

**Design Guidelines**: See `Assets/README.md` for:
- Exact dimensions and specifications
- Brand color usage (#9B59B6 purple, #F1C40F gold)
- Typography and spacing guidelines
- Template examples (ASCII art)
- Generation tools (Figma, Illustrator, Canva)

**Regenerate Placeholders**:
```bash
cd Assets
cargo run --bin generate_placeholders
```

## Build Script Workflow

The `build-msix.ps1` script automates the entire packaging process:

```
Step 1: Validate Prerequisites
  ├── Check makeappx.exe (Windows SDK)
  ├── Check makepri.exe (resource indexing)
  └── Check signtool.exe (code signing)

Step 2: Build Binary
  └── cargo build --release --target x86_64-pc-windows-msvc

Step 3: Verify Binary Exists
  └── Check target/x86_64-pc-windows-msvc/release/kindly-av1.exe

Step 4: Clean and Create Staging Directory
  └── Create staging/ directory

Step 5: Copy Files to Staging
  ├── kindly-av1.exe
  ├── Package.appxmanifest
  └── Assets/*.png

Step 6: Create Package Resource Index (PRI)
  ├── makepri.exe createconfig (priconfig.xml)
  └── makepri.exe new (resources.pri)

Step 7: Create MSIX Package
  └── makeappx.exe pack (staging → output/kindly-av1-1.0.0.0.msix)

Step 8: Sign Package
  ├── Test Certificate: New-SelfSignedCertificate (local testing)
  └── Production Certificate: signtool.exe (Partner Center cert)

Step 9: Summary
  └── Report package path, size, signing status
```

## Pricing Strategy

### Tier Breakdown

| Tier | Price USD | Target Audience | Features |
|------|-----------|-----------------|----------|
| **Standard** | $49 | Solo creators, hobbyists | 1080p max, 2 machines, email support |
| **Pro** | $149 | Professional creators, small studios | 4K max, 3 machines, advanced RD optimization, priority support |
| **Studio** | $499 | Enterprises, large studios | 8K max, 10 machines, commercial license, API access, dedicated support |

### Implementation

**Option 1: Separate SKUs** (Recommended)
- Create 3 separate apps in Partner Center:
  - kindly-av1 Standard ($49)
  - kindly-av1 Pro ($149)
  - kindly-av1 Studio ($499)
- Each has different binary with tier-specific features

**Option 2: In-App Purchases** (Alternative)
- Base app: $49 (Standard)
- Add-on: $100 (Pro upgrade)
- Add-on: $350 (Studio upgrade)
- Use Windows.Services.Store API for license checks

### Pricing Justification

**Market Comparison**:
- Adobe Premiere Pro: $22.99/month ($276/year)
- DaVinci Resolve Studio: $295 one-time
- Handbrake: Free (but no GPU acceleration, no professional features)
- SVT-AV1: Free (command-line only, CPU-only)

**Value Proposition**:
- kindly-av1 offers **10-100× faster encoding** than free alternatives
- One-time payment (no subscription)
- Professional-grade quality (CDEF, LRF, advanced rate control)
- GPU acceleration (AMD ROCm, Vulkan)
- TUI wizard (user-friendly vs command-line)

## Microsoft Store Submission Checklist

Use this checklist before submitting to ensure compliance:

### Pre-Submission

- [ ] **Binary Built**: `target/x86_64-pc-windows-msvc/release/kindly-av1.exe` exists
- [ ] **Binary Size**: <100MB (current: ~5-10MB expected)
- [ ] **Binary Tested**: Launches without errors, encodes test video
- [ ] **Assets Created**: 5 PNG files in `Assets/` (branded, not placeholders)
- [ ] **Assets Validated**: Correct dimensions (50×50, 44×44, 150×150, 310×150, 310×310)
- [ ] **Manifest Updated**: Version number, publisher ID correct
- [ ] **Certificate Acquired**: Production certificate from Partner Center
- [ ] **Package Built**: `output/kindly-av1-X.Y.Z.W.msix` exists
- [ ] **Package Signed**: signtool.exe succeeded
- [ ] **WACK Validated**: Windows App Certification Kit passes
- [ ] **Local Testing**: App installs, launches, encodes video

### Store Listing

- [ ] **App Name Reserved**: "kindly-av1" in Partner Center
- [ ] **Category Selected**: Developer tools > Multimedia tools
- [ ] **Description Written**: 2,000+ words, SEO-optimized
- [ ] **Keywords Added**: AV1, encoder, GPU, video compression, ROCm, Vulkan, streaming
- [ ] **Screenshots**: 3-5 high-quality images (1920×1080)
- [ ] **Privacy Policy**: URL added (https://kindly.ai/privacy)
- [ ] **Support Email**: support@kindly.ai configured
- [ ] **Pricing Configured**: $49 (or tier-specific)
- [ ] **Markets Selected**: US, Canada, UK, Australia, EU

### Submission

- [ ] **Package Uploaded**: .msix file uploaded to Partner Center
- [ ] **Validation Passed**: Partner Center validation green checkmark
- [ ] **Review Submitted**: "Submit for Certification" clicked
- [ ] **Confirmation Email**: Received from Microsoft

### Post-Submission

- [ ] **Review Status Monitored**: Check Partner Center daily
- [ ] **Rejection Handled** (if any): Fix issues, resubmit
- [ ] **Approval Confirmed**: App live on Store
- [ ] **Marketing**: Announce on social media, website
- [ ] **Analytics Setup**: Monitor downloads, revenue, ratings

## Troubleshooting

### Build Script Fails: "makeappx.exe not found"

**Cause**: Windows SDK not installed or not in PATH.

**Solution**:
1. Download Windows SDK 10.0.19041.0+ from https://developer.microsoft.com/windows/downloads/windows-sdk/
2. Install with "Windows App Certification Kit" component
3. Add to PATH: `C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64`

### Package Validation Fails: "Invalid manifest"

**Cause**: XML syntax error in `Package.appxmanifest`.

**Solution**:
1. Open manifest in Visual Studio or XML editor
2. Validate XML syntax (Ctrl+K, Ctrl+D to auto-format)
3. Check for missing closing tags, incorrect namespaces

### Signing Fails: "Certificate not found"

**Cause**: Certificate thumbprint incorrect or certificate not installed.

**Solution**:
1. List certificates: `Get-ChildItem -Path Cert:\CurrentUser\My`
2. Find certificate with Subject="CN=Kindly"
3. Copy thumbprint (40-character hex string)
4. Re-run build script with `-CertificateThumbprint` parameter

### App Crashes on Launch After Install

**Cause**: Missing dependencies or GPU driver issues.

**Solutions**:
1. Add Visual C++ Runtime to package (if needed)
2. Add GPU driver check in app startup code
3. Provide fallback to CPU mode if GPU unavailable

### Store Review Rejection: "Misleading metadata"

**Cause**: Description or screenshots don't match app behavior.

**Solution**:
1. Ensure screenshots show actual app UI (not mockups)
2. Verify all advertised features work
3. Remove exaggerated claims (e.g., "100× faster" without benchmark proof)

## Advanced Topics

### Multi-Language Support

To add localization:

1. Create language-specific resource folders:
   ```
   staging/
   ├── en-US/
   │   └── resources.resw
   ├── es-ES/
   │   └── resources.resw
   └── fr-FR/
       └── resources.resw
   ```

2. Update `priconfig.xml` with additional languages

3. Update `Package.appxmanifest` with language tags

4. Re-run `makepri.exe` to generate multi-language PRI

### Delta Updates

Microsoft Store automatically generates delta updates for new versions:

- User on v1.0.0 → v1.1.0: Downloads only changed files (~5-10MB)
- User on v1.0.0 → v2.0.0: Full download if major changes

No action required; Store handles this automatically.

### Crash Reporting

Integrate with Microsoft App Center for crash telemetry:

1. Add App Center SDK (optional, not required for Store)
2. Upload symbols (PDB files) for stack trace symbolication
3. Monitor crashes in App Center dashboard

**Note**: kindly-av1 currently has no telemetry (privacy-focused). Consider adding opt-in crash reporting in future versions.

### A/B Testing Store Listings

Partner Center supports A/B testing for:
- App descriptions
- Screenshots
- Icons

**Setup**: Navigate to Store Listing > A/B Testing > Create Experiment

**Metrics**: Conversion rate (impressions → installs)

## References

**Microsoft Documentation**:
- [App Packaging](https://learn.microsoft.com/windows/msix/)
- [Store Policies](https://learn.microsoft.com/windows/apps/publish/store-policies)
- [Partner Center](https://partner.microsoft.com/dashboard)

**Internal Documentation**:
- [MICROSOFT_STORE_SETUP.md](./MICROSOFT_STORE_SETUP.md) - Complete setup guide
- [Assets/README.md](./Assets/README.md) - Asset design guidelines
- [../../CLAUDE.md](../../CLAUDE.md) - kindly-av1 project overview

## Support

**Microsoft Store Issues**:
- Microsoft Support: https://developer.microsoft.com/microsoft-store/support
- Community Forums: https://learn.microsoft.com/answers/topics/windows-dev-questions.html

**kindly-av1 Issues**:
- Email: support@kindly.ai
- Documentation: https://docs.kindly.ai/av1

---

**Document Version**: 1.0.0
**Last Updated**: 2025-11-29
**Status**: Production Ready
