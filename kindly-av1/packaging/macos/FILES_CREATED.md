# Mac App Store Packaging - Files Created

Complete Mac App Store packaging for kindly-av1 created on 2025-11-29.

## Directory Structure

```
packaging/macos/
├── kindly-av1.app/                      # App bundle (ready for binary)
│   └── Contents/
│       ├── Info.plist                   # 146 lines - App metadata
│       ├── MacOS/                       # Binary directory (empty, filled by build-app.sh)
│       └── Resources/
│           └── kindly-av1.entitlements  # 81 lines - Sandboxing permissions
├── build-app.sh                         # 240 lines - Build and sign script ✓ executable
├── create-icns.sh                       # 119 lines - Icon creation script ✓ executable
├── verify-setup.sh                      # 307 lines - Setup verification script ✓ executable
├── MAC_APP_STORE_SETUP.md               # 743 lines - Complete setup guide
├── ENTITLEMENTS.md                      # 541 lines - Entitlements explanation
├── README.md                            # 591 lines - Quick start guide
└── FILES_CREATED.md                     # This file
```

## Files Summary

| File | Lines | Purpose | Status |
|------|-------|---------|--------|
| **Info.plist** | 146 | App metadata, bundle ID, version, file types | ✓ Valid XML |
| **kindly-av1.entitlements** | 81 | Sandboxing permissions (GPU, files, JIT) | ✓ Valid XML |
| **build-app.sh** | 240 | Build universal binary, sign, create .pkg | ✓ Executable |
| **create-icns.sh** | 119 | Convert PNG to .icns (10 sizes) | ✓ Executable |
| **verify-setup.sh** | 307 | Verify setup completeness, check files | ✓ Executable |
| **MAC_APP_STORE_SETUP.md** | 743 | Complete guide (enrollment, certificates, upload) | ✓ Documentation |
| **ENTITLEMENTS.md** | 541 | Entitlements explained (security, sandboxing) | ✓ Documentation |
| **README.md** | 591 | Quick start, scripts usage, troubleshooting | ✓ Documentation |
| **FILES_CREATED.md** | (this) | Files created summary | ✓ Documentation |

**Total**: 2,768 lines (excluding this file)

## Configuration Details

### Info.plist Key Values

| Key | Value |
|-----|-------|
| **CFBundleIdentifier** | software.kindly.av1 |
| **CFBundleVersion** | 1.0.0 |
| **CFBundleShortVersionString** | 1.0.0 |
| **LSMinimumSystemVersion** | 11.0 |
| **CFBundleExecutable** | kindly-av1 |
| **LSApplicationCategoryType** | public.app-category.video |
| **CFBundleIconFile** | AppIcon |

### Entitlements (Required)

| Entitlement | Purpose |
|-------------|---------|
| `com.apple.security.app-sandbox` | Enable sandboxing (MANDATORY) |
| `com.apple.security.files.user-selected.read-write` | Access user-selected files |
| `com.apple.security.device.gpu` | GPU access (Metal/Vulkan/ROCm) |
| `com.apple.security.cs.allow-unsigned-executable-memory` | JIT shader compilation |

### Entitlements (Optional, Currently Disabled)

| Entitlement | Purpose | Enable If |
|-------------|---------|----------|
| `com.apple.security.network.client` | Outbound network | License validation, updates |
| `com.apple.security.network.server` | Inbound network | HTTP API, OBS overlay |
| `com.apple.security.device.camera` | Camera access | Live encoding |
| `com.apple.security.device.audio-input` | Microphone access | Audio encoding |

## Scripts

### build-app.sh

**Usage**:
```bash
# Universal binary (arm64 + x86_64)
./build-app.sh --universal --sign "3rd Party Mac Developer Application: Your Name (TEAM_ID)"

# Single architecture
./build-app.sh --sign "3rd Party Mac Developer Application: Your Name (TEAM_ID)"

# Unsigned (testing only)
./build-app.sh
```

**Steps**:
1. Build Rust binary with `cargo build --release --target [arch]`
2. Create universal binary with `lipo` (if `--universal`)
3. Copy binary to `kindly-av1.app/Contents/MacOS/kindly-av1`
4. Validate `Info.plist` and entitlements
5. Sign app bundle with `codesign` (if `--sign`)
6. Create `.pkg` installer with `pkgbuild` and `productsign`
7. Verify signatures with `codesign --verify` and `pkgutil --check-signature`

**Output**:
- `kindly-av1.app` (signed app bundle)
- `kindly-av1-1.0.0.pkg` (signed installer package for App Store upload)

### create-icns.sh

**Usage**:
```bash
# Specify input PNG
./create-icns.sh /path/to/icon.png

# Use default (docs/logo.png if exists)
./create-icns.sh
```

**Steps**:
1. Create `.iconset` directory
2. Generate 10 icon sizes (16×16 to 1024×1024, including @2x Retina)
3. Convert `.iconset` to `.icns` with `iconutil`
4. Copy to `kindly-av1.app/Contents/Resources/AppIcon.icns`
5. Clean up `.iconset` directory

**Icon Sizes Generated**:
- 16×16, 32×32 (16@2x), 128×128, 256×256 (128@2x), 512×512, 1024×1024 (512@2x)

### verify-setup.sh

**Usage**:
```bash
./verify-setup.sh
```

**Checks**:
1. Directory structure (app bundle, Contents, MacOS, Resources)
2. Required files (Info.plist, entitlements, scripts, docs)
3. Script permissions (executable flags)
4. Info.plist validation (XML syntax, required keys)
5. Entitlements validation (XML syntax, required entitlements)
6. Developer tools (codesign, pkgbuild, productsign, iconutil, sips, xcrun)
7. Code signing certificates (Mac App Distribution, Mac Installer Distribution)
8. App icon (AppIcon.icns presence and size)
9. Binary (kindly-av1 presence, size, architecture)
10. Documentation (completeness)

**Exit Codes**:
- `0` - All checks passed or warnings only
- `1` - Errors found (critical issues)

## Documentation

### MAC_APP_STORE_SETUP.md (743 lines)

**Comprehensive guide covering**:
- Apple Developer Program enrollment ($99/year)
- Bundle ID registration (software.kindly.av1)
- Certificate creation (App Distribution + Installer Distribution)
- App Store Connect setup (app record, metadata, pricing)
- Provisioning profiles
- Building and signing (universal binaries)
- Uploading to App Store (Xcode/Transporter/altool)
- App Store review process (guidelines, timeline, common rejections)
- Pricing and financial (commission rates, payment, tax)
- Sandboxing considerations (file access, GPU access, debugging)
- Troubleshooting (build errors, upload errors, sandbox violations)
- Resources and next steps

**Key Sections**:
1. Prerequisites (enrollment, environment)
2. Bundle ID Registration
3. Certificates (App Distribution, Installer Distribution)
4. App Store Connect Setup (app record, metadata, screenshots, pricing)
5. Building and Signing (universal binaries, verification)
6. Uploading to App Store (3 methods)
7. App Store Review (guidelines, rejection reasons, timeline)
8. Post-Approval (release options, analytics, updates)
9. Pricing and Financial (commission, payment, tax)
10. Sandboxing (file access, GPU access, debugging violations)
11. Troubleshooting (common errors and solutions)

### ENTITLEMENTS.md (541 lines)

**In-depth entitlements explanation**:
- What are entitlements?
- Why they matter (security, App Store requirements)
- kindly-av1 entitlements breakdown (9 total: 4 required, 5 optional)
- Each entitlement explained:
  1. App Sandbox (CRITICAL)
  2. User-Selected File Access (CRITICAL)
  3. GPU Access (CRITICAL)
  4. JIT Compilation (CRITICAL for GPU)
  5. Network Client (Optional)
  6. Network Server (Optional)
  7. Camera Access (Optional)
  8. Microphone Access (Optional)
  9. Disable Library Validation (Use with Caution)
- Hardened Runtime explanation
- Entitlements vs Info.plist permissions
- Debugging sandbox violations (logging, common violations)
- Testing entitlements (extraction, validation, GPU test)
- App Store review considerations (scrutinized entitlements, review notes template)
- Summary and next steps

### README.md (591 lines)

**Quick start and reference**:
- Quick start (6 steps: prerequisites, icon, build, verify, upload, submit)
- Directory structure (detailed tree)
- Scripts (build-app.sh, create-icns.sh, verify-setup.sh usage)
- Configuration files (Info.plist, entitlements)
- Certificates (required certificates, installation, verification)
- Sandboxing (file access, GPU access, debugging violations)
- Testing (local testing, sandbox testing, signature verification, entitlements verification)
- Troubleshooting (common errors and solutions)
- Documentation (references to other guides)
- Pricing (Mac App Store commission, Small Business Program, IAP alternative)
- License (trade secret protection)
- Support (email, website, docs, Discord)
- Next steps (10-step checklist with timeline estimate)

## Next Steps Checklist

**Setup (1-2 days)**:
- [ ] 1. Enroll in Apple Developer Program ($99/year): https://developer.apple.com/programs/enroll/
- [ ] 2. Wait for approval (24-48 hours)
- [ ] 3. Register Bundle ID: `software.kindly.av1` at https://developer.apple.com/account/resources/identifiers/
- [ ] 4. Download certificates:
  - [ ] Mac App Distribution: `3rd Party Mac Developer Application: Your Name (TEAM_ID)`
  - [ ] Mac Installer Distribution: `3rd Party Mac Developer Installer: Your Name (TEAM_ID)`
- [ ] 5. Install certificates (double-click .cer files, verify in Keychain Access)
- [ ] 6. Create app record in App Store Connect: https://appstoreconnect.apple.com/apps

**Build (1 hour)**:
- [ ] 7. Create app icon: `./create-icns.sh /path/to/icon.png` (1024×1024 PNG)
- [ ] 8. Build and sign: `./build-app.sh --universal --sign "3rd Party Mac Developer Application: ..."`
- [ ] 9. Verify build: `./verify-setup.sh` (check for errors)
- [ ] 10. Test app: `open kindly-av1.app` (ensure GPU encoding works)

**Upload (30 minutes)**:
- [ ] 11. Upload to App Store Connect:
  - Option A: Xcode → Organizer → Distribute App
  - Option B: Transporter.app
  - Option C: `xcrun altool --upload-app -f kindly-av1-1.0.0.pkg ...`
- [ ] 12. Wait for processing (5-30 minutes)

**Submit (15 minutes)**:
- [ ] 13. Complete metadata in App Store Connect:
  - [ ] Screenshots (5-10, showing TUI, encoding, stats)
  - [ ] Description (4000 chars, highlight GPU acceleration, speed, quality)
  - [ ] Keywords (100 chars: AV1,video,encoder,GPU,accelerated,...)
  - [ ] Pricing ($49/$149/$499 tiers)
  - [ ] Privacy policy URL (required)
  - [ ] Support URL (required)
  - [ ] Review notes (explain GPU/JIT entitlements)
- [ ] 14. Submit for review
- [ ] 15. Wait for approval (1-7 days)

**Timeline Estimate**: 3-10 days total (setup + build + upload + review)

## Trade Secret Protection

**CRITICAL**: kindly-av1 is **proprietary software** with **trade secret protection**.

**Binary-Only Distribution**:
- ✓ App Store receives signed binary only (no source code)
- ✓ Computational capsule architecture is protected
- ✓ GPU coordination algorithms are protected
- ✓ ROCm/Vulkan backends are protected

**Apple Review**:
- ✓ Apple reviews binary functionality (not source code)
- ✓ No source code disclosure required
- ✓ Trade secrets remain protected

**Commit Tags**:
- ALL commits related to this packaging MUST use `[TRADE SECRET]` tag
- Example: `[TRADE SECRET] Add Mac App Store packaging for kindly-av1`

## Validation

**Run verification script**:
```bash
cd /home/samuel/Primitives/kindly-av1/packaging/macos
./verify-setup.sh
```

**Expected output**:
- ✓ All required files present
- ✓ Scripts executable
- ✓ Info.plist valid XML (on macOS)
- ✓ Entitlements valid XML (on macOS)
- ⚠ Warnings if certificates/icon/binary not yet created (expected until build)

**Note**: On Linux (development machine), plutil/security commands are not available. Validation will show warnings for macOS-specific checks. This is expected and does not indicate errors.

## Support

**Questions or Issues?**
- Email: support@kindly.software
- Documentation: MAC_APP_STORE_SETUP.md (comprehensive guide)
- Entitlements: ENTITLEMENTS.md (detailed explanation)
- Quick start: README.md (step-by-step instructions)

## License

kindly-av1 packaging files are part of the kindly-av1 proprietary software suite.

**Copyright**: © 2025 Kindly Software. All rights reserved.

**Trade Secret Protection**: See `/home/samuel/Primitives/kindly-av1/TRADE_SECRET_NOTICE.md`

---

**Created**: 2025-11-29
**Location**: `/home/samuel/Primitives/kindly-av1/packaging/macos/`
**Total Files**: 8 files (3 scripts, 3 docs, 2 config)
**Total Lines**: 2,768 lines (excluding this file)
**Status**: ✓ Complete and ready for use
