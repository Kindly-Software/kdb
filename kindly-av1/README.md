# kindly-av1

**World's Fastest Lockfree GPU-Accelerated AV1 Encoder**

[![License](https://img.shields.io/badge/license-Proprietary-purple.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-nightly-gold.svg)](https://www.rust-lang.org/)

## Features

- **GPU-Accelerated Motion Estimation** - ROCm (AMD) and Vulkan compute acceleration for 100-500x speedup
- **100% Lockfree Architecture** - Zero mutex/RwLock, sub-microsecond coordination latency
- **Crash-Safe Checkpoint/Resume** - Atomic state persistence, resume encoding after any interruption
- **Real-Time TUI Dashboard** - Live encoding metrics with keyboard controls
- **AV1 Compliant Output** - AV1 spec compliant bitstream, validated with dav1d

## Installation

### From Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/kindly-dev/kindly-av1.git
cd kindly-av1

# Build release binary
cargo build --release

# Install to system
cargo install --path .
```

### Pre-built Binaries

Download from [Gumroad](https://gumroad.com/kindly) (paid license required).

## Quick Start

### Basic Encoding

```bash
# Encode video to AV1
kindly-av1 encode input.mp4 -o output.av1

# With quality setting (CRF 0-63, lower = better)
kindly-av1 encode input.mp4 -o output.av1 --crf 28

# With GPU acceleration
kindly-av1 encode input.mp4 -o output.av1 --gpu rocm
```

### Preset Selection

```bash
# Fast encoding (streaming, real-time)
kindly-av1 encode input.mp4 -o output.av1 --preset fast

# Balanced (default)
kindly-av1 encode input.mp4 -o output.av1 --preset medium

# High quality (archival)
kindly-av1 encode input.mp4 -o output.av1 --preset slow
```

### Checkpoint/Resume

```bash
# Start encoding with checkpoint
kindly-av1 encode input.mp4 -o output.av1 --checkpoint encode.ckpt

# Resume after interruption
kindly-av1 encode input.mp4 -o output.av1 --checkpoint encode.ckpt --resume
```

## CLI Commands

### encode

Encode video file to AV1 format.

```bash
kindly-av1 encode <INPUT> -o <OUTPUT> [OPTIONS]
```

**Options:**

| Option | Description | Default |
|--------|-------------|---------|
| `-o, --output <FILE>` | Output file path | Required |
| `--preset <PRESET>` | Encoding preset (ultrafast/fast/medium/slow/veryslow) | medium |
| `--crf <0-63>` | Constant Rate Factor (quality) | 28 |
| `--bitrate <KBPS>` | Target bitrate in kbps | - |
| `--gpu <auto\|rocm\|vulkan\|cpu>` | GPU backend | auto |
| `--threads <N\|auto>` | Thread count | auto |
| `--checkpoint <FILE>` | Checkpoint file for resume | - |
| `--resume` | Resume from checkpoint | false |
| `--keyint <N>` | Keyframe interval | 250 |
| `--tile-columns <N>` | Tile columns for parallelism | auto |
| `--tile-rows <N>` | Tile rows for parallelism | auto |

### info

Display video file information.

```bash
kindly-av1 info <INPUT>
```

### benchmark

Run encoding benchmark.

```bash
kindly-av1 benchmark --duration <SECONDS> --preset <PRESET>
```

### license

Manage license activation.

```bash
kindly-av1 license activate <LICENSE_KEY>
kindly-av1 license status
kindly-av1 license deactivate
```

## TUI Dashboard

During encoding, a real-time dashboard displays:

```
💜 kindly-av1 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✨ Encoding: input.mp4 → output.av1 [1080p@60fps]
█████████████████████░░░░░░░░░░░░░░░░░░░░░ 52.3% [1,247/2,384 frames]
⚡ 127.3 fps │ ETA 8.9s │ PSNR 42.1 │ SSIM 0.987 │ 2.4 Mbps │ GPU 94%
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
[Space] Pause │ [Q] Cancel │ [+/-] Quality │ [G] GPU toggle
```

**Keyboard Controls:**

| Key | Action |
|-----|--------|
| `Space` | Pause/Resume encoding |
| `Q` | Cancel encoding |
| `+` / `-` | Adjust quality (CRF) |
| `G` | Toggle GPU acceleration |
| `S` | Save checkpoint (when paused) |

## Performance

### Target Performance (1080p)

| Preset | CPU (fps) | GPU (fps) | Quality |
|--------|-----------|-----------|---------|
| fast | 15-20 | 60+ | Good |
| medium | 8-12 | 30-40 | Better |
| slow | 2-4 | 10-15 | Best |

### GPU Requirements

**ROCm (AMD):**
- AMD Radeon RX 5000 series or newer
- ROCm 5.0+ installed
- Linux only

**Vulkan:**
- Any Vulkan 1.2+ compatible GPU
- Cross-platform (Linux, Windows, macOS)

## System Requirements

### Minimum

- CPU: x86_64 with AVX2 support
- RAM: 4 GB
- Disk: 100 MB for binary
- OS: Linux (Ubuntu 20.04+, Fedora 36+)

### Recommended

- CPU: AMD Ryzen 7 / Intel Core i7 (8+ cores)
- RAM: 16 GB
- GPU: AMD Radeon RX 6700+ with ROCm
- OS: Ubuntu 22.04 LTS

## License

kindly-av1 is proprietary software. A license is required for use.

### License Tiers

| Tier | Price | Features |
|------|-------|----------|
| **Creator** | $49 | 1080p max, 2 machines, email support |
| **Professional** | $149 | 4K max, 3 machines, priority support |
| **Enterprise** | $499 | 8K max, 10 machines, dedicated support |

Purchase at [gumroad.com/kindly](https://gumroad.com/kindly)

## Support

- **Email:** support@kindly.dev
- **Documentation:** [docs.kindly.dev/kindly-av1](https://docs.kindly.dev/kindly-av1)
- **Issues:** [GitHub Issues](https://github.com/kindly-dev/kindly-av1/issues)

## Acknowledgments

Built with the [Computational Capsule Architecture (Chaos)](https://github.com/kindly-dev/atomic_capsule) - a 100% lockfree framework for high-performance systems.

---

**Copyright 2025 Kindly. All Rights Reserved.**
