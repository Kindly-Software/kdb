//! kdb_handler - Protocol Handler for kdb:// URLs
//!
//! Opens terminal with setup command for user confirmation.
//! Handles `kdb://setup?license=XXX` URLs from browser registration.
//!
//! ## Features
//! - Validates license format (prevents shell injection)
//! - Opens platform-specific terminal
//! - Shows Y/N confirmation prompt
//! - Supports macOS, Linux, Windows
//!
//! ## Security
//! - License format validation: KDB-{TIER}-{timestamp}-{hash}
//! - Only alphanumeric + hyphens allowed
//! - Action whitelist (setup, configure, attach, debug)
//! - Q34 audit logging of handler invocations
//!
//! ## Architecture
//! - T1 Atomic (validation)
//! - T0 Auditable (logging)
//!
//! ## UCE35 Compliance
//! - Q10: T1 Atomic tier for fast validation
//! - Q33: 100% safe Rust, no unsafe
//! - Q34: Action logging for audit trail

use std::collections::HashMap;
use std::env;
use std::process::{Command, Stdio};

// Phase 4: Protection capsules (conditionally compiled)
#[cfg(feature = "client-protection")]
use kdb_mcp::client::{
    P0ProtectionLayer, ProtectionError,
    SelfDestructHandler, TamperReason,
};

// ============================================================================
// Version and Metadata
// ============================================================================

const VERSION: &str = "1.0.0";

// ============================================================================
// Allowed Actions (Whitelist)
// ============================================================================

/// Whitelist of allowed actions for security
const ALLOWED_ACTIONS: &[&str] = &["setup", "configure", "attach", "debug"];

/// Valid license tiers
const VALID_TIERS: &[&str] = &["HOBBY", "PRO", "ENGINEER", "TEAMS", "ENTERPRISE", "PROMO"];

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug)]
enum HandlerError {
    MissingUrl,
    InvalidUrl(String),
    InvalidAction(String),
    InvalidLicense(String),
    MissingLicense,
    InjectionAttempt(String),
    TerminalLaunchFailed(String),
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandlerError::MissingUrl => write!(f, "Missing URL argument"),
            HandlerError::InvalidUrl(msg) => write!(f, "Invalid URL: {}", msg),
            HandlerError::InvalidAction(action) => write!(f, "Invalid action: {}", action),
            HandlerError::InvalidLicense(msg) => write!(f, "Invalid license: {}", msg),
            HandlerError::MissingLicense => write!(f, "Missing license parameter"),
            HandlerError::InjectionAttempt(msg) => write!(f, "Potential injection attempt: {}", msg),
            HandlerError::TerminalLaunchFailed(msg) => write!(f, "Failed to launch terminal: {}", msg),
        }
    }
}

impl std::error::Error for HandlerError {}

// ============================================================================
// Parsed URL Structure
// ============================================================================

/// Parsed kdb:// URL
#[derive(Debug)]
struct KdbUrl {
    action: String,
    license: String,
    params: HashMap<String, String>,
}

// ============================================================================
// URL Parsing and Validation
// ============================================================================

/// Parse and validate a kdb:// URL
///
/// Format: kdb://action?license=XXX&other=params
///
/// # Security
/// - Validates action against whitelist
/// - Validates license format: KDB-{TIER}-{timestamp}-{hash}
/// - Only allows alphanumeric + hyphens (prevents shell injection)
fn parse_kdb_url(url: &str) -> Result<KdbUrl, HandlerError> {
    // Step 1: Strip kdb:// prefix (case-insensitive)
    let url_lower = url.to_lowercase();
    let path = if url_lower.starts_with("kdb://") {
        &url[6..]
    } else if url_lower.starts_with("kdb:") {
        &url[4..]
    } else {
        return Err(HandlerError::InvalidUrl("URL must start with kdb://".to_string()));
    };

    // Step 2: Split action and query string
    let (action, query) = if let Some(idx) = path.find('?') {
        (&path[..idx], &path[idx + 1..])
    } else {
        (path, "")
    };

    // Step 3: Validate action against whitelist
    let action = action.to_lowercase();
    if !ALLOWED_ACTIONS.contains(&action.as_str()) {
        return Err(HandlerError::InvalidAction(action));
    }

    // Step 4: Parse query parameters
    let mut params = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        if let Some((key, value)) = pair.split_once('=') {
            // URL decode
            let decoded_value = url_decode(value);
            params.insert(key.to_lowercase(), decoded_value);
        }
    }

    // Step 5: Extract and validate license
    let license = params.get("license")
        .cloned()
        .ok_or(HandlerError::MissingLicense)?;

    validate_license_format(&license)?;

    Ok(KdbUrl {
        action,
        license,
        params,
    })
}

/// Validate license format: KDB-{TIER}-{timestamp}-{hash}
///
/// # Security
/// - Only allows alphanumeric characters and hyphens
/// - Must start with "KDB-"
/// - Must have valid tier
/// - Prevents shell injection by rejecting special characters
fn validate_license_format(license: &str) -> Result<(), HandlerError> {
    // Check for potential injection characters FIRST
    let dangerous_chars = ['`', '$', ';', '|', '&', '<', '>', '(', ')', '{', '}', '[', ']', '"', '\'', '\\', '\n', '\r', '\0'];
    for c in license.chars() {
        if dangerous_chars.contains(&c) {
            return Err(HandlerError::InjectionAttempt(format!(
                "License contains dangerous character: {:?}",
                c
            )));
        }
    }

    // Only allow alphanumeric and hyphens
    for c in license.chars() {
        if !c.is_ascii_alphanumeric() && c != '-' {
            return Err(HandlerError::InvalidLicense(format!(
                "Invalid character '{}' - only alphanumeric and hyphens allowed",
                c
            )));
        }
    }

    // Must start with KDB-
    if !license.starts_with("KDB-") {
        return Err(HandlerError::InvalidLicense(
            "License must start with 'KDB-'".to_string()
        ));
    }

    // Parse format: KDB-{TIER}-{timestamp}-{hash}
    let parts: Vec<&str> = license.split('-').collect();
    if parts.len() < 4 {
        return Err(HandlerError::InvalidLicense(
            "Invalid license format: expected KDB-TIER-TIMESTAMP-HASH".to_string()
        ));
    }

    // Validate tier
    let tier = parts[1].to_uppercase();
    if !VALID_TIERS.contains(&tier.as_str()) {
        return Err(HandlerError::InvalidLicense(format!(
            "Invalid tier '{}': expected one of {:?}",
            tier, VALID_TIERS
        )));
    }

    // Validate timestamp (should be numeric, 10 digits for Unix timestamp)
    let timestamp = parts[2];
    if !timestamp.chars().all(|c| c.is_ascii_digit()) {
        return Err(HandlerError::InvalidLicense(
            "Invalid timestamp: must be numeric".to_string()
        ));
    }

    // Validate hash (should be alphanumeric, typically 8+ chars)
    let hash = parts[3..].join("-"); // Rejoin in case hash contains hyphens
    if hash.len() < 4 {
        return Err(HandlerError::InvalidLicense(
            "Invalid hash: too short".to_string()
        ));
    }

    Ok(())
}

/// Simple URL decoding (handles %XX sequences)
fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            result.push('%');
            result.push_str(&hex);
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}

// ============================================================================
// Setup Command Generation
// ============================================================================

/// Generate platform-specific setup command with Y/N confirmation
fn generate_setup_command(license: &str) -> String {
    // Truncate license for display (first 8 + last 4 chars)
    let display_license = if license.len() > 16 {
        format!("{}...{}", &license[..8], &license[license.len() - 4..])
    } else {
        license.to_string()
    };

    #[cfg(target_os = "windows")]
    {
        // Windows batch command
        format!(
            r#"@echo off
echo ==========================================
echo    KDB Auto-Setup
echo ==========================================
echo.
echo License: {}
echo.
echo This will:
echo   1. Create ~/.kdb/license file
echo   2. Install @kindly-software-inc/kdb
echo   3. Configure MCP clients automatically
echo.
choice /C YN /M "Continue with setup"
if errorlevel 2 goto :cancel
if errorlevel 1 goto :setup
:setup
echo.
echo [1/3] Creating license file...
if not exist "%USERPROFILE%\.kdb" mkdir "%USERPROFILE%\.kdb"
echo {}> "%USERPROFILE%\.kdb\license"
echo [2/3] Installing kdb...
call npm install -g @kindly-software-inc/kdb
echo [3/3] Configuring MCP clients...
call npx kdb-configure --auto
echo.
echo ==========================================
echo    Setup complete!
echo ==========================================
pause
goto :eof
:cancel
echo.
echo Setup cancelled. To set up manually:
echo   npm install -g @kindly-software-inc/kdb
echo   npx kdb-configure --license "{}"
pause
"#,
            display_license, license, license
        )
    }

    #[cfg(not(target_os = "windows"))]
    {
        // macOS/Linux bash command
        format!(
            r#"#!/bin/bash
set -e

# Colors (respects NO_COLOR)
if [ -t 1 ] && [ -z "${{NO_COLOR:-}}" ]; then
    GREEN='\033[0;32m'
    CYAN='\033[0;36m'
    YELLOW='\033[1;33m'
    RED='\033[0;31m'
    BOLD='\033[1m'
    NC='\033[0m'
else
    GREEN='' CYAN='' YELLOW='' RED='' BOLD='' NC=''
fi

echo ""
echo "${{BOLD}}===========================================${{NC}}"
echo "${{BOLD}}   KDB Auto-Setup${{NC}}"
echo "==========================================="
echo ""
echo "License: ${{CYAN}}{}${{NC}}"
echo ""
echo "This will:"
echo "  1. Create ~/.kdb/license file"
echo "  2. Install @kindly-software-inc/kdb"
echo "  3. Configure MCP clients automatically"
echo ""
read -p "Continue with setup? [Y/n] " choice
case "$choice" in
    n|N )
        echo ""
        echo "${{YELLOW}}Setup cancelled.${{NC}}"
        echo "To set up manually:"
        echo "  npm install -g @kindly-software-inc/kdb"
        echo "  npx kdb-configure --license \"{}\""
        exit 0
        ;;
esac

echo ""
echo "${{CYAN}}[1/3]${{NC}} Creating license file..."
mkdir -p ~/.kdb
echo "{}" > ~/.kdb/license
echo "${{GREEN}}[OK]${{NC}} License saved to ~/.kdb/license"

echo ""
echo "${{CYAN}}[2/3]${{NC}} Installing kdb..."
npm install -g @kindly-software-inc/kdb
echo "${{GREEN}}[OK]${{NC}} kdb installed"

echo ""
echo "${{CYAN}}[3/3]${{NC}} Configuring MCP clients..."
npx kdb-configure --auto
echo "${{GREEN}}[OK]${{NC}} MCP clients configured"

echo ""
echo "${{GREEN}}===========================================${{NC}}"
echo "${{GREEN}}   Setup complete!${{NC}}"
echo "${{GREEN}}===========================================${{NC}}"
echo ""
echo "Next steps:"
echo "  - Restart your AI assistant (Claude Code, Cursor, etc.)"
echo "  - Type @kdb to start debugging"
echo ""
"#,
            display_license, license, license
        )
    }
}

// ============================================================================
// Terminal Launch (Platform-Specific)
// ============================================================================

/// Open terminal with the setup command
#[cfg(target_os = "macos")]
fn open_terminal_with_command(command: &str) -> Result<(), HandlerError> {
    // Use osascript to open Terminal.app with command
    let script = format!(
        r#"tell application "Terminal"
    activate
    do script "{}"
end tell"#,
        command.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
    );

    // Write command to temp file first for complex scripts
    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join("kdb_setup.sh");

    if let Err(e) = std::fs::write(&script_path, command) {
        return Err(HandlerError::TerminalLaunchFailed(format!(
            "Failed to write temp script: {}",
            e
        )));
    }

    // Make executable
    let _ = Command::new("chmod")
        .args(["+x", script_path.to_str().unwrap_or("")])
        .output();

    // Use osascript to open Terminal and run the script
    let applescript = format!(
        r#"tell application "Terminal"
    activate
    do script "{}"
end tell"#,
        script_path.to_str().unwrap_or("").replace('"', "\\\"")
    );

    let output = Command::new("osascript")
        .args(["-e", &applescript])
        .output()
        .map_err(|e| HandlerError::TerminalLaunchFailed(format!("osascript failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(HandlerError::TerminalLaunchFailed(format!(
            "osascript error: {}",
            stderr
        )));
    }

    Ok(())
}

/// Open terminal with the setup command (Linux)
#[cfg(target_os = "linux")]
fn open_terminal_with_command(command: &str) -> Result<(), HandlerError> {
    // Write command to temp file
    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join("kdb_setup.sh");

    if let Err(e) = std::fs::write(&script_path, command) {
        return Err(HandlerError::TerminalLaunchFailed(format!(
            "Failed to write temp script: {}",
            e
        )));
    }

    // Make executable
    let _ = Command::new("chmod")
        .args(["+x", script_path.to_str().unwrap_or("")])
        .output();

    let script_str = script_path.to_str().unwrap_or("");

    // Try terminals in order of preference
    let terminals: Vec<(&str, Vec<&str>)> = vec![
        ("gnome-terminal", vec!["--", script_str]),
        ("konsole", vec!["-e", script_str]),
        ("xfce4-terminal", vec!["-e", script_str]),
        ("mate-terminal", vec!["-e", script_str]),
        ("terminator", vec!["-e", script_str]),
        ("kitty", vec![script_str]),
        ("alacritty", vec!["-e", script_str]),
        ("xterm", vec!["-e", script_str]),
    ];

    for (terminal, args) in terminals {
        // Check if terminal exists
        if Command::new("which")
            .arg(terminal)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            let result = Command::new(terminal)
                .args(&args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();

            if result.is_ok() {
                return Ok(());
            }
        }
    }

    Err(HandlerError::TerminalLaunchFailed(
        "No supported terminal found. Tried: gnome-terminal, konsole, xfce4-terminal, xterm".to_string()
    ))
}

/// Open terminal with the setup command (Windows)
#[cfg(target_os = "windows")]
fn open_terminal_with_command(command: &str) -> Result<(), HandlerError> {
    // Write command to temp batch file
    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join("kdb_setup.bat");

    if let Err(e) = std::fs::write(&script_path, command) {
        return Err(HandlerError::TerminalLaunchFailed(format!(
            "Failed to write temp script: {}",
            e
        )));
    }

    // Use cmd.exe /k to keep window open
    let result = Command::new("cmd")
        .args(["/k", script_path.to_str().unwrap_or("")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(HandlerError::TerminalLaunchFailed(format!(
            "Failed to launch cmd.exe: {}",
            e
        ))),
    }
}

/// Fallback for unsupported platforms
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn open_terminal_with_command(_command: &str) -> Result<(), HandlerError> {
    Err(HandlerError::TerminalLaunchFailed(
        "Unsupported platform. Please run the setup command manually.".to_string()
    ))
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() {
    let args: Vec<String> = env::args().collect();

    // Phase 4: P0 Protection Layer (Anti-Debug, Emulator Detection, License Validation, Self-Destruct)
    // CRITICAL: Protects trade secret URL parsing, terminal launch patterns, license validation
    #[cfg(feature = "client-protection")]
    let license_key_for_protection = env::var("KDB_LICENSE_KEY").unwrap_or_default();

    #[cfg(feature = "client-protection")]
    let protection = P0ProtectionLayer::new(&license_key_for_protection);

    #[cfg(feature = "client-protection")]
    let self_destruct = SelfDestructHandler::new();

    #[cfg(feature = "client-protection")]
    {
        match protection.check_all() {
            Ok(()) => {
                // Protection passed - proceed with handler
            }
            Err(e) => {
                // CRITICAL: Tamper detected - self-destruct immediately
                eprintln!("[kdb-handler] Protection failure: {}", e);

                let tamper_reason = match e {
                    ProtectionError::DebuggerDetected => TamperReason::DebuggerAttached,
                    ProtectionError::EmulatorDetected => TamperReason::EmulatorDetected,
                    ProtectionError::LicenseInvalid => TamperReason::IntegrityViolation,
                    ProtectionError::TamperDetected => TamperReason::IntegrityViolation,
                };

                // Trigger self-destruct (this does NOT return - process exits)
                self_destruct.trigger(tamper_reason);
                std::process::exit(137); // SIGKILL simulation (backup, should never reach)
            }
        }
    }

    // Check for --version
    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("kdb_handler {}", VERSION);
        return;
    }

    // Check for --help
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    // Get URL from command line (passed by OS protocol handler)
    let url = match args.get(1) {
        Some(u) => u,
        None => {
            eprintln!("Error: {}", HandlerError::MissingUrl);
            eprintln!("Usage: kdb_handler <kdb://action?license=XXX>");
            std::process::exit(1);
        }
    };

    // Parse and validate URL
    let parsed = match parse_kdb_url(url) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Expected format: kdb://setup?license=KDB-TIER-TIMESTAMP-HASH");
            std::process::exit(1);
        }
    };

    // Log action (Q34 audit)
    log_action(&parsed);

    // Handle action
    match parsed.action.as_str() {
        "setup" | "configure" => {
            let command = generate_setup_command(&parsed.license);
            if let Err(e) = open_terminal_with_command(&command) {
                eprintln!("Error: {}", e);
                eprintln!("\nTo set up manually, run:");
                eprintln!("  npm install -g @kindly-software-inc/kdb");
                eprintln!("  npx kdb-configure --auto --license \"{}\"", mask_license(&parsed.license));
                std::process::exit(1);
            }
        }
        "attach" | "debug" => {
            eprintln!("Action '{}' requires a running MCP client.", parsed.action);
            eprintln!("Please use kdb://setup to configure your MCP client first.");
            std::process::exit(1);
        }
        _ => {
            eprintln!("Unknown action: {}", parsed.action);
            std::process::exit(1);
        }
    }
}

/// Log action for audit trail (Q34)
fn log_action(parsed: &KdbUrl) {
    // In production, this would write to audit log
    // For now, just print to stderr for debugging
    if env::var("KDB_DEBUG").is_ok() {
        let masked = mask_license(&parsed.license);
        eprintln!(
            "[kdb_handler] action={} license={} params={:?}",
            parsed.action, masked, parsed.params.keys().collect::<Vec<_>>()
        );
    }
}

/// Mask license key for logging (show first 8 + last 4 chars)
fn mask_license(license: &str) -> String {
    if license.len() <= 12 {
        "*".repeat(license.len())
    } else {
        format!("{}...{}", &license[..8], &license[license.len() - 4..])
    }
}

/// Print help message
fn print_help() {
    println!(
        r#"kdb_handler {} - Protocol Handler for kdb:// URLs

USAGE:
    kdb_handler <url>

ARGUMENTS:
    <url>    A kdb:// URL (e.g., kdb://setup?license=KDB-HOBBY-1234-abcd)

OPTIONS:
    -h, --help       Show this help message
    -v, --version    Show version

SUPPORTED ACTIONS:
    setup       Install kdb and configure MCP clients
    configure   Same as setup
    attach      (Coming soon) Attach to a process
    debug       (Coming soon) Start debug session

SECURITY:
    - Only allows alphanumeric characters and hyphens in license
    - Validates license format (KDB-TIER-TIMESTAMP-HASH)
    - Whitelist-based action validation
    - Opens terminal with Y/N confirmation before any changes

EXAMPLES:
    kdb_handler "kdb://setup?license=KDB-HOBBY-1234567890-abcdef12"
    kdb_handler "kdb://configure?license=KDB-PRO-1234567890-xyz"

For more information, visit: https://kindly.software/docs
"#,
        VERSION
    );
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // URL Parsing Tests
    // ========================================================================

    #[test]
    fn test_parse_kdb_url_valid() {
        let url = "kdb://setup?license=KDB-HOBBY-1234567890-abcdef12";
        let result = parse_kdb_url(url);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.action, "setup");
        assert_eq!(parsed.license, "KDB-HOBBY-1234567890-abcdef12");
    }

    #[test]
    fn test_parse_kdb_url_case_insensitive() {
        let url = "KDB://SETUP?license=KDB-HOBBY-1234567890-abcdef12";
        let result = parse_kdb_url(url);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.action, "setup");
    }

    #[test]
    fn test_parse_kdb_url_configure_action() {
        let url = "kdb://configure?license=KDB-PRO-9876543210-xyz123";
        let result = parse_kdb_url(url);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.action, "configure");
    }

    #[test]
    fn test_parse_kdb_url_missing_license() {
        let url = "kdb://setup";
        let result = parse_kdb_url(url);
        assert!(matches!(result, Err(HandlerError::MissingLicense)));
    }

    #[test]
    fn test_parse_kdb_url_invalid_action() {
        let url = "kdb://delete?license=KDB-HOBBY-1234567890-abcdef12";
        let result = parse_kdb_url(url);
        assert!(matches!(result, Err(HandlerError::InvalidAction(_))));
    }

    #[test]
    fn test_parse_kdb_url_invalid_prefix() {
        let url = "https://setup?license=KDB-HOBBY-1234567890-abcdef12";
        let result = parse_kdb_url(url);
        assert!(matches!(result, Err(HandlerError::InvalidUrl(_))));
    }

    // ========================================================================
    // License Validation Tests
    // ========================================================================

    #[test]
    fn test_validate_license_valid_hobby() {
        let result = validate_license_format("KDB-HOBBY-1234567890-abcdef12");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_license_valid_pro() {
        let result = validate_license_format("KDB-PRO-9876543210-xyz123ab");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_license_valid_engineer() {
        let result = validate_license_format("KDB-ENGINEER-1111111111-test1234");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_license_valid_teams() {
        let result = validate_license_format("KDB-TEAMS-2222222222-team1234");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_license_valid_enterprise() {
        let result = validate_license_format("KDB-ENTERPRISE-3333333333-ent12345");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_license_invalid_tier() {
        let result = validate_license_format("KDB-INVALID-1234567890-abcdef12");
        assert!(matches!(result, Err(HandlerError::InvalidLicense(_))));
    }

    #[test]
    fn test_validate_license_missing_prefix() {
        let result = validate_license_format("HOBBY-1234567890-abcdef12");
        assert!(matches!(result, Err(HandlerError::InvalidLicense(_))));
    }

    #[test]
    fn test_validate_license_too_short() {
        let result = validate_license_format("KDB-HOBBY-123");
        assert!(matches!(result, Err(HandlerError::InvalidLicense(_))));
    }

    // ========================================================================
    // Injection Prevention Tests (CRITICAL)
    // ========================================================================

    #[test]
    fn test_injection_semicolon() {
        let result = validate_license_format("KDB-HOBBY-1234567890-abc; rm -rf /");
        assert!(matches!(result, Err(HandlerError::InjectionAttempt(_))));
    }

    #[test]
    fn test_injection_backtick() {
        let result = validate_license_format("KDB-HOBBY-1234567890-`whoami`");
        assert!(matches!(result, Err(HandlerError::InjectionAttempt(_))));
    }

    #[test]
    fn test_injection_dollar() {
        let result = validate_license_format("KDB-HOBBY-1234567890-$(cat /etc/passwd)");
        assert!(matches!(result, Err(HandlerError::InjectionAttempt(_))));
    }

    #[test]
    fn test_injection_pipe() {
        let result = validate_license_format("KDB-HOBBY-1234567890-abc|cat");
        assert!(matches!(result, Err(HandlerError::InjectionAttempt(_))));
    }

    #[test]
    fn test_injection_ampersand() {
        let result = validate_license_format("KDB-HOBBY-1234567890-abc&echo pwned");
        assert!(matches!(result, Err(HandlerError::InjectionAttempt(_))));
    }

    #[test]
    fn test_injection_redirect() {
        let result = validate_license_format("KDB-HOBBY-1234567890-abc>file");
        assert!(matches!(result, Err(HandlerError::InjectionAttempt(_))));
    }

    #[test]
    fn test_injection_quotes() {
        let result = validate_license_format("KDB-HOBBY-1234567890-abc\"test");
        assert!(matches!(result, Err(HandlerError::InjectionAttempt(_))));
    }

    #[test]
    fn test_injection_newline() {
        let result = validate_license_format("KDB-HOBBY-1234567890-abc\nrm -rf /");
        assert!(matches!(result, Err(HandlerError::InjectionAttempt(_))));
    }

    #[test]
    fn test_injection_null() {
        let result = validate_license_format("KDB-HOBBY-1234567890-abc\0test");
        assert!(matches!(result, Err(HandlerError::InjectionAttempt(_))));
    }

    #[test]
    fn test_injection_braces() {
        let result = validate_license_format("KDB-HOBBY-1234567890-{test}");
        assert!(matches!(result, Err(HandlerError::InjectionAttempt(_))));
    }

    // ========================================================================
    // URL Decoding Tests
    // ========================================================================

    #[test]
    fn test_url_decode_simple() {
        assert_eq!(url_decode("hello"), "hello");
    }

    #[test]
    fn test_url_decode_space() {
        assert_eq!(url_decode("hello+world"), "hello world");
    }

    #[test]
    fn test_url_decode_percent() {
        assert_eq!(url_decode("hello%20world"), "hello world");
    }

    #[test]
    fn test_url_decode_special() {
        assert_eq!(url_decode("%3D"), "=");
        assert_eq!(url_decode("%26"), "&");
    }

    // ========================================================================
    // License Masking Tests
    // ========================================================================

    #[test]
    fn test_mask_license_long() {
        let masked = mask_license("KDB-HOBBY-1234567890-abcdef12");
        assert_eq!(masked, "KDB-HOBB...ef12");
    }

    #[test]
    fn test_mask_license_short() {
        let masked = mask_license("short");
        assert_eq!(masked, "*****");
    }

    // ========================================================================
    // Command Generation Tests
    // ========================================================================

    #[test]
    fn test_generate_setup_command_contains_license() {
        let command = generate_setup_command("KDB-HOBBY-1234567890-abcdef12");
        // Should contain the full license for the actual command
        assert!(command.contains("KDB-HOBBY-1234567890-abcdef12"));
    }

    #[test]
    fn test_generate_setup_command_contains_confirmation() {
        let command = generate_setup_command("KDB-HOBBY-1234567890-abcdef12");
        // Should have a confirmation prompt
        assert!(command.to_lowercase().contains("continue") || command.to_lowercase().contains("choice"));
    }

    #[test]
    fn test_generate_setup_command_contains_npm_install() {
        let command = generate_setup_command("KDB-HOBBY-1234567890-abcdef12");
        // Should contain npm install command
        assert!(command.contains("npm install"));
    }

    #[test]
    fn test_generate_setup_command_contains_kdb_configure() {
        let command = generate_setup_command("KDB-HOBBY-1234567890-abcdef12");
        // Should contain kdb-configure command
        assert!(command.contains("kdb-configure"));
    }
}
