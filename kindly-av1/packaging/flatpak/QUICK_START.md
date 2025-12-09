# kindly-av1 Flatpak - Quick Start

**One-command build and test**:

```bash
cd /home/samuel/Primitives/kindly-av1/packaging/flatpak
./build-flatpak.sh && flatpak install --user kindly-av1.flatpak
```

## Build Output

The build script will:
1. Compile kindly-av1 release binary (2-5 min)
2. Install Flatpak runtime (if needed, ~200 MB download)
3. Build Flatpak package (~1 min)
4. Create `kindly-av1.flatpak` bundle (~5-15 MB)

## Test Run

```bash
# Show help
flatpak run software.kindly.av1 --help

# Encode sample video
flatpak run software.kindly.av1 input.y4m -o output.ivf --preset 6 --crf 28

# Check GPU access
flatpak run --command=sh software.kindly.av1
$ ls -la /dev/dri/
```

## Flathub Submission

**See FLATHUB_SETUP.md** for complete 11-section guide (538 lines).

**Quick steps**:
1. Create GitHub release with binary tarball
2. Fork https://github.com/flathub/flathub
3. Add manifest (update URL to release)
4. Submit pull request
5. Wait 1-2 weeks for approval

## Files Overview

| File | Lines | Purpose |
|------|-------|---------|
| `software.kindly.av1.yml` | 67 | Flatpak manifest (dependencies, build, install) |
| `software.kindly.av1.metainfo.xml` | 92 | AppStream metadata (name, description, icon) |
| `icons/software.kindly.av1.svg` | 44 | Vector icon (scalable) |
| `icons/software.kindly.av1-256.png` | 2.3 KB | PNG fallback |
| `build-flatpak.sh` | 129 | Automated build script |
| `FLATHUB_SETUP.md` | 538 | Comprehensive publishing guide |
| `README.md` | 194 | User documentation |

## GPU Access

kindly-av1 requires `--device=dri` permission for GPU encoding:
- **Vulkan**: AMD (RADV), NVIDIA, Intel (ANV)
- **ROCm**: AMD GPU compute
- **CUDA**: NVIDIA proprietary drivers

**Without GPU**: 100× slower (CPU-only encoding)

## Proprietary Software

kindly-av1 is **binary-only** (trade secret protection). Flathub allows proprietary apps:
- Spotify, Discord, Slack, NVIDIA GeForceNOW (precedents)
- Requires license disclosure: `LicenseRef-proprietary` ✅
- Review time: 1-2 weeks (new apps), 2-3 days (updates)

## Support

- **Quick Reference**: `README.md`
- **Flathub Guide**: `FLATHUB_SETUP.md`
- **Troubleshooting**: Both docs include troubleshooting sections
- **Website**: https://kindly.software

---

**Ready to build?** Run `./build-flatpak.sh`
