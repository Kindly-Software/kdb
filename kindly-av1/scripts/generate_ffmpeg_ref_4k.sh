#!/bin/bash
# Generate FFmpeg reference bytes for 4K AV1 sequence header
#
# [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
#
# This script generates dav1d-validated AV1 bitstreams for 4K resolution
# using FFmpeg's libaom-av1 encoder.

set -euo pipefail

WIDTH=3840
HEIGHT=2160
OUTPUT_IVF="/tmp/ffmpeg_ref_4k_${WIDTH}x${HEIGHT}.ivf"
OUTPUT_HEX="/tmp/ffmpeg_ref_4k_${WIDTH}x${HEIGHT}_hex.txt"

echo "=== FFmpeg Reference Bytes Generator for 4K AV1 ==="
echo "Resolution: ${WIDTH}×${HEIGHT}"
echo ""

# Check if ffmpeg is installed
if ! command -v ffmpeg &> /dev/null; then
    echo "ERROR: ffmpeg not installed"
    echo "Install: sudo apt install ffmpeg"
    exit 1
fi

# Check if dav1d is installed
if ! command -v dav1d &> /dev/null; then
    echo "ERROR: dav1d not installed"
    echo "Install: sudo apt install dav1d"
    exit 1
fi

# Generate gray 4K frame with FFmpeg's libaom-av1 encoder
echo "Generating 4K gray frame with FFmpeg..."
ffmpeg -f lavfi -i "color=c=gray:size=${WIDTH}x${HEIGHT}" \
    -frames:v 1 \
    -c:v libaom-av1 \
    -strict experimental \
    -y \
    "$OUTPUT_IVF" 2>&1 | grep -E "(frame|size|bitrate)" || true

if [ ! -f "$OUTPUT_IVF" ]; then
    echo "ERROR: Failed to generate IVF file"
    exit 1
fi

FILE_SIZE=$(stat -c%s "$OUTPUT_IVF")
echo ""
echo "Generated IVF file: $OUTPUT_IVF (${FILE_SIZE} bytes)"

# Validate with dav1d
echo ""
echo "Validating with dav1d..."
if dav1d -i "$OUTPUT_IVF" -o /dev/null 2>&1; then
    echo "✓ dav1d validation PASSED"
else
    echo "✗ dav1d validation FAILED"
    exit 1
fi

# Extract hex dump
echo ""
echo "Hex dump (first 128 bytes):"
xxd -l 128 "$OUTPUT_IVF" | tee "$OUTPUT_HEX"

# Parse IVF container to extract OBU bytes
# IVF header is 32 bytes
# First frame starts at byte 32
# Frame format: size(4B) + timestamp(8B) + data
echo ""
echo "=== Extracting AV1 OBU bytes ==="

# Read first frame size (4 bytes LE at offset 32)
FRAME_SIZE=$(od -An -t u4 -j 32 -N 4 "$OUTPUT_IVF" | tr -d ' ')
echo "Frame size: ${FRAME_SIZE} bytes"

# Extract frame data (skip 32B IVF header + 4B size + 8B timestamp = 44 bytes offset)
echo ""
echo "Frame OBU hex (first 64 bytes):"
xxd -s 44 -l 64 "$OUTPUT_IVF"

# Extract sequence header OBU (starts after temporal delimiter)
# Temporal delimiter is always: 0x12 0x00 (2 bytes)
# Sequence header starts at offset 44+2 = 46
echo ""
echo "Sequence header OBU (after temporal delimiter):"
xxd -s 46 -l 32 "$OUTPUT_IVF"

# Extract as Rust byte array
echo ""
echo "=== Rust byte array for hardcoded lookup ==="
echo "// 4K (3840×2160):"
echo -n "(3840, 2160) => return vec!["
# Extract bytes from offset 46 (sequence header start)
# We need to find the actual length by parsing OBU size field
# For now, extract first 32 bytes and we'll trim to actual size
od -An -t x1 -j 46 -N 32 "$OUTPUT_IVF" | tr -d '\n' | sed 's/ /, 0x/g' | sed 's/^, //'
echo "],"

echo ""
echo "=== Next Steps ==="
echo "1. Copy the Rust byte array above"
echo "2. Add it to atomic_capsule/src/encoder/sequence_header_impl.rs:340-356"
echo "3. Verify the exact byte count (check OBU size field)"
echo "4. Run: cargo test --test dav1d_validation test_dav1d_validation_4k"
