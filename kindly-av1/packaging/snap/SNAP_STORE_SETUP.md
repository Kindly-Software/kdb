# kindly-av1 Snap Store Publishing Guide

Complete guide to publishing kindly-av1 on Ubuntu Snap Store for FREE distribution.

## Quick Reference

| Step | Command | Duration |
|------|---------|----------|
| 1. Build snap | `./build-snap.sh` | 5-10 min |
| 2. Test locally | `sudo snap install kindly-av1_1.0.0_amd64.snap --dangerous` | <1 min |
| 3. Create account | Visit https://snapcraft.io/account | 2-5 min |
| 4. Login | `snapcraft login` | <1 min |
| 5. Register name | `snapcraft register kindly-av1` | <1 min |
| 6. Upload | `snapcraft upload kindly-av1_1.0.0_amd64.snap --release=stable` | 2-5 min |
| 7. Review | Wait for automated review | <24 hours |
| 8. Published | Check https://snapcraft.io/kindly-av1 | — |

## Prerequisites

- Ubuntu 20.04+ or Ubuntu-based distribution
- Snapcraft installed: `sudo snap install snapcraft --classic`
- Ubuntu One account (created during Step 3)
- Internet connection

## Step 1: Build Snap Package

```bash
cd /home/samuel/Primitives/kindly-av1/packaging/snap
./build-snap.sh
```

**Output**: `kindly-av1_1.0.0_amd64.snap` (~10-30MB depending on dependencies)

**Build Time**: 5-10 minutes (includes Rust release build + snapcraft)

**Troubleshooting**:
- `snapcraft: command not found` → Install snapcraft: `sudo snap install snapcraft --classic`
- Binary not found → Check Cargo.toml target: `x86_64-unknown-linux-gnu`
- Snapcraft errors → Run `snapcraft clean` and retry

## Step 2: Test Snap Locally

Before uploading to Snap Store, test the snap package locally:

```bash
# Install snap (--dangerous flag bypasses store signature check)
sudo snap install kindly-av1_1.0.0_amd64.snap --dangerous

# Test basic functionality
kindly-av1 --help
kindly-av1 --version

# Test GPU detection
kindly-av1 info /path/to/test/video.mp4

# Remove test installation
sudo snap remove kindly-av1
```

**Expected Output**:
```
kindly-av1 1.0.0
GPU-Accelerated AV1 Video Encoder

USAGE:
    kindly-av1 [OPTIONS] <INPUT> <OUTPUT>
...
```

**GPU Access Test**:
```bash
# Check Vulkan access (should list GPUs)
snap run --shell kindly-av1
> vulkaninfo --summary
```

If GPU not detected:
- Install mesa-vulkan-drivers: `sudo apt install mesa-vulkan-drivers`
- For NVIDIA: `sudo ubuntu-drivers autoinstall`
- Verify ICD: `ls /var/lib/snapd/lib/vulkan/icd.d/`

## Step 3: Create Ubuntu One Account

Visit: https://snapcraft.io/account

**Required Information**:
- Email address
- Full name
- Username (for snap store URL)

**Account Types**:
- **Personal**: Free, unlimited snaps, perfect for kindly-av1
- **Brand**: For organization publishing (optional)

**Verification**: Check email for verification link.

## Step 4: Login to Snapcraft

```bash
snapcraft login
```

**Prompts**:
```
Email: your-email@example.com
Password: ********
```

**Two-Factor Authentication**: If enabled on Ubuntu One account, enter 6-digit code.

**Success Message**:
```
Login successful. You now have these capabilities:

snaps:       No restriction
channels:    No restriction
permissions: package_access, package_manage, package_metrics
expires:     2026-11-29T00:00:00.000
```

**Session Duration**: 1 year (re-login annually)

## Step 5: Register Snap Name

Reserve the `kindly-av1` name on Snap Store:

```bash
snapcraft register kindly-av1
```

**Success Message**:
```
Congrats! You are now the publisher of 'kindly-av1'.
```

**Name Rules**:
- Lowercase letters, numbers, hyphens only
- Must be unique (first-come, first-served)
- Cannot start/end with hyphen

**If Name Taken**:
- Try variations: `kindly-av1-encoder`, `kindlyav1`, `kindly-av1-gpu`
- Update `snapcraft.yaml` name field to match
- Rebuild snap with new name

## Step 6: Upload Snap Package

Upload snap to Snap Store and release to stable channel:

```bash
snapcraft upload kindly-av1_1.0.0_amd64.snap --release=stable
```

**Upload Progress**:
```
Uploading kindly-av1_1.0.0_amd64.snap [================] 100%
Processing... |
```

**Success Message**:
```
Revision 1 of 'kindly-av1' created.
Track    Arch    Channel    Version    Revision
latest   amd64   stable     1.0.0      1
```

**Channels**:
- `stable` - Production releases (recommended for v1.0.0+)
- `candidate` - Release candidates
- `beta` - Beta testing
- `edge` - Development builds

**Multiple Channels** (optional):
```bash
# Release to beta first
snapcraft upload kindly-av1_1.0.0_amd64.snap --release=beta

# Promote to stable after testing
snapcraft release kindly-av1 1 stable
```

## Step 7: Automated Review

Snap Store runs automated security/quality checks:

**Review Process**:
1. **Upload** (instant) - Snap uploaded to Snap Store servers
2. **Automated Review** (<30 minutes) - Security scans, confinement checks
3. **Manual Review** (<24 hours, if needed) - Human review for classic confinement or sensitive interfaces

**Review Results**:
```bash
# Check review status
snapcraft status kindly-av1
```

**Output**:
```
Track    Arch    Channel    Version    Revision
latest   amd64   stable     1.0.0      1
                 candidate  ^          ^
                 beta       ^          ^
                 edge       ^          ^
```

**Common Review Failures**:

| Issue | Cause | Fix |
|-------|-------|-----|
| Classic confinement denied | `confinement: classic` without justification | Use `strict` confinement, request classic via forum |
| Missing interface declaration | Using plugs without declaration | Add plugs to `apps` section in snapcraft.yaml |
| Security scan flags | Binary contains suspicious code | Review ASSUM compliance, add documentation |

**Manual Review Trigger**:
- Classic confinement request
- `system-files` or `personal-files` plugs
- Network-bind on privileged ports (<1024)

**Manual Review Timeline**: 1-3 business days (Canonical staff review)

## Step 8: Published Snap

Once review passes, snap is live on Snap Store:

**Store URL**: https://snapcraft.io/kindly-av1

**Update Store Listing** (recommended):
1. Visit: https://snapcraft.io/kindly-av1/listing
2. Add description, screenshots, contact info
3. Set icon (upload 256x256 PNG)
4. Add categories (Video, AudioVideo)
5. Set website URL: https://kindly.dev

**Store Listing Best Practices**:
- **Description**: Clear, concise summary (use snapcraft.yaml description)
- **Screenshots**: Show CLI interface, TUI dashboard, encoding progress
- **Icon**: Use kindly-av1 logo (Byzantine purple `#9B59B6` background)
- **Contact**: support@kindly.dev, https://github.com/kindly-ai/kindly-av1
- **Categories**: AudioVideo, Video, Utilities

## Installation (Users)

Once published, users can install with:

```bash
# Install from Snap Store
sudo snap install kindly-av1

# Run encoder
kindly-av1 input.mp4 output.ivf --crf 23 --gpu
```

**Auto-updates**: Snaps auto-update daily (configurable)

**GPU Access** (users may need to configure):
```bash
# AMD GPUs
sudo apt install mesa-vulkan-drivers

# NVIDIA GPUs
sudo ubuntu-drivers autoinstall
```

## Automatic Updates

Snap Store handles automatic updates:

**Update Frequency**: Daily (checks every 4 hours by default)

**Manual Update**:
```bash
sudo snap refresh kindly-av1
```

**Disable Auto-update** (not recommended):
```bash
sudo snap refresh --hold kindly-av1
```

**Version Rollback**:
```bash
# List revisions
snap list --all kindly-av1

# Revert to previous revision
sudo snap revert kindly-av1
```

## Publishing New Versions

Upload new snap versions to same name:

```bash
# Update version in snapcraft.yaml
sed -i 's/version: .*/version: "1.1.0"/' snapcraft.yaml

# Rebuild snap
./build-snap.sh

# Upload new version
snapcraft upload kindly-av1_1.1.0_amd64.snap --release=stable
```

**Revision Numbers**: Increment automatically (1, 2, 3, ...)

**Users Get Updates**: Within 24 hours (automatic snap refresh)

## Pricing and Monetization

**Snap Store Distribution**: FREE (no fees, unlimited users)

**Monetization Options**:
1. **License Keys** (Current Model):
   - Snap provides FREE binary distribution
   - Monetize via Gumroad license keys
   - License validation in binary (offline-capable)

2. **Freemium Model**:
   - Snap provides full-featured FREE version
   - Pro features unlocked via license key
   - Example: GPU acceleration requires license

3. **Support Packages**:
   - Snap is FREE, charge for priority support
   - Documentation/tutorials behind paywall

**Recommended**: License-based activation (current kindly-av1 model)
- Snap distributes binary widely (discovery)
- License keys monetize (Gumroad pay-what-you-want)
- No Snap Store fees or commission

## Snap Store Analytics

Track installation/usage metrics:

**View Metrics**:
1. Visit: https://snapcraft.io/kindly-av1/metrics
2. Login with Ubuntu One account
3. View: Installs, Active Users, Countries, Versions

**Metrics Available**:
- Daily/weekly/monthly active installations
- Geographic distribution (countries/regions)
- Snap version distribution
- Channel usage (stable/beta/edge)

**Integration**: No code changes required (automatic telemetry)

## Confinement Modes

kindly-av1 uses `strict` confinement:

| Mode | Description | Use Case |
|------|-------------|----------|
| **strict** | Sandboxed, limited host access | kindly-av1 (GPU via opengl plug) |
| devmode | Development mode, full access | Testing only |
| classic | No confinement, full access | System tools (rare) |

**GPU Access via Strict Confinement**:
- `opengl` plug: Vulkan/Mesa GPU access
- `hardware-observe` plug: GPU device detection
- Works with AMD (ROCm), NVIDIA (CUDA), Intel (Vulkan)

**Classic Confinement Request** (if needed):
- Forum post: https://forum.snapcraft.io/c/store-requests/19
- Justification: Explain why strict confinement insufficient
- Timeline: 1-2 weeks (manual review)

## Troubleshooting

### Build Errors

**Error**: `snapcraft: command not found`

**Fix**:
```bash
sudo snap install snapcraft --classic
```

---

**Error**: `Failed to stage: kindly-av1 not found`

**Fix**: Binary must exist at `../../target/x86_64-unknown-linux-gnu/release/kindly-av1`
```bash
cargo build --release --target x86_64-unknown-linux-gnu --bin kindly-av1
```

---

**Error**: `Architecture not supported`

**Fix**: Build for x86_64 only (ARM64 coming in v1.1.0):
```yaml
architectures:
  - build-on: amd64
    build-for: amd64
```

### Upload Errors

**Error**: `Name already registered`

**Fix**: Choose different name or contact current owner (via Snap Store forum)

---

**Error**: `Classic confinement requires approval`

**Fix**: Change to `strict` confinement (recommended for kindly-av1):
```yaml
confinement: strict
```

### Runtime Errors

**Error**: GPU not detected after snap install

**Fix**: Install GPU drivers on host:
```bash
# AMD
sudo apt install mesa-vulkan-drivers

# NVIDIA
sudo ubuntu-drivers autoinstall
```

---

**Error**: Permission denied accessing `/home/user/videos`

**Fix**: Snap home plug has access to `$HOME`, but not root-owned paths. Move videos to user home:
```bash
cp /media/videos/*.mp4 ~/Videos/
kindly-av1 ~/Videos/input.mp4 ~/Videos/output.ivf
```

---

**Error**: `cannot access removable media`

**Fix**: Manually connect removable-media plug (not auto-connected):
```bash
sudo snap connect kindly-av1:removable-media
```

## Best Practices

### Security

- **Strict Confinement**: Use strict confinement (already configured)
- **Minimal Plugs**: Only request necessary interfaces (opengl, home, network)
- **License Validation**: Keep Ed25519 crypto offline-capable (no network required)
- **Updates**: Push security updates to stable channel promptly

### Distribution

- **Free Binary**: Snap Store provides FREE, unlimited distribution
- **License Monetization**: Charge for license keys (Gumroad), not snap
- **Discovery**: Snap Store has 6M+ active users, good for brand awareness
- **Multi-Platform**: Add ARM64 support in v1.1.0 for Raspberry Pi

### Maintenance

- **Automated Testing**: Test snap locally before upload (`./build-snap.sh`)
- **Beta Channel**: Use beta channel for release candidates
- **Changelog**: Document changes in snapcraft.yaml description
- **Support**: Monitor GitHub issues, respond to snap-specific problems

### Marketing

- **Store Listing**: Add screenshots, icon, detailed description
- **Categories**: List in Video, AudioVideo, Utilities
- **Keywords**: av1, encoder, gpu, rocm, vulkan, 4k, video
- **Website**: Link https://kindly.dev for detailed docs
- **Social**: Promote snap install command on Twitter/Reddit

## Support

### Snap Store Support

- **Forum**: https://forum.snapcraft.io
- **Documentation**: https://snapcraft.io/docs
- **Bug Reports**: https://bugs.launchpad.net/snapcraft

### kindly-av1 Support

- **Email**: support@kindly.dev
- **GitHub**: https://github.com/kindly-ai/kindly-av1/issues
- **Website**: https://kindly.dev
- **Discord**: https://discord.gg/kindly (coming soon)

## References

- **Snapcraft Documentation**: https://snapcraft.io/docs
- **Publishing Guide**: https://snapcraft.io/docs/releasing-your-app
- **Confinement**: https://snapcraft.io/docs/snap-confinement
- **Interfaces**: https://snapcraft.io/docs/supported-interfaces
- **GPU Access**: https://snapcraft.io/docs/opengl-interface

---

**Last Updated**: 2025-11-29
**kindly-av1 Version**: 1.0.0
**Snap Base**: core22 (Ubuntu 22.04 LTS)
