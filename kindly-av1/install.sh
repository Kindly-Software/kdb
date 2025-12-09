#!/usr/bin/env bash
#
# kindly-av1 Installation Script
# Usage: curl -sSL https://get.kindly.dev/av1 | bash
#        wget -qO- https://get.kindly.dev/av1 | bash
#
# UCE34/COCA Compliant: T0 Auditable platform detection
# Architecture: Auto-detect platform → Download installer → Run installer

set -e

# Color output for better UX
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# Banner
echo -e "${PURPLE}╔════════════════════════════════════════╗${NC}"
echo -e "${PURPLE}║   kindly-av1 Installation Script      ║${NC}"
echo -e "${PURPLE}║   GPU-Accelerated AV1 Encoder          ║${NC}"
echo -e "${PURPLE}╚════════════════════════════════════════╝${NC}"
echo ""

# Configuration
GITHUB_OWNER="kindly-team"
GITHUB_REPO="kindly-av1"
RELEASE_TAG="${KINDLY_AV1_VERSION:-v1.0.0}"
DOWNLOAD_URL="https://github.com/${GITHUB_OWNER}/${GITHUB_REPO}/releases/download/${RELEASE_TAG}"

# Detect platform
detect_platform() {
    local os=""
    local arch=""

    # Detect OS
    case "$(uname -s)" in
        Linux*)     os="linux" ;;
        Darwin*)    os="darwin" ;;
        MINGW*|MSYS*|CYGWIN*) os="windows" ;;
        *)
            echo -e "${RED}✗ Unsupported operating system: $(uname -s)${NC}"
            exit 1
            ;;
    esac

    # Detect architecture
    case "$(uname -m)" in
        x86_64|amd64)   arch="x86_64" ;;
        aarch64|arm64)  arch="aarch64" ;;
        *)
            echo -e "${RED}✗ Unsupported architecture: $(uname -m)${NC}"
            exit 1
            ;;
    esac

    # Determine installer filename
    case "${os}-${arch}" in
        linux-x86_64)       echo "kindly-av1-x86_64-unknown-linux-musl.tar.gz" ;;
        linux-aarch64)      echo "kindly-av1-aarch64-unknown-linux-musl.tar.gz" ;;
        darwin-x86_64)      echo "kindly-av1-x86_64-apple-darwin.tar.gz" ;;
        darwin-aarch64)     echo "kindly-av1-aarch64-apple-darwin.tar.gz" ;;
        windows-x86_64)     echo "kindly-av1-x86_64-pc-windows-msvc.zip" ;;
        windows-aarch64)    echo "kindly-av1-aarch64-pc-windows-msvc.zip" ;;
        *)
            echo -e "${RED}✗ Unsupported platform: ${os}-${arch}${NC}"
            exit 1
            ;;
    esac
}

# Check for required tools
check_dependencies() {
    local missing_deps=()

    # Check for curl or wget
    if ! command -v curl &> /dev/null && ! command -v wget &> /dev/null; then
        missing_deps+=("curl or wget")
    fi

    # Check for tar (Unix-like systems)
    if [[ "$(uname -s)" != MINGW* ]] && [[ "$(uname -s)" != MSYS* ]]; then
        if ! command -v tar &> /dev/null; then
            missing_deps+=("tar")
        fi
    fi

    if [ ${#missing_deps[@]} -ne 0 ]; then
        echo -e "${RED}✗ Missing required dependencies: ${missing_deps[*]}${NC}"
        echo -e "${YELLOW}  Please install them and try again.${NC}"
        exit 1
    fi
}

# Download file with progress
download_file() {
    local url="$1"
    local output="$2"

    if command -v curl &> /dev/null; then
        curl -L --progress-bar -o "$output" "$url"
    elif command -v wget &> /dev/null; then
        wget --show-progress -O "$output" "$url"
    else
        echo -e "${RED}✗ No download tool available (curl or wget required)${NC}"
        exit 1
    fi
}

# Main installation logic
main() {
    echo -e "${GREEN}🔍 Step 1/3: Detecting platform...${NC}"

    ASSET_NAME=$(detect_platform)
    PLATFORM_NAME="$(uname -s) ($(uname -m))"

    echo -e "   Platform: ${PURPLE}${PLATFORM_NAME}${NC}"
    echo -e "   Asset: ${PURPLE}${ASSET_NAME}${NC}"
    echo ""

    check_dependencies

    echo -e "${GREEN}📥 Step 2/3: Downloading installer...${NC}"

    TEMP_DIR=$(mktemp -d)
    ARCHIVE_PATH="${TEMP_DIR}/${ASSET_NAME}"
    DOWNLOAD_FILE="${DOWNLOAD_URL}/${ASSET_NAME}"

    echo -e "   URL: ${DOWNLOAD_FILE}"

    if ! download_file "$DOWNLOAD_FILE" "$ARCHIVE_PATH"; then
        echo -e "${RED}✗ Download failed${NC}"
        echo -e "${YELLOW}  Please check your internet connection and try again.${NC}"
        rm -rf "$TEMP_DIR"
        exit 1
    fi

    echo ""
    echo -e "${GREEN}📦 Step 3/3: Extracting and running installer...${NC}"

    # Extract archive
    EXTRACT_DIR="${TEMP_DIR}/extract"
    mkdir -p "$EXTRACT_DIR"

    if [[ "$ASSET_NAME" == *.tar.gz ]]; then
        tar -xzf "$ARCHIVE_PATH" -C "$EXTRACT_DIR"
    elif [[ "$ASSET_NAME" == *.zip ]]; then
        unzip -q "$ARCHIVE_PATH" -d "$EXTRACT_DIR"
    else
        echo -e "${RED}✗ Unknown archive format: ${ASSET_NAME}${NC}"
        rm -rf "$TEMP_DIR"
        exit 1
    fi

    # Find and run installer
    INSTALLER_BINARY=$(find "$EXTRACT_DIR" -name "kindly-av1-installer" -o -name "kindly-av1-installer.exe" | head -1)

    if [ -z "$INSTALLER_BINARY" ]; then
        echo -e "${RED}✗ Installer binary not found in archive${NC}"
        rm -rf "$TEMP_DIR"
        exit 1
    fi

    # Make installer executable (Unix-like systems)
    chmod +x "$INSTALLER_BINARY" 2>/dev/null || true

    # Run installer with any passed arguments
    echo ""
    "$INSTALLER_BINARY" "$@"

    # Cleanup
    rm -rf "$TEMP_DIR"
}

# Run main installation
main "$@"
