# kindly-av1 Smart Installer

One-command installation for the kindly-av1 GPU-accelerated AV1 video encoder.

## Design Goal

> "A 12-year-old YouTuber can install and use kindly-av1 without help"

## Features

- **Zero Configuration**: Detects platform automatically (OS + CPU architecture)
- **One Command**: `curl -sSL https://get.kindly.dev/av1 | bash`
- **Progress Tracking**: Clear 4-step progress with friendly emoji indicators
- **Automatic PATH**: Configures shell PATH automatically (Unix) or provides clear instructions (Windows)
- **Error Handling**: User-friendly error messages without stack traces
- **Checksum Verification**: SHA-256 verification prevents corrupted downloads
- **Retry Logic**: Automatic retry with exponential backoff
- **License Support**: Optional license key via command line

## Architecture

**Tier**: T0 Auditable (UCE34 Q1-Q7)
**Framework**: Chaos Computational Capsule Architecture
**Binary Size**: <3MB (optimized with LTO + strip)

### Capsule Structure

```
├── PlatformCapsule (T0)      # Platform detection (OS + arch)
├── DownloadCapsule (T5)      # Streaming download with checksum
├── InstallCapsule (T9)       # Archive extraction + permissions
└── PathSetupCapsule (T0)     # PATH configuration (shell-aware)
```

## Supported Platforms

| OS | Architecture | Archive Format |
|----|--------------|----------------|
| Linux | x86_64 | tar.gz |
| Linux | aarch64 | tar.gz |
| macOS | x86_64 (Intel) | tar.gz |
| macOS | aarch64 (Apple Silicon) | tar.gz |
| Windows | x86_64 | zip |

## Installation Locations

- **Linux/macOS**: `~/.local/bin/kindly-av1`
- **Windows**: `%LOCALAPPDATA%\kindly-av1\kindly-av1.exe`

## Usage

### Basic Installation (No License)

```bash
# Download and run installer
curl -sSL https://get.kindly.dev/av1 | bash
```

### Installation with License Key

```bash
# Pass license key as argument
curl -sSL https://get.kindly.dev/av1 | bash -s YOUR-LICENSE-KEY
```

### Direct Binary Usage

```bash
# Download installer binary
curl -sSL -o kindly-av1-installer https://github.com/kindly-team/kindly-av1/releases/latest/download/kindly-av1-installer-$(uname -s)-$(uname -m)
chmod +x kindly-av1-installer

# Run installer
./kindly-av1-installer

# With license key
./kindly-av1-installer YOUR-LICENSE-KEY
```

## Installation Steps

The installer performs 4 steps with clear progress:

```
╔════════════════════════════════════════╗
║   kindly-av1 Smart Installer v1.0.0   ║
║   GPU-Accelerated AV1 Encoder          ║
╚════════════════════════════════════════╝

🔍 Step 1/4: Detecting platform...
   Platform: Linux (x86_64)
   Asset: kindly-av1-x86_64-unknown-linux-musl.tar.gz

📥 Step 2/4: Downloading kindly-av1...
   Downloading... 100% (15.2MB / 15.2MB)
✓ Download complete (15932416 bytes)

📦 Step 3/4: Installing kindly-av1...
✓ Installed to /home/user/.local/bin/kindly-av1

🔧 Step 4/4: Configuring PATH...
✓ Added /home/user/.local/bin to PATH

╔════════════════════════════════════════╗
║    ✓ Installation Complete!            ║
╚════════════════════════════════════════╝

Binary installed at:
  /home/user/.local/bin/kindly-av1

Next steps:
  1. Restart your terminal (or run: source ~/.bashrc)
  2. Activate your license: kindly-av1 license activate <KEY>
  3. Start encoding: kindly-av1 encode input.mp4 -o output.av1

Get help:
  Documentation: https://docs.kindly.dev/kindly-av1
  Support: support@kindly.dev

Thank you for choosing kindly-av1! 💜
```

## PATH Configuration

### Unix (Linux/macOS)

Automatically detects shell and updates appropriate configuration file:

- **bash**: Appends to `~/.bashrc` (and `~/.bash_profile` on macOS)
- **zsh**: Appends to `~/.zshrc`
- **fish**: Appends to `~/.config/fish/config.fish`

### Windows

Uses `setx` to modify User PATH (no admin required). Changes take effect after terminal restart.

## Error Handling

User-friendly error messages without technical jargon:

- **Network errors**: "Could not connect. Check your internet connection."
- **Disk full**: "Not enough disk space."
- **Permission denied**: "Cannot write to install directory."
- **Checksum mismatch**: "Download corrupted. Please try again."

## Building the Installer

### Development Build

```bash
cd installer
cargo build
```

### Release Build (Optimized)

```bash
cargo build --release
```

Binary location: `target/release/kindly-av1-installer`

### Build Configuration

- **Optimization**: `opt-level = "z"` (optimize for size)
- **LTO**: Enabled for maximum binary size reduction
- **Strip**: Debug symbols removed
- **Panic**: `abort` (no unwinding overhead)

## Framework Compliance

### UCE34 Compliance

- **Q1-Q7**: Simple correctness workflow (debugging/testing not needed for installer)
- **Q10**: T0 Auditable tier (all operations reversible and logged)
- **Q11**: 100% Rust implementation
- **Q33**: No `#[derive(ComputationalCapsule)]` (T0 doesn't require verification)

### Chaos Compliance

- **No Mutex/RwLock**: Pure sequential operations (no concurrency)
- **#[repr(C)]**: All capsule structs use C representation
- **Documented**: Every struct has purpose documentation

### ASSUM Compliance

- **Zero Unsafe**: 100% safe Rust code
- **Error Handling**: All errors handled with user-friendly messages
- **No Panics**: All fallible operations return `Result`

## Dependencies

### Production Dependencies

- **ureq**: HTTP client with TLS support (minimal, no async)
- **dirs**: Cross-platform directory paths
- **sha2**: SHA-256 checksum verification
- **tar**: TAR archive extraction (Unix)
- **flate2**: GZIP decompression (Unix)
- **zip**: ZIP extraction (Windows only)
- **thiserror**: Error type definitions

### Development Dependencies

None (minimal dependency footprint)

## Testing

```bash
# Run tests
cargo test

# Run tests with output
cargo test -- --nocapture
```

## File Structure

```
installer/
├── Cargo.toml              # Package manifest
├── README.md               # This file
└── src/
    ├── main.rs             # Entry point + orchestration
    ├── platform.rs         # Platform detection capsule
    ├── download.rs         # Download logic capsule
    ├── install.rs          # Installation capsule
    └── path_setup.rs       # PATH configuration capsule
```

## License

Proprietary - Copyright 2025 Kindly. All Rights Reserved.

## Support

- **Email**: support@kindly.dev
- **Documentation**: https://docs.kindly.dev/kindly-av1
- **Issues**: https://github.com/kindly-team/kindly-av1/issues
