#!/bin/bash
# create-icns.sh - Create macOS app icon from PNG source
# Usage: ./create-icns.sh [input.png]

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RESOURCES="$SCRIPT_DIR/kindly-av1.app/Contents/Resources"
ICONSET="$RESOURCES/AppIcon.iconset"
OUTPUT_ICNS="$RESOURCES/AppIcon.icns"

# Input image (default to project logo if available)
INPUT_PNG="${1:-$SCRIPT_DIR/../../docs/logo.png}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# Check dependencies
if ! command -v sips > /dev/null 2>&1; then
    log_error "sips command not found (macOS only)"
    exit 1
fi

if ! command -v iconutil > /dev/null 2>&1; then
    log_error "iconutil command not found (macOS 10.7+)"
    exit 1
fi

# Check input file
if [ ! -f "$INPUT_PNG" ]; then
    log_error "Input PNG not found: $INPUT_PNG"
    log_info "Usage: $0 <input.png>"
    log_info "Example: $0 ~/Downloads/kindly-av1-icon.png"
    log_info ""
    log_info "Icon requirements:"
    log_info "  - PNG format"
    log_info "  - Square (1024x1024 recommended)"
    log_info "  - Transparent background optional"
    log_info "  - High resolution for best results"
    exit 1
fi

log_info "Creating macOS app icon from: $INPUT_PNG"

# Create iconset directory
mkdir -p "$ICONSET"

# Required icon sizes for macOS
# Format: filename size
SIZES=(
    "icon_16x16.png 16"
    "icon_16x16@2x.png 32"
    "icon_32x32.png 32"
    "icon_32x32@2x.png 64"
    "icon_128x128.png 128"
    "icon_128x128@2x.png 256"
    "icon_256x256.png 256"
    "icon_256x256@2x.png 512"
    "icon_512x512.png 512"
    "icon_512x512@2x.png 1024"
)

log_info "Generating icon sizes..."

for entry in "${SIZES[@]}"; do
    read -r filename size <<< "$entry"
    output="$ICONSET/$filename"

    log_info "  Creating $filename (${size}x${size})"

    sips -z "$size" "$size" "$INPUT_PNG" --out "$output" > /dev/null 2>&1

    if [ ! -f "$output" ]; then
        log_error "Failed to create $filename"
        exit 1
    fi
done

log_info "Converting iconset to .icns..."

# Convert iconset to .icns
iconutil -c icns "$ICONSET" -o "$OUTPUT_ICNS"

if [ ! -f "$OUTPUT_ICNS" ]; then
    log_error "Failed to create .icns file"
    exit 1
fi

# Clean up iconset directory
rm -rf "$ICONSET"

log_info "Icon created successfully: $OUTPUT_ICNS"

# Display icon info
ICON_SIZE=$(du -h "$OUTPUT_ICNS" | cut -f1)
log_info "Icon size: $ICON_SIZE"

# Verify icon
if ! file "$OUTPUT_ICNS" | grep -q "Mac OS X icon"; then
    log_warn "Icon verification failed, but file exists"
else
    log_info "Icon verified"
fi

echo ""
log_info "========================================="
log_info "Icon creation complete!"
log_info "========================================="
log_info "Output: $OUTPUT_ICNS"
log_info "Next: Run ./build-app.sh to build the app bundle"
log_info "========================================="
