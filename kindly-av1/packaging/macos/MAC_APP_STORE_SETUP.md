# Mac App Store Setup Guide - kindly-av1

Complete guide for publishing kindly-av1 on the Mac App Store.

## Prerequisites

### 1. Apple Developer Program Enrollment

**Cost**: $99/year (same account used for notarization)

**Enrollment**: https://developer.apple.com/programs/enroll/

**Steps**:
1. Sign in with Apple ID
2. Choose Individual or Organization enrollment
3. Complete payment ($99 USD annually)
4. Verify email and phone number
5. Accept Apple Developer Program License Agreement
6. Wait 24-48 hours for approval

**Account Types**:
- **Individual**: Personal projects, sole proprietorship
- **Organization**: Companies, requires D-U-N-S number and legal verification (2-4 weeks)

### 2. Development Environment

**Requirements**:
- macOS 11.0 or later (for building)
- Xcode 13.0+ (for signing and uploading)
- Xcode Command Line Tools: `xcode-select --install`
- Valid Apple Developer Program membership

**Install Xcode**:
```bash
# From App Store
open "macappstore://apps.apple.com/app/xcode/id497799835"

# Or download from developer.apple.com
# After installation:
sudo xcode-select --switch /Applications/Xcode.app
xcodebuild -version
```

## Bundle ID Registration

### 1. Create App ID

**Location**: https://developer.apple.com/account/resources/identifiers/list

**Steps**:
1. Sign in to Apple Developer account
2. Navigate to **Certificates, Identifiers & Profiles**
3. Click **Identifiers** → **+** (Add)
4. Select **App IDs** → Continue
5. Choose **App** type → Continue
6. Fill in details:
   - **Description**: kindly-av1 GPU-Accelerated AV1 Encoder
   - **Bundle ID**: `software.kindly.av1` (Explicit)
   - **Capabilities**: Enable:
     - ✓ App Sandbox (required)
     - ✓ Hardened Runtime (required)
     - ✓ GPU/Metal (required for encoder)
7. Click **Continue** → **Register**

### 2. Capabilities Configuration

**Required Capabilities**:
- ✓ App Sandbox (mandatory for Mac App Store)
- ✓ Hardened Runtime (mandatory for notarization)
- ✓ User Selected Files (read/write access)
- ✓ GPU/Metal Access (for ROCm/Vulkan/Metal)

**Optional Capabilities**:
- Network Client (for license validation, Gumroad webhooks)
- Camera/Microphone (for live encoding)
- Apple Events (for automation/scripting)

## Certificates

### 1. Mac App Distribution Certificate

**Purpose**: Sign the app bundle for App Store submission

**Steps**:
1. Navigate to **Certificates, Identifiers & Profiles** → **Certificates**
2. Click **+** (Add)
3. Select **Mac App Distribution** → Continue
4. Create Certificate Signing Request (CSR):
   ```bash
   # Open Keychain Access → Certificate Assistant → Request a Certificate from a Certificate Authority
   # Fill in:
   # - User Email Address: your@email.com
   # - Common Name: kindly-av1 Mac App Distribution
   # - CA Email: (leave blank)
   # - Request: Saved to disk
   # - Let me specify key pair information: ✓
   # - Key Size: 2048 bits
   # - Algorithm: RSA
   ```
5. Upload CSR file → Continue
6. Download certificate → Double-click to install in Keychain

**Verify Installation**:
```bash
security find-identity -v -p codesigning
# Should show: "3rd Party Mac Developer Application: Your Name (TEAM_ID)"
```

### 2. Mac Installer Distribution Certificate

**Purpose**: Sign the .pkg installer for App Store submission

**Steps**:
1. Repeat above process, selecting **Mac Installer Distribution**
2. CSR Common Name: `kindly-av1 Mac Installer Distribution`
3. Download and install certificate

**Verify Installation**:
```bash
security find-identity -v -p basic
# Should show: "3rd Party Mac Developer Installer: Your Name (TEAM_ID)"
```

## Provisioning Profile

**Note**: Mac App Store apps use **automatic provisioning** in Xcode. Manual provisioning profiles are optional for CLI tools.

**If using manual provisioning**:
1. Navigate to **Profiles** → **+** (Add)
2. Select **Mac App Store** → Continue
3. Choose App ID: `software.kindly.av1` → Continue
4. Select certificates → Continue
5. Name profile: `kindly-av1 Mac App Store` → Generate
6. Download and double-click to install

## App Store Connect Setup

### 1. Create App Record

**Location**: https://appstoreconnect.apple.com/apps

**Steps**:
1. Sign in with Apple Developer account
2. Click **My Apps** → **+** (Add) → **New App**
3. Fill in details:
   - **Platform**: macOS
   - **Name**: kindly-av1
   - **Primary Language**: English (U.S.)
   - **Bundle ID**: software.kindly.av1
   - **SKU**: KINDLY-AV1-001 (unique identifier, your choice)
   - **User Access**: Full Access
4. Click **Create**

### 2. App Information

**Category**: Video (primary), Developer Tools (secondary)

**Age Rating**:
- No restricted content
- Age: 4+

**Privacy Policy URL**: `https://kindly.software/privacy` (required)

**Support URL**: `https://kindly.software/support` (required)

**Marketing URL**: `https://kindly.software` (optional)

### 3. Pricing and Availability

**Pricing Tiers** (matching Gumroad):

| Tier | Price | Target |
|------|-------|--------|
| **Hobbyist** | $49 | Individual creators, students, hobbyists |
| **Professional** | $149 | Professional video editors, freelancers |
| **Studio** | $499 | Production studios, agencies |

**Alternative: In-App Purchase (IAP) for License Tiers**

Instead of three separate apps, use one app with IAP:
- Base app: Free (watermarked output, 720p max)
- IAP Unlock: $49/$149/$499 (removes watermark, unlocks features)

**IAP Benefits**:
- Single app listing (simpler for users)
- Apple handles payment (no Gumroad webhook needed)
- 30% Apple commission (vs Gumroad's 10%)
- Automatic license management

**IAP Drawbacks**:
- Higher commission (30% vs 10%)
- Apple's payment terms (can't use external payment)
- Subscription model preferred by Apple (one-time purchase allowed)

**Availability**:
- All countries (or select specific markets)
- Pricing varies by region (auto-converted by Apple)

### 4. Version Information

**Version**: 1.0.0

**Copyright**: `© 2025 Kindly Software. All rights reserved.`

**What's New in This Version**:
```
Initial release of kindly-av1, the world's fastest GPU-accelerated AV1 encoder.

Features:
• 10-100× faster encoding than CPU-based encoders
• ROCm/Vulkan/Metal GPU acceleration
• Professional-grade quality with VMAF 95+ scores
• Supports 4K, 8K, and HDR video
• Real-time encoding for 1080p video
• Advanced rate control (CRF, VBR, CBR)
• Scene detection and GOP optimization
• Interactive TUI wizard for easy setup
```

### 5. App Preview and Screenshots

**Requirements**:
- **App Icon**: 1024x1024 PNG (no transparency, no rounded corners)
- **Screenshots**:
  - 1280x800 (minimum)
  - 2880x1800 (Retina recommended)
  - 5-10 screenshots showing key features
  - Localized for each language (optional)

**Screenshot Ideas**:
1. TUI wizard welcome screen
2. Encoding progress with real-time stats
3. Quality comparison (before/after, file size)
4. Advanced settings panel
5. Benchmark results (vs x264/x265/SVT-AV1)

**Tools**:
```bash
# Capture screenshot
screencapture -i -o screenshot.png

# Resize for App Store
sips -Z 2880 screenshot.png --out screenshot-2880x1800.png
```

### 6. App Description

**Promotional Text** (170 chars, editable without new review):
```
Professional GPU-accelerated AV1 encoding. 10-100× faster than CPU encoders. Perfect for video editors, streamers, and content creators.
```

**Description** (4000 chars max):
```
kindly-av1 is the world's fastest GPU-accelerated AV1 encoder, delivering professional-grade video compression with unprecedented speed.

KEY FEATURES

• GPU ACCELERATION
  Harness the full power of your Mac's GPU with ROCm, Vulkan, and Metal backends.
  10-100× faster than traditional CPU encoders.

• PROFESSIONAL QUALITY
  Achieve VMAF 95+ quality scores with advanced perceptual optimization.
  Supports 4K, 8K, and HDR video with 10-bit color depth.

• REAL-TIME ENCODING
  Encode 1080p video in real-time for streaming and live production.

• ADVANCED RATE CONTROL
  Choose from CRF (constant quality), VBR (variable bitrate), or CBR (constant bitrate).
  Scene detection and GOP optimization for maximum efficiency.

• EASY TO USE
  Interactive TUI wizard guides you through setup.
  Command-line interface for automation and batch processing.

PERFORMANCE BENCHMARKS

• 1080p encode: 200-400 FPS (vs 20 FPS CPU)
• 4K encode: 50-100 FPS (vs 5 FPS CPU)
• 8K encode: 15-30 FPS (vs 1 FPS CPU)

SUPPORTED FORMATS

Input: MP4, MKV, AVI, MOV, WebM, Y4M, YUV
Output: IVF, MP4 (with container muxing)

SYSTEM REQUIREMENTS

• macOS 11.0 or later
• Apple Silicon (M1/M2/M3) or Intel Mac with discrete GPU
• 8 GB RAM minimum (16 GB recommended for 4K+)
• Metal-compatible GPU

LICENSING

Choose the tier that fits your needs:
• Hobbyist: $49 - For individual creators and students
• Professional: $149 - For video editors and freelancers
• Studio: $499 - For production studios and agencies

TRADE SECRET TECHNOLOGY

kindly-av1 uses proprietary computational capsule architecture for lockfree GPU coordination, delivering breakthrough performance that competitors cannot match.

SUPPORT

Email: support@kindly.software
Documentation: https://kindly.software/docs
Discord: https://discord.gg/kindly-av1
```

### 7. Keywords

**Maximum**: 100 characters (comma-separated, no spaces)

**Examples**:
```
AV1,video,encoder,GPU,accelerated,compression,streaming,4K,8K,HDR,VMAF,ROCm,Vulkan,Metal,codec
```

### 8. Review Notes

**App Store Review Notes** (for Apple reviewers):
```
kindly-av1 is a professional video encoder using GPU acceleration.

TEST ACCOUNT (optional):
Email: reviewer@kindly.software
License Key: [provide test license key]

SANDBOXING NOTES:
- App requires GPU access (com.apple.security.device.gpu entitlement)
- User-selected file access for reading/writing video files
- JIT compilation for Metal/Vulkan shader compilation
- No network access required (license validation optional)

TESTING INSTRUCTIONS:
1. Launch app from Terminal or double-click .app bundle
2. Drag & drop a video file or use TUI wizard
3. Encode completes in seconds (GPU-accelerated)
4. Output file saved to user-selected location

TRADE SECRET CODE:
The encoder uses proprietary lockfree GPU coordination algorithms.
Binary-only distribution protects intellectual property.
```

## Building and Signing

### 1. Build Universal Binary

```bash
cd packaging/macos

# Build for both architectures
./build-app.sh --universal --sign "3rd Party Mac Developer Application: Your Name (TEAM_ID)"
```

**Output**:
- `kindly-av1.app` - Signed app bundle
- `kindly-av1-1.0.0.pkg` - Signed installer package

### 2. Verify Signature

```bash
# Check code signature
codesign -dvv kindly-av1.app

# Verify entitlements
codesign -d --entitlements - kindly-av1.app

# Validate for App Store
spctl -a -vv kindly-av1.app

# Check package signature
pkgutil --check-signature kindly-av1-1.0.0.pkg
```

## Uploading to App Store

### Method 1: Xcode (Recommended)

**Steps**:
1. Open Xcode
2. **Window** → **Organizer**
3. Drag `kindly-av1-1.0.0.pkg` into Organizer
4. Click **Distribute App**
5. Select **App Store Connect** → Next
6. Choose **Upload** → Next
7. Select signing certificate → Next
8. Review summary → Upload
9. Wait for processing (5-30 minutes)

### Method 2: Transporter.app

**Download**: https://apps.apple.com/app/transporter/id1450874784

**Steps**:
1. Launch Transporter.app
2. Sign in with Apple ID
3. Drag `kindly-av1-1.0.0.pkg` into window
4. Click **Deliver**
5. Wait for upload to complete

### Method 3: altool (Command Line)

**Setup**:
```bash
# Create app-specific password at appleid.apple.com
# Store in keychain:
xcrun altool --store-password-in-keychain-item "AC_PASSWORD" \
    -u YOUR_APPLE_ID \
    -p YOUR_APP_SPECIFIC_PASSWORD
```

**Upload**:
```bash
xcrun altool --upload-app \
    -f kindly-av1-1.0.0.pkg \
    -t macos \
    -u YOUR_APPLE_ID \
    -p "@keychain:AC_PASSWORD"
```

**Check Status**:
```bash
xcrun altool --list-apps \
    -u YOUR_APPLE_ID \
    -p "@keychain:AC_PASSWORD"
```

## App Store Review

### 1. Submit for Review

**App Store Connect Steps**:
1. Navigate to app in App Store Connect
2. Click **+ Version** → Create new version (1.0.0)
3. Upload build (select from processed builds)
4. Complete all required metadata
5. Add screenshots and app preview (optional)
6. Set pricing and availability
7. Click **Submit for Review**

### 2. Review Guidelines Compliance

**Critical Guidelines for CLI Tools**:

**2.4.5 Software Requirements**:
- ✓ Apps using public APIs are allowed
- ✓ CLI tools are allowed (Terminal-based apps)
- ✓ Must provide user value (video encoding qualifies)

**2.5 Software Requirements**:
- ✓ App must run in sandboxed environment
- ✓ No private APIs (ROCm/Vulkan are public)
- ✓ No undocumented frameworks

**4.0 Design**:
- ✓ CLI apps must have clear purpose
- ✓ TUI wizard provides user-friendly interface
- ✓ Screenshots demonstrate functionality

**5.1 Privacy**:
- ✓ No data collection (all processing local)
- ✓ No network access (unless license validation)
- ✓ Privacy policy required (even if no data collected)

### 3. Common Rejection Reasons (and How to Avoid)

**Guideline 2.1 - App Completeness**:
- ❌ App crashes on launch
- ✅ Test thoroughly on clean macOS install
- ✅ Include demo video files in app bundle (optional)

**Guideline 2.3 - Accurate Metadata**:
- ❌ Screenshots don't match app functionality
- ✅ Capture real TUI interface, not mockups

**Guideline 2.5 - Software Requirements**:
- ❌ Requires external dependencies (ROCm drivers)
- ✅ Bundle all dependencies OR detect/guide installation
- ✅ Graceful fallback if GPU unavailable (CPU encode)

**Guideline 4.2 - Minimum Functionality**:
- ❌ App is just a wrapper around command-line tool
- ✅ TUI wizard adds significant user value
- ✅ Progress reporting, error handling, presets

**Guideline 5.1.1 - Data Collection and Storage**:
- ❌ Missing privacy policy
- ✅ Host privacy policy at https://kindly.software/privacy

### 4. Review Timeline

**Typical Review Times**:
- **Initial Review**: 1-3 days (can be up to 7 days)
- **Resubmission**: 1-2 days (faster for minor fixes)
- **Expedited Review**: Request if critical (limited to 2/year)

**Expedite Review Request**:
https://developer.apple.com/contact/app-store/?topic=expedite

## Post-Approval

### 1. Release Options

**Manual Release**:
- Review → Approve → Hold for manual release
- Choose exact release date/time

**Automatic Release**:
- Review → Approve → Release immediately

**Phased Release**:
- Review → Approve → Phased release over 7 days
- Gradual rollout (1% → 50% → 100%)

### 2. App Analytics

**Location**: App Store Connect → Analytics

**Metrics**:
- App Units (downloads)
- Sales (revenue)
- App Store Views (impressions)
- Conversion Rate (views → downloads)
- Crashes and Hangs

### 3. Version Updates

**Submitting Updates**:
1. Increment version (1.0.0 → 1.1.0)
2. Update `Info.plist` CFBundleVersion/CFBundleShortVersionString
3. Rebuild and re-sign
4. Upload new build to App Store Connect
5. Create new version in App Store Connect
6. Fill in "What's New" text
7. Submit for review (faster than initial review)

## Pricing and Financial

### 1. App Store Commission

**Standard Rates**:
- **30%** for first year of subscription or one-time purchase
- **15%** after first year of subscription (Small Business Program)

**Small Business Program** (if eligible):
- Requires <$1M annual revenue from App Store
- Reduced to **15% commission** on all sales
- Apply at: https://developer.apple.com/app-store/small-business-program/

### 2. Payment and Tax

**Bank Account Setup**:
- App Store Connect → Agreements, Tax, and Banking
- W-9 (US) or W-8BEN (non-US) tax form
- Bank account details (ACH/Wire)

**Payment Schedule**:
- Monthly payments (45 days after end of fiscal month)
- Minimum threshold: $150 (varies by region)

**Tax Reporting**:
- Apple withholds tax if W-8BEN not filed (30% backup withholding)
- File appropriate tax forms in Agreements, Tax, and Banking

### 3. Pricing vs Gumroad

**Cost Comparison** (for $149 Professional tier):

| Platform | Commission | Net Revenue | Notes |
|----------|------------|-------------|-------|
| **Gumroad** | 10% + $0.30 | $133.80 | Lower fees, external payment |
| **App Store** | 30% | $104.30 | Higher fees, integrated payment |
| **App Store (Small Business)** | 15% | $126.65 | Requires <$1M revenue |

**Recommendation**:
- **Dual Distribution**: Offer on both platforms
  - App Store: Discoverability, integrated payment, macOS users
  - Gumroad: Lower fees, flexibility, direct customer relationship
- **Price Parity**: Same price on both platforms (Apple's rules)
- **License Key**: Gumroad generates key, user activates in app

## Sandboxing Considerations

### 1. File Access

**Entitlement**: `com.apple.security.files.user-selected.read-write`

**Allowed**:
- ✓ Files explicitly selected by user (drag & drop, file dialog)
- ✓ Files in user-selected directory and subdirectories

**Not Allowed**:
- ❌ Arbitrary file system access
- ❌ Reading files without user permission

**Implementation**:
```rust
// Use macOS file dialogs for sandboxed access
use std::process::Command;

fn select_input_file() -> Option<PathBuf> {
    // Use osascript to show file picker (sandboxing-safe)
    let output = Command::new("osascript")
        .args(&[
            "-e",
            "POSIX path of (choose file with prompt \"Select video file:\")"
        ])
        .output()
        .ok()?;

    Some(PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
}
```

### 2. GPU Access

**Entitlement**: `com.apple.security.device.gpu`

**Requirements**:
- ✓ Metal framework allowed
- ✓ GPU compute shaders allowed
- ✓ ROCm/Vulkan allowed (via public APIs)

**Restrictions**:
- ❌ Kernel extensions not allowed (use Metal/Vulkan instead)
- ❌ Direct GPU memory mapping (use Metal buffers)

### 3. Network (Optional)

**Entitlement**: `com.apple.security.network.client`

**Use Cases**:
- License validation (Gumroad API)
- Crash reporting
- Update checks

**Privacy Disclosure**:
- Must declare in privacy policy
- Must request user permission if sensitive data

## Troubleshooting

### Build Errors

**Error**: "No signing identity found"
```bash
# List available identities
security find-identity -v -p codesigning

# If empty, re-download certificates from developer.apple.com
# Double-click .cer files to install in Keychain Access
```

**Error**: "Info.plist validation failed"
```bash
# Validate plist syntax
plutil -lint kindly-av1.app/Contents/Info.plist

# Fix formatting
plutil -convert xml1 kindly-av1.app/Contents/Info.plist
```

**Error**: "Entitlements not found"
```bash
# Check entitlements file exists
ls -l kindly-av1.app/Contents/Resources/*.entitlements

# Verify entitlements embedded
codesign -d --entitlements - kindly-av1.app
```

### Upload Errors

**Error**: "Invalid Package. The package does not include a signature."
```bash
# Re-sign package with Installer certificate
productsign --sign "3rd Party Mac Developer Installer: Your Name (TEAM_ID)" \
    kindly-av1-1.0.0.pkg kindly-av1-1.0.0-signed.pkg

# Verify signature
pkgutil --check-signature kindly-av1-1.0.0-signed.pkg
```

**Error**: "Invalid Provisioning Profile"
```bash
# Re-download provisioning profile
# Xcode → Preferences → Accounts → Download Manual Profiles

# Or use automatic signing in Xcode
```

### Sandbox Violations

**Error**: "App tries to access files outside sandbox"
```bash
# Enable sandbox logging
log stream --predicate 'process == "kindly-av1" AND eventMessage CONTAINS "sandbox"' --level debug

# Common violations:
# - Reading ~/.zshrc or ~/.bashrc (use --no-rcfile flag)
# - Writing to /tmp (use NSTemporaryDirectory())
# - Accessing /usr/local (bundle dependencies in .app)
```

## Resources

**Official Documentation**:
- App Store Review Guidelines: https://developer.apple.com/app-store/review/guidelines/
- App Store Connect Help: https://help.apple.com/app-store-connect/
- App Sandbox Design Guide: https://developer.apple.com/library/archive/documentation/Security/Conceptual/AppSandboxDesignGuide/
- Hardened Runtime: https://developer.apple.com/documentation/security/hardened_runtime
- Distributing Mac Apps: https://developer.apple.com/documentation/xcode/distributing-your-app-for-beta-testing-and-releases

**Community Resources**:
- WWDC Videos: https://developer.apple.com/videos/
- Developer Forums: https://developer.apple.com/forums/
- Stack Overflow: https://stackoverflow.com/questions/tagged/macos+app-store

**Tools**:
- Xcode: https://developer.apple.com/xcode/
- Transporter: https://apps.apple.com/app/transporter/id1450874784
- RocketSim (for testing): https://www.rocketsim.app/

## Next Steps

1. **Enroll in Apple Developer Program** ($99/year)
2. **Register Bundle ID**: `software.kindly.av1`
3. **Download Certificates**: Mac App Distribution + Mac Installer Distribution
4. **Create App Icon**: `./create-icns.sh your-icon.png`
5. **Build and Sign**: `./build-app.sh --universal --sign "3rd Party Mac Developer Application: Your Name (TEAM_ID)"`
6. **Create App Record** in App Store Connect
7. **Upload Build**: Via Xcode/Transporter/altool
8. **Submit for Review**: Complete metadata and submit
9. **Monitor Review**: App Store Connect → Activity
10. **Release**: Manual or automatic after approval

For questions: support@kindly.software
