#!/bin/bash
# build-snap.sh - Build kindly-av1 snap package
set -euo pipefail

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

echo -e "${PURPLE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${PURPLE}  kindly-av1 Snap Build Script${NC}"
echo -e "${PURPLE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# Check if snapcraft is installed
if ! command -v snapcraft &> /dev/null; then
    echo -e "${RED}✗ Error: snapcraft is not installed${NC}"
    echo ""
    echo "Install snapcraft with:"
    echo "  sudo snap install snapcraft --classic"
    echo ""
    exit 1
fi

# Check if in correct directory
if [ ! -f "$PROJECT_ROOT/Cargo.toml" ]; then
    echo -e "${RED}✗ Error: Cannot find Cargo.toml in project root${NC}"
    echo "Expected: $PROJECT_ROOT/Cargo.toml"
    exit 1
fi

# Clean previous builds
echo -e "${BLUE}→ Cleaning previous builds...${NC}"
cd "$PROJECT_ROOT"
cargo clean

# Build release binary
echo -e "${BLUE}→ Building release binary (x86_64-unknown-linux-gnu)...${NC}"
cargo build --release --target x86_64-unknown-linux-gnu --bin kindly-av1

# Verify binary exists
BINARY_PATH="$PROJECT_ROOT/target/x86_64-unknown-linux-gnu/release/kindly-av1"
if [ ! -f "$BINARY_PATH" ]; then
    echo -e "${RED}✗ Error: Binary not found at $BINARY_PATH${NC}"
    exit 1
fi

# Check binary size
BINARY_SIZE=$(stat -c%s "$BINARY_PATH" 2>/dev/null || stat -f%z "$BINARY_PATH")
BINARY_SIZE_MB=$((BINARY_SIZE / 1024 / 1024))
echo -e "${GREEN}✓ Binary built successfully (${BINARY_SIZE_MB}MB)${NC}"

# Strip debug symbols (reduce size)
echo -e "${BLUE}→ Stripping debug symbols...${NC}"
strip "$BINARY_PATH"
STRIPPED_SIZE=$(stat -c%s "$BINARY_PATH" 2>/dev/null || stat -f%z "$BINARY_PATH")
STRIPPED_SIZE_MB=$((STRIPPED_SIZE / 1024 / 1024))
echo -e "${GREEN}✓ Binary stripped (${STRIPPED_SIZE_MB}MB)${NC}"

# Change to snap packaging directory
cd "$SCRIPT_DIR"

# Clean snapcraft build artifacts
echo -e "${BLUE}→ Cleaning snapcraft artifacts...${NC}"
snapcraft clean

# Build snap package
echo -e "${BLUE}→ Building snap package...${NC}"
snapcraft

# Find generated snap
SNAP_FILE=$(find . -maxdepth 1 -name "kindly-av1_*.snap" -type f | head -1)

if [ -z "$SNAP_FILE" ]; then
    echo -e "${RED}✗ Error: Snap package not found${NC}"
    exit 1
fi

# Get snap file size
SNAP_SIZE=$(stat -c%s "$SNAP_FILE" 2>/dev/null || stat -f%z "$SNAP_FILE")
SNAP_SIZE_MB=$((SNAP_SIZE / 1024 / 1024))

echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}  Build Complete!${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "${PURPLE}Snap Package:${NC} $SNAP_FILE"
echo -e "${PURPLE}Package Size:${NC} ${SNAP_SIZE_MB}MB"
echo ""
echo -e "${YELLOW}Next Steps:${NC}"
echo "  1. Install locally:"
echo "     sudo snap install $SNAP_FILE --dangerous"
echo ""
echo "  2. Test installation:"
echo "     kindly-av1 --help"
echo ""
echo "  3. Upload to Snap Store:"
echo "     snapcraft upload $SNAP_FILE --release=stable"
echo ""
echo -e "${BLUE}Documentation:${NC}"
echo "  See SNAP_STORE_SETUP.md for publishing guide"
echo ""
