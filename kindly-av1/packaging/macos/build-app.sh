#!/bin/bash
# build-app.sh - Build and sign kindly-av1 Mac App Store bundle
# Usage: ./build-app.sh [--universal] [--sign IDENTITY]

set -euo pipefail

# Configuration
APP_NAME="kindly-av1"
BUNDLE_ID="software.kindly.av1"
VERSION="1.0.0"
MIN_OS="11.0"

# Directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
APP_BUNDLE="$SCRIPT_DIR/$APP_NAME.app"
CONTENTS="$APP_BUNDLE/Contents"
MACOS="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"
TARGET_DIR="$PROJECT_ROOT/target"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Logging
log_info() { echo -e "${GREEN}[INFO]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# Parse arguments
UNIVERSAL=false
SIGN_IDENTITY=""
while [[ $# -gt 0 ]]; do
    case $1 in
        --universal)
            UNIVERSAL=true
            shift
            ;;
        --sign)
            SIGN_IDENTITY="$2"
            shift 2
            ;;
        *)
            log_error "Unknown argument: $1"
            echo "Usage: $0 [--universal] [--sign IDENTITY]"
            exit 1
            ;;
    esac
done

# Step 1: Build binaries
log_info "Building kindly-av1..."

if [ "$UNIVERSAL" = true ]; then
    log_info "Building universal binary (arm64 + x86_64)..."

    # Build for Apple Silicon
    log_info "Building for aarch64-apple-darwin..."
    cd "$PROJECT_ROOT"
    cargo build --release --target aarch64-apple-darwin

    # Build for Intel
    log_info "Building for x86_64-apple-darwin..."
    cargo build --release --target x86_64-apple-darwin

    # Create universal binary
    log_info "Creating universal binary with lipo..."
    mkdir -p "$MACOS"
    lipo -create \
        "$TARGET_DIR/aarch64-apple-darwin/release/$APP_NAME" \
        "$TARGET_DIR/x86_64-apple-darwin/release/$APP_NAME" \
        -output "$MACOS/$APP_NAME"

    log_info "Universal binary created: $(file "$MACOS/$APP_NAME")"
else
    # Build for host architecture only
    log_info "Building for host architecture..."
    cd "$PROJECT_ROOT"
    cargo build --release

    # Copy binary
    mkdir -p "$MACOS"
    cp "$TARGET_DIR/release/$APP_NAME" "$MACOS/"

    log_info "Binary copied: $(file "$MACOS/$APP_NAME")"
fi

# Step 2: Set executable permissions
chmod +x "$MACOS/$APP_NAME"

# Step 3: Copy resources (icon, entitlements already in place)
log_info "Checking resources..."
if [ ! -f "$RESOURCES/AppIcon.icns" ]; then
    log_warn "AppIcon.icns not found. Run ./create-icns.sh first."
fi

if [ ! -f "$RESOURCES/${APP_NAME}.entitlements" ]; then
    log_error "Entitlements file not found at $RESOURCES/${APP_NAME}.entitlements"
    exit 1
fi

# Step 4: Verify Info.plist
if [ ! -f "$CONTENTS/Info.plist" ]; then
    log_error "Info.plist not found at $CONTENTS/Info.plist"
    exit 1
fi

# Validate plist
if ! plutil -lint "$CONTENTS/Info.plist" > /dev/null 2>&1; then
    log_error "Info.plist validation failed"
    exit 1
fi
log_info "Info.plist validated"

# Step 5: Code signing
if [ -n "$SIGN_IDENTITY" ]; then
    log_info "Signing app bundle with identity: $SIGN_IDENTITY"

    # Sign the binary first
    codesign --force --sign "$SIGN_IDENTITY" \
        --entitlements "$RESOURCES/${APP_NAME}.entitlements" \
        --options runtime \
        --timestamp \
        "$MACOS/$APP_NAME"

    # Sign the app bundle
    codesign --force --sign "$SIGN_IDENTITY" \
        --entitlements "$RESOURCES/${APP_NAME}.entitlements" \
        --options runtime \
        --timestamp \
        --deep \
        "$APP_BUNDLE"

    log_info "Code signature applied"

    # Verify signature
    codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"
    log_info "Code signature verified"

    # Display signature info
    codesign -dvv "$APP_BUNDLE" 2>&1 | head -n 20
else
    log_warn "No signing identity specified. Bundle is unsigned."
    log_warn "For Mac App Store submission, sign with: --sign '3rd Party Mac Developer Application: Your Name (TEAM_ID)'"
fi

# Step 6: Create installer package (for App Store submission)
if [ -n "$SIGN_IDENTITY" ]; then
    log_info "Creating .pkg installer..."

    PKG_FILE="$SCRIPT_DIR/${APP_NAME}-${VERSION}.pkg"

    # Build component package
    pkgbuild --component "$APP_BUNDLE" \
        --install-location /Applications \
        --identifier "$BUNDLE_ID" \
        --version "$VERSION" \
        "$PKG_FILE.tmp"

    # Sign package with Mac Installer Distribution certificate
    INSTALLER_IDENTITY="${SIGN_IDENTITY//Application/Installer}"
    productsign --sign "$INSTALLER_IDENTITY" \
        "$PKG_FILE.tmp" \
        "$PKG_FILE"

    rm "$PKG_FILE.tmp"

    log_info "Package created: $PKG_FILE"

    # Verify package signature
    pkgutil --check-signature "$PKG_FILE"

    log_info "Package signature verified"
else
    log_warn "Skipping .pkg creation (no signing identity)"
fi

# Step 7: Validate for App Store submission
if [ -n "$SIGN_IDENTITY" ]; then
    log_info "Validating app bundle for App Store..."

    # Check for required keys in Info.plist
    REQUIRED_KEYS=(
        "CFBundleIdentifier"
        "CFBundleVersion"
        "CFBundleShortVersionString"
        "LSMinimumSystemVersion"
        "CFBundleExecutable"
    )

    for key in "${REQUIRED_KEYS[@]}"; do
        if ! /usr/libexec/PlistBuddy -c "Print :$key" "$CONTENTS/Info.plist" > /dev/null 2>&1; then
            log_error "Missing required Info.plist key: $key"
            exit 1
        fi
    done

    log_info "Info.plist validation passed"

    # Check entitlements
    if ! codesign -d --entitlements - "$APP_BUNDLE" > /dev/null 2>&1; then
        log_error "Failed to extract entitlements from signed bundle"
        exit 1
    fi

    log_info "Entitlements validation passed"
fi

# Summary
echo ""
log_info "========================================="
log_info "Build complete!"
log_info "========================================="
log_info "App Bundle: $APP_BUNDLE"
log_info "Version: $VERSION"
log_info "Bundle ID: $BUNDLE_ID"
log_info "Min macOS: $MIN_OS"
if [ -n "$SIGN_IDENTITY" ]; then
    log_info "Signed: Yes ($SIGN_IDENTITY)"
    log_info "Package: $SCRIPT_DIR/${APP_NAME}-${VERSION}.pkg"
else
    log_info "Signed: No (use --sign for App Store submission)"
fi
log_info "========================================="
echo ""

# Next steps
log_info "Next steps:"
if [ -z "$SIGN_IDENTITY" ]; then
    echo "  1. Sign the app: ./build-app.sh --sign '3rd Party Mac Developer Application: Your Name (TEAM_ID)'"
else
    echo "  1. Test the app: open $APP_BUNDLE"
    echo "  2. Upload to App Store Connect:"
    echo "     xcrun altool --upload-app -f ${APP_NAME}-${VERSION}.pkg -u YOUR_APPLE_ID -p @keychain:AC_PASSWORD"
    echo "     OR use Transporter.app"
    echo "  3. Submit for review in App Store Connect"
fi
