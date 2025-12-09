# kindly-av1 Flatpak Packaging

Quick reference for building and distributing kindly-av1 as a Flatpak.

## Quick Start

```bash
# Build Flatpak locally
./build-flatpak.sh

# Install locally
flatpak install --user kindly-av1.flatpak

# Run
flatpak run software.kindly.av1 --help
flatpak run software.kindly.av1 input.y4m -o output.ivf --preset 6 --crf 28
```

## Files

| File | Purpose |
|------|---------|
| `software.kindly.av1.yml` | Flatpak manifest (dependencies, build, install) |
| `software.kindly.av1.metainfo.xml` | AppStream metadata (name, description, screenshots) |
| `icons/software.kindly.av1.svg` | Vector icon (scalable) |
| `icons/software.kindly.av1-256.png` | PNG icon fallback (256×256) |
| `build-flatpak.sh` | Build script (compiles binary, builds Flatpak, creates bundle) |
| `FLATHUB_SETUP.md` | Comprehensive Flathub publishing guide |

## Build Process

The build script performs these steps:

1. **Build release binary**: `cargo build --release --target x86_64-unknown-linux-gnu`
2. **Install Flatpak runtime**: `org.freedesktop.Platform//23.08`
3. **Clean build directory**: Remove previous builds
4. **Run flatpak-builder**: Build Flatpak from manifest
5. **Export to repository**: Create/update local Flatpak repo
6. **Create bundle**: Generate `kindly-av1.flatpak` for distribution

## Permissions Explained

kindly-av1 requires these Flatpak permissions:

| Permission | Why Needed |
|------------|------------|
| `--device=dri` | GPU access for hardware-accelerated encoding (Vulkan/ROCm/CUDA) |
| `--filesystem=home` | Read input videos, write output files |
| `--filesystem=/media` | Access removable drives (USB, external HDDs) |
| `--share=network` | License validation, telemetry (optional) |
| `--socket=x11` | X11 display for GUI/progress feedback (future) |
| `--socket=wayland` | Wayland display support (modern Linux) |

**Security**: All permissions are justified for video encoding workflow. No excessive access requested.

## GPU Access

The `--device=dri` permission grants access to:

- **Vulkan API**: AMD (RADV), NVIDIA (proprietary), Intel (ANV)
- **ROCm**: AMD GPU compute via `/dev/dri/renderD*`
- **CUDA**: NVIDIA proprietary drivers (if installed)
- **OpenGL/OpenCL**: Legacy GPU APIs

Without GPU access, kindly-av1 **cannot function** (100× slower on CPU).

## Distribution Options

### 1. Local Testing
```bash
# Build and install locally
./build-flatpak.sh
flatpak install --user kindly-av1.flatpak
```

### 2. Flathub (Official App Store)
**See `FLATHUB_SETUP.md`** for complete submission guide.

**Quick steps**:
1. Fork https://github.com/flathub/flathub
2. Add `software.kindly.av1.yml` to your fork
3. Submit pull request
4. Wait for review (1-2 weeks for new apps)

**Proprietary software allowed** on Flathub (requires extra review).

### 3. Self-Hosted Repository
```bash
# Build with custom repo path
./build-flatpak.sh --repo-path /var/www/html/flatpak-repo

# Users add your repo
flatpak remote-add kindly-av1 https://kindly.software/flatpak-repo
flatpak install kindly-av1 software.kindly.av1
```

## Testing

```bash
# Build locally
./build-flatpak.sh

# Install from bundle
flatpak install --user kindly-av1.flatpak

# Test basic functionality
flatpak run software.kindly.av1 --version
flatpak run software.kindly.av1 --help

# Test GPU access
flatpak run software.kindly.av1 sample.y4m -o test.ivf --preset 6

# Check GPU device access
flatpak run --command=sh software.kindly.av1
$ ls -la /dev/dri/  # Should show renderD128, card0, etc.
```

## Updating

When releasing a new version:

1. **Update version** in `software.kindly.av1.metainfo.xml`:
   ```xml
   <releases>
     <release version="1.1.0" date="2025-02-01">
       <description>
         <p>New features...</p>
       </description>
     </release>
   </releases>
   ```

2. **Rebuild**:
   ```bash
   ./build-flatpak.sh
   ```

3. **Submit to Flathub** (if using Flathub):
   - Update manifest URL + SHA256 hash
   - Submit PR to flathub/flathub repo
   - Wait for approval (2-3 days for updates)

## Troubleshooting

### Build fails with "Binary not found"
**Fix**: Ensure you've built the release binary first:
```bash
cd /home/samuel/Primitives/kindly-av1
cargo build --release --target x86_64-unknown-linux-gnu
```

### "Permission denied" accessing GPU
**Fix**: Ensure `--device=dri` is in manifest. If still fails:
```bash
flatpak override --user --device=dri software.kindly.av1
```

### Flatpak runtime not found
**Fix**: Install Freedesktop runtime:
```bash
flatpak install flathub org.freedesktop.Platform//23.08
flatpak install flathub org.freedesktop.Sdk//23.08
```

### Manifest validation errors
**Fix**: Run linter:
```bash
pip3 install flatpak-builder-lint
flatpak-builder-lint manifest software.kindly.av1.yml
```

## License

kindly-av1 is **proprietary software** (binary-only distribution). See `LICENSE` in project root.

Flatpak packaging files (this directory) are **MIT licensed** for community contributions.

## Support

- **Homepage**: https://kindly.software
- **GitHub**: https://github.com/kindly-ai/kindly-av1
- **Flathub Setup**: See `FLATHUB_SETUP.md`
- **Email**: support@kindly.software

## Resources

- **Flatpak Documentation**: https://docs.flatpak.org/
- **Flathub Submission Guide**: https://github.com/flathub/flathub/wiki
- **AppStream Specification**: https://www.freedesktop.org/software/appstream/docs/
- **kindly-av1 Docs**: https://docs.kindly.software/av1

---

**Next Steps**: See `FLATHUB_SETUP.md` for Flathub publishing process.
