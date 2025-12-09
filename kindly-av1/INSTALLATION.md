# Kindly-AV1 Installation Guide

Welcome to Kindly-AV1! This guide will help you install the world's fastest AV1 encoder on your system.

## System Requirements

### Minimum Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| **OS** | Linux (x86_64), macOS 12+, Windows 10+ | Linux (Ubuntu 22.04+) |
| **CPU** | 4 cores, AVX2 support | 8+ cores, AVX-512 |
| **RAM** | 8 GB | 16+ GB |
| **Storage** | 500 MB free | 2+ GB (for temp files) |
| **GPU** | Optional | AMD RX 6000+ or NVIDIA RTX 3060+ |

### Checking Your System

Before installing, verify your system meets the requirements:

```bash
# Check CPU cores and features
lscpu | grep -E 'CPU\(s\)|avx2|avx512'

# Check available RAM
free -h

# Check available disk space
df -h .

# Check GPU (Linux with AMD)
rocminfo 2>/dev/null | head -20

# Check GPU (Linux with NVIDIA)
nvidia-smi 2>/dev/null | head -10
```

## Quick Install (Linux/macOS)

### One-Line Install

```bash
curl -fsSL https://kindly.video/install.sh | bash
```

This script:
1. Detects your platform (Linux/macOS, x86_64/aarch64)
2. Downloads the appropriate binary
3. Installs to `~/.local/bin/kindly-av1`
4. Adds to PATH if needed

### Verify Installation

```bash
kindly-av1 --version
# Output: kindly-av1 1.0.0 (Byzantine Purple Build)

kindly-av1 help
# Shows available commands
```

## Manual Installation

### Linux (x86_64)

```bash
# Download the latest release
wget https://kindly.video/releases/latest/kindly-av1-linux-x86_64.tar.gz

# Extract
tar -xzf kindly-av1-linux-x86_64.tar.gz

# Install (requires sudo for /usr/local/bin)
sudo mv kindly-av1 /usr/local/bin/

# Or install to user directory (no sudo)
mkdir -p ~/.local/bin
mv kindly-av1 ~/.local/bin/
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc

# Verify
kindly-av1 --version
```

### macOS (Intel/Apple Silicon)

```bash
# Intel Mac
curl -LO https://kindly.video/releases/latest/kindly-av1-darwin-x86_64.tar.gz
tar -xzf kindly-av1-darwin-x86_64.tar.gz

# Apple Silicon (M1/M2/M3)
curl -LO https://kindly.video/releases/latest/kindly-av1-darwin-aarch64.tar.gz
tar -xzf kindly-av1-darwin-aarch64.tar.gz

# Install
sudo mv kindly-av1 /usr/local/bin/

# Verify
kindly-av1 --version
```

**Note for macOS:** You may need to allow the app in System Preferences > Security & Privacy after first run.

### Windows

1. Download `kindly-av1-windows-x86_64.zip` from [kindly.video/download](https://kindly.video/download)

2. Extract to a folder (e.g., `C:\Program Files\kindly-av1\`)

3. Add to PATH:
   - Open Start Menu, search "Environment Variables"
   - Click "Edit the system environment variables"
   - Click "Environment Variables..."
   - Under "User variables", select "Path", click "Edit"
   - Click "New", add `C:\Program Files\kindly-av1`
   - Click OK on all dialogs

4. Open a new Command Prompt or PowerShell and verify:
   ```cmd
   kindly-av1 --version
   ```

## GPU Setup (Optional but Recommended)

### AMD ROCm (Linux)

Kindly-AV1 supports AMD GPUs via ROCm for 10-100x faster encoding.

```bash
# Ubuntu 22.04+
sudo apt update
sudo apt install rocm-hip-libraries rocm-dev

# Verify ROCm
rocminfo | grep "Name:"

# Test with kindly-av1
kindly-av1 encode video.mp4 --gpu rocm
```

**Supported GPUs:** RX 6000 series, RX 7000 series, Radeon Pro, MI100/MI200

### NVIDIA CUDA (Linux/Windows)

```bash
# Ubuntu
sudo apt install nvidia-cuda-toolkit

# Verify CUDA
nvcc --version

# Test with kindly-av1
kindly-av1 encode video.mp4 --gpu cuda
```

**Supported GPUs:** RTX 3060+, RTX 4060+, Quadro/Tesla with Compute 7.0+

### Vulkan (Cross-platform Fallback)

If ROCm or CUDA isn't available, Kindly-AV1 can use Vulkan:

```bash
# Linux
sudo apt install vulkan-tools libvulkan-dev

# Verify
vulkaninfo | head -20

# Test
kindly-av1 encode video.mp4 --gpu vulkan
```

## License Activation

Kindly-AV1 requires a license key for full features.

### Activate License

```bash
# Activate with your Gumroad license key
kindly-av1 license activate YOUR_LICENSE_KEY_HERE

# Check license status
kindly-av1 license status
```

### License Tiers

| Tier | Features | Machines |
|------|----------|----------|
| **Creator** ($49) | 1080p max, email support | 2 |
| **Professional** ($149) | 4K max, priority support | 3 |
| **Enterprise** ($499) | 8K max, dedicated support | 10 |

### Deactivate (to move to new machine)

```bash
kindly-av1 license deactivate
```

## Troubleshooting

### "Command not found"

Make sure the binary is in your PATH:

```bash
# Check if kindly-av1 is in PATH
which kindly-av1

# If not found, add to PATH
export PATH="$HOME/.local/bin:$PATH"
# Add the above line to ~/.bashrc or ~/.zshrc to make it permanent
```

### "Permission denied"

```bash
# Make the binary executable
chmod +x kindly-av1

# Or use sudo for system-wide install
sudo mv kindly-av1 /usr/local/bin/
```

### "GPU not detected"

```bash
# Check GPU availability
kindly-av1 info --gpu

# If no GPU found:
# 1. Verify driver installation (rocminfo, nvidia-smi, vulkaninfo)
# 2. Update drivers to latest version
# 3. Try --gpu auto to let kindly-av1 detect
```

### "License invalid"

```bash
# Check your license key is correct
kindly-av1 license status

# If machine limit reached, deactivate on another machine first:
# (on old machine)
kindly-av1 license deactivate

# (on new machine)
kindly-av1 license activate YOUR_KEY
```

### Performance Issues

```bash
# Check system resources during encoding
htop  # or top

# Try reducing thread count if system is overloaded
kindly-av1 encode video.mp4 --threads 4

# Or increase threads for faster encoding
kindly-av1 encode video.mp4 --threads auto  # Uses all cores
```

## Updating

### Linux/macOS

```bash
# Re-run the install script
curl -fsSL https://kindly.video/install.sh | bash

# Or manually download new version and replace
```

### Windows

1. Download the new version from [kindly.video/download](https://kindly.video/download)
2. Replace the old `kindly-av1.exe` with the new one

## Uninstalling

### Linux/macOS

```bash
# Remove binary
rm ~/.local/bin/kindly-av1
# or
sudo rm /usr/local/bin/kindly-av1

# Remove configuration (optional)
rm -rf ~/.kindly-av1
```

### Windows

1. Delete `C:\Program Files\kindly-av1\`
2. Remove from PATH (Environment Variables)
3. Delete `%APPDATA%\kindly-av1\` (optional)

## Getting Help

- **Documentation:** [docs.kindly.dev/kindly-av1](https://docs.kindly.dev/kindly-av1)
- **Email:** support@kindly.dev
- **Getting Started:** See [GETTING_STARTED.md](GETTING_STARTED.md)
- **License Issues:** [gumroad.com/library](https://gumroad.com/library)

---

**Next Steps:** Check out [GETTING_STARTED.md](GETTING_STARTED.md) for your first encode!
