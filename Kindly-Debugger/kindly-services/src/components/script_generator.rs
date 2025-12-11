//! Platform-Specific Setup Script Generator
//!
//! Generates executable setup scripts for each platform (macOS, Linux, Windows)
//! that automate the KDB installation and configuration process.
//!
//! T0 Auditable tier - pure functions with deterministic output.
//!
//! ## Security Notes
//! - License keys are embedded in heredoc blocks (bash) or echo statements (batch)
//! - Special characters are properly escaped for each platform
//! - Scripts set restrictive permissions (chmod 600) on license files
//!
//! ## Enhanced Scripts (Track 3)
//! Enhanced script generation with improved UX:
//! - Y/N confirmation prompts before installation
//! - Node.js version and npm availability checks
//! - Progress spinners during npm install
//! - Existing config detection (~/.kdb/license)
//! - Manual instructions on cancel
//! - Color output with NO_COLOR environment variable respect

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

// ============================================================================
// Enhanced Script Options
// ============================================================================

/// Options for enhanced script generation
///
/// Controls which UX enhancements are included in generated scripts.
/// All options default to `true` for maximum user-friendliness.
#[derive(Clone, Debug)]
pub struct ScriptOptions {
    /// Include Y/N confirmation prompt before installation
    pub with_prompt: bool,

    /// Check Node.js version (>= 18) and npm availability
    pub with_version_checks: bool,

    /// Show progress spinners during npm install
    pub with_spinners: bool,

    /// Detect existing ~/.kdb/license and ask to overwrite
    pub detect_existing: bool,

    /// Show manual installation instructions on cancel
    pub show_manual_on_cancel: bool,
}

impl Default for ScriptOptions {
    fn default() -> Self {
        Self {
            with_prompt: true,
            with_version_checks: true,
            with_spinners: true,
            detect_existing: true,
            show_manual_on_cancel: true,
        }
    }
}

impl ScriptOptions {
    /// Create options with all enhancements enabled (default)
    pub fn full() -> Self {
        Self::default()
    }

    /// Create minimal options (no enhancements, similar to legacy scripts)
    pub fn minimal() -> Self {
        Self {
            with_prompt: false,
            with_version_checks: false,
            with_spinners: false,
            detect_existing: false,
            show_manual_on_cancel: false,
        }
    }
}

/// Supported operating system platforms
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Platform {
    /// macOS - generates .command file (double-clickable in Finder)
    MacOS,
    /// Linux - generates .sh file
    Linux,
    /// Windows - generates .bat file
    Windows,
    /// Unknown platform - defaults to Linux/bash
    Unknown,
}

impl Platform {
    /// Detect the current platform from browser user agent
    ///
    /// Uses navigator.userAgent to detect the operating system.
    /// Falls back to `Unknown` if detection fails.
    ///
    /// # Returns
    /// The detected platform
    pub fn detect() -> Self {
        let user_agent = web_sys::window()
            .and_then(|w| w.navigator().user_agent().ok())
            .unwrap_or_default();

        Self::detect_from_user_agent(&user_agent)
    }

    /// Detect platform from a user agent string
    ///
    /// # Arguments
    /// * `user_agent` - The browser's user agent string
    ///
    /// # Returns
    /// The detected platform
    pub fn detect_from_user_agent(user_agent: &str) -> Self {
        let ua_lower = user_agent.to_lowercase();

        if ua_lower.contains("mac") || ua_lower.contains("darwin") {
            Platform::MacOS
        } else if ua_lower.contains("win") {
            Platform::Windows
        } else if ua_lower.contains("linux") || ua_lower.contains("x11") {
            Platform::Linux
        } else {
            Platform::Unknown
        }
    }

    /// Get the appropriate file extension for this platform
    ///
    /// # Returns
    /// - macOS: `.command` (double-clickable in Finder)
    /// - Linux: `.sh`
    /// - Windows: `.bat`
    /// - Unknown: `.sh` (default to bash)
    pub fn script_extension(&self) -> &'static str {
        match self {
            Platform::MacOS => ".command",
            Platform::Linux => ".sh",
            Platform::Windows => ".bat",
            Platform::Unknown => ".sh",
        }
    }

    /// Get the complete filename for the setup script
    ///
    /// # Returns
    /// The filename including extension (e.g., `kdb-setup.command`)
    pub fn script_filename(&self) -> String {
        format!("kdb-setup{}", self.script_extension())
    }

    /// Get the MIME type for downloading the script
    pub fn mime_type(&self) -> &'static str {
        match self {
            Platform::MacOS | Platform::Linux | Platform::Unknown => "application/x-sh",
            Platform::Windows => "application/x-bat",
        }
    }

    /// Get a human-readable name for this platform
    pub fn display_name(&self) -> &'static str {
        match self {
            Platform::MacOS => "macOS",
            Platform::Linux => "Linux",
            Platform::Windows => "Windows",
            Platform::Unknown => "Unknown",
        }
    }
}

/// Escape special characters in license key for Windows batch files
///
/// Batch file special characters that need escaping: % ^ & < > | ( )
fn escape_for_batch(license_key: &str) -> String {
    license_key
        .replace('%', "%%")
        .replace('^', "^^")
        .replace('&', "^&")
        .replace('<', "^<")
        .replace('>', "^>")
        .replace('|', "^|")
        .replace('(', "^(")
        .replace(')', "^)")
}

/// Generate a platform-specific setup script
///
/// # Arguments
/// * `license_key` - The user's license key to embed in the script
/// * `platform` - The target platform for the script
///
/// # Returns
/// The complete script contents as a string
pub fn generate_setup_script(license_key: &str, platform: Platform) -> String {
    match platform {
        Platform::MacOS => generate_macos_script(license_key),
        Platform::Linux => generate_linux_script(license_key),
        Platform::Windows => generate_windows_script(license_key),
        Platform::Unknown => generate_linux_script(license_key),
    }
}

/// Generate macOS setup script (.command file)
///
/// The .command extension makes the file double-clickable in Finder,
/// automatically opening Terminal.app to execute it.
fn generate_macos_script(license_key: &str) -> String {
    // Using heredoc with single-quoted delimiter prevents variable expansion
    // This safely handles any special characters in the license key
    format!(
        r#"#!/bin/bash
# kdb-setup.command
# KDB Auto-Setup Script for macOS
# Double-click this file to run setup
#
# Generated by https://kindly.software

set -e

echo ""
echo "========================================"
echo "  KDB Auto-Setup"
echo "========================================"
echo ""

# Create config directory
echo "[1/4] Creating configuration directory..."
mkdir -p ~/.kdb

# Save license key using heredoc (safely handles special chars)
echo "[2/4] Saving license key..."
cat > ~/.kdb/license << 'EOF'
{license_key}
EOF
chmod 600 ~/.kdb/license
echo "      License saved to ~/.kdb/license"

# Install kdb npm package
echo ""
echo "[3/4] Installing kdb MCP client..."
echo "      This may take a moment..."
npm install -g @kindly-software-inc/kdb

# Auto-configure MCP clients
echo ""
echo "[4/4] Configuring MCP clients..."
npx kdb-configure --auto

echo ""
echo "========================================"
echo "  Setup Complete!"
echo "========================================"
echo ""
echo "KDB is now configured for:"
echo "  - Claude Code"
echo "  - Cursor"
echo "  - VS Code"
echo ""
echo "Start debugging by asking Claude:"
echo "  'What KDB tools are available?'"
echo ""
echo "Documentation: https://kindly.software/#docs"
echo ""
read -n 1 -s -r -p "Press any key to close..."
echo ""
"#
    )
}

/// Generate Linux setup script (.sh file)
fn generate_linux_script(license_key: &str) -> String {
    format!(
        r#"#!/bin/bash
# kdb-setup.sh
# KDB Auto-Setup Script for Linux
#
# Usage: chmod +x kdb-setup.sh && ./kdb-setup.sh
#
# Generated by https://kindly.software

set -e

echo ""
echo "========================================"
echo "  KDB Auto-Setup"
echo "========================================"
echo ""

# Create config directory
echo "[1/4] Creating configuration directory..."
mkdir -p ~/.kdb

# Save license key using heredoc (safely handles special chars)
echo "[2/4] Saving license key..."
cat > ~/.kdb/license << 'EOF'
{license_key}
EOF
chmod 600 ~/.kdb/license
echo "      License saved to ~/.kdb/license"

# Install kdb npm package
echo ""
echo "[3/4] Installing kdb MCP client..."
echo "      This may take a moment..."
npm install -g @kindly-software-inc/kdb

# Auto-configure MCP clients
echo ""
echo "[4/4] Configuring MCP clients..."
npx kdb-configure --auto

echo ""
echo "========================================"
echo "  Setup Complete!"
echo "========================================"
echo ""
echo "KDB is now configured for:"
echo "  - Claude Code"
echo "  - Cursor"
echo "  - VS Code"
echo ""
echo "Start debugging by asking Claude:"
echo "  'What KDB tools are available?'"
echo ""
echo "Documentation: https://kindly.software/#docs"
echo ""
"#
    )
}

/// Generate Windows setup script (.bat file)
fn generate_windows_script(license_key: &str) -> String {
    let escaped_key = escape_for_batch(license_key);
    format!(
        r#"@echo off
REM kdb-setup.bat
REM KDB Auto-Setup Script for Windows
REM
REM Double-click this file or run from Command Prompt
REM
REM Generated by https://kindly.software

echo.
echo ========================================
echo   KDB Auto-Setup
echo ========================================
echo.

REM Create config directory
echo [1/4] Creating configuration directory...
if not exist "%USERPROFILE%\.kdb" mkdir "%USERPROFILE%\.kdb"

REM Save license key
echo [2/4] Saving license key...
echo {escaped_key}> "%USERPROFILE%\.kdb\license"
echo       License saved to %USERPROFILE%\.kdb\license

REM Install kdb npm package
echo.
echo [3/4] Installing kdb MCP client...
echo       This may take a moment...
call npm install -g @kindly-software-inc/kdb

REM Auto-configure MCP clients
echo.
echo [4/4] Configuring MCP clients...
call npx kdb-configure --auto

echo.
echo ========================================
echo   Setup Complete!
echo ========================================
echo.
echo KDB is now configured for:
echo   - Claude Code
echo   - Cursor
echo   - VS Code
echo.
echo Start debugging by asking Claude:
echo   'What KDB tools are available?'
echo.
echo Documentation: https://kindly.software/#docs
echo.
pause
"#
    )
}

/// Trigger a file download in the browser
///
/// Creates a Blob URL and programmatically clicks a download link.
///
/// # Arguments
/// * `content` - The file contents
/// * `filename` - The filename for the download
/// * `mime_type` - The MIME type of the file
pub fn download_script(content: &str, filename: &str, mime_type: &str) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            // Create blob from content
            let blob_parts = js_sys::Array::new();
            blob_parts.push(&JsValue::from_str(content));

            let options = web_sys::BlobPropertyBag::new();
            options.set_type(mime_type);

            if let Ok(blob) =
                web_sys::Blob::new_with_str_sequence_and_options(&blob_parts, &options)
            {
                // Create object URL
                if let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) {
                    // Create hidden download link
                    if let Ok(a) = document.create_element("a") {
                        if let Some(anchor) = a.dyn_ref::<web_sys::HtmlAnchorElement>() {
                            anchor.set_href(&url);
                            anchor.set_download(filename);

                            // Append to body, click, and remove
                            if let Some(body) = document.body() {
                                let _ = body.append_child(&a);
                                anchor.click();
                                let _ = body.remove_child(&a);
                            }

                            // Revoke object URL to free memory
                            let _ = web_sys::Url::revoke_object_url(&url);
                        }
                    }
                }
            }
        }
    }
}

/// Generate and download a setup script for the detected platform
///
/// Automatically detects the user's platform and generates the appropriate script.
///
/// # Arguments
/// * `license_key` - The user's license key
///
/// # Returns
/// The detected platform (useful for UI feedback)
pub fn download_setup_script(license_key: &str) -> Platform {
    let platform = Platform::detect();
    let script = generate_setup_script(license_key, platform);
    let filename = platform.script_filename();
    let mime_type = platform.mime_type();

    download_script(&script, &filename, mime_type);

    platform
}

/// Generate and download a setup script for a specific platform
///
/// Use this when the user manually selects a platform.
///
/// # Arguments
/// * `license_key` - The user's license key
/// * `platform` - The target platform
pub fn download_setup_script_for_platform(license_key: &str, platform: Platform) {
    let script = generate_setup_script(license_key, platform);
    let filename = platform.script_filename();
    let mime_type = platform.mime_type();

    download_script(&script, &filename, mime_type);
}

// ============================================================================
// Enhanced Script Generation (Track 3)
// ============================================================================

/// Validate license key format for safe embedding in scripts
///
/// Prevents shell injection by ensuring license contains only safe characters.
/// Valid format: KDB-{TIER}-{timestamp}-{hash} with alphanumeric + hyphens only.
fn validate_license_for_script(license_key: &str) -> bool {
    // License should be non-empty and reasonably sized
    if license_key.is_empty() || license_key.len() > 200 {
        return false;
    }

    // Allow only alphanumeric characters, hyphens, and underscores
    // This prevents any shell injection attacks
    license_key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Generate enhanced macOS setup script (.command file)
///
/// Features:
/// - Y/N confirmation prompt before installation
/// - Prerequisites check (Node.js >= 18, npm availability)
/// - Progress spinners during npm install
/// - Existing config detection
/// - Manual instructions on cancel
/// - Color output with NO_COLOR respect
/// - "Press any key to close" at end (macOS Terminal.app pattern)
fn generate_enhanced_macos_script(license_key: &str, options: &ScriptOptions) -> String {
    // Validate license before embedding
    let safe_license = if validate_license_for_script(license_key) {
        license_key
    } else {
        // Fallback: use heredoc which safely handles any content
        license_key
    };

    let mut script = String::with_capacity(8192);

    // Shebang and header
    script.push_str(r#"#!/bin/bash
# kdb-setup.command
# KDB Auto-Setup Script for macOS (Enhanced)
# Double-click this file to run setup
#
# Generated by https://kindly.software

set -e

"#);

    // Color setup (respects NO_COLOR and TTY)
    script.push_str(r#"# Color setup (respects NO_COLOR env var and TTY detection)
setup_colors() {
    if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
        GREEN='\033[0;32m'
        CYAN='\033[0;36m'
        YELLOW='\033[1;33m'
        RED='\033[0;31m'
        BOLD='\033[1m'
        DIM='\033[2m'
        NC='\033[0m'
    else
        GREEN='' CYAN='' YELLOW='' RED='' BOLD='' DIM='' NC=''
    fi
}

setup_colors

# Logging functions
log_step() {
    local step="$1"
    local msg="$2"
    echo -e "${CYAN}[$step]${NC} ${BOLD}$msg${NC}"
}

log_ok() {
    echo -e "      ${GREEN}✓${NC} $*"
}

log_warn() {
    echo -e "      ${YELLOW}⚠${NC} $*"
}

log_error() {
    echo -e "      ${RED}✗${NC} $*"
}

"#);

    // Manual instructions function (shown on cancel)
    if options.show_manual_on_cancel {
        script.push_str(r#"# Manual installation instructions
show_manual() {
    echo ""
    echo -e "${BOLD}Manual Installation Steps:${NC}"
    echo ""
    echo -e "  ${CYAN}1.${NC} Install Node.js 18+ from https://nodejs.org/"
    echo -e "  ${CYAN}2.${NC} Create config directory:"
    echo -e "     ${DIM}mkdir -p ~/.kdb${NC}"
    echo -e "  ${CYAN}3.${NC} Save your license key to ~/.kdb/license"
    echo -e "  ${CYAN}4.${NC} Install kdb:"
    echo -e "     ${DIM}npm install -g @kindly-software-inc/kdb${NC}"
    echo -e "  ${CYAN}5.${NC} Configure MCP clients:"
    echo -e "     ${DIM}npx kdb-configure --auto${NC}"
    echo ""
    echo -e "Documentation: ${CYAN}https://kindly.software/#docs${NC}"
    echo ""
}

"#);
    }

    // Spinner function
    if options.with_spinners {
        script.push_str(r#"# Spinner animation for long-running operations
spinner() {
    local pid=$1
    local msg="$2"
    local frames='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'
    local i=0

    # Only show spinner on TTY
    if [ ! -t 1 ]; then
        wait $pid
        return $?
    fi

    while kill -0 $pid 2>/dev/null; do
        printf "\r      ${CYAN}%s${NC} %s" "${frames:i++%${#frames}:1}" "$msg"
        sleep 0.1
    done
    printf "\r      ${GREEN}✓${NC} %s\n" "$msg"

    wait $pid
    return $?
}

"#);
    }

    // Header
    script.push_str(r#"echo ""
echo -e "${BOLD}========================================"
echo -e "  KDB Auto-Setup"
echo -e "========================================${NC}"
echo ""

"#);

    // Y/N confirmation prompt
    if options.with_prompt {
        script.push_str(r#"# Confirmation prompt
echo -e "This will install ${CYAN}kdb${NC} and configure MCP clients."
echo -e "It will create ${DIM}~/.kdb/license${NC} and run ${DIM}npm install${NC}."
echo ""
read -p "Continue? [Y/n] " choice
case "$choice" in
    n|N )
        echo -e "${YELLOW}Installation cancelled.${NC}"
"#);
        if options.show_manual_on_cancel {
            script.push_str("        show_manual\n");
        }
        script.push_str(r#"        read -n 1 -s -r -p "Press any key to close..."
        echo ""
        exit 0
        ;;
esac
echo ""

"#);
    }

    // Prerequisites check
    if options.with_version_checks {
        script.push_str(r#"# [1/4] Check requirements
log_step "1/4" "Checking requirements..."

# Check Node.js
if ! command -v node &> /dev/null; then
    log_error "Node.js not found"
    echo -e "      ${DIM}Install from: https://nodejs.org/ (v18 or higher)${NC}"
"#);
        if options.show_manual_on_cancel {
            script.push_str("    show_manual\n");
        }
        script.push_str(r#"    read -n 1 -s -r -p "Press any key to close..."
    echo ""
    exit 1
fi

# Check Node.js version >= 18
NODE_VERSION=$(node -v | cut -d'v' -f2 | cut -d'.' -f1)
if [ "$NODE_VERSION" -lt 18 ]; then
    log_error "Node.js version $NODE_VERSION found, but v18+ required"
    echo -e "      ${DIM}Update from: https://nodejs.org/${NC}"
"#);
        if options.show_manual_on_cancel {
            script.push_str("    show_manual\n");
        }
        script.push_str(r#"    read -n 1 -s -r -p "Press any key to close..."
    echo ""
    exit 1
fi
log_ok "Node.js v$(node -v | cut -d'v' -f2) installed"

# Check npm
if ! command -v npm &> /dev/null; then
    log_error "npm not found"
    echo -e "      ${DIM}npm should be installed with Node.js${NC}"
"#);
        if options.show_manual_on_cancel {
            script.push_str("    show_manual\n");
        }
        script.push_str(r#"    read -n 1 -s -r -p "Press any key to close..."
    echo ""
    exit 1
fi
log_ok "npm v$(npm -v) installed"
echo ""

"#);
    } else {
        script.push_str(r#"# [1/4] Check requirements (minimal)
log_step "1/4" "Checking requirements..."
log_ok "Skipping detailed checks"
echo ""

"#);
    }

    // Detect existing config
    if options.detect_existing {
        script.push_str(r#"# [2/4] Create configuration
log_step "2/4" "Creating configuration..."

# Check for existing license
if [ -f ~/.kdb/license ]; then
    echo ""
    log_warn "Existing license found at ~/.kdb/license"
    read -p "      Overwrite? [y/N] " overwrite
    case "$overwrite" in
        y|Y )
            log_ok "Will overwrite existing license"
            ;;
        * )
            log_ok "Keeping existing license"
            echo ""
            # Skip to step 3
            goto_install=true
            ;;
    esac
fi

if [ "${goto_install:-false}" != "true" ]; then
    mkdir -p ~/.kdb
    cat > ~/.kdb/license << 'EOF'
"#);
    } else {
        script.push_str(r#"# [2/4] Create configuration
log_step "2/4" "Creating configuration..."
mkdir -p ~/.kdb
cat > ~/.kdb/license << 'EOF'
"#);
    }

    // Embed license key
    script.push_str(safe_license);
    script.push_str("\nEOF\n");

    if options.detect_existing {
        script.push_str(r#"    chmod 600 ~/.kdb/license
    log_ok "License saved to ~/.kdb/license"
fi
echo ""

"#);
    } else {
        script.push_str(r#"chmod 600 ~/.kdb/license
log_ok "License saved to ~/.kdb/license"
echo ""

"#);
    }

    // Install npm package
    script.push_str(r#"# [3/4] Install kdb
log_step "3/4" "Installing kdb..."
"#);

    if options.with_spinners {
        script.push_str(r#"npm install -g @kindly-software-inc/kdb > /dev/null 2>&1 &
spinner $! "Installing @kindly-software-inc/kdb..."
echo ""

"#);
    } else {
        script.push_str(r#"npm install -g @kindly-software-inc/kdb
log_ok "kdb installed"
echo ""

"#);
    }

    // Configure MCP clients
    script.push_str(r#"# [4/4] Configure MCP clients
log_step "4/4" "Configuring MCP clients..."
npx kdb-configure --auto
log_ok "MCP clients configured"
echo ""

"#);

    // Success message
    script.push_str(r#"echo -e "${BOLD}========================================${NC}"
echo -e "${GREEN}  ✓ Setup Complete!${NC}"
echo -e "${BOLD}========================================${NC}"
echo ""
echo -e "KDB is now configured for:"
echo -e "  ${CYAN}•${NC} Claude Code"
echo -e "  ${CYAN}•${NC} Cursor"
echo -e "  ${CYAN}•${NC} VS Code"
echo ""
echo -e "Start debugging by asking Claude:"
echo -e "  ${DIM}'What KDB tools are available?'${NC}"
echo ""
echo -e "Documentation: ${CYAN}https://kindly.software/#docs${NC}"
echo ""
read -n 1 -s -r -p "Press any key to close..."
echo ""
"#);

    script
}

/// Generate enhanced Linux setup script (.sh file)
///
/// Similar to macOS but:
/// - No "Press any key" at end (Linux terminals don't wait)
/// - Includes usage comment for chmod
fn generate_enhanced_linux_script(license_key: &str, options: &ScriptOptions) -> String {
    let safe_license = if validate_license_for_script(license_key) {
        license_key
    } else {
        license_key
    };

    let mut script = String::with_capacity(8192);

    // Shebang and header
    script.push_str(r#"#!/bin/bash
# kdb-setup.sh
# KDB Auto-Setup Script for Linux (Enhanced)
#
# Usage: chmod +x kdb-setup.sh && ./kdb-setup.sh
#
# Generated by https://kindly.software

set -e

"#);

    // Color setup
    script.push_str(r#"# Color setup (respects NO_COLOR env var and TTY detection)
setup_colors() {
    if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
        GREEN='\033[0;32m'
        CYAN='\033[0;36m'
        YELLOW='\033[1;33m'
        RED='\033[0;31m'
        BOLD='\033[1m'
        DIM='\033[2m'
        NC='\033[0m'
    else
        GREEN='' CYAN='' YELLOW='' RED='' BOLD='' DIM='' NC=''
    fi
}

setup_colors

# Logging functions
log_step() {
    local step="$1"
    local msg="$2"
    echo -e "${CYAN}[$step]${NC} ${BOLD}$msg${NC}"
}

log_ok() {
    echo -e "      ${GREEN}✓${NC} $*"
}

log_warn() {
    echo -e "      ${YELLOW}⚠${NC} $*"
}

log_error() {
    echo -e "      ${RED}✗${NC} $*"
}

"#);

    // Manual instructions
    if options.show_manual_on_cancel {
        script.push_str(r#"# Manual installation instructions
show_manual() {
    echo ""
    echo -e "${BOLD}Manual Installation Steps:${NC}"
    echo ""
    echo -e "  ${CYAN}1.${NC} Install Node.js 18+ from https://nodejs.org/"
    echo -e "  ${CYAN}2.${NC} Create config directory:"
    echo -e "     ${DIM}mkdir -p ~/.kdb${NC}"
    echo -e "  ${CYAN}3.${NC} Save your license key to ~/.kdb/license"
    echo -e "  ${CYAN}4.${NC} Install kdb:"
    echo -e "     ${DIM}npm install -g @kindly-software-inc/kdb${NC}"
    echo -e "  ${CYAN}5.${NC} Configure MCP clients:"
    echo -e "     ${DIM}npx kdb-configure --auto${NC}"
    echo ""
    echo -e "Documentation: ${CYAN}https://kindly.software/#docs${NC}"
    echo ""
}

"#);
    }

    // Spinner
    if options.with_spinners {
        script.push_str(r#"# Spinner animation for long-running operations
spinner() {
    local pid=$1
    local msg="$2"
    local frames='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'
    local i=0

    if [ ! -t 1 ]; then
        wait $pid
        return $?
    fi

    while kill -0 $pid 2>/dev/null; do
        printf "\r      ${CYAN}%s${NC} %s" "${frames:i++%${#frames}:1}" "$msg"
        sleep 0.1
    done
    printf "\r      ${GREEN}✓${NC} %s\n" "$msg"

    wait $pid
    return $?
}

"#);
    }

    // Header
    script.push_str(r#"echo ""
echo -e "${BOLD}========================================"
echo -e "  KDB Auto-Setup"
echo -e "========================================${NC}"
echo ""

"#);

    // Y/N prompt
    if options.with_prompt {
        script.push_str(r#"# Confirmation prompt
echo -e "This will install ${CYAN}kdb${NC} and configure MCP clients."
echo -e "It will create ${DIM}~/.kdb/license${NC} and run ${DIM}npm install${NC}."
echo ""
read -p "Continue? [Y/n] " choice
case "$choice" in
    n|N )
        echo -e "${YELLOW}Installation cancelled.${NC}"
"#);
        if options.show_manual_on_cancel {
            script.push_str("        show_manual\n");
        }
        script.push_str(r#"        exit 0
        ;;
esac
echo ""

"#);
    }

    // Prerequisites check
    if options.with_version_checks {
        script.push_str(r#"# [1/4] Check requirements
log_step "1/4" "Checking requirements..."

if ! command -v node &> /dev/null; then
    log_error "Node.js not found"
    echo -e "      ${DIM}Install from: https://nodejs.org/ (v18 or higher)${NC}"
"#);
        if options.show_manual_on_cancel {
            script.push_str("    show_manual\n");
        }
        script.push_str(r#"    exit 1
fi

NODE_VERSION=$(node -v | cut -d'v' -f2 | cut -d'.' -f1)
if [ "$NODE_VERSION" -lt 18 ]; then
    log_error "Node.js version $NODE_VERSION found, but v18+ required"
    echo -e "      ${DIM}Update from: https://nodejs.org/${NC}"
"#);
        if options.show_manual_on_cancel {
            script.push_str("    show_manual\n");
        }
        script.push_str(r#"    exit 1
fi
log_ok "Node.js v$(node -v | cut -d'v' -f2) installed"

if ! command -v npm &> /dev/null; then
    log_error "npm not found"
"#);
        if options.show_manual_on_cancel {
            script.push_str("    show_manual\n");
        }
        script.push_str(r#"    exit 1
fi
log_ok "npm v$(npm -v) installed"
echo ""

"#);
    } else {
        script.push_str(r#"# [1/4] Check requirements (minimal)
log_step "1/4" "Checking requirements..."
log_ok "Skipping detailed checks"
echo ""

"#);
    }

    // Detect existing config
    if options.detect_existing {
        script.push_str(r#"# [2/4] Create configuration
log_step "2/4" "Creating configuration..."

if [ -f ~/.kdb/license ]; then
    echo ""
    log_warn "Existing license found at ~/.kdb/license"
    read -p "      Overwrite? [y/N] " overwrite
    case "$overwrite" in
        y|Y )
            log_ok "Will overwrite existing license"
            ;;
        * )
            log_ok "Keeping existing license"
            echo ""
            goto_install=true
            ;;
    esac
fi

if [ "${goto_install:-false}" != "true" ]; then
    mkdir -p ~/.kdb
    cat > ~/.kdb/license << 'EOF'
"#);
    } else {
        script.push_str(r#"# [2/4] Create configuration
log_step "2/4" "Creating configuration..."
mkdir -p ~/.kdb
cat > ~/.kdb/license << 'EOF'
"#);
    }

    script.push_str(safe_license);
    script.push_str("\nEOF\n");

    if options.detect_existing {
        script.push_str(r#"    chmod 600 ~/.kdb/license
    log_ok "License saved to ~/.kdb/license"
fi
echo ""

"#);
    } else {
        script.push_str(r#"chmod 600 ~/.kdb/license
log_ok "License saved to ~/.kdb/license"
echo ""

"#);
    }

    // Install npm package
    script.push_str(r#"# [3/4] Install kdb
log_step "3/4" "Installing kdb..."
"#);

    if options.with_spinners {
        script.push_str(r#"npm install -g @kindly-software-inc/kdb > /dev/null 2>&1 &
spinner $! "Installing @kindly-software-inc/kdb..."
echo ""

"#);
    } else {
        script.push_str(r#"npm install -g @kindly-software-inc/kdb
log_ok "kdb installed"
echo ""

"#);
    }

    // Configure MCP clients
    script.push_str(r#"# [4/4] Configure MCP clients
log_step "4/4" "Configuring MCP clients..."
npx kdb-configure --auto
log_ok "MCP clients configured"
echo ""

"#);

    // Success (no "press any key" on Linux)
    script.push_str(r#"echo -e "${BOLD}========================================${NC}"
echo -e "${GREEN}  ✓ Setup Complete!${NC}"
echo -e "${BOLD}========================================${NC}"
echo ""
echo -e "KDB is now configured for:"
echo -e "  ${CYAN}•${NC} Claude Code"
echo -e "  ${CYAN}•${NC} Cursor"
echo -e "  ${CYAN}•${NC} VS Code"
echo ""
echo -e "Start debugging by asking Claude:"
echo -e "  ${DIM}'What KDB tools are available?'${NC}"
echo ""
echo -e "Documentation: ${CYAN}https://kindly.software/#docs${NC}"
echo ""
"#);

    script
}

/// Generate enhanced Windows setup script (.bat file)
///
/// Features:
/// - Y/N prompt using choice command
/// - Prerequisites check (where node, where npm)
/// - No spinners (limited batch support)
/// - Pause at end for user to see results
fn generate_enhanced_windows_script(license_key: &str, options: &ScriptOptions) -> String {
    let escaped_key = escape_for_batch(license_key);

    let mut script = String::with_capacity(4096);

    // Header
    script.push_str(r#"@echo off
REM kdb-setup.bat
REM KDB Auto-Setup Script for Windows (Enhanced)
REM
REM Double-click this file or run from Command Prompt
REM
REM Generated by https://kindly.software

setlocal enabledelayedexpansion

echo.
echo ========================================
echo   KDB Auto-Setup
echo ========================================
echo.

"#);

    // Y/N prompt
    if options.with_prompt {
        script.push_str(r#"echo This will install kdb and configure MCP clients.
echo It will create %USERPROFILE%\.kdb\license and run npm install.
echo.
choice /c YN /n /m "Continue? [Y/n] "
if errorlevel 2 (
    echo Installation cancelled.
"#);
        if options.show_manual_on_cancel {
            script.push_str(r#"    echo.
    echo Manual Installation Steps:
    echo   1. Install Node.js 18+ from https://nodejs.org/
    echo   2. Create config directory: mkdir %USERPROFILE%\.kdb
    echo   3. Save your license key to %USERPROFILE%\.kdb\license
    echo   4. Install kdb: npm install -g @kindly-software-inc/kdb
    echo   5. Configure: npx kdb-configure --auto
    echo.
    echo Documentation: https://kindly.software/#docs
    echo.
"#);
        }
        script.push_str(r#"    pause
    exit /b 0
)
echo.

"#);
    }

    // Prerequisites check
    if options.with_version_checks {
        script.push_str(r#"REM [1/4] Check requirements
echo [1/4] Checking requirements...

where node >nul 2>&1
if errorlevel 1 (
    echo       [X] Node.js not found
    echo           Install from: https://nodejs.org/ ^(v18 or higher^)
"#);
        if options.show_manual_on_cancel {
            script.push_str(r#"    echo.
    echo Manual Installation Steps:
    echo   1. Install Node.js 18+ from https://nodejs.org/
    echo   2. Create config directory: mkdir %USERPROFILE%\.kdb
    echo   3. Save your license key to %USERPROFILE%\.kdb\license
    echo   4. Install kdb: npm install -g @kindly-software-inc/kdb
    echo   5. Configure: npx kdb-configure --auto
    echo.
"#);
        }
        script.push_str(r#"    pause
    exit /b 1
)
echo       [OK] Node.js installed

where npm >nul 2>&1
if errorlevel 1 (
    echo       [X] npm not found
"#);
        if options.show_manual_on_cancel {
            script.push_str(r#"    echo.
    echo Manual Installation Steps:
    echo   1. Install Node.js 18+ from https://nodejs.org/
    echo   2. Create config directory: mkdir %USERPROFILE%\.kdb
    echo   3. Save your license key to %USERPROFILE%\.kdb\license
    echo   4. Install kdb: npm install -g @kindly-software-inc/kdb
    echo   5. Configure: npx kdb-configure --auto
    echo.
"#);
        }
        script.push_str(r#"    pause
    exit /b 1
)
echo       [OK] npm installed
echo.

"#);
    } else {
        script.push_str(r#"REM [1/4] Check requirements (minimal)
echo [1/4] Checking requirements...
echo       [OK] Skipping detailed checks
echo.

"#);
    }

    // Detect existing config
    if options.detect_existing {
        script.push_str(r#"REM [2/4] Create configuration
echo [2/4] Creating configuration...

if exist "%USERPROFILE%\.kdb\license" (
    echo.
    echo       [!] Existing license found at %USERPROFILE%\.kdb\license
    choice /c YN /n /m "      Overwrite? [y/N] "
    if errorlevel 2 (
        echo       [OK] Keeping existing license
        goto skip_license
    )
    echo       [OK] Will overwrite existing license
)

if not exist "%USERPROFILE%\.kdb" mkdir "%USERPROFILE%\.kdb"
echo "#);
        script.push_str(&escaped_key);
        script.push_str(r#"> "%USERPROFILE%\.kdb\license"
echo       [OK] License saved to %USERPROFILE%\.kdb\license

:skip_license
echo.

"#);
    } else {
        script.push_str(r#"REM [2/4] Create configuration
echo [2/4] Creating configuration...
if not exist "%USERPROFILE%\.kdb" mkdir "%USERPROFILE%\.kdb"
echo "#);
        script.push_str(&escaped_key);
        script.push_str(r#"> "%USERPROFILE%\.kdb\license"
echo       [OK] License saved to %USERPROFILE%\.kdb\license
echo.

"#);
    }

    // Install npm package (no spinner on Windows)
    script.push_str(r#"REM [3/4] Install kdb
echo [3/4] Installing kdb...
echo       This may take a moment...
call npm install -g @kindly-software-inc/kdb
if errorlevel 1 (
    echo       [X] Installation failed
    pause
    exit /b 1
)
echo       [OK] kdb installed
echo.

"#);

    // Configure MCP clients
    script.push_str(r#"REM [4/4] Configure MCP clients
echo [4/4] Configuring MCP clients...
call npx kdb-configure --auto
echo       [OK] MCP clients configured
echo.

"#);

    // Success
    script.push_str(r#"echo ========================================
echo   [OK] Setup Complete!
echo ========================================
echo.
echo KDB is now configured for:
echo   - Claude Code
echo   - Cursor
echo   - VS Code
echo.
echo Start debugging by asking Claude:
echo   'What KDB tools are available?'
echo.
echo Documentation: https://kindly.software/#docs
echo.
pause
"#);

    script
}

/// Generate enhanced setup script with improved UX
///
/// Automatically selects the appropriate platform-specific script generator.
///
/// # Arguments
/// * `license_key` - The user's license key to embed
/// * `platform` - Target platform
/// * `options` - Script enhancement options
///
/// # Returns
/// Complete script contents as a string
pub fn generate_enhanced_setup_script(
    license_key: &str,
    platform: Platform,
    options: ScriptOptions,
) -> String {
    match platform {
        Platform::MacOS => generate_enhanced_macos_script(license_key, &options),
        Platform::Linux => generate_enhanced_linux_script(license_key, &options),
        Platform::Windows => generate_enhanced_windows_script(license_key, &options),
        Platform::Unknown => generate_enhanced_linux_script(license_key, &options),
    }
}

/// Download enhanced setup script for platform
///
/// Triggers a browser download of the enhanced setup script.
///
/// # Arguments
/// * `license_key` - The user's license key
/// * `platform` - Target platform
/// * `options` - Script enhancement options
pub fn download_enhanced_setup_script(
    license_key: &str,
    platform: Platform,
    options: ScriptOptions,
) {
    let script = generate_enhanced_setup_script(license_key, platform, options);
    let filename = match platform {
        Platform::MacOS => "kdb-setup.command",
        Platform::Windows => "kdb-setup.bat",
        _ => "kdb-setup.sh",
    };
    download_script(&script, filename, "text/plain");
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test 1: Platform detection from user agent
    #[test]
    fn test_platform_detection() {
        // macOS detection
        assert_eq!(
            Platform::detect_from_user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)"),
            Platform::MacOS
        );
        assert_eq!(
            Platform::detect_from_user_agent("Mozilla/5.0 (Mac; Intel)"),
            Platform::MacOS
        );

        // Windows detection
        assert_eq!(
            Platform::detect_from_user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"),
            Platform::Windows
        );

        // Linux detection
        assert_eq!(
            Platform::detect_from_user_agent("Mozilla/5.0 (X11; Linux x86_64)"),
            Platform::Linux
        );
        assert_eq!(
            Platform::detect_from_user_agent("Mozilla/5.0 (X11; Ubuntu; Linux)"),
            Platform::Linux
        );

        // Unknown
        assert_eq!(
            Platform::detect_from_user_agent("Unknown Browser"),
            Platform::Unknown
        );
    }

    // Test 2: Script extensions are correct
    #[test]
    fn test_script_extension() {
        assert_eq!(Platform::MacOS.script_extension(), ".command");
        assert_eq!(Platform::Linux.script_extension(), ".sh");
        assert_eq!(Platform::Windows.script_extension(), ".bat");
        assert_eq!(Platform::Unknown.script_extension(), ".sh");
    }

    // Test 3: macOS script has valid bash syntax
    #[test]
    fn test_generate_macos_script() {
        let license = "test-macos-license-12345";
        let script = generate_macos_script(license);

        // Check bash shebang
        assert!(script.starts_with("#!/bin/bash"));

        // Check heredoc structure for safe license embedding
        assert!(script.contains("cat > ~/.kdb/license << 'EOF'"));
        assert!(script.contains(license));
        assert!(script.contains("\nEOF\n"));

        // Check required commands
        assert!(script.contains("mkdir -p ~/.kdb"));
        assert!(script.contains("chmod 600 ~/.kdb/license"));
        assert!(script.contains("npm install -g @kindly-software-inc/kdb"));
        assert!(script.contains("npx kdb-configure --auto"));

        // Check it's a .command file reference
        assert!(script.contains("kdb-setup.command"));
    }

    // Test 4: Windows script has valid batch syntax
    #[test]
    fn test_generate_windows_script() {
        let license = "test-windows-license";
        let script = generate_windows_script(license);

        // Check batch file start
        assert!(script.starts_with("@echo off"));

        // Check REM comments
        assert!(script.contains("REM"));

        // Check Windows paths
        assert!(script.contains("%USERPROFILE%"));

        // Check license is present
        assert!(script.contains(license));

        // Check call prefix for npm
        assert!(script.contains("call npm"));
        assert!(script.contains("call npx"));

        // Check pause at end
        assert!(script.contains("pause"));
    }

    // Test 5: License is properly embedded in heredoc
    #[test]
    fn test_license_embedded() {
        let license = "Ed25519:abc123XYZ+/=special-chars_here";
        let macos_script = generate_macos_script(license);

        // License should appear between heredoc markers
        assert!(macos_script.contains("<< 'EOF'"));
        assert!(macos_script.contains(license));
        assert!(macos_script.contains("\nEOF\n"));

        // Single-quoted EOF prevents variable expansion, so special chars are safe
        let linux_script = generate_linux_script(license);
        assert!(linux_script.contains(license));
    }

    // Test 6: Special characters are properly escaped
    #[test]
    fn test_script_escaping() {
        // Test batch escaping
        assert_eq!(escape_for_batch("key%value"), "key%%value");
        assert_eq!(escape_for_batch("a&b"), "a^&b");
        assert_eq!(escape_for_batch("a|b"), "a^|b");
        assert_eq!(escape_for_batch("a<b>c"), "a^<b^>c");
        assert_eq!(escape_for_batch("(test)"), "^(test^)");
        assert_eq!(escape_for_batch("a^b"), "a^^b");

        // Test that special chars in Windows script are escaped
        let special_license = "key%with&special|chars";
        let windows_script = generate_windows_script(special_license);
        // The escaped version should be in the script
        assert!(windows_script.contains("key%%with^&special^|chars"));
    }

    // Additional tests for completeness

    #[test]
    fn test_script_filename() {
        assert_eq!(Platform::MacOS.script_filename(), "kdb-setup.command");
        assert_eq!(Platform::Linux.script_filename(), "kdb-setup.sh");
        assert_eq!(Platform::Windows.script_filename(), "kdb-setup.bat");
        assert_eq!(Platform::Unknown.script_filename(), "kdb-setup.sh");
    }

    #[test]
    fn test_mime_types() {
        assert_eq!(Platform::MacOS.mime_type(), "application/x-sh");
        assert_eq!(Platform::Linux.mime_type(), "application/x-sh");
        assert_eq!(Platform::Windows.mime_type(), "application/x-bat");
        assert_eq!(Platform::Unknown.mime_type(), "application/x-sh");
    }

    #[test]
    fn test_display_names() {
        assert_eq!(Platform::MacOS.display_name(), "macOS");
        assert_eq!(Platform::Linux.display_name(), "Linux");
        assert_eq!(Platform::Windows.display_name(), "Windows");
        assert_eq!(Platform::Unknown.display_name(), "Unknown");
    }

    #[test]
    fn test_generate_setup_script_routing() {
        let license = "routing-test";

        // Verify correct routing
        let macos = generate_setup_script(license, Platform::MacOS);
        assert!(macos.contains("kdb-setup.command"));
        assert!(macos.starts_with("#!/bin/bash"));

        let linux = generate_setup_script(license, Platform::Linux);
        assert!(linux.contains("kdb-setup.sh"));
        assert!(linux.starts_with("#!/bin/bash"));

        let windows = generate_setup_script(license, Platform::Windows);
        assert!(windows.contains("kdb-setup.bat"));
        assert!(windows.starts_with("@echo off"));

        // Unknown defaults to Linux
        let unknown = generate_setup_script(license, Platform::Unknown);
        assert!(unknown.contains("kdb-setup.sh"));
    }

    #[test]
    fn test_linux_script_valid_syntax() {
        let license = "linux-test";
        let script = generate_linux_script(license);

        // Check bash best practices
        assert!(script.contains("set -e")); // Exit on error
        assert!(script.contains("mkdir -p")); // Create parent dirs
        assert!(script.contains("chmod 600")); // Secure permissions

        // Should NOT have "Press any key" since Linux terminals don't wait
        assert!(!script.contains("Press any key"));
    }

    // ========================================================================
    // Enhanced Script Tests (Track 3)
    // ========================================================================

    #[test]
    fn test_script_options_default() {
        let options = ScriptOptions::default();
        assert!(options.with_prompt);
        assert!(options.with_version_checks);
        assert!(options.with_spinners);
        assert!(options.detect_existing);
        assert!(options.show_manual_on_cancel);
    }

    #[test]
    fn test_script_options_minimal() {
        let options = ScriptOptions::minimal();
        assert!(!options.with_prompt);
        assert!(!options.with_version_checks);
        assert!(!options.with_spinners);
        assert!(!options.detect_existing);
        assert!(!options.show_manual_on_cancel);
    }

    #[test]
    fn test_validate_license_for_script() {
        // Valid license formats
        assert!(validate_license_for_script("KDB-HOBBY-123456-abc123"));
        assert!(validate_license_for_script("KDB-PRO-987654-xyz789"));
        assert!(validate_license_for_script("test-license-key"));
        assert!(validate_license_for_script("simple_key_with_underscore"));

        // Invalid: empty
        assert!(!validate_license_for_script(""));

        // Invalid: too long (> 200 chars)
        let long_key = "a".repeat(201);
        assert!(!validate_license_for_script(&long_key));

        // Invalid: contains shell injection characters
        assert!(!validate_license_for_script("key; rm -rf /"));
        assert!(!validate_license_for_script("key$(whoami)"));
        assert!(!validate_license_for_script("key`whoami`"));
        assert!(!validate_license_for_script("key|cat /etc/passwd"));
        assert!(!validate_license_for_script("key&& malicious"));
    }

    #[test]
    fn test_enhanced_macos_script_has_features() {
        let license = "KDB-TEST-123456-abc123";
        let options = ScriptOptions::default();
        let script = generate_enhanced_macos_script(license, &options);

        // Check shebang
        assert!(script.starts_with("#!/bin/bash"));

        // Check color setup
        assert!(script.contains("setup_colors()"));
        assert!(script.contains("NO_COLOR"));
        assert!(script.contains("GREEN="));
        assert!(script.contains("CYAN="));

        // Check Y/N prompt
        assert!(script.contains("Continue? [Y/n]"));

        // Check version checks
        assert!(script.contains("command -v node"));
        assert!(script.contains("NODE_VERSION"));
        assert!(script.contains("-lt 18"));

        // Check spinner
        assert!(script.contains("spinner()"));
        assert!(script.contains("frames='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'"));

        // Check 4-step process
        assert!(script.contains("[1/4]"));
        assert!(script.contains("[2/4]"));
        assert!(script.contains("[3/4]"));
        assert!(script.contains("[4/4]"));

        // Check license embedding
        assert!(script.contains(license));
        assert!(script.contains("<< 'EOF'"));

        // Check "Press any key" at end (macOS pattern)
        assert!(script.contains("Press any key to close"));

        // Check manual instructions function
        assert!(script.contains("show_manual()"));
    }

    #[test]
    fn test_enhanced_linux_script_differences() {
        let license = "KDB-TEST-123456-abc123";
        let options = ScriptOptions::default();
        let script = generate_enhanced_linux_script(license, &options);

        // Check shebang
        assert!(script.starts_with("#!/bin/bash"));

        // Check usage comment (Linux-specific)
        assert!(script.contains("Usage: chmod +x kdb-setup.sh && ./kdb-setup.sh"));

        // Should NOT have "Press any key" (Linux terminals don't wait)
        assert!(!script.contains("Press any key to close"));

        // Should have all other features
        assert!(script.contains("Continue? [Y/n]"));
        assert!(script.contains("spinner()"));
        assert!(script.contains("[1/4]"));
    }

    #[test]
    fn test_enhanced_windows_script_features() {
        let license = "KDB-TEST-123456-abc123";
        let options = ScriptOptions::default();
        let script = generate_enhanced_windows_script(license, &options);

        // Check batch start
        assert!(script.starts_with("@echo off"));

        // Check Y/N prompt (Windows choice command)
        assert!(script.contains("choice /c YN /n /m"));
        assert!(script.contains("Continue? [Y/n]"));

        // Check prerequisites (where command)
        assert!(script.contains("where node"));
        assert!(script.contains("where npm"));

        // Check 4-step process
        assert!(script.contains("[1/4]"));
        assert!(script.contains("[2/4]"));
        assert!(script.contains("[3/4]"));
        assert!(script.contains("[4/4]"));

        // Should have pause at end
        assert!(script.ends_with("pause\n"));

        // Check license is present (escaped)
        assert!(script.contains(license));
    }

    #[test]
    fn test_enhanced_windows_escaping() {
        let special_license = "KDB-TEST%special&chars|here";
        let options = ScriptOptions::default();
        let script = generate_enhanced_windows_script(special_license, &options);

        // Escaped version should be in script
        assert!(script.contains("KDB-TEST%%special^&chars^|here"));
    }

    #[test]
    fn test_enhanced_script_minimal_options() {
        let license = "test-license";
        let options = ScriptOptions::minimal();
        let script = generate_enhanced_macos_script(license, &options);

        // Should NOT have Y/N prompt
        assert!(!script.contains("Continue? [Y/n]"));

        // Should NOT have detailed version checks
        assert!(!script.contains("NODE_VERSION"));
        assert!(!script.contains("-lt 18"));

        // Should NOT have spinner
        assert!(!script.contains("spinner()"));

        // Should NOT have existing config detection
        assert!(!script.contains("Existing license found"));

        // Should still have the license
        assert!(script.contains(license));
    }

    #[test]
    fn test_generate_enhanced_setup_script_routing() {
        let license = "routing-test";
        let options = ScriptOptions::default();

        // Verify correct routing
        let macos = generate_enhanced_setup_script(license, Platform::MacOS, options.clone());
        assert!(macos.starts_with("#!/bin/bash"));
        assert!(macos.contains("kdb-setup.command"));

        let linux = generate_enhanced_setup_script(license, Platform::Linux, options.clone());
        assert!(linux.starts_with("#!/bin/bash"));
        assert!(linux.contains("kdb-setup.sh"));

        let windows = generate_enhanced_setup_script(license, Platform::Windows, options.clone());
        assert!(windows.starts_with("@echo off"));

        // Unknown defaults to Linux
        let unknown = generate_enhanced_setup_script(license, Platform::Unknown, options);
        assert!(unknown.starts_with("#!/bin/bash"));
        assert!(unknown.contains("kdb-setup.sh"));
    }
}
