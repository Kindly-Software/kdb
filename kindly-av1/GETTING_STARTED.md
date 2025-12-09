# Getting Started with Kindly-AV1

This guide will have you encoding your first video in under 5 minutes!

## Your First Encode

### The Simplest Way

Just provide your video file:

```bash
kindly-av1 video.mp4
```

That's it! Kindly-AV1 will:
- Auto-detect it's a video file
- Choose optimal settings based on file size
- Create `video.av1` in the same directory
- Show real-time progress with ETA

### With Output Path

```bash
kindly-av1 video.mp4 -o compressed.av1
```

### Using the Wizard (Recommended for Beginners)

If you prefer a guided experience:

```bash
kindly-av1 wizard
```

The wizard asks simple questions:
1. **Select your video file** (with arrow keys)
2. **What's your goal?** (Smallest file / Best quality / Balanced)
3. **How fast?** (Quick / Normal / Patient)
4. **Confirm and encode!**

## Quality Presets

Choose based on your needs:

| Preset | Speed | Quality | Best For |
|--------|-------|---------|----------|
| `fast` | Fastest | Good | Preview, drafts |
| `balanced` | Medium | Great | Daily use, sharing |
| `quality` | Slower | Excellent | Archive, professional |
| `veryslow` | Slowest | Maximum | Final delivery |

```bash
# Fast encode (for previewing)
kindly-av1 video.mp4 --preset fast

# High quality (for final export)
kindly-av1 video.mp4 --preset quality
```

**Auto-selection:** Without `--preset`, Kindly-AV1 chooses based on file size:
- Under 100 MB: `fast`
- 100 MB - 1 GB: `balanced`
- Over 1 GB: `quality`

## Quality Control (CRF)

CRF (Constant Rate Factor) controls quality vs. file size:

| CRF | Quality | Use Case |
|-----|---------|----------|
| 18-22 | Visually lossless | Archive, professional |
| 23-28 | High quality | Streaming, sharing |
| 29-35 | Good quality | Social media |
| 36-45 | Lower quality | Drafts, previews |

```bash
# High quality (larger file)
kindly-av1 video.mp4 --crf 22

# Smaller file (some quality loss)
kindly-av1 video.mp4 --crf 32
```

**Default:** CRF 28 (great balance of quality and size)

## GPU Acceleration

If you have a supported GPU, encoding is 10-100x faster:

```bash
# Auto-detect GPU
kindly-av1 video.mp4 --gpu auto

# Force AMD ROCm
kindly-av1 video.mp4 --gpu rocm

# Force NVIDIA CUDA
kindly-av1 video.mp4 --gpu cuda

# Force CPU (no GPU)
kindly-av1 video.mp4 --gpu cpu
```

Check GPU status:
```bash
kindly-av1 info --gpu
```

## Common Tasks

### Compress for Sharing

Goal: Small file size, fast upload, good quality

```bash
kindly-av1 video.mp4 --preset balanced --crf 30
```

### Archive Original Quality

Goal: Maximum quality preservation

```bash
kindly-av1 video.mp4 --preset quality --crf 20
```

### Quick Preview

Goal: Fast encode to check content

```bash
kindly-av1 video.mp4 --preset fast --crf 35
```

### Batch Encode Multiple Files

```bash
# Linux/macOS
for f in *.mp4; do
    kindly-av1 "$f" -o "${f%.mp4}.av1"
done

# Windows PowerShell
Get-ChildItem *.mp4 | ForEach-Object {
    kindly-av1 $_.Name -o ($_.BaseName + ".av1")
}
```

## Understanding the Progress Display

During encoding, you'll see:

```
[kindly-av1] Encoding video.mp4

[#################                       ] 42.3% | 24.7 fps | ETA: 2m 15s | 8.2:1
```

- **Progress bar:** Visual percentage complete
- **Percentage:** Exact progress
- **FPS:** Encoding speed (frames per second)
- **ETA:** Estimated time remaining (smoothed for stability)
- **Compression ratio:** How much smaller (8.2:1 = 8x smaller)

### Keyboard Controls

While encoding, you can:
- `Space` - Pause/Resume
- `i` - Show detailed info
- `q` - Quit (with checkpoint for resume)
- `+`/`-` - Adjust thread count

## Tips for Best Results

### 1. Choose the Right Preset

- **Streaming to YouTube/Twitch?** Use `balanced` or `fast`
- **Archiving precious memories?** Use `quality`
- **Social media clips?** Use `fast` with higher CRF (30-35)

### 2. Let the GPU Do the Work

GPU encoding is massively faster. If you have a supported GPU:

```bash
kindly-av1 video.mp4 --gpu auto
```

### 3. Use Resume for Long Encodes

For long videos, Kindly-AV1 creates checkpoints:

```bash
# If encoding is interrupted, just run the same command
kindly-av1 video.mp4 --resume
```

### 4. Check Input Video Info

Before encoding, see what you're working with:

```bash
kindly-av1 info video.mp4
```

Output:
```
File: video.mp4
Duration: 10:32
Resolution: 1920x1080
Frame Rate: 24.00 fps
Frames: 15,168
Size: 2.4 GB
```

## Troubleshooting

### "Video file not found"

```bash
# Check the file exists
ls -la video.mp4

# Use full path if in different directory
kindly-av1 /path/to/video.mp4
```

### "Encoding too slow"

```bash
# Use faster preset
kindly-av1 video.mp4 --preset fast

# Enable GPU if available
kindly-av1 video.mp4 --gpu auto

# Reduce output quality (larger CRF = faster)
kindly-av1 video.mp4 --crf 35
```

### "Output file too large"

```bash
# Increase CRF (higher = smaller file)
kindly-av1 video.mp4 --crf 32

# Or use target bitrate
kindly-av1 video.mp4 --bitrate 2000  # 2 Mbps
```

### "Output file too small/blurry"

```bash
# Decrease CRF (lower = higher quality)
kindly-av1 video.mp4 --crf 22

# Use quality preset
kindly-av1 video.mp4 --preset quality
```

### "Not enough disk space"

Kindly-AV1 needs temporary space during encoding. Free up space or:

```bash
# Encode to different drive
kindly-av1 video.mp4 -o /other/drive/output.av1
```

## Example Workflows

### Content Creator: Edit Export

After editing in Premiere/DaVinci/Final Cut:

```bash
# Export from editor as ProRes/DNxHD first
# Then compress with Kindly-AV1

kindly-av1 export.mov --preset balanced --crf 26 -o final.av1
```

### Archivist: Preserve Family Videos

```bash
# Maximum quality preservation
kindly-av1 family_video.mp4 --preset quality --crf 18 -o archive/family_2024.av1
```

### Social Media: Quick Clips

```bash
# Fast encode, smaller files for upload
kindly-av1 clip.mp4 --preset fast --crf 32 -o twitter_clip.av1
```

### Batch Convert DVD Rips

```bash
# Process all VOB files
for f in VIDEO_TS/*.VOB; do
    kindly-av1 "$f" --preset balanced -o "converted/$(basename "$f" .VOB).av1"
done
```

## Command Reference

```
kindly-av1 [OPTIONS] <INPUT> [-o OUTPUT]

ARGUMENTS:
  <INPUT>     Input video file (mp4, mkv, mov, webm, avi, y4m)

OPTIONS:
  -o, --output <FILE>    Output file path (default: input.av1)
  --preset <PRESET>      Encoding preset (fast, balanced, quality, veryslow)
  --crf <0-63>           Quality (lower = better, default: 28)
  --bitrate <KBPS>       Target bitrate in kbps
  --gpu <auto|rocm|cuda|vulkan|cpu>  GPU backend
  --threads <N|auto>     Thread count (default: auto)
  --resume               Resume from checkpoint
  -v, --verbose          Show detailed output
  -h, --help             Show help

COMMANDS:
  encode     Encode video (default if input is video file)
  wizard     Interactive guided setup
  info       Show video information
  benchmark  Run performance benchmark
  license    License management (activate, status, deactivate)
  help       Show help for a command
```

## Next Steps

- **Advanced options:** See `kindly-av1 help encode`
- **GPU setup:** See [INSTALLATION.md](INSTALLATION.md#gpu-setup-optional-but-recommended)
- **Documentation:** [docs.kindly.dev/kindly-av1](https://docs.kindly.dev/kindly-av1)
- **Support:** support@kindly.dev

---

Happy encoding! If you have any questions, we're here to help.
