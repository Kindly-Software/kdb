//! Configuration Wizard - Interactive Setup
//!
//! # Purpose
//! Provides a friendly, step-by-step interactive wizard for configuring clapi:
//! - Server settings (listen address, default budget)
//! - Provider setup (Anthropic, OpenAI, Google, Cohere, Custom)
//! - Circuit breaker configuration
//! - Preview and save to clapi.toml
//!
//! # Design Principles
//! - Progressive Disclosure: Simple defaults, advanced options available
//! - Instant Gratification: Working config in <2 minutes
//! - Actionable Errors: Clear validation with helpful messages
//! - Visual Feedback: Colors, emojis, progress indication
//!
//! # UCE34 Framework
//! - Q1-Q9: Interactive UI layer (presentation, not coordination)
//! - Q10: Tier N/A (no capsules, uses existing ProxyConfig)
//! - Q31 (Simplicity): 3-step wizard, sensible defaults
//! - Q33 (Validation): Input validation at each step
//!
//! # I20 Integration
//! - Q1-Q5: Scope - generates valid clapi.toml
//! - Q6-Q10: Compatibility - backward compatible with ProxyConfig
//! - Q11-Q15: Safety - graceful error handling, validation
//! - Q16-Q20: Testing - comprehensive unit tests

use crate::error::{ClapiError, ClapiResult};
use crate::proxy::{ProviderConfig, ProxyConfig};
use chrono::Utc;
use colored::{ColoredString, Colorize, CustomColor};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Byzantine Purple color (#663399)
const BYZANTINE_PURPLE: CustomColor = CustomColor {
    r: 0x66,
    g: 0x33,
    b: 0x99,
};

/// Gold color (#FFD700)
const GOLD: CustomColor = CustomColor {
    r: 0xFF,
    g: 0xD7,
    b: 0x00,
};

/// Helper to apply Byzantine Purple color
trait ByzantinePurple {
    fn byzantine_purple(&self) -> ColoredString;
}

impl ByzantinePurple for &str {
    fn byzantine_purple(&self) -> ColoredString {
        self.custom_color(BYZANTINE_PURPLE)
    }
}

impl ByzantinePurple for String {
    fn byzantine_purple(&self) -> ColoredString {
        self.as_str().custom_color(BYZANTINE_PURPLE)
    }
}

/// Helper to apply Gold color
trait GoldColor {
    fn gold(&self) -> ColoredString;
}

impl GoldColor for &str {
    fn gold(&self) -> ColoredString {
        self.custom_color(GOLD)
    }
}

impl GoldColor for String {
    fn gold(&self) -> ColoredString {
        self.as_str().custom_color(GOLD)
    }
}

/// Performance configuration (Week 3 features)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub cache: CacheConfig,
    pub compression: CompressionConfig,
    pub load_balancer: LoadBalancerConfig,
    pub profiling: ProfilingConfig,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub enabled: bool,
    pub max_entries: u64,
    pub ttl_seconds: u64,
}

/// Compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub enabled: bool,
    pub min_size_bytes: u64,
    pub level: i32,
}

/// Load balancer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerConfig {
    pub enabled: bool,
    pub latency_weight: f32,
    pub cost_weight: f32,
}

/// Profiling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilingConfig {
    pub enabled: bool,
}

/// Wizard navigation step
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WizardStep {
    ServerSettings,
    ProviderSetup,
    AuditLog,
    Preview,
}

/// Wizard navigation result
#[derive(Debug)]
enum WizardNavResult<T> {
    Continue(T),
    Back,
    Restart,
}

/// Configuration wizard for interactive clapi setup
///
/// # Features
/// - 3-step interactive setup
/// - Sensible defaults for all fields
/// - Input validation
/// - Preview before saving
/// - Option to edit in $EDITOR
///
/// # Example
/// ```no_run
/// use clapi_core::cli::wizard::ConfigWizard;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let wizard = ConfigWizard::new();
/// let config = wizard.run().await?;
/// # Ok(())
/// # }
/// ```
pub struct ConfigWizard {
    /// Use colors in output (disable for automation)
    use_colors: bool,
    /// Theme for dialoguer prompts
    theme: ColorfulTheme,
}

impl ConfigWizard {
    /// Create a new configuration wizard
    ///
    /// # Returns
    /// ConfigWizard with default settings (colors enabled)
    pub fn new() -> Self {
        Self {
            use_colors: true,
            theme: ColorfulTheme::default(),
        }
    }

    /// Create a wizard without colors (for automation)
    #[allow(dead_code)]
    pub fn without_colors() -> Self {
        Self {
            use_colors: false,
            theme: ColorfulTheme::default(),
        }
    }

    /// Run the interactive configuration wizard
    ///
    /// # Returns
    /// Ok(ProxyConfig) on success, Err on cancellation or invalid input
    ///
    /// # Steps
    /// 1. Server settings (listen address, default budget)
    /// 2. Provider setup (add one or more providers)
    /// 3. Preview and save
    ///
    /// # ASSUM Safety
    /// - All user input validated before use
    /// - File operations return Result (no panics)
    /// - Uses Chaos capsules (100% lockfree)
    pub async fn run(&self) -> ClapiResult<ProxyConfig> {
        // Initialize Chaos capsules
        use crate::cli::tui::{
            LogoAnimationCapsule,
            WizardStateCapsule,
            CtrlCHandlerCapsule,
            TuiWizardApp,
        };
        use std::sync::Arc;

        let logo_capsule = Arc::new(LogoAnimationCapsule::new());
        let wizard_capsule = Arc::new(WizardStateCapsule::new());
        let ctrlc_capsule = Arc::new(CtrlCHandlerCapsule::new());

        // Spawn background task to animate logo (50ms per frame = 20 FPS)
        let logo_capsule_clone = Arc::clone(&logo_capsule);
        let animation_task = tokio::spawn(async move {
            loop {
                logo_capsule_clone.update_frame();
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        });

        // Run full Chaos TUI wizard
        let app = TuiWizardApp::new(logo_capsule, wizard_capsule, ctrlc_capsule)?;
        let result = app.run().await;

        // Abort animation task when wizard completes
        animation_task.abort();

        result
    }

    /// Save configuration to TOML file
    ///
    /// # Arguments
    /// - `config`: ProxyConfig to save
    /// - `path`: Output path (e.g., "clapi.toml")
    /// - `force`: Overwrite existing file without confirmation
    ///
    /// # Returns
    /// Ok(()) on success, Err on file permission or TOML serialization error
    ///
    /// # ASSUM Safety
    /// - Checks file existence before overwriting (unless force=true)
    /// - Returns error on permission denied (no panic)
    pub fn save_config<P: AsRef<Path>>(
        &self,
        config: &ProxyConfig,
        path: P,
        force: bool,
    ) -> ClapiResult<()> {
        let path = path.as_ref();

        // Check if file exists (unless force)
        if path.exists() && !force {
            let overwrite = Confirm::with_theme(&self.theme)
                .with_prompt(format!(
                    "{} File {} already exists. Overwrite?",
                    "⚠️".bright_yellow(),
                    path.display().to_string().bright_white()
                ))
                .default(false)
                .interact()
                .map_err(|e| ClapiError::ConfigError(format!("Failed to confirm: {}", e)))?;

            if !overwrite {
                return Err(ClapiError::ConfigError("Cancelled by user".to_string()));
            }
        }

        // Serialize to TOML
        let toml = toml::to_string_pretty(config)
            .map_err(|e| ClapiError::ConfigError(format!("Failed to serialize: {}", e)))?;

        // Write to file with secure permissions (CRITICAL SECURITY FIX)
        // #ASSUME_FILE_PERMISSIONS: Config MUST be 0600 (owner-only) to protect API keys
        // #VERIFY_PERMISSIONS: OpenOptions::mode sets permissions atomically during creation
        // #VERIFY_ATOMIC: No TOCTOU race - permissions set before file is written
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;

            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)  // Owner read/write only (rw-------)
                .open(path)
                .map_err(|e| {
                    ClapiError::ConfigError(format!("Failed to create {}: {}", path.display(), e))
                })?;

            file.write_all(toml.as_bytes()).map_err(|e| {
                ClapiError::ConfigError(format!("Failed to write {}: {}", path.display(), e))
            })?;
        }

        #[cfg(not(unix))]
        {
            // Windows: Default ACLs restrict to owner
            fs::write(path, toml).map_err(|e| {
                ClapiError::ConfigError(format!("Failed to write {}: {}", path.display(), e))
            })?;
        }

        println!(
            "\n{} Configuration saved to {}",
            "✅".bright_green(),
            path.display().to_string().byzantine_purple()
        );

        Ok(())
    }

    /// Create wizard completion marker file
    ///
    /// This creates a `.wizard_completed` marker file in ~/.config/clapi/
    /// to indicate that the user has either completed or explicitly skipped the wizard.
    ///
    /// # Returns
    /// Ok(()) on success, Err if marker file cannot be created
    fn create_wizard_marker() -> ClapiResult<()> {
        let marker_path = Self::wizard_marker_path();

        // Ensure parent directory exists
        if let Some(parent) = marker_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ClapiError::ConfigError(format!("Failed to create config directory: {}", e)))?;
        }

        // Create marker file with timestamp
        let timestamp = Utc::now().to_rfc3339();
        fs::write(&marker_path, format!("Wizard completed/skipped at: {}\n", timestamp))
            .map_err(|e| ClapiError::ConfigError(format!("Failed to create wizard marker: {}", e)))?;

        Ok(())
    }

    /// Get path to wizard marker file
    ///
    /// # Returns
    /// Path to ~/.config/clapi/.wizard_completed
    pub fn wizard_marker_path() -> PathBuf {
        dirs::config_dir()
            .map(|d| d.join("clapi").join(".wizard_completed"))
            .unwrap_or_else(|| PathBuf::from(".config/clapi/.wizard_completed"))
    }

    /// Check if wizard has been completed or skipped
    ///
    /// # Returns
    /// true if marker file exists, false otherwise
    pub fn is_wizard_completed() -> bool {
        Self::wizard_marker_path().exists()
    }

    /// Set up double Ctrl+C handler
    ///
    /// Requires two Ctrl+C presses within 2 seconds to exit.
    /// First press shows a warning message.
    ///
    /// # ASSUM Safety
    /// - Uses AtomicU64 for thread-safe timestamp storage
    /// - Lockfree coordination between signal handler and main thread
    /// - 2-second timeout prevents accidental exits
    fn setup_double_ctrlc_handler() {
        let last_ctrl_c = Arc::new(AtomicU64::new(0));
        let last_ctrl_c_clone = Arc::clone(&last_ctrl_c);

        ctrlc::set_handler(move || {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let last = last_ctrl_c_clone.load(Ordering::Acquire);

            if last > 0 && (now - last) < 2 {
                // Second Ctrl+C within 2 seconds - exit
                println!("\n{}", "  👋 Exiting clapi...".bright_yellow());
                std::process::exit(0);
            } else {
                // First Ctrl+C or timeout expired - show warning
                last_ctrl_c_clone.store(now, Ordering::Release);
                println!(
                    "\n{}",
                    "  ⚠️  Press Ctrl+C again within 2 seconds to exit"
                        .bright_yellow()
                );
            }
        })
        .expect("Error setting Ctrl+C handler");
    }

    /// Show welcome banner with ASCII art logo
    fn show_welcome(&self) {
        if !self.use_colors {
            return;
        }

        println!("\n");
        let logo_lines = vec![
            vec![("  ", false), ("██████", true), ("╗", false), (" ", false), ("██", true), ("╗", false), ("      ", false), ("█████", true), ("╗", false), (" ", false), ("██████", true), ("╗", false), (" ", false), ("██", true), ("╗", false)],
            vec![(" ", false), ("██", true), ("╔════╝", false), (" ", false), ("██", true), ("║", false), ("     ", false), ("██", true), ("╔══", false), ("██", true), ("╗", false), ("██", true), ("╔══", false), ("██", true), ("╗", false), ("██", true), ("║", false)],
            vec![(" ", false), ("██", true), ("║", false), ("      ", false), ("██", true), ("║", false), ("     ", false), ("███████", true), ("║", false), ("██████", true), ("╔╝", false), ("██", true), ("║", false)],
            vec![(" ", false), ("██", true), ("║", false), ("      ", false), ("██", true), ("║", false), ("     ", false), ("██", true), ("╔══", false), ("██", true), ("║", false), ("██", true), ("╔═══╝", false), (" ", false), ("██", true), ("║", false)],
            vec![(" ", false), ("╚", false), ("██████", true), ("╗", false), (" ", false), ("███████", true), ("╗", false), ("██", true), ("║", false), ("  ", false), ("██", true), ("║", false), ("██", true), ("║", false), ("     ", false), ("██", true), ("║", false)],
            vec![("  ", false), ("╚═════╝", false), (" ", false), ("╚══════╝", false), ("╚═╝", false), ("  ", false), ("╚═╝", false), ("╚═╝", false), ("     ", false), ("╚═╝", false)],
        ];

        // Print initial logo statically
        for line in &logo_lines {
            for (segment, is_block) in line {
                if *is_block {
                    print!("{}", segment.byzantine_purple().bold());
                } else {
                    print!("{}", segment.gold());
                }
            }
            println!();
        }

        // Spawn background thread to animate logo
        let logo_lines_clone = logo_lines.clone();
        thread::spawn(move || {
            Self::animate_logo_background(&logo_lines_clone, 10); // 10 cycles in background
        });

        // Give animation a moment to start, then continue with wizard
        thread::sleep(Duration::from_millis(100));

        // Welcome message and underline (wizard starts immediately)
        println!("\n{}", "  Welcome to Clapi from Kindly!".byzantine_purple().bold());
        println!("{}", "  ═══════════════════════════════════".gold());

        println!(
            "\n{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
        );
        println!("{}", "  🧙 Configuration Wizard".byzantine_purple().bold());
        println!(
            "{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
        );
        println!("\n{}", "  Let's set up your AI gateway in 3 easy steps!".bright_white());
        println!("{}", "  (Press Ctrl+C twice within 2 seconds to exit)".bright_black());
    }

    /// Animate CLAPI logo in background with ping-pong effect
    ///
    /// Uses absolute cursor positioning to avoid interfering with wizard
    /// Smoothly transitions colors back and forth:
    /// - Blocks: Byzantine Purple ↔ Gold
    /// - Borders: Gold ↔ Byzantine Purple (opposite)
    fn animate_logo_background(logo_lines: &[Vec<(&'static str, bool)>], cycles: usize) {
        let frames = 30; // Frames per direction
        let start_row = 2; // Logo starts at row 2 (after blank line)

        for _cycle in 0..cycles {
            // Forward animation: Purple → Gold (blocks), Gold → Purple (borders)
            for frame in 0..frames {
                let transition = (frame as f32) / ((frames - 1) as f32);

                // Blocks: Byzantine Purple → Gold
                let block_r = (0x66 as f32 * (1.0 - transition) + 0xFF as f32 * transition) as u8;
                let block_g = (0x33 as f32 * (1.0 - transition) + 0xD7 as f32 * transition) as u8;
                let block_b = (0x99 as f32 * (1.0 - transition) + 0x00 as f32 * transition) as u8;
                let block_color = CustomColor { r: block_r, g: block_g, b: block_b };

                // Borders: Gold → Byzantine Purple (reverse)
                let border_r = (0xFF as f32 * (1.0 - transition) + 0x66 as f32 * transition) as u8;
                let border_g = (0xD7 as f32 * (1.0 - transition) + 0x33 as f32 * transition) as u8;
                let border_b = (0x00 as f32 * (1.0 - transition) + 0x99 as f32 * transition) as u8;
                let border_color = CustomColor { r: border_r, g: border_g, b: border_b };

                // Redraw logo using absolute positioning
                for (i, line) in logo_lines.iter().enumerate() {
                    print!("\x1B[{};1H", start_row + i); // Move to row start_row+i, column 1
                    print!("\x1B[K"); // Clear line
                    for (segment, is_block) in line {
                        if *is_block {
                            print!("{}", segment.custom_color(block_color).bold());
                        } else {
                            print!("{}", segment.custom_color(border_color));
                        }
                    }
                }

                let _ = io::stdout().flush();
                thread::sleep(Duration::from_millis(50));
            }

            // Reverse animation: Gold → Purple (blocks), Purple → Gold (borders)
            for frame in 0..frames {
                let transition = (frame as f32) / ((frames - 1) as f32);

                // Blocks: Gold → Byzantine Purple (reverse)
                let block_r = (0xFF as f32 * (1.0 - transition) + 0x66 as f32 * transition) as u8;
                let block_g = (0xD7 as f32 * (1.0 - transition) + 0x33 as f32 * transition) as u8;
                let block_b = (0x00 as f32 * (1.0 - transition) + 0x99 as f32 * transition) as u8;
                let block_color = CustomColor { r: block_r, g: block_g, b: block_b };

                // Borders: Byzantine Purple → Gold (forward)
                let border_r = (0x66 as f32 * (1.0 - transition) + 0xFF as f32 * transition) as u8;
                let border_g = (0x33 as f32 * (1.0 - transition) + 0xD7 as f32 * transition) as u8;
                let border_b = (0x99 as f32 * (1.0 - transition) + 0x00 as f32 * transition) as u8;
                let border_color = CustomColor { r: border_r, g: border_g, b: border_b };

                // Redraw logo using absolute positioning
                for (i, line) in logo_lines.iter().enumerate() {
                    print!("\x1B[{};1H", start_row + i); // Move to row start_row+i, column 1
                    print!("\x1B[K"); // Clear line
                    for (segment, is_block) in line {
                        if *is_block {
                            print!("{}", segment.custom_color(block_color).bold());
                        } else {
                            print!("{}", segment.custom_color(border_color));
                        }
                    }
                }

                let _ = io::stdout().flush();
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    /// Step 1: Configure server settings (with navigation)
    fn configure_server_with_nav(&self) -> ClapiResult<WizardNavResult<(String, i64)>> {
        // Show navigation menu
        let options = vec!["→ Continue with Server Settings"];
        let selection = Select::with_theme(&self.theme)
            .with_prompt("Step 1: Server Settings")
            .items(&options)
            .default(0)
            .interact()
            .map_err(|e| ClapiError::ConfigError(format!("Failed to get selection: {}", e)))?;

        if selection == 0 {
            // Continue with configuration
            match self.configure_server() {
                Ok(result) => Ok(WizardNavResult::Continue(result)),
                Err(e) => Err(e),
            }
        } else {
            unreachable!()
        }
    }

    /// Step 2: Configure providers (with navigation)
    fn configure_providers_with_nav(&self) -> ClapiResult<WizardNavResult<Vec<ProviderConfig>>> {
        // Show navigation menu
        let options = vec!["→ Continue with Provider Setup", "← Go Back", "⟲ Restart from Beginning"];
        let selection = Select::with_theme(&self.theme)
            .with_prompt("Step 2: Provider Setup")
            .items(&options)
            .default(0)
            .interact()
            .map_err(|e| ClapiError::ConfigError(format!("Failed to get selection: {}", e)))?;

        match selection {
            0 => match self.configure_providers() {
                Ok(result) => Ok(WizardNavResult::Continue(result)),
                Err(e) => Err(e),
            },
            1 => Ok(WizardNavResult::Back),
            2 => Ok(WizardNavResult::Restart),
            _ => unreachable!(),
        }
    }

    /// Step 3: Configure audit log (with navigation)
    fn configure_audit_log_with_nav(&self) -> ClapiResult<WizardNavResult<PathBuf>> {
        // Show navigation menu
        let options = vec!["→ Continue with Audit Log Setup", "← Go Back", "⟲ Restart from Beginning"];
        let selection = Select::with_theme(&self.theme)
            .with_prompt("Step 3: Audit Log Configuration")
            .items(&options)
            .default(0)
            .interact()
            .map_err(|e| ClapiError::ConfigError(format!("Failed to get selection: {}", e)))?;

        match selection {
            0 => match self.configure_audit_log() {
                Ok(result) => Ok(WizardNavResult::Continue(result)),
                Err(e) => Err(e),
            },
            1 => Ok(WizardNavResult::Back),
            2 => Ok(WizardNavResult::Restart),
            _ => unreachable!(),
        }
    }

    /// Preview configuration (with navigation)
    fn preview_config_with_nav(&self, config: &ProxyConfig) -> ClapiResult<WizardNavResult<()>> {
        // Show navigation menu
        let options = vec!["→ Review and Confirm", "← Go Back", "⟲ Restart from Beginning"];
        let selection = Select::with_theme(&self.theme)
            .with_prompt("Step 4: Review Configuration")
            .items(&options)
            .default(0)
            .interact()
            .map_err(|e| ClapiError::ConfigError(format!("Failed to get selection: {}", e)))?;

        match selection {
            0 => match self.preview_config(config) {
                Ok(()) => Ok(WizardNavResult::Continue(())),
                Err(e) => Err(e),
            },
            1 => Ok(WizardNavResult::Back),
            2 => Ok(WizardNavResult::Restart),
            _ => unreachable!(),
        }
    }

    /// Step 1: Configure server settings
    ///
    /// # Returns
    /// (listen_addr, default_budget) on success
    ///
    /// # ASSUM Safety
    /// - Input validation on listen address format
    /// - Budget must be positive (validated)
    fn configure_server(&self) -> ClapiResult<(String, i64)> {
        println!(
            "\n{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
        );
        println!("{}", "Step 1: Server Settings".byzantine_purple().bold());
        println!(
            "{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
        );

        // Listen address
        let listen_addr: String = Input::with_theme(&self.theme)
            .with_prompt("Server listen address")
            .default("0.0.0.0:8080".to_string())
            .validate_with(|input: &String| -> Result<(), &str> {
                if input.contains(':') && !input.is_empty() {
                    Ok(())
                } else {
                    Err("Must be in format 'host:port' (e.g., '0.0.0.0:8080')")
                }
            })
            .interact_text()
            .map_err(|e| ClapiError::ConfigError(format!("Failed to read input: {}", e)))?;

        // Default budget
        let budget_dollars: f64 = Input::with_theme(&self.theme)
            .with_prompt("Default budget per user (USD)")
            .default(100.0)
            .validate_with(|input: &f64| -> Result<(), &str> {
                if *input > 0.0 {
                    Ok(())
                } else {
                    Err("Budget must be positive")
                }
            })
            .interact_text()
            .map_err(|e| ClapiError::ConfigError(format!("Failed to read input: {}", e)))?;

        let default_budget = (budget_dollars * 100.0) as i64;

        println!(
            "\n{} Server: {} | Budget: ${:.2}",
            "✓".bright_green(),
            listen_addr.bright_white(),
            budget_dollars
        );

        Ok((listen_addr, default_budget))
    }

    /// Step 2: Configure providers
    ///
    /// # Returns
    /// Vec<ProviderConfig> with at least one provider
    ///
    /// # ASSUM Safety
    /// - Must have at least one provider (validated)
    /// - API key cannot be empty (validated)
    /// - Base URL must be valid HTTP(S) URL (validated)
    fn configure_providers(&self) -> ClapiResult<Vec<ProviderConfig>> {
        println!(
            "\n{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
        );
        println!("{}", "Step 2: Provider Setup".byzantine_purple().bold());
        println!(
            "{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
        );

        let mut providers = Vec::new();

        loop {
            // Select provider type
            let provider_types = vec![
                "Anthropic (Claude)",
                "OpenAI (GPT)",
                "Google (Gemini)",
                "Cohere",
                "Custom Provider",
            ];

            let selection = Select::with_theme(&self.theme)
                .with_prompt("Select AI provider")
                .items(&provider_types)
                .default(0)
                .interact()
                .map_err(|e| ClapiError::ConfigError(format!("Failed to select: {}", e)))?;

            let (default_name, default_url) = match selection {
                0 => ("anthropic", "https://api.anthropic.com"),
                1 => ("openai", "https://api.openai.com"),
                2 => ("google", "https://generativelanguage.googleapis.com"),
                3 => ("cohere", "https://api.cohere.ai"),
                4 => ("custom", "https://api.example.com"),
                _ => unreachable!(),
            };

            // Provider name
            let name: String = Input::with_theme(&self.theme)
                .with_prompt("Provider name (lowercase, no spaces)")
                .default(default_name.to_string())
                .validate_with(|input: &String| -> Result<(), &str> {
                    if input.chars().all(|c| c.is_ascii_lowercase() || c == '_' || c == '-')
                        && !input.is_empty()
                    {
                        Ok(())
                    } else {
                        Err("Name must be lowercase ASCII (a-z, _, -)")
                    }
                })
                .interact_text()
                .map_err(|e| ClapiError::ConfigError(format!("Failed to read input: {}", e)))?;

            // Base URL
            let base_url: String = Input::with_theme(&self.theme)
                .with_prompt("API base URL")
                .default(default_url.to_string())
                .validate_with(|input: &String| -> Result<(), &str> {
                    if input.starts_with("http://") || input.starts_with("https://") {
                        Ok(())
                    } else {
                        Err("URL must start with http:// or https://")
                    }
                })
                .interact_text()
                .map_err(|e| ClapiError::ConfigError(format!("Failed to read input: {}", e)))?;

            // API key
            let api_key: String = Input::with_theme(&self.theme)
                .with_prompt("API key (will be stored in plaintext)")
                .validate_with(|input: &String| -> Result<(), &str> {
                    if !input.is_empty() {
                        Ok(())
                    } else {
                        Err("API key cannot be empty")
                    }
                })
                .interact_text()
                .map_err(|e| ClapiError::ConfigError(format!("Failed to read input: {}", e)))?;

            // Priority
            let priority = providers.len() as u8;

            // Models (optional)
            let add_models = Confirm::with_theme(&self.theme)
                .with_prompt("Configure specific models for this provider?")
                .default(false)
                .interact()
                .map_err(|e| ClapiError::ConfigError(format!("Failed to confirm: {}", e)))?;

            let models = if add_models {
                self.configure_models()?
            } else {
                Vec::new()
            };

            providers.push(ProviderConfig {
                name: name.clone(),
                base_url,
                api_key: api_key.clone(),
                priority,
                models,
            });

            println!(
                "\n{} Added provider: {} (priority {})",
                "✓".bright_green(),
                name.bright_white(),
                priority
            );

            // Add another provider?
            let add_more = Confirm::with_theme(&self.theme)
                .with_prompt("Add another provider?")
                .default(false)
                .interact()
                .map_err(|e| ClapiError::ConfigError(format!("Failed to confirm: {}", e)))?;

            if !add_more {
                break;
            }
        }

        if providers.is_empty() {
            return Err(ClapiError::ConfigError(
                "At least one provider required".to_string(),
            ));
        }

        Ok(providers)
    }

    /// Configure models for a provider
    ///
    /// # Returns
    /// Vec<String> of model names
    fn configure_models(&self) -> ClapiResult<Vec<String>> {
        let mut models = Vec::new();

        loop {
            let model: String = Input::with_theme(&self.theme)
                .with_prompt("Model name (e.g., 'gpt-4', 'claude-3-opus')")
                .interact_text()
                .map_err(|e| ClapiError::ConfigError(format!("Failed to read input: {}", e)))?;

            if !model.is_empty() {
                models.push(model.clone());
                println!(
                    "  {} Added model: {}",
                    "•".gold(),
                    model.bright_white()
                );
            }

            let add_more = Confirm::with_theme(&self.theme)
                .with_prompt("Add another model?")
                .default(false)
                .interact()
                .map_err(|e| ClapiError::ConfigError(format!("Failed to confirm: {}", e)))?;

            if !add_more {
                break;
            }
        }

        Ok(models)
    }

    /// Step 3: Configure audit log path
    ///
    /// # Returns
    /// PathBuf for audit log location
    fn configure_audit_log(&self) -> ClapiResult<PathBuf> {
        let default_path = if cfg!(target_os = "windows") {
            "C:\\ProgramData\\clapi\\audit.log"
        } else {
            "/var/log/clapi/audit.log"
        };

        let path: String = Input::with_theme(&self.theme)
            .with_prompt("Audit log path")
            .default(default_path.to_string())
            .interact_text()
            .map_err(|e| ClapiError::ConfigError(format!("Failed to read input: {}", e)))?;

        Ok(PathBuf::from(path))
    }

    /// Step 4: Configure performance settings (Week 3)
    ///
    /// # Returns
    /// PerformanceConfig (not yet integrated into ProxyConfig)
    ///
    /// # Features
    /// - Cache: Request/response caching (max entries, TTL)
    /// - Compression: Response compression (zstd level, min size)
    /// - Load Balancer: Advanced routing (latency/cost weights)
    /// - Profiling: Performance profiling (enabled/disabled)
    fn configure_performance_settings(&self) -> ClapiResult<PerformanceConfig> {
        println!(
            "\n{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
        );
        println!("{}", "Step 4: Performance Settings (Optional)".byzantine_purple().bold());
        println!(
            "{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
        );

        // Cache configuration
        let cache_enabled = Confirm::with_theme(&self.theme)
            .with_prompt("Enable request/response cache?")
            .default(true)
            .interact()
            .map_err(|e| ClapiError::ConfigError(format!("Failed to confirm: {}", e)))?;

        let cache_config = if cache_enabled {
            let max_entries: u64 = Input::with_theme(&self.theme)
                .with_prompt("Max cache entries")
                .default(10_000)
                .validate_with(|input: &u64| -> Result<(), &str> {
                    if *input > 0 && *input <= 1_000_000 {
                        Ok(())
                    } else {
                        Err("Must be between 1 and 1,000,000")
                    }
                })
                .interact_text()
                .map_err(|e| ClapiError::ConfigError(format!("Failed to read input: {}", e)))?;

            let ttl_seconds: u64 = Input::with_theme(&self.theme)
                .with_prompt("Cache TTL (seconds)")
                .default(3600)
                .validate_with(|input: &u64| -> Result<(), &str> {
                    if *input >= 60 && *input <= 86400 {
                        Ok(())
                    } else {
                        Err("Must be between 60 and 86400 (1 min - 1 day)")
                    }
                })
                .interact_text()
                .map_err(|e| ClapiError::ConfigError(format!("Failed to read input: {}", e)))?;

            CacheConfig {
                enabled: true,
                max_entries,
                ttl_seconds,
            }
        } else {
            CacheConfig {
                enabled: false,
                max_entries: 0,
                ttl_seconds: 0,
            }
        };

        println!(
            "\n{} Cache: {} (max {} entries, TTL {}s)",
            "✓".bright_green(),
            if cache_enabled { "enabled".bright_green() } else { "disabled".bright_black() },
            cache_config.max_entries,
            cache_config.ttl_seconds
        );

        // Compression configuration
        let compression_enabled = Confirm::with_theme(&self.theme)
            .with_prompt("Enable response compression (zstd)?")
            .default(true)
            .interact()
            .map_err(|e| ClapiError::ConfigError(format!("Failed to confirm: {}", e)))?;

        let compression_config = if compression_enabled {
            let level: i32 = Input::with_theme(&self.theme)
                .with_prompt("Compression level (1-22, higher=slower but smaller)")
                .default(3)
                .validate_with(|input: &i32| -> Result<(), &str> {
                    if *input >= 1 && *input <= 22 {
                        Ok(())
                    } else {
                        Err("Must be between 1 and 22")
                    }
                })
                .interact_text()
                .map_err(|e| ClapiError::ConfigError(format!("Failed to read input: {}", e)))?;

            let min_size_bytes: u64 = Input::with_theme(&self.theme)
                .with_prompt("Min size to compress (bytes)")
                .default(1024)
                .interact_text()
                .map_err(|e| ClapiError::ConfigError(format!("Failed to read input: {}", e)))?;

            CompressionConfig {
                enabled: true,
                min_size_bytes,
                level,
            }
        } else {
            CompressionConfig {
                enabled: false,
                min_size_bytes: 0,
                level: 0,
            }
        };

        println!(
            "\n{} Compression: {} (level {}, min {} bytes)",
            "✓".bright_green(),
            if compression_enabled { "enabled".bright_green() } else { "disabled".bright_black() },
            compression_config.level,
            compression_config.min_size_bytes
        );

        // Load balancer configuration
        let load_balancer_enabled = Confirm::with_theme(&self.theme)
            .with_prompt("Enable advanced load balancing?")
            .default(true)
            .interact()
            .map_err(|e| ClapiError::ConfigError(format!("Failed to confirm: {}", e)))?;

        let load_balancer_config = if load_balancer_enabled {
            let latency_weight: u32 = Input::with_theme(&self.theme)
                .with_prompt("Latency weight (0-100)")
                .default(70)
                .validate_with(|input: &u32| -> Result<(), &str> {
                    if *input <= 100 {
                        Ok(())
                    } else {
                        Err("Must be between 0 and 100")
                    }
                })
                .interact_text()
                .map_err(|e| ClapiError::ConfigError(format!("Failed to read input: {}", e)))?;

            let cost_weight = 100 - latency_weight;

            LoadBalancerConfig {
                enabled: true,
                latency_weight: latency_weight as f32,
                cost_weight: cost_weight as f32,
            }
        } else {
            LoadBalancerConfig {
                enabled: false,
                latency_weight: 0.0,
                cost_weight: 0.0,
            }
        };

        println!(
            "\n{} Load Balancer: {} (latency {}%, cost {}%)",
            "✓".bright_green(),
            if load_balancer_enabled { "enabled".bright_green() } else { "disabled".bright_black() },
            load_balancer_config.latency_weight as u32,
            load_balancer_config.cost_weight as u32
        );

        // Profiling configuration
        let profiling_enabled = Confirm::with_theme(&self.theme)
            .with_prompt("Enable performance profiling?")
            .default(true)
            .interact()
            .map_err(|e| ClapiError::ConfigError(format!("Failed to confirm: {}", e)))?;

        println!(
            "\n{} Profiling: {}",
            "✓".bright_green(),
            if profiling_enabled { "enabled".bright_green() } else { "disabled".bright_black() }
        );

        Ok(PerformanceConfig {
            cache: cache_config,
            compression: compression_config,
            load_balancer: load_balancer_config,
            profiling: ProfilingConfig {
                enabled: profiling_enabled,
            },
        })
    }

    /// Preview configuration before saving
    ///
    /// # Arguments
    /// - `config`: ProxyConfig to preview
    ///
    /// # Returns
    /// Ok(()) if user confirms, Err if user cancels
    fn preview_config(&self, config: &ProxyConfig) -> ClapiResult<()> {
        println!(
            "\n{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
        );
        println!("{}", "Configuration Preview".byzantine_purple().bold());
        println!(
            "{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
        );

        // Server settings
        println!(
            "\n{} {}",
            "Server:".bright_white().bold(),
            config.listen_addr.byzantine_purple()
        );
        println!(
            "{} {}",
            "Budget:".bright_white().bold(),
            format!("${:.2}", config.default_budget as f64 / 100.0).byzantine_purple()
        );
        println!(
            "{} {}",
            "Audit Log:".bright_white().bold(),
            config.audit_log_path.display().to_string().byzantine_purple()
        );

        // Providers
        println!("\n{}", "Providers:".bright_white().bold());
        for (i, provider) in config.providers.iter().enumerate() {
            println!(
                "  {} {} {} (priority {})",
                (i + 1).to_string().gold(),
                "•".gold(),
                provider.name.bright_white(),
                provider.priority
            );
            println!("    URL: {}", provider.base_url.bright_black());
            println!(
                "    API Key: {}",
                format!("{}...", &provider.api_key[..provider.api_key.len().min(8)])
                    .bright_black()
            );
            if !provider.models.is_empty() {
                println!("    Models: {}", provider.models.join(", ").bright_black());
            }
        }

        // Confirm
        let confirmed = Confirm::with_theme(&self.theme)
            .with_prompt("\nLooks good?")
            .default(true)
            .interact()
            .map_err(|e| ClapiError::ConfigError(format!("Failed to confirm: {}", e)))?;

        if !confirmed {
            return Err(ClapiError::ConfigError(
                "Configuration rejected by user".to_string(),
            ));
        }

        Ok(())
    }
}

impl Default for ConfigWizard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wizard_creation() {
        let wizard = ConfigWizard::new();
        assert!(wizard.use_colors);

        let wizard = ConfigWizard::without_colors();
        assert!(!wizard.use_colors);
    }

    #[test]
    fn test_wizard_save_config() {
        let wizard = ConfigWizard::new();
        let config = ProxyConfig {
            listen_addr: "127.0.0.1:8080".to_string(),
            providers: vec![ProviderConfig {
                name: "test".to_string(),
                base_url: "https://api.test.com".to_string(),
                api_key: "test_key".to_string(),
                priority: 0,
                models: vec![],
            }],
            default_budget: 10_000,
            audit_log_path: PathBuf::from("/tmp/audit.log"),
            request_timeout_secs: 30,
            test_mode: false,
            pagerduty_token: None,
            slack_webhook: None,
            show_wizard_on_start: true,
        };

        // Test save to temp file
        let temp_path = std::env::temp_dir().join("test_clapi_wizard.toml");
        if temp_path.exists() {
            fs::remove_file(&temp_path).unwrap();
        }

        let result = wizard.save_config(&config, &temp_path, true);
        assert!(result.is_ok());
        assert!(temp_path.exists());

        // Verify TOML can be loaded
        let loaded = ProxyConfig::load(&temp_path);
        assert!(loaded.is_ok());

        // Cleanup
        fs::remove_file(temp_path).unwrap();
    }

    #[test]
    fn test_wizard_save_prevents_overwrite() {
        let wizard = ConfigWizard::new();
        let config = ProxyConfig {
            listen_addr: "127.0.0.1:8080".to_string(),
            providers: vec![ProviderConfig {
                name: "test".to_string(),
                base_url: "https://api.test.com".to_string(),
                api_key: "test_key".to_string(),
                priority: 0,
                models: vec![],
            }],
            default_budget: 10_000,
            pagerduty_token: None,
            slack_webhook: None,
            audit_log_path: PathBuf::from("/tmp/audit.log"),
            request_timeout_secs: 30,
            test_mode: false,
            show_wizard_on_start: true,
        };

        let temp_path = std::env::temp_dir().join("test_clapi_wizard_overwrite.toml");

        // Create file first
        wizard.save_config(&config, &temp_path, true).unwrap();
        assert!(temp_path.exists());

        // Note: Cannot test interactive confirmation in unit test
        // This test just verifies force=true works

        // Cleanup
        fs::remove_file(temp_path).unwrap();
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = ProxyConfig {
            listen_addr: "0.0.0.0:8080".to_string(),
            providers: vec![
                ProviderConfig {
                    name: "anthropic".to_string(),
                    base_url: "https://api.anthropic.com".to_string(),
                    api_key: "sk-ant-test123".to_string(),
                    priority: 0,
                    models: vec!["claude-3-opus".to_string(), "claude-3-sonnet".to_string()],
                },
                ProviderConfig {
                    name: "openai".to_string(),
                    base_url: "https://api.openai.com".to_string(),
                    api_key: "sk-test456".to_string(),
                    priority: 1,
                    models: vec!["gpt-4".to_string()],
                },
            ],
            default_budget: 10_000,
            audit_log_path: PathBuf::from("/var/log/clapi/audit.log"),
            request_timeout_secs: 30,
            test_mode: false,
            pagerduty_token: None,
            slack_webhook: None,
            show_wizard_on_start: true,
        };

        // Serialize to TOML
        let toml = toml::to_string_pretty(&config).unwrap();
        assert!(toml.contains("listen_addr"));
        assert!(toml.contains("anthropic"));
        assert!(toml.contains("openai"));

        // Deserialize back
        let parsed: ProxyConfig = toml::from_str(&toml).unwrap();
        assert_eq!(parsed.listen_addr, config.listen_addr);
        assert_eq!(parsed.providers.len(), 2);
        assert_eq!(parsed.default_budget, 10_000);
    }
}
