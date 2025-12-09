#!/bin/bash
# verify-setup.sh - Verify Mac App Store packaging setup
# Usage: ./verify-setup.sh

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}✓${NC} $*"; }
log_warn() { echo -e "${YELLOW}⚠${NC} $*"; }
log_error() { echo -e "${RED}✗${NC} $*"; }
log_header() { echo -e "\n${BLUE}$*${NC}"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

ERROR_COUNT=0
WARN_COUNT=0

# Header
echo ""
echo "========================================="
echo "Mac App Store Packaging Verification"
echo "========================================="
echo ""

# 1. Directory Structure
log_header "1. Directory Structure"

if [ -d "kindly-av1.app/Contents/MacOS" ]; then
    log_info "App bundle structure exists"
else
    log_error "Missing: kindly-av1.app/Contents/MacOS/"
    ((ERROR_COUNT++))
fi

if [ -d "kindly-av1.app/Contents/Resources" ]; then
    log_info "Resources directory exists"
else
    log_error "Missing: kindly-av1.app/Contents/Resources/"
    ((ERROR_COUNT++))
fi

# 2. Required Files
log_header "2. Required Files"

FILES=(
    "kindly-av1.app/Contents/Info.plist"
    "kindly-av1.app/Contents/Resources/kindly-av1.entitlements"
    "build-app.sh"
    "create-icns.sh"
    "MAC_APP_STORE_SETUP.md"
    "ENTITLEMENTS.md"
    "README.md"
)

for file in "${FILES[@]}"; do
    if [ -f "$file" ]; then
        log_info "Found: $file"
    else
        log_error "Missing: $file"
        ((ERROR_COUNT++))
    fi
done

# 3. Scripts Executable
log_header "3. Script Permissions"

if [ -x "build-app.sh" ]; then
    log_info "build-app.sh is executable"
else
    log_error "build-app.sh is not executable (run: chmod +x build-app.sh)"
    ((ERROR_COUNT++))
fi

if [ -x "create-icns.sh" ]; then
    log_info "create-icns.sh is executable"
else
    log_error "create-icns.sh is not executable (run: chmod +x create-icns.sh)"
    ((ERROR_COUNT++))
fi

# 4. Info.plist Validation
log_header "4. Info.plist Validation"

if command -v plutil > /dev/null 2>&1; then
    if plutil -lint kindly-av1.app/Contents/Info.plist > /dev/null 2>&1; then
        log_info "Info.plist is valid XML"
    else
        log_error "Info.plist validation failed (invalid XML)"
        ((ERROR_COUNT++))
    fi

    # Check required keys
    REQUIRED_KEYS=(
        "CFBundleIdentifier"
        "CFBundleVersion"
        "CFBundleShortVersionString"
        "CFBundleExecutable"
        "LSMinimumSystemVersion"
    )

    for key in "${REQUIRED_KEYS[@]}"; do
        if /usr/libexec/PlistBuddy -c "Print :$key" kindly-av1.app/Contents/Info.plist > /dev/null 2>&1; then
            VALUE=$(/usr/libexec/PlistBuddy -c "Print :$key" kindly-av1.app/Contents/Info.plist 2>/dev/null)
            log_info "Info.plist has $key = $VALUE"
        else
            log_error "Info.plist missing required key: $key"
            ((ERROR_COUNT++))
        fi
    done
else
    log_warn "plutil not found (macOS only), skipping Info.plist validation"
    ((WARN_COUNT++))
fi

# 5. Entitlements Validation
log_header "5. Entitlements Validation"

ENTITLEMENTS_FILE="kindly-av1.app/Contents/Resources/kindly-av1.entitlements"

if [ -f "$ENTITLEMENTS_FILE" ]; then
    if command -v plutil > /dev/null 2>&1; then
        if plutil -lint "$ENTITLEMENTS_FILE" > /dev/null 2>&1; then
            log_info "Entitlements file is valid XML"

            # Check required entitlements
            REQUIRED_ENTITLEMENTS=(
                "com.apple.security.app-sandbox"
                "com.apple.security.files.user-selected.read-write"
                "com.apple.security.device.gpu"
                "com.apple.security.cs.allow-unsigned-executable-memory"
            )

            for key in "${REQUIRED_ENTITLEMENTS[@]}"; do
                if grep -q "$key" "$ENTITLEMENTS_FILE"; then
                    log_info "Entitlement present: $key"
                else
                    log_error "Missing required entitlement: $key"
                    ((ERROR_COUNT++))
                fi
            done
        else
            log_error "Entitlements file validation failed (invalid XML)"
            ((ERROR_COUNT++))
        fi
    fi
else
    log_error "Entitlements file not found: $ENTITLEMENTS_FILE"
    ((ERROR_COUNT++))
fi

# 6. Developer Tools
log_header "6. Developer Tools"

TOOLS=(
    "codesign:Code signing tool"
    "pkgbuild:Package builder"
    "productsign:Package signer"
    "iconutil:Icon converter"
    "sips:Image resizer"
    "xcrun:Xcode utilities"
)

for entry in "${TOOLS[@]}"; do
    IFS=':' read -r cmd desc <<< "$entry"
    if command -v "$cmd" > /dev/null 2>&1; then
        log_info "$desc ($cmd) is available"
    else
        log_warn "$desc ($cmd) not found (required for building/signing)"
        ((WARN_COUNT++))
    fi
done

# 7. Certificates
log_header "7. Code Signing Certificates"

if command -v security > /dev/null 2>&1; then
    # Check for Mac App Distribution certificate
    if security find-identity -v -p codesigning 2>/dev/null | grep -q "3rd Party Mac Developer Application"; then
        log_info "Mac App Distribution certificate found"
    else
        log_warn "Mac App Distribution certificate not found (required for signing)"
        log_warn "  Download from: https://developer.apple.com/account/resources/certificates/list"
        ((WARN_COUNT++))
    fi

    # Check for Mac Installer Distribution certificate
    if security find-identity -v -p basic 2>/dev/null | grep -q "3rd Party Mac Developer Installer"; then
        log_info "Mac Installer Distribution certificate found"
    else
        log_warn "Mac Installer Distribution certificate not found (required for .pkg signing)"
        log_warn "  Download from: https://developer.apple.com/account/resources/certificates/list"
        ((WARN_COUNT++))
    fi
else
    log_warn "security command not found (macOS only)"
    ((WARN_COUNT++))
fi

# 8. App Icon
log_header "8. App Icon"

if [ -f "kindly-av1.app/Contents/Resources/AppIcon.icns" ]; then
    ICON_SIZE=$(du -h kindly-av1.app/Contents/Resources/AppIcon.icns 2>/dev/null | cut -f1)
    log_info "App icon exists (size: $ICON_SIZE)"
else
    log_warn "App icon not found (run: ./create-icns.sh <icon.png>)"
    log_warn "  Icon requirements: 1024×1024 PNG, square, high resolution"
    ((WARN_COUNT++))
fi

# 9. Binary
log_header "9. Binary"

if [ -f "kindly-av1.app/Contents/MacOS/kindly-av1" ]; then
    BINARY_SIZE=$(du -h kindly-av1.app/Contents/MacOS/kindly-av1 2>/dev/null | cut -f1)
    log_info "Binary exists (size: $BINARY_SIZE)"

    # Check if binary is executable
    if [ -x "kindly-av1.app/Contents/MacOS/kindly-av1" ]; then
        log_info "Binary is executable"
    else
        log_warn "Binary is not executable (will be set during build)"
        ((WARN_COUNT++))
    fi

    # Check architecture
    if command -v file > /dev/null 2>&1; then
        ARCH=$(file kindly-av1.app/Contents/MacOS/kindly-av1 2>/dev/null || echo "unknown")
        if echo "$ARCH" | grep -q "universal"; then
            log_info "Binary is universal (arm64 + x86_64)"
        elif echo "$ARCH" | grep -q "arm64"; then
            log_info "Binary is arm64 only (Apple Silicon)"
            log_warn "Consider building universal binary with: ./build-app.sh --universal"
        elif echo "$ARCH" | grep -q "x86_64"; then
            log_info "Binary is x86_64 only (Intel)"
            log_warn "Consider building universal binary with: ./build-app.sh --universal"
        else
            log_warn "Binary architecture unknown: $ARCH"
            ((WARN_COUNT++))
        fi
    fi
else
    log_warn "Binary not found (run: ./build-app.sh to build)"
    log_warn "  Binary will be created at: kindly-av1.app/Contents/MacOS/kindly-av1"
    ((WARN_COUNT++))
fi

# 10. Documentation
log_header "10. Documentation"

DOC_FILES=(
    "MAC_APP_STORE_SETUP.md"
    "ENTITLEMENTS.md"
    "README.md"
)

for doc in "${DOC_FILES[@]}"; do
    if [ -f "$doc" ]; then
        LINE_COUNT=$(wc -l < "$doc")
        log_info "$doc ($LINE_COUNT lines)"
    else
        log_error "Missing documentation: $doc"
        ((ERROR_COUNT++))
    fi
done

# Summary
echo ""
echo "========================================="
echo "Verification Summary"
echo "========================================="

if [ $ERROR_COUNT -eq 0 ] && [ $WARN_COUNT -eq 0 ]; then
    log_info "All checks passed! ✓"
    echo ""
    echo "Next steps:"
    echo "  1. Create app icon: ./create-icns.sh <icon.png>"
    echo "  2. Build and sign: ./build-app.sh --universal --sign 'IDENTITY'"
    echo "  3. Upload to App Store: See MAC_APP_STORE_SETUP.md"
elif [ $ERROR_COUNT -eq 0 ]; then
    log_warn "$WARN_COUNT warnings (setup incomplete but no errors)"
    echo ""
    echo "Warnings are expected if you haven't:"
    echo "  - Enrolled in Apple Developer Program"
    echo "  - Downloaded code signing certificates"
    echo "  - Created app icon"
    echo "  - Built the binary"
    echo ""
    echo "Follow README.md quick start guide to complete setup."
else
    log_error "$ERROR_COUNT errors, $WARN_COUNT warnings"
    echo ""
    echo "Fix errors before proceeding. See above for details."
    exit 1
fi

echo "========================================="
echo ""

exit 0
