# kindly-av1 Snap Packaging

Ubuntu Snap Store packaging for kindly-av1 GPU-accelerated AV1 encoder.

## Quick Start

```bash
# Build snap package
./build-snap.sh

# Test locally
sudo snap install kindly-av1_1.0.0_amd64.snap --dangerous
kindly-av1 --help

# Upload to Snap Store (after account setup)
snapcraft login
snapcraft register kindly-av1
snapcraft upload kindly-av1_1.0.0_amd64.snap --release=stable
```

## Directory Structure

```
packaging/snap/
├── snapcraft.yaml              # Snap configuration (confinement, plugs, parts)
├── snap/
│   ├── gui/
│   │   ├── kindly-av1.desktop  # Desktop entry for app launcher
│   │   └── kindly-av1.png      # Icon (256x256, Byzantine purple #9B59B6)
│   └── hooks/
│       └── configure           # GPU detection hook (auto-configures backend)
├── build-snap.sh               # Build script (Cargo + snapcraft)
├── SNAP_STORE_SETUP.md         # Complete publishing guide (8 steps)
└── README.md                   # This file
```

## Prerequisites

- Ubuntu 20.04+ or Ubuntu-based distribution
- Snapcraft: `sudo snap install snapcraft --classic`
- Rust toolchain (for building kindly-av1 binary)
- Internet connection (for Snap Store upload)

## Building

### 1. Build Snap Package

```bash
./build-snap.sh
```

**Steps**:
1. Builds kindly-av1 release binary (`cargo build --release`)
2. Strips debug symbols (reduce size)
3. Runs snapcraft to package binary + dependencies
4. Outputs: `kindly-av1_1.0.0_amd64.snap`

**Build Time**: 5-10 minutes (includes Rust compilation)

**Output Size**: ~10-30MB (binary + Vulkan runtime)

### 2. Test Locally

```bash
sudo snap install kindly-av1_1.0.0_amd64.snap --dangerous
kindly-av1 --version
kindly-av1 --help
```

**GPU Test**:
```bash
# Should detect GPU via Vulkan
kindly-av1 info /path/to/test/video.mp4
```

**Cleanup**:
```bash
sudo snap remove kindly-av1
```

## Publishing

See **SNAP_STORE_SETUP.md** for complete guide.

### Quick Reference

```bash
# 1. Login to Snap Store
snapcraft login

# 2. Register snap name (first-time only)
snapcraft register kindly-av1

# 3. Upload snap
snapcraft upload kindly-av1_1.0.0_amd64.snap --release=stable

# 4. View status
snapcraft status kindly-av1
```

**Review Time**: <24 hours (automated + manual review)

**Store URL**: https://snapcraft.io/kindly-av1 (after approval)

## Configuration

### snapcraft.yaml

Key settings:

| Setting | Value | Notes |
|---------|-------|-------|
| `name` | `kindly-av1` | Snap Store name |
| `version` | `1.0.0` | Semver version |
| `confinement` | `strict` | Sandboxed (GPU via opengl plug) |
| `base` | `core22` | Ubuntu 22.04 LTS runtime |
| `grade` | `stable` | Production release |

### Plugs (Interfaces)

| Plug | Purpose | Auto-connect |
|------|---------|--------------|
| `home` | Access user files ($HOME) | Yes |
| `removable-media` | Access external drives | No (manual) |
| `network` | License validation, OBS WebSocket | Yes |
| `network-bind` | HTTP server (OBS overlay) | Yes |
| `opengl` | Vulkan GPU access | Yes |
| `hardware-observe` | GPU device detection | Yes |

**Manual Connection** (removable-media):
```bash
sudo snap connect kindly-av1:removable-media
```

### GPU Access

Snap uses `opengl` plug for GPU access:

- **Vulkan**: Primary backend (AMD, NVIDIA, Intel)
- **ROCm**: Host-installed (not bundled in snap)
- **Mesa Drivers**: Bundled (libvulkan1, mesa-vulkan-drivers)

**Host Requirements**:
```bash
# AMD GPUs
sudo apt install mesa-vulkan-drivers

# NVIDIA GPUs
sudo ubuntu-drivers autoinstall
```

**Vulkan ICD Discovery**:
- Snap layout binds: `/usr/share/vulkan` → `$SNAP/usr/share/vulkan`
- Environment: `VK_ICD_FILENAMES` points to `/var/lib/snapd/lib/vulkan/icd.d/`

## Hooks

### configure

Runs on snap install/configure to detect GPU backend.

**Auto-detection**:
1. Check Vulkan: `vulkaninfo` command
2. Check ROCm: `/opt/rocm` directory
3. Fallback: CPU-only mode

**User Override**:
```bash
# Force Vulkan backend
snap set kindly-av1 gpu-backend=vulkan

# Force CPU fallback
snap set kindly-av1 gpu-backend=cpu
```

**Dashboard Port**:
```bash
# Change OBS overlay HTTP port (default: 8765)
snap set kindly-av1 dashboard-port=9000
```

## Desktop Integration

### kindly-av1.desktop

Desktop entry provides:
- Application launcher icon
- File association (MP4, MKV, AVI)
- Terminal-based UI (launches in terminal)
- Categories: AudioVideo, Video, Recorder

**Installation**: Auto-installed with snap, appears in application menu.

### Icon

**Placeholder**: `snap/gui/kindly-av1.png.placeholder`

**Generate Real Icon** (ImageMagick):
```bash
convert -size 256x256 xc:#663399 \
    -gravity center -pointsize 64 -fill white \
    -annotate +0+0 "K" \
    snap/gui/kindly-av1.png
```

**Requirements**:
- 256x256 PNG
- Byzantine purple background (#663399 or #9B59B6)
- White "K" monogram or kindly-av1 logo

## Troubleshooting

### Build Errors

**Error**: `snapcraft: command not found`

**Fix**: Install snapcraft
```bash
sudo snap install snapcraft --classic
```

---

**Error**: `kindly-av1 binary not found`

**Fix**: Build release binary first
```bash
cargo build --release --target x86_64-unknown-linux-gnu --bin kindly-av1
```

---

**Error**: `Failed to stage: No such file or directory`

**Fix**: Check `source` path in snapcraft.yaml matches binary location:
```yaml
source: ../../target/x86_64-unknown-linux-gnu/release/
```

### Runtime Errors

**Error**: GPU not detected

**Fix**: Install GPU drivers on host
```bash
# AMD
sudo apt install mesa-vulkan-drivers

# NVIDIA
sudo ubuntu-drivers autoinstall
```

---

**Error**: `cannot access /media/external/video.mp4`

**Fix**: Connect removable-media plug
```bash
sudo snap connect kindly-av1:removable-media
```

---

**Error**: License validation fails

**Fix**: Ensure network plug connected (auto-connected by default):
```bash
snap connections kindly-av1 | grep network
```

### Upload Errors

**Error**: `Name already registered`

**Fix**: Choose different name or contact current owner via Snap Store forum.

---

**Error**: `Classic confinement requires approval`

**Fix**: Use `strict` confinement (already configured). Classic not needed for kindly-av1.

## Distribution Model

### Free Binary, Paid Licenses

- **Snap Store**: FREE, unlimited distribution (no fees)
- **Monetization**: Gumroad license keys (pay-what-you-want)
- **Activation**: Offline license validation (Ed25519 crypto)
- **Tiers**: Creator ($49), Professional ($149), Enterprise ($499)

### User Experience

1. **Install**: `sudo snap install kindly-av1` (FREE)
2. **Trial**: Run without license (720p@30fps limit)
3. **Purchase**: Buy license at https://kindly.dev/pricing
4. **Activate**: `kindly-av1 license activate <KEY>`
5. **Encode**: Full 4K/8K GPU acceleration unlocked

## Updating

### New Version Release

```bash
# 1. Update version in snapcraft.yaml
sed -i 's/version: .*/version: "1.1.0"/' snapcraft.yaml

# 2. Rebuild snap
./build-snap.sh

# 3. Upload new version
snapcraft upload kindly-av1_1.1.0_amd64.snap --release=stable
```

**Auto-updates**: Users receive updates within 24 hours (automatic snap refresh).

### Beta Testing

```bash
# Release to beta channel first
snapcraft upload kindly-av1_1.1.0-beta1_amd64.snap --release=beta

# Users opt-in to beta
sudo snap install kindly-av1 --channel=beta

# Promote to stable after testing
snapcraft release kindly-av1 <revision> stable
```

## Metrics

View installation metrics:

**URL**: https://snapcraft.io/kindly-av1/metrics

**Available Metrics**:
- Daily/weekly/monthly installs
- Active users
- Geographic distribution
- Version distribution

**Access**: Login with Ubuntu One account (snap publisher)

## Support

### Snap Store

- **Forum**: https://forum.snapcraft.io
- **Docs**: https://snapcraft.io/docs
- **Bugs**: https://bugs.launchpad.net/snapcraft

### kindly-av1

- **Email**: support@kindly.dev
- **GitHub**: https://github.com/kindly-ai/kindly-av1
- **Website**: https://kindly.dev

## References

- **Snapcraft Docs**: https://snapcraft.io/docs
- **GPU Access**: https://snapcraft.io/docs/opengl-interface
- **Confinement**: https://snapcraft.io/docs/snap-confinement
- **Publishing**: See SNAP_STORE_SETUP.md

---

**Last Updated**: 2025-11-29
**kindly-av1 Version**: 1.0.0
**Snap Base**: core22 (Ubuntu 22.04 LTS)
