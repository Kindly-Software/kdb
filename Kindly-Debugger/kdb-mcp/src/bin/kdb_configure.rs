//! kdb-configure - Universal MCP Auto-Configuration CLI
//!
//! Command-line interface for auto-configuring MCP clients to use kdb (Kindly Debugger).
//!
//! ## Features
//! - Auto-detection of installed MCP clients (Claude Code, Cursor, VSCode, etc.)
//! - Safe configuration merging with backup and rollback
//! - Dry-run mode for preview without changes
//! - Environment variable and dotenv resolution
//!
//! ## Usage
//! ```bash
//! kdb-configure --detect          # List detected clients
//! kdb-configure --dry-run         # Preview changes
//! kdb-configure --auto            # Auto-approve all prompts
//! kdb-configure --clients=claude_code,cursor  # Specific clients only
//! ```
//!
//! ## Architecture
//! - **T6 Mixed Orchestrator**: Multi-stage configuration pipeline
//! - **T1 Atomic Detectors**: Client detection capsules
//! - **T0 Auditable**: Config generation with Q34 audit trail
//!
//! ## UCE35 Compliance
//! - Q10: T6 Mixed tier for orchestration
//! - Q33: 100% lockfree operations
//! - Q34: Backup hash verification for audit compliance

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use kdb_mcp::configure::{
    // Platform detection
    PlatformDetectorCapsule,
    PlatformInfo,
    get_kdb_data_dir,
    // Environment resolution
    EnvResolutionCapsule,
    // Detector registry
    DetectorRegistryCapsule,
    DetectedClient,
    // Config merger
    ConfigMergerCapsule,
    KdbConfig,
    // Built-in detectors
    ClaudeCodeDetector,
    ClaudeDesktopDetector,
    CURSOR_DETECTOR,
    VSCODE_DETECTOR,
    GENERIC_HTTP_DETECTOR,
};

// ============================================================================
// Version and Banner
// ============================================================================

const VERSION: &str = "2.1.0";
const BANNER: &str = r#"
  _    _ _                         __ _
 | |  | | |                       / _(_)
 | | _| | |__         ___ ___  _ | |_ _  __ _ _   _ _ __ ___
 | |/ / | '_ \ _____ / __/ _ \| '  _| |/ _` | | | | '__/ _ \
 |   <| | |_) |_____| (_| (_) | | | | | (_| | |_| | | |  __/
 |_|\_\_|_.__/       \___\___/|_|_| |_|\__, |\__,_|_|  \___|
                                        __/ |
                                       |___/
"#;

// ============================================================================
// CLI Options
// ============================================================================

/// Parsed CLI options
#[derive(Debug, Default)]
struct CliOptions {
    /// Auto-approve all prompts (no interactive confirmation)
    auto_approve: bool,
    /// Overwrite existing kdb configs without backup
    force_overwrite: bool,
    /// Show changes without applying
    dry_run: bool,
    /// Detection only mode (don't modify anything)
    detect_only: bool,
    /// Specific clients to configure (None = all detected)
    specific_clients: Option<Vec<String>>,
    /// Show help message
    show_help: bool,
    /// Show version
    show_version: bool,
    /// List available backups
    list_backups: bool,
    /// Rollback to specific backup
    rollback: Option<String>,
    /// Verbose output
    verbose: bool,
}

// ============================================================================
// Static Detector Instances
// ============================================================================

// Claude Code and Claude Desktop need static instances for registration
static CLAUDE_CODE_DETECTOR: ClaudeCodeDetector = ClaudeCodeDetector;
static CLAUDE_DESKTOP_DETECTOR: ClaudeDesktopDetector = ClaudeDesktopDetector;

// ============================================================================
// ANSI Colors (if terminal supports)
// ============================================================================

mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const CYAN: &str = "\x1b[36m";
    #[allow(dead_code)]
    pub const WHITE: &str = "\x1b[37m";
}

/// Check if terminal supports colors
fn supports_color() -> bool {
    // Check NO_COLOR environment variable (standard)
    if env::var("NO_COLOR").is_ok() {
        return false;
    }
    // Check TERM
    if let Ok(term) = env::var("TERM") {
        return term != "dumb";
    }
    // Default to color on TTY
    atty_isatty()
}

/// Simple TTY check without external dependency
fn atty_isatty() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::isatty(libc::STDOUT_FILENO) != 0 }
    }
    #[cfg(not(unix))]
    {
        true // Default to true on non-Unix
    }
}

/// Color wrapper that respects NO_COLOR
fn color(code: &str, text: &str) -> String {
    if supports_color() {
        format!("{}{}{}", code, text, colors::RESET)
    } else {
        text.to_string()
    }
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() {
    let args: Vec<String> = env::args().collect();
    let options = parse_args(&args);

    // Handle special commands first
    if options.show_help {
        print_help();
        return;
    }

    if options.show_version {
        println!("kdb-configure {}", VERSION);
        return;
    }

    // Show banner
    if !options.detect_only && supports_color() {
        println!("{}{}{}", colors::CYAN, BANNER, colors::RESET);
    }
    println!(
        "{}kdb Auto-Configuration v{}{}",
        colors::BOLD,
        VERSION,
        colors::RESET
    );
    println!("{}", "=".repeat(40));
    println!();

    // Handle backup operations
    if options.list_backups {
        list_backups();
        return;
    }

    if let Some(ref backup_id) = options.rollback {
        rollback_backup(backup_id);
        return;
    }

    // Initialize capsules
    let platform_detector = PlatformDetectorCapsule::new();
    let env_resolver = EnvResolutionCapsule::new();
    let registry = DetectorRegistryCapsule::new();
    let merger = ConfigMergerCapsule::new();

    // Register all detectors
    register_all_detectors(&registry);

    // Detect platform
    let platform = platform_detector.detect();
    if options.verbose {
        println!(
            "{}Platform:{} {} ({})",
            colors::DIM,
            colors::RESET,
            platform.platform.as_str(),
            platform.arch.as_str()
        );
        println!();
    }

    // Detect-only mode
    if options.detect_only {
        run_detect_only(&registry, &platform, options.verbose);
        return;
    }

    // Run auto-configuration
    run_auto_configure(&registry, &env_resolver, &merger, &platform, &options);
}

// ============================================================================
// Argument Parsing
// ============================================================================

fn parse_args(args: &[String]) -> CliOptions {
    let mut opts = CliOptions::default();

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--auto" | "-a" => opts.auto_approve = true,
            "--force" | "-f" => opts.force_overwrite = true,
            "--dry-run" | "-n" => opts.dry_run = true,
            "--detect" | "-d" => opts.detect_only = true,
            "--list-backups" | "-l" => opts.list_backups = true,
            "--verbose" | "-v" => opts.verbose = true,
            "-h" | "--help" => opts.show_help = true,
            "--version" => opts.show_version = true,
            _ if arg.starts_with("--clients=") => {
                let clients_str = arg.strip_prefix("--clients=").unwrap();
                opts.specific_clients = Some(
                    clients_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                );
            }
            _ if arg.starts_with("--rollback=") => {
                opts.rollback = Some(arg.strip_prefix("--rollback=").unwrap().to_string());
            }
            "--rollback" => {
                // Next arg is the backup ID
                i += 1;
                if i < args.len() {
                    opts.rollback = Some(args[i].clone());
                }
            }
            _ => {
                // Ignore unknown args
            }
        }
        i += 1;
    }

    // Check environment variables
    if env::var("KDB_AUTO_CONFIGURE").map(|v| v == "true" || v == "1").unwrap_or(false) {
        opts.auto_approve = true;
    }
    if env::var("KDB_CONFIGURE_FORCE").map(|v| v == "true" || v == "1").unwrap_or(false) {
        opts.force_overwrite = true;
    }

    opts
}

// ============================================================================
// Detector Registration
// ============================================================================

fn register_all_detectors(registry: &DetectorRegistryCapsule) {
    // Register Claude Code (Priority 1500 - Enterprise)
    if let Err(e) = registry.register(&CLAUDE_CODE_DETECTOR) {
        eprintln!(
            "{}Warning:{} Failed to register Claude Code detector: {}",
            colors::YELLOW,
            colors::RESET,
            e
        );
    }

    // Register Claude Desktop (Priority 1400)
    if let Err(e) = registry.register(&CLAUDE_DESKTOP_DETECTOR) {
        eprintln!(
            "{}Warning:{} Failed to register Claude Desktop detector: {}",
            colors::YELLOW,
            colors::RESET,
            e
        );
    }

    // Register Cursor (Priority 1000 - IDE)
    if let Err(e) = registry.register(&CURSOR_DETECTOR) {
        eprintln!(
            "{}Warning:{} Failed to register Cursor detector: {}",
            colors::YELLOW,
            colors::RESET,
            e
        );
    }

    // Register VSCode (Priority 900 - IDE)
    if let Err(e) = registry.register(&VSCODE_DETECTOR) {
        eprintln!(
            "{}Warning:{} Failed to register VS Code detector: {}",
            colors::YELLOW,
            colors::RESET,
            e
        );
    }

    // Register Generic HTTP (Priority 100 - Fallback)
    if let Err(e) = registry.register(&GENERIC_HTTP_DETECTOR) {
        eprintln!(
            "{}Warning:{} Failed to register Generic HTTP detector: {}",
            colors::YELLOW,
            colors::RESET,
            e
        );
    }
}

// ============================================================================
// Detection Only Mode
// ============================================================================

fn run_detect_only(registry: &DetectorRegistryCapsule, platform: &PlatformInfo, verbose: bool) {
    println!("{}Detecting MCP clients...{}\n", colors::BOLD, colors::RESET);

    let start = Instant::now();
    let result = registry.detect_all(platform);
    let duration = start.elapsed();

    if result.clients.is_empty() {
        println!("{}No MCP clients detected.{}\n", colors::YELLOW, colors::RESET);
        println!("Supported clients:");
        println!("  - Claude Code (CLI and VSCode extension)");
        println!("  - Claude Desktop");
        println!("  - Cursor");
        println!("  - VS Code (with MCP extension)");
        println!("  - Continue.dev");
        println!("\nFor manual setup, see: {}https://kindly.software/setup{}", colors::CYAN, colors::RESET);
    } else {
        println!(
            "{}Found {} client(s):{}\n",
            colors::GREEN,
            result.clients.len(),
            colors::RESET
        );

        for client in &result.clients {
            let status = if client.kdb_configured {
                color(colors::GREEN, "[configured]")
            } else if client.config_exists {
                color(colors::YELLOW, "[exists, kdb not configured]")
            } else {
                color(colors::RED, "[not configured]")
            };

            println!(
                "  {} {} {}",
                color(colors::BOLD, &client.client_name),
                color(colors::DIM, "-"),
                status
            );
            println!(
                "    {}Path:{} {}",
                colors::DIM,
                colors::RESET,
                client.config_path.display()
            );
            if verbose {
                println!(
                    "    {}Priority:{} {} | {}Method:{} {:?}",
                    colors::DIM,
                    colors::RESET,
                    client.priority,
                    colors::DIM,
                    colors::RESET,
                    client.detection_method
                );
            }
            println!();
        }

        if verbose {
            println!(
                "{}Detection completed in {:.2}ms ({} detectors checked){}",
                colors::DIM,
                duration.as_secs_f64() * 1000.0,
                result.detectors_checked,
                colors::RESET
            );
        }
    }
}

// ============================================================================
// Auto-Configuration
// ============================================================================

fn run_auto_configure(
    registry: &DetectorRegistryCapsule,
    env_resolver: &EnvResolutionCapsule,
    merger: &ConfigMergerCapsule,
    platform: &PlatformInfo,
    options: &CliOptions,
) {
    let start = Instant::now();

    // Step 1: Detect clients
    println!("{}[1/4]{} Detecting MCP clients...", colors::CYAN, colors::RESET);
    let result = registry.detect_all(platform);

    if result.clients.is_empty() {
        println!(
            "\n{}No MCP clients found.{} Install Claude Code, Cursor, or VS Code first.",
            colors::YELLOW,
            colors::RESET
        );
        println!("See: {}https://kindly.software/setup{}", colors::CYAN, colors::RESET);
        return;
    }

    // Filter clients if specific ones requested
    let clients: Vec<&DetectedClient> = if let Some(ref specific) = options.specific_clients {
        result
            .clients
            .iter()
            .filter(|c| specific.contains(&c.client_id.to_string()))
            .collect()
    } else {
        result.clients.iter().collect()
    };

    if clients.is_empty() {
        println!(
            "\n{}No matching clients found.{} Requested: {:?}",
            colors::YELLOW,
            colors::RESET,
            options.specific_clients
        );
        return;
    }

    println!(
        "  Found {} client(s): {}",
        clients.len(),
        clients
            .iter()
            .map(|c| &*c.client_name)
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Step 2: Resolve license key
    println!(
        "\n{}[2/4]{} Resolving license key...",
        colors::CYAN,
        colors::RESET
    );
    let license_key = resolve_license_key(env_resolver);
    if let Some(ref key) = license_key {
        let masked = mask_license_key(key);
        println!("  License key: {}", masked);
    } else {
        println!(
            "  {}No license key found.{} Using placeholder.",
            colors::YELLOW,
            colors::RESET
        );
        println!("  Set KDB_LICENSE_KEY environment variable or sign up at:");
        println!("  {}https://api.kindly.software/api/v1/signup{}", colors::CYAN, colors::RESET);
    }

    // Step 3: Create backups
    println!(
        "\n{}[3/4]{} Creating backups...",
        colors::CYAN,
        colors::RESET
    );
    let backup_dir = create_backup_dir(platform);
    println!("  Backup directory: {}", backup_dir.display());

    // Step 4: Configure clients
    println!(
        "\n{}[4/4]{} Configuring clients...",
        colors::CYAN,
        colors::RESET
    );

    let mut configured_count = 0;
    let mut skipped_count = 0;
    let mut error_count = 0;

    for client in &clients {
        let result = configure_client(
            client,
            merger,
            &license_key,
            &backup_dir,
            options,
        );

        match result {
            ConfigureResult::Configured => {
                println!(
                    "  {} {}",
                    color(colors::GREEN, "[OK]"),
                    client.client_name
                );
                configured_count += 1;
            }
            ConfigureResult::Skipped(reason) => {
                println!(
                    "  {} {} - {}",
                    color(colors::YELLOW, "[SKIPPED]"),
                    client.client_name,
                    reason
                );
                skipped_count += 1;
            }
            ConfigureResult::DryRun(changes) => {
                println!(
                    "  {} {} - Would apply {} change(s)",
                    color(colors::BLUE, "[DRY-RUN]"),
                    client.client_name,
                    changes
                );
                skipped_count += 1;
            }
            ConfigureResult::Error(err) => {
                println!(
                    "  {} {} - {}",
                    color(colors::RED, "[ERROR]"),
                    client.client_name,
                    err
                );
                error_count += 1;
            }
        }
    }

    // Summary
    let duration = start.elapsed();
    println!("\n{}", "=".repeat(40));

    if error_count > 0 {
        println!(
            "{}Configuration completed with errors.{}",
            colors::RED,
            colors::RESET
        );
    } else if configured_count > 0 {
        println!(
            "{}Configuration complete!{}",
            colors::GREEN,
            colors::RESET
        );
    } else {
        println!(
            "{}No changes made.{}",
            colors::YELLOW,
            colors::RESET
        );
    }

    println!();
    println!("  Configured: {}", configured_count);
    println!("  Skipped:    {}", skipped_count);
    println!("  Errors:     {}", error_count);
    println!("  Duration:   {:.2}ms", duration.as_secs_f64() * 1000.0);
    println!("  Backups:    {}", backup_dir.display());

    if configured_count > 0 {
        println!("\n{}Next steps:{}", colors::BOLD, colors::RESET);
        println!("  1. Restart your MCP client(s)");
        println!("  2. Verify kdb is available:");
        println!("     - Claude Code: Type {}@kdb{} in chat", colors::CYAN, colors::RESET);
        println!("     - Cursor: Check MCP panel");
        println!("  3. Get help: {}https://kindly.software/docs{}", colors::CYAN, colors::RESET);
    }

    // Show rollback command
    if configured_count > 0 && !options.dry_run {
        let backup_id = backup_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        println!(
            "\nTo rollback: {}kdb-configure --rollback {}{}",
            colors::DIM,
            backup_id,
            colors::RESET
        );
    }
}

// ============================================================================
// Client Configuration
// ============================================================================

enum ConfigureResult {
    Configured,
    Skipped(String),
    DryRun(usize),
    Error(String),
}

fn configure_client(
    client: &DetectedClient,
    merger: &ConfigMergerCapsule,
    license_key: &Option<String>,
    backup_dir: &Path,
    options: &CliOptions,
) -> ConfigureResult {
    // Skip if already configured and not forcing
    if client.kdb_configured && !options.force_overwrite {
        return ConfigureResult::Skipped("kdb already configured".to_string());
    }

    // Build kdb config
    let kdb_config = build_kdb_config(license_key);

    // Read existing config or create empty
    let existing_content = if client.config_exists {
        match fs::read_to_string(&client.config_path) {
            Ok(content) => content,
            Err(e) => {
                return ConfigureResult::Error(format!("Failed to read config: {}", e));
            }
        }
    } else {
        r#"{"mcpServers": {}}"#.to_string()
    };

    // Dry-run mode: just show what would happen
    if options.dry_run {
        // Count changes (would be 1 for add/update kdb)
        let changes = if client.kdb_configured { 1 } else { 1 };
        return ConfigureResult::DryRun(changes);
    }

    // Interactive confirmation (unless auto-approve)
    if !options.auto_approve && !confirm_configure(client) {
        return ConfigureResult::Skipped("user declined".to_string());
    }

    // Create backup
    let backup_path = backup_dir.join(format!(
        "{}_{}.json.bak",
        client.client_id,
        chrono_timestamp()
    ));

    // Merge config
    let merge_result = if client.config_exists {
        merger.merge_json(&existing_content, &kdb_config, Some(&backup_path))
    } else {
        merger.merge_json(&existing_content, &kdb_config, None)
    };

    match merge_result {
        Ok(result) => {
            // Ensure parent directory exists
            if let Some(parent) = client.config_path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    return ConfigureResult::Error(format!(
                        "Failed to create directory: {}",
                        e
                    ));
                }
            }

            // Write merged config
            if let Err(e) = fs::write(&client.config_path, &result.merged_content) {
                return ConfigureResult::Error(format!("Failed to write config: {}", e));
            }

            ConfigureResult::Configured
        }
        Err(e) => ConfigureResult::Error(format!("Merge failed: {}", e)),
    }
}

fn build_kdb_config(license_key: &Option<String>) -> KdbConfig {
    let mut env = HashMap::new();

    if let Some(key) = license_key {
        env.insert("KDB_LICENSE_KEY".to_string(), key.clone());
    } else {
        // Use placeholder that user should replace
        env.insert(
            "KDB_LICENSE_KEY".to_string(),
            "${KDB_LICENSE_KEY}".to_string(),
        );
    }

    KdbConfig {
        command: "npx".to_string(),
        args: vec!["@kindly-software-inc/kdb".to_string()],
        env,
    }
}

fn confirm_configure(client: &DetectedClient) -> bool {
    print!(
        "Configure {}{}{}? [Y/n] ",
        colors::BOLD,
        client.client_name,
        colors::RESET
    );
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    let input = input.trim().to_lowercase();
    input.is_empty() || input == "y" || input == "yes"
}

// ============================================================================
// License Key Resolution
// ============================================================================

fn resolve_license_key(env_resolver: &EnvResolutionCapsule) -> Option<String> {
    // Priority order:
    // 1. KDB_LICENSE_KEY environment variable
    // 2. ~/.kdb/license file
    // 3. .env file in current directory

    // Check environment variable first
    if let Some(resolved) = env_resolver.resolve("KDB_LICENSE_KEY") {
        if !resolved.value.is_empty() && !resolved.value.starts_with("${") {
            return Some(resolved.value);
        }
    }

    // Check license file
    if let Some(home) = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")) {
        let license_path = PathBuf::from(home).join(".kdb").join("license");
        if let Ok(content) = fs::read_to_string(&license_path) {
            let key = content.trim();
            if !key.is_empty() {
                return Some(key.to_string());
            }
        }
    }

    None
}

fn mask_license_key(key: &str) -> String {
    if key.len() <= 8 {
        "*".repeat(key.len())
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

// ============================================================================
// Backup Management
// ============================================================================

fn create_backup_dir(platform: &PlatformInfo) -> PathBuf {
    let data_dir = get_kdb_data_dir(platform.platform);
    let backup_root = data_dir.join("backups");
    let backup_dir = backup_root.join(chrono_timestamp());

    if let Err(e) = fs::create_dir_all(&backup_dir) {
        eprintln!(
            "{}Warning:{} Failed to create backup directory: {}",
            colors::YELLOW,
            colors::RESET,
            e
        );
        // Fallback to temp directory
        return env::temp_dir().join("kdb-backups").join(chrono_timestamp());
    }

    backup_dir
}

fn list_backups() {
    println!("{}Listing backups...{}\n", colors::BOLD, colors::RESET);

    let platform = PlatformDetectorCapsule::new().detect();
    let data_dir = get_kdb_data_dir(platform.platform);
    let backup_root = data_dir.join("backups");

    if !backup_root.exists() {
        println!("No backups found at: {}", backup_root.display());
        return;
    }

    let mut entries: Vec<_> = match fs::read_dir(&backup_root) {
        Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
        Err(e) => {
            eprintln!("{}Error:{} Failed to read backup directory: {}", colors::RED, colors::RESET, e);
            return;
        }
    };

    if entries.is_empty() {
        println!("No backups found.");
        return;
    }

    // Sort by name (newest first, since names are timestamps)
    entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

    println!("Available backups:\n");
    for entry in entries.iter().take(20) {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Count files in backup
        let file_count = if entry.path().is_dir() {
            fs::read_dir(entry.path())
                .map(|d| d.count())
                .unwrap_or(0)
        } else {
            0
        };

        println!(
            "  {} ({} file(s))",
            color(colors::CYAN, &name_str),
            file_count
        );
    }

    if entries.len() > 20 {
        println!("\n  ... and {} more", entries.len() - 20);
    }

    println!("\nTo rollback: kdb-configure --rollback <backup-id>");
}

fn rollback_backup(backup_id: &str) {
    println!(
        "{}Rolling back to backup: {}{}",
        colors::BOLD,
        backup_id,
        colors::RESET
    );

    let platform = PlatformDetectorCapsule::new().detect();
    let data_dir = get_kdb_data_dir(platform.platform);
    let backup_dir = data_dir.join("backups").join(backup_id);

    if !backup_dir.exists() {
        eprintln!(
            "\n{}Error:{} Backup not found: {}",
            colors::RED,
            colors::RESET,
            backup_dir.display()
        );
        return;
    }

    // List backup files
    let backup_files: Vec<_> = match fs::read_dir(&backup_dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
        Err(e) => {
            eprintln!(
                "\n{}Error:{} Failed to read backup: {}",
                colors::RED,
                colors::RESET,
                e
            );
            return;
        }
    };

    if backup_files.is_empty() {
        println!("No backup files found in: {}", backup_dir.display());
        return;
    }

    println!("\nBackup contains {} file(s):", backup_files.len());
    for file in &backup_files {
        println!("  - {}", file.file_name().to_string_lossy());
    }

    // Confirm rollback
    print!("\nProceed with rollback? [y/N] ");
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return;
    }

    let input = input.trim().to_lowercase();
    if input != "y" && input != "yes" {
        println!("Rollback cancelled.");
        return;
    }

    // TODO: Implement actual rollback by parsing backup file names
    // and restoring to original locations
    println!(
        "\n{}Rollback not yet implemented.{} Please restore files manually from:",
        colors::YELLOW,
        colors::RESET
    );
    println!("  {}", backup_dir.display());
}

// ============================================================================
// Utilities
// ============================================================================

/// Generate timestamp string for backups
fn chrono_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let secs = duration.as_secs();

    // Convert to date components (simplified, no chrono dependency)
    // Format: YYYY-MM-DD_HH-MM-SS
    let days = secs / 86400;
    let time = secs % 86400;
    let hours = time / 3600;
    let mins = (time % 3600) / 60;
    let s = time % 60;

    // Approximate year/month/day (not accounting for leap years, etc.)
    // This is good enough for backup identification
    let years = 1970 + (days / 365);
    let day_of_year = days % 365;
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;

    format!(
        "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
        years, month.min(12), day.min(31), hours, mins, s
    )
}

// ============================================================================
// Help Message
// ============================================================================

fn print_help() {
    println!(
        "{}kdb-configure{} - Universal MCP Auto-Configuration\n",
        colors::BOLD,
        colors::RESET
    );
    println!("{}USAGE:{}", colors::BOLD, colors::RESET);
    println!("    kdb-configure [OPTIONS]\n");
    println!("{}OPTIONS:{}", colors::BOLD, colors::RESET);
    println!("    {}--auto, -a{}          Auto-approve all prompts", colors::CYAN, colors::RESET);
    println!("    {}--force, -f{}         Overwrite existing kdb configs", colors::CYAN, colors::RESET);
    println!("    {}--dry-run, -n{}       Show changes without applying", colors::CYAN, colors::RESET);
    println!("    {}--detect, -d{}        Detection only (don't modify)", colors::CYAN, colors::RESET);
    println!("    {}--clients=<list>{}    Specific clients (comma-separated)", colors::CYAN, colors::RESET);
    println!("    {}--rollback <id>{}     Rollback to backup", colors::CYAN, colors::RESET);
    println!("    {}--list-backups, -l{}  List available backups", colors::CYAN, colors::RESET);
    println!("    {}--verbose, -v{}       Verbose output", colors::CYAN, colors::RESET);
    println!("    {}-h, --help{}          Show this help", colors::CYAN, colors::RESET);
    println!("    {}--version{}           Show version", colors::CYAN, colors::RESET);
    println!();
    println!("{}ENVIRONMENT VARIABLES:{}", colors::BOLD, colors::RESET);
    println!("    {}KDB_LICENSE_KEY{}      Your license key", colors::CYAN, colors::RESET);
    println!("    {}KDB_AUTO_CONFIGURE{}   Set to 'true' for auto-approve", colors::CYAN, colors::RESET);
    println!("    {}KDB_CONFIGURE_FORCE{}  Set to 'true' for force mode", colors::CYAN, colors::RESET);
    println!();
    println!("{}EXAMPLES:{}", colors::BOLD, colors::RESET);
    println!("    # Detect installed MCP clients");
    println!("    kdb-configure --detect");
    println!();
    println!("    # Preview changes without applying");
    println!("    kdb-configure --dry-run");
    println!();
    println!("    # Auto-configure all detected clients");
    println!("    kdb-configure --auto");
    println!();
    println!("    # Configure specific clients only");
    println!("    kdb-configure --clients=claude_code,cursor");
    println!();
    println!("    # Rollback to previous backup");
    println!("    kdb-configure --rollback 2025-12-11_14-30-00");
    println!();
    println!("{}SUPPORTED CLIENTS:{}", colors::BOLD, colors::RESET);
    println!("    - Claude Code (CLI and VSCode extension)");
    println!("    - Claude Desktop");
    println!("    - Cursor");
    println!("    - VS Code (with MCP extension)");
    println!("    - Continue.dev");
    println!();
    println!("For more information, visit:");
    println!("    {}https://kindly.software/docs/setup{}", colors::CYAN, colors::RESET);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args_empty() {
        let args = vec!["kdb-configure".to_string()];
        let opts = parse_args(&args);
        assert!(!opts.auto_approve);
        assert!(!opts.force_overwrite);
        assert!(!opts.dry_run);
        assert!(!opts.detect_only);
    }

    #[test]
    fn test_parse_args_auto() {
        let args = vec!["kdb-configure".to_string(), "--auto".to_string()];
        let opts = parse_args(&args);
        assert!(opts.auto_approve);
    }

    #[test]
    fn test_parse_args_clients() {
        let args = vec![
            "kdb-configure".to_string(),
            "--clients=claude_code,cursor".to_string(),
        ];
        let opts = parse_args(&args);
        assert!(opts.specific_clients.is_some());
        let clients = opts.specific_clients.unwrap();
        assert_eq!(clients.len(), 2);
        assert!(clients.contains(&"claude_code".to_string()));
        assert!(clients.contains(&"cursor".to_string()));
    }

    #[test]
    fn test_parse_args_rollback() {
        let args = vec![
            "kdb-configure".to_string(),
            "--rollback".to_string(),
            "2025-12-11_14-30-00".to_string(),
        ];
        let opts = parse_args(&args);
        assert_eq!(opts.rollback, Some("2025-12-11_14-30-00".to_string()));
    }

    #[test]
    fn test_parse_args_rollback_equals() {
        let args = vec![
            "kdb-configure".to_string(),
            "--rollback=2025-12-11_14-30-00".to_string(),
        ];
        let opts = parse_args(&args);
        assert_eq!(opts.rollback, Some("2025-12-11_14-30-00".to_string()));
    }

    #[test]
    fn test_mask_license_key_short() {
        assert_eq!(mask_license_key("abc"), "***");
    }

    #[test]
    fn test_mask_license_key_long() {
        let masked = mask_license_key("KDB-HOBBY-12345678-abcdef");
        assert!(masked.starts_with("KDB-"));
        assert!(masked.ends_with("cdef"));
        assert!(masked.contains("..."));
    }

    #[test]
    fn test_chrono_timestamp_format() {
        let ts = chrono_timestamp();
        // Should be in format YYYY-MM-DD_HH-MM-SS
        assert!(ts.contains('-'));
        assert!(ts.contains('_'));
        assert_eq!(ts.len(), 19);
    }

    #[test]
    fn test_build_kdb_config_with_key() {
        let config = build_kdb_config(&Some("test-key".to_string()));
        assert_eq!(config.command, "npx");
        assert_eq!(config.args, vec!["@kindly-software-inc/kdb"]);
        assert_eq!(config.env.get("KDB_LICENSE_KEY"), Some(&"test-key".to_string()));
    }

    #[test]
    fn test_build_kdb_config_without_key() {
        let config = build_kdb_config(&None);
        assert_eq!(config.env.get("KDB_LICENSE_KEY"), Some(&"${KDB_LICENSE_KEY}".to_string()));
    }

    #[test]
    fn test_supports_color_no_color() {
        // Test NO_COLOR env var handling (can't easily test without mocking)
        // This test just ensures the function doesn't panic
        let _ = supports_color();
    }

    #[test]
    fn test_color_wrapper() {
        let result = color(colors::GREEN, "test");
        // Result depends on supports_color()
        assert!(result.contains("test"));
    }
}
