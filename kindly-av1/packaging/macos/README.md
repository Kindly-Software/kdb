# Mac App Store Packaging - kindly-av1

Complete Mac App Store packaging for kindly-av1 GPU-accelerated AV1 encoder.

## Quick Start

### 1. Prerequisites

- **Apple Developer Program**: $99/year enrollment at https://developer.apple.com/programs/
- **macOS**: 11.0 or later
- **Xcode**: 13.0+ with Command Line Tools
- **Certificates**: Mac App Distribution + Mac Installer Distribution

### 2. Create App Icon

```bash
cd packaging/macos

# Create icon from 1024x1024 PNG source
./create-icns.sh /path/to/your/icon.png

# Verify icon created
ls -lh kindly-av1.app/Contents/Resources/AppIcon.icns
```

**Icon Requirements**:
- PNG format, 1024x1024 pixels
- Transparent background optional
- No rounded corners (macOS adds automatically)
- High contrast for Retina displays

### 3. Build and Sign

```bash
# Build universal binary (Apple Silicon + Intel)
./build-app.sh --universal --sign "3rd Party Mac Developer Application: Your Name (TEAM_ID)"

# Or build for host architecture only
./build-app.sh --sign "3rd Party Mac Developer Application: Your Name (TEAM_ID)"
```

**Output**:
- `kindly-av1.app` - Signed app bundle (ready for testing)
- `kindly-av1-1.0.0.pkg` - Signed installer package (ready for App Store upload)

### 4. Verify Build

```bash
# Check code signature
codesign -dvv kindly-av1.app

# Verify entitlements
codesign -d --entitlements - kindly-av1.app

# Test app bundle
open kindly-av1.app
# Or: ./kindly-av1.app/Contents/MacOS/kindly-av1 --help
```

### 5. Upload to App Store

**Method 1: Xcode Organizer** (Recommended)
1. Open Xcode
2. Window → Organizer
3. Drag `kindly-av1-1.0.0.pkg` into Organizer
4. Click **Distribute App** → **App Store Connect** → **Upload**
5. Wait for processing (5-30 minutes)

**Method 2: Command Line**
```bash
# Store app-specific password in keychain first
xcrun altool --upload-app \
    -f kindly-av1-1.0.0.pkg \
    -t macos \
    -u YOUR_APPLE_ID \
    -p "@keychain:AC_PASSWORD"
```

**Method 3: Transporter.app**
1. Download from Mac App Store: https://apps.apple.com/app/transporter/id1450874784
2. Launch Transporter.app
3. Drag `kindly-av1-1.0.0.pkg` into window
4. Click **Deliver**

### 6. Submit for Review

1. Navigate to https://appstoreconnect.apple.com/apps
2. Select kindly-av1 app
3. Choose uploaded build
4. Complete metadata (screenshots, description, pricing)
5. Click **Submit for Review**
6. Wait 1-7 days for review

## Directory Structure

```
packaging/macos/
├── kindly-av1.app/                 # App bundle (created by build-app.sh)
│   └── Contents/
│       ├── Info.plist              # App metadata
│       ├── MacOS/
│       │   └── kindly-av1          # Binary (universal or single-arch)
│       ├── Resources/
│       │   ├── AppIcon.icns        # App icon (created by create-icns.sh)
│       │   └── kindly-av1.entitlements  # Sandboxing permissions
│       └── _CodeSignature/         # Code signature (created by codesign)
├── build-app.sh                    # Build and sign script (executable)
├── create-icns.sh                  # Icon creation script (executable)
├── kindly-av1-1.0.0.pkg            # Installer package (created by build-app.sh)
├── MAC_APP_STORE_SETUP.md          # Complete setup guide (3,500+ lines)
├── ENTITLEMENTS.md                 # Entitlements explanation (700+ lines)
└── README.md                       # This file
```

## Scripts

### build-app.sh

**Purpose**: Build universal binary, create app bundle, sign with certificates, package for App Store.

**Usage**:
```bash
# Universal binary (Apple Silicon + Intel)
./build-app.sh --universal --sign "3rd Party Mac Developer Application: Your Name (TEAM_ID)"

# Single architecture (host only)
./build-app.sh --sign "3rd Party Mac Developer Application: Your Name (TEAM_ID)"

# Unsigned (testing only, not for App Store)
./build-app.sh --universal
```

**Steps**:
1. Build Rust binary with `cargo build --release`
2. Create universal binary with `lipo` (if `--universal`)
3. Copy binary to `kindly-av1.app/Contents/MacOS/`
4. Validate `Info.plist` and entitlements
5. Sign app bundle with Developer ID
6. Create `.pkg` installer with `pkgbuild` and `productsign`
7. Verify signatures

**Output**:
- `kindly-av1.app` (signed)
- `kindly-av1-1.0.0.pkg` (signed)

### create-icns.sh

**Purpose**: Convert PNG source image to macOS `.icns` icon format (10 sizes).

**Usage**:
```bash
# Specify input PNG
./create-icns.sh /path/to/icon.png

# Use default location (if logo.png exists in docs/)
./create-icns.sh
```

**Steps**:
1. Create `.iconset` directory
2. Generate 10 icon sizes (16×16 to 1024×1024, including @2x Retina)
3. Convert `.iconset` to `.icns` with `iconutil`
4. Copy to `kindly-av1.app/Contents/Resources/AppIcon.icns`

**Icon Sizes**:
- 16×16, 32×32 (16@2x), 128×128, 256×256 (128@2x), 512×512, 1024×1024 (512@2x)

## Configuration Files

### Info.plist

**Location**: `kindly-av1.app/Contents/Info.plist`

**Key Fields**:
- **CFBundleIdentifier**: `software.kindly.av1` (unique bundle ID)
- **CFBundleVersion**: `1.0.0` (build number, increment for updates)
- **CFBundleShortVersionString**: `1.0.0` (user-facing version)
- **LSMinimumSystemVersion**: `11.0` (macOS Big Sur minimum)
- **CFBundleExecutable**: `kindly-av1` (binary name)
- **CFBundleIconFile**: `AppIcon` (icon file, no .icns extension)
- **LSApplicationCategoryType**: `public.app-category.video` (App Store category)

**Supported File Types**:
- Video: `.mp4`, `.mkv`, `.avi`, `.mov`, `.webm`, `.y4m`, `.yuv`
- AV1: `.ivf`, `.obu`

**Privacy Descriptions** (Info.plist keys):
- `NSCameraUsageDescription` - "kindly-av1 may access camera for live video encoding."
- `NSMicrophoneUsageDescription` - "kindly-av1 may access microphone for audio encoding."
- `NSAppleEventsUsageDescription` - "kindly-av1 needs access to AppleScript events for automation integration."

**Note**: Camera/microphone permissions are **placeholders**. Remove if not implementing live capture.

### kindly-av1.entitlements

**Location**: `kindly-av1.app/Contents/Resources/kindly-av1.entitlements`

**Critical Entitlements** (see ENTITLEMENTS.md for details):

| Entitlement | Purpose |
|-------------|---------|
| `com.apple.security.app-sandbox` | Enable sandboxing (MANDATORY) |
| `com.apple.security.files.user-selected.read-write` | Access user-selected files |
| `com.apple.security.device.gpu` | GPU access (Metal/Vulkan/ROCm) |
| `com.apple.security.cs.allow-unsigned-executable-memory` | JIT shader compilation |

**Optional Entitlements** (currently commented out):
- `com.apple.security.network.client` - Outbound network (license validation)
- `com.apple.security.network.server` - Inbound network (HTTP API)
- `com.apple.security.device.camera` - Camera access (live encoding)
- `com.apple.security.device.audio-input` - Microphone access (audio encoding)

**Enable Optional Entitlements**:
1. Uncomment in `kindly-av1.entitlements`
2. Add corresponding `NSUsageDescription` in `Info.plist`
3. Rebuild and re-sign app

## Certificates

### Required Certificates

**1. Mac App Distribution Certificate**

**Purpose**: Sign app bundle for App Store submission

**Common Name**: `3rd Party Mac Developer Application: Your Name (TEAM_ID)`

**Download**: https://developer.apple.com/account/resources/certificates/list

**Verify Installation**:
```bash
security find-identity -v -p codesigning
# Output: "3rd Party Mac Developer Application: Your Name (TEAM_ID)"
```

**2. Mac Installer Distribution Certificate**

**Purpose**: Sign `.pkg` installer for App Store submission

**Common Name**: `3rd Party Mac Developer Installer: Your Name (TEAM_ID)`

**Download**: https://developer.apple.com/account/resources/certificates/list

**Verify Installation**:
```bash
security find-identity -v -p basic
# Output: "3rd Party Mac Developer Installer: Your Name (TEAM_ID)"
```

### Certificate Installation

**Download Certificates**:
1. Navigate to https://developer.apple.com/account/resources/certificates/list
2. Click **+** (Add) → Select certificate type → Continue
3. Create CSR (Certificate Signing Request):
   - Open **Keychain Access** → **Certificate Assistant** → **Request a Certificate from a Certificate Authority**
   - User Email: `your@email.com`
   - Common Name: `kindly-av1 Mac App Distribution`
   - CA Email: (leave blank)
   - Request: **Saved to disk**
   - Key Size: **2048 bits**
   - Algorithm: **RSA**
4. Upload CSR → Download certificate → Double-click to install

**Verify Certificates**:
```bash
# List all code signing identities
security find-identity -v -p codesigning

# List all installer identities
security find-identity -v -p basic

# Expected output:
# 1) ABC123... "3rd Party Mac Developer Application: Your Name (TEAM_ID)"
# 2) DEF456... "3rd Party Mac Developer Installer: Your Name (TEAM_ID)"
```

## Sandboxing

### File Access

**Entitlement**: `com.apple.security.files.user-selected.read-write`

**Allowed**:
- ✓ Files selected by user (file dialog, drag & drop, command-line arguments)
- ✓ Files in user-selected directory and subdirectories

**Not Allowed**:
- ❌ Arbitrary file system access
- ❌ Reading files without user permission

**Implementation**:
```rust
// Use macOS file dialogs for sandbox-safe file selection
use std::process::Command;

fn select_input_file() -> Option<PathBuf> {
    let output = Command::new("osascript")
        .args(&["-e", "POSIX path of (choose file with prompt \"Select video file:\")"])
        .output()
        .ok()?;
    Some(PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
}

// User selects file → macOS grants security-scoped bookmark
// Subsequent reads/writes work normally
let file = std::fs::File::open(path)?; // ✓ Works
```

### GPU Access

**Entitlement**: `com.apple.security.device.gpu`

**Allowed**:
- ✓ Metal framework (Apple's GPU API)
- ✓ Vulkan via MoltenVK
- ✓ ROCm HIP runtime (AMD GPU compute)
- ✓ GPU memory allocation, shader compilation

**Testing**:
```rust
// Verify GPU access works
fn test_gpu() -> Result<(), String> {
    let device = metal::Device::system_default()
        .ok_or("GPU access denied")?;
    println!("✓ GPU: {}", device.name());
    Ok(())
}
```

### Debugging Sandbox Violations

**Enable Logging**:
```bash
# Stream sandbox violations in real-time
log stream --predicate 'process == "kindly-av1" AND eventMessage CONTAINS "sandbox"' --level debug

# Example violation:
# Sandbox: kindly-av1(12345) deny(1) file-read-data /Users/john/.zshrc
```

**Common Violations**:
1. Reading dotfiles (`.zshrc`, `.bashrc`) → Don't access shell configs
2. Writing to `/tmp` → Use `std::env::temp_dir()` instead
3. Accessing `/usr/local` → Bundle libraries in `.app/Contents/Frameworks/`
4. Network without entitlement → Add `com.apple.security.network.client`

## Testing

### Local Testing

```bash
# Test unsigned app (development)
./build-app.sh --universal
open kindly-av1.app

# Test signed app (App Store simulation)
./build-app.sh --universal --sign "3rd Party Mac Developer Application: Your Name (TEAM_ID)"
open kindly-av1.app
```

### Sandbox Testing

```bash
# Check if app is sandboxed
ps aux | grep kindly-av1
asctl sandbox check --pid <PID>

# Expected: "kindly-av1 (pid 12345): Sandboxed"
```

### Signature Verification

```bash
# Verify app signature
codesign --verify --deep --strict --verbose=2 kindly-av1.app

# Expected: "kindly-av1.app: valid on disk"

# Verify package signature
pkgutil --check-signature kindly-av1-1.0.0.pkg

# Expected: "Status: signed by a certificate trusted by macOS"
```

### Entitlements Verification

```bash
# Extract embedded entitlements
codesign -d --entitlements - kindly-av1.app

# Expected output (XML):
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>com.apple.security.app-sandbox</key>
    <true/>
    <key>com.apple.security.device.gpu</key>
    <true/>
    ...
</dict>
</plist>
```

## Troubleshooting

### "No signing identity found"

**Problem**: `codesign` can't find certificate

**Solution**:
```bash
# List available identities
security find-identity -v -p codesigning

# If empty, re-download certificates from developer.apple.com
# Double-click .cer files to install in Keychain Access
```

### "Info.plist validation failed"

**Problem**: Invalid XML syntax in Info.plist

**Solution**:
```bash
# Validate plist
plutil -lint kindly-av1.app/Contents/Info.plist

# Fix formatting
plutil -convert xml1 kindly-av1.app/Contents/Info.plist
```

### "Entitlements not found"

**Problem**: Entitlements file missing or not embedded

**Solution**:
```bash
# Check file exists
ls -l kindly-av1.app/Contents/Resources/*.entitlements

# Re-sign with entitlements
codesign --force --sign "3rd Party Mac Developer Application: Your Name (TEAM_ID)" \
    --entitlements kindly-av1.app/Contents/Resources/kindly-av1.entitlements \
    --options runtime \
    kindly-av1.app
```

### "Package signature invalid"

**Problem**: `.pkg` not signed with Installer certificate

**Solution**:
```bash
# Re-sign with correct certificate
productsign --sign "3rd Party Mac Developer Installer: Your Name (TEAM_ID)" \
    kindly-av1-1.0.0.pkg kindly-av1-1.0.0-signed.pkg

# Verify
pkgutil --check-signature kindly-av1-1.0.0-signed.pkg
```

### App crashes on launch (sandboxing)

**Problem**: App tries to access files outside sandbox

**Solution**:
```bash
# Enable sandbox logging
log stream --predicate 'process == "kindly-av1"' --level debug

# Look for "deny(1)" messages
# Fix code to use user-selected files only
```

### GPU access denied

**Problem**: Missing `com.apple.security.device.gpu` entitlement

**Solution**:
1. Uncomment entitlement in `kindly-av1.entitlements`
2. Rebuild and re-sign: `./build-app.sh --universal --sign "..."`
3. Test: `./kindly-av1.app/Contents/MacOS/kindly-av1 --gpu-test`

## Documentation

### Complete Guides

- **MAC_APP_STORE_SETUP.md** (3,500+ lines)
  - Apple Developer enrollment ($99/year)
  - Bundle ID registration
  - Certificate creation
  - App Store Connect setup
  - Provisioning profiles
  - Uploading to App Store
  - Pricing and financial details
  - Review process and guidelines
  - Troubleshooting

- **ENTITLEMENTS.md** (700+ lines)
  - What are entitlements?
  - Why they matter
  - Each entitlement explained
  - Security implications
  - Testing entitlements
  - App Store review considerations
  - Debugging sandbox violations

- **README.md** (this file)
  - Quick start guide
  - Directory structure
  - Scripts usage
  - Configuration files
  - Certificate management
  - Testing procedures

### External Resources

**Apple Documentation**:
- App Store Review Guidelines: https://developer.apple.com/app-store/review/guidelines/
- App Store Connect Help: https://help.apple.com/app-store-connect/
- App Sandbox Guide: https://developer.apple.com/library/archive/documentation/Security/Conceptual/AppSandboxDesignGuide/
- Hardened Runtime: https://developer.apple.com/documentation/security/hardened_runtime
- Distributing Mac Apps: https://developer.apple.com/documentation/xcode/distributing-your-app-for-beta-testing-and-releases

**Community Resources**:
- WWDC Videos: https://developer.apple.com/videos/
- Developer Forums: https://developer.apple.com/forums/
- Stack Overflow: https://stackoverflow.com/questions/tagged/macos+app-store

## Pricing

### Mac App Store Commission

| Tier | Price | Apple Commission (30%) | Net Revenue |
|------|-------|------------------------|-------------|
| **Hobbyist** | $49 | $14.70 | $34.30 |
| **Professional** | $149 | $44.70 | $104.30 |
| **Studio** | $499 | $149.70 | $349.30 |

**Small Business Program** (if annual revenue <$1M):
- Reduced to **15% commission** (vs 30%)
- Apply at: https://developer.apple.com/app-store/small-business-program/

### Alternative: In-App Purchase

**Model**: Free base app + IAP unlock tiers

**Benefits**:
- Single app listing (simpler for users)
- Apple handles payment (no Gumroad webhook needed)
- Automatic license management

**Drawbacks**:
- Higher commission (30% vs Gumroad's 10%)
- Apple's payment terms (can't use external payment like Gumroad)

## License

kindly-av1 is **proprietary software** with **trade secret protection**. Binary distribution only (source code not included in App Store build).

**TRADE SECRET NOTICE**: The computational capsule architecture and GPU coordination algorithms are protected as trade secrets. See `TRADE_SECRET_NOTICE.md` in project root.

## Support

**Email**: support@kindly.software
**Website**: https://kindly.software
**Documentation**: https://kindly.software/docs
**Discord**: https://discord.gg/kindly-av1

## Next Steps

1. ✓ Review this README
2. ✓ Read MAC_APP_STORE_SETUP.md (complete guide)
3. ✓ Read ENTITLEMENTS.md (understand permissions)
4. ✓ Enroll in Apple Developer Program ($99/year)
5. ✓ Download certificates (App Distribution + Installer Distribution)
6. ✓ Create app icon: `./create-icns.sh icon.png`
7. ✓ Build and sign: `./build-app.sh --universal --sign "..."`
8. ✓ Test locally: `open kindly-av1.app`
9. ✓ Upload to App Store: Via Xcode/Transporter/altool
10. ✓ Submit for review in App Store Connect

**Estimated Timeline**:
- Setup (1-2 days): Enrollment, certificates, app record
- Build (1 hour): Icon creation, building, signing
- Upload (30 minutes): Package upload and processing
- Review (1-7 days): Apple review and approval
- **Total: 3-10 days from start to App Store availability**

Good luck! 🚀
