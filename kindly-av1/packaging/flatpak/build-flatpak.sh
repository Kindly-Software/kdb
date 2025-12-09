#!/usr/bin/env bash
# Build script for kindly-av1 Flatpak package
# Usage: ./build-flatpak.sh [--repo-path /path/to/repo]

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
APP_ID="software.kindly.av1"
MANIFEST="${APP_ID}.yml"
BUILD_DIR="build"
REPO_DIR="repo"
EXPORT_DIR="export"
BUNDLE_NAME="kindly-av1.flatpak"

# Parse arguments
CUSTOM_REPO=""
while [[ $# -gt 0 ]]; do
    case $1 in
        --repo-path)
            CUSTOM_REPO="$2"
            shift 2
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

# Use custom repo if specified
if [[ -n "$CUSTOM_REPO" ]]; then
    REPO_DIR="$CUSTOM_REPO"
fi

echo -e "${GREEN}=== kindly-av1 Flatpak Build Script ===${NC}"
echo ""

# Step 1: Build release binary first
echo -e "${YELLOW}[1/6] Building kindly-av1 release binary...${NC}"
cd ../../
if ! cargo build --release --target x86_64-unknown-linux-gnu; then
    echo -e "${RED}ERROR: Failed to build release binary${NC}"
    exit 1
fi
cd packaging/flatpak
echo -e "${GREEN}✓ Release binary built${NC}"
echo ""

# Step 2: Verify binary exists
BINARY_PATH="../../target/x86_64-unknown-linux-gnu/release/kindly-av1"
if [[ ! -f "$BINARY_PATH" ]]; then
    echo -e "${RED}ERROR: Binary not found at $BINARY_PATH${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Binary verified at $BINARY_PATH${NC}"
echo ""

# Step 3: Install Flatpak platform/SDK if needed
echo -e "${YELLOW}[2/6] Checking Flatpak runtime...${NC}"
if ! flatpak list | grep -q "org.freedesktop.Platform.*23.08"; then
    echo "Installing org.freedesktop.Platform 23.08..."
    flatpak install -y flathub org.freedesktop.Platform//23.08
fi
if ! flatpak list | grep -q "org.freedesktop.Sdk.*23.08"; then
    echo "Installing org.freedesktop.Sdk 23.08..."
    flatpak install -y flathub org.freedesktop.Sdk//23.08
fi
echo -e "${GREEN}✓ Flatpak runtime ready${NC}"
echo ""

# Step 4: Clean previous builds
echo -e "${YELLOW}[3/6] Cleaning previous builds...${NC}"
rm -rf "$BUILD_DIR" "$EXPORT_DIR" "$BUNDLE_NAME"
echo -e "${GREEN}✓ Build directory cleaned${NC}"
echo ""

# Step 5: Build Flatpak
echo -e "${YELLOW}[4/6] Building Flatpak with flatpak-builder...${NC}"
if ! flatpak-builder --force-clean "$BUILD_DIR" "$MANIFEST"; then
    echo -e "${RED}ERROR: flatpak-builder failed${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Flatpak built successfully${NC}"
echo ""

# Step 6: Create/update repository
echo -e "${YELLOW}[5/6] Exporting to repository...${NC}"
if [[ ! -d "$REPO_DIR" ]]; then
    echo "Creating new repository at $REPO_DIR..."
    mkdir -p "$REPO_DIR"
fi
if ! flatpak-builder --repo="$REPO_DIR" --force-clean "$BUILD_DIR" "$MANIFEST"; then
    echo -e "${RED}ERROR: Failed to export to repository${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Repository updated at $REPO_DIR${NC}"
echo ""

# Step 7: Create single-file bundle
echo -e "${YELLOW}[6/6] Creating single-file bundle...${NC}"
if ! flatpak build-bundle "$REPO_DIR" "$BUNDLE_NAME" "$APP_ID"; then
    echo -e "${RED}ERROR: Failed to create bundle${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Bundle created: $BUNDLE_NAME${NC}"
echo ""

# Step 8: Summary
BUNDLE_SIZE=$(du -h "$BUNDLE_NAME" | cut -f1)
echo -e "${GREEN}=== Build Complete ===${NC}"
echo ""
echo "Flatpak bundle: $BUNDLE_NAME ($BUNDLE_SIZE)"
echo "Repository:     $REPO_DIR"
echo ""
echo "Next steps:"
echo "  1. Test locally:  flatpak install --user $BUNDLE_NAME"
echo "  2. Run:           flatpak run $APP_ID --help"
echo "  3. Publish:       See FLATHUB_SETUP.md"
echo ""
echo -e "${GREEN}To add local repo:${NC}"
echo "  flatpak remote-add --user kindly-av1-local $REPO_DIR --no-gpg-verify"
echo "  flatpak install --user kindly-av1-local $APP_ID"
echo ""
