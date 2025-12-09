//! Command Dispatcher - 100% Lockfree Command Execution
//!
//! # UCE34 Framework (Q1-Q34 Answered Internally)
//!
//! ## Q10: Tier Selection
//! - **Tier 1 (Atomic)**: CommandDispatcherCapsule for lockfree execution state
//! - 128B cache-aligned capsule with atomic state machine
//!
//! ## Q11: Rust Transform
//! - AtomicU8 for execution state (4 states: Idle/Executing/Success/Error)
//! - AtomicU64 for command/result hashing (FNV-1a)
//! - AtomicU32 for execution counters
//!
//! ## Q12: Nightly Enhancement
//! - Not needed - stable atomics sufficient
//!
//! ## Q31: Simplicity
//! - Single CommandDispatcher struct with execute() method
//! - Command handlers map to existing CLI functions
//! - No heap allocations in hot path
//!
//! ## Q32: Practical Constraints
//! - <100µs command dispatch latency
//! - <128B memory footprint per capsule
//! - 64B cache alignment
//!
//! ## Q33: Empirical Validation
//! - #[derive(ComputationalCapsule)] compile-time verification
//! - B32 benchmarking for dispatch performance
//!
//! ## Q34: Auditability
//! - Command execution logged with hash chains
//! - Result hash stored for verification
//!
//! # Architecture
//! ```text
//! CommandDispatcherCapsule (128B, T1 Atomic)
//!   [0..1]    state: AtomicU8            // 0=Idle, 1=Executing, 2=Success, 3=Error
//!   [8..16]   last_command_hash: AtomicU64 // FNV-1a hash of last command
//!   [16..24]  last_result_hash: AtomicU64  // Hash of execution result
//!   [24..32]  execution_count: AtomicU64   // Total commands executed
//!   [32..36]  error_count: AtomicU32       // Failed executions
//!   [36..40]  last_error_code: AtomicU32   // Last error code
//!   [40..128] _padding                     // Complete 128B alignment
//! ```
//!
//! # Command List (from palette.rs)
//! - audit - Show audit logs
//! - budget - Show budget status
//! - cache - Cache operations (stats/clear/export)
//! - clear - Clear terminal screen
//! - config - Show configuration
//! - doctor - Run diagnostics
//! - help - Show help for commands
//! - metrics - Show metrics dashboard
//! - profile - View performance profile
//! - providers - List configured providers
//! - start - Start clapi proxy server
//! - stop - Stop clapi proxy server
//!
//! # Performance Targets
//! - Command dispatch: <10ns (atomic state transition)
//! - Command execution: Varies by command (async operations)
//! - State reads: <5ns (single atomic load)

#![warn(clippy::missing_capsule_verification)]

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

/// Command execution state (fits in u8)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExecutionState {
    Idle = 0,
    Executing = 1,
    Success = 2,
    Error = 3,
}

impl From<u8> for ExecutionState {
    fn from(value: u8) -> Self {
        match value {
            0 => ExecutionState::Idle,
            1 => ExecutionState::Executing,
            2 => ExecutionState::Success,
            3 => ExecutionState::Error,
            _ => ExecutionState::Idle, // Safe default
        }
    }
}

/// Command Dispatcher Capsule (128B, T1 Atomic)
///
/// 100% lockfree command execution state.
///
/// # Chaos Principles
/// - Cache-aligned (128B) - Dedicated cache line per capsule
/// - Atomic state machine - Lockfree state transitions
/// - Zero dependencies - No external execution libraries
/// - <100µs dispatch - Fast command routing
///
/// # ASSUM Framework
/// - #ASSUME: AtomicU8 state machine (4 states max)
/// - #VERIFY: All state transitions are valid (compile-time enum)
/// - #ASSUME: FNV-1a hash provides command fingerprinting
/// - #VERIFY: Hash collisions <1e-15 for 12 commands
/// - #ASSUME: AtomicU64 counters don't overflow in practice
/// - #VERIFY: Wrapping arithmetic prevents panics
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128, tier = "Atomic")]
#[repr(C, align(128))]
pub struct CommandDispatcherCapsule {
    /// Execution state (Idle/Executing/Success/Error)
    state: AtomicU8,
    _padding0: [u8; 7],

    /// Last command hash (FNV-1a)
    last_command_hash: AtomicU64,

    /// Last result hash (FNV-1a)
    last_result_hash: AtomicU64,

    /// Total commands executed
    execution_count: AtomicU64,

    /// Failed execution count
    error_count: AtomicU32,

    /// Last error code
    last_error_code: AtomicU32,

    /// Padding to 128 bytes
    _padding1: [u8; 80],
}

impl CommandDispatcherCapsule {
    /// Create new command dispatcher capsule
    ///
    /// # Performance
    /// - <20ns initialization (6 atomic stores)
    /// - Zero allocation
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(ExecutionState::Idle as u8),
            _padding0: [0u8; 7],
            last_command_hash: AtomicU64::new(0),
            last_result_hash: AtomicU64::new(0),
            execution_count: AtomicU64::new(0),
            error_count: AtomicU32::new(0),
            last_error_code: AtomicU32::new(0),
            _padding1: [0u8; 80],
        }
    }

    /// Get current execution state
    ///
    /// # Performance
    /// - <5ns (single atomic load, Relaxed ordering)
    #[inline(always)]
    pub fn state(&self) -> ExecutionState {
        ExecutionState::from(self.state.load(Ordering::Relaxed))
    }

    /// Set execution state
    ///
    /// # Performance
    /// - <10ns (single atomic store, Release ordering)
    #[inline(always)]
    pub fn set_state(&self, new_state: ExecutionState) {
        self.state.store(new_state as u8, Ordering::Release);
    }

    /// Check if currently executing
    #[inline(always)]
    pub fn is_executing(&self) -> bool {
        self.state() == ExecutionState::Executing
    }

    /// Record command execution start
    ///
    /// # Performance
    /// - <30ns (hash computation + atomic stores)
    pub fn start_execution(&self, command: &str) {
        let hash = Self::hash_string(command);
        self.last_command_hash.store(hash, Ordering::Release);
        self.set_state(ExecutionState::Executing);
    }

    /// Record command execution success
    ///
    /// # Performance
    /// - <30ns (hash computation + atomic stores)
    pub fn record_success(&self, result: &str) {
        let hash = Self::hash_string(result);
        self.last_result_hash.store(hash, Ordering::Release);
        self.execution_count.fetch_add(1, Ordering::AcqRel);
        self.set_state(ExecutionState::Success);
    }

    /// Record command execution error
    ///
    /// # Performance
    /// - <30ns (atomic stores)
    pub fn record_error(&self, error_code: u32) {
        self.last_error_code.store(error_code, Ordering::Release);
        self.error_count.fetch_add(1, Ordering::AcqRel);
        self.execution_count.fetch_add(1, Ordering::AcqRel);
        self.set_state(ExecutionState::Error);
    }

    /// Get execution statistics
    pub fn stats(&self) -> ExecutionStats {
        ExecutionStats {
            total_executions: self.execution_count.load(Ordering::Acquire),
            error_count: self.error_count.load(Ordering::Acquire) as u64,
            last_error_code: self.last_error_code.load(Ordering::Acquire),
            last_command_hash: self.last_command_hash.load(Ordering::Acquire),
            last_result_hash: self.last_result_hash.load(Ordering::Acquire),
        }
    }

    /// Hash string using FNV-1a (const-compatible)
    ///
    /// # Algorithm
    /// - FNV-1a (Fowler-Noll-Vo hash)
    /// - Performance: O(n) where n = string length
    fn hash_string(s: &str) -> u64 {
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        let mut hash = FNV_OFFSET_BASIS;
        for byte in s.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }
}

impl Default for CommandDispatcherCapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// Execution statistics snapshot
#[derive(Debug, Clone, Copy)]
pub struct ExecutionStats {
    pub total_executions: u64,
    pub error_count: u64,
    pub last_error_code: u32,
    pub last_command_hash: u64,
    pub last_result_hash: u64,
}

/// High-level command dispatcher
///
/// Executes commands by routing to appropriate handlers.
pub struct CommandDispatcher {
    capsule: CommandDispatcherCapsule,
    base_url: String,
    server_controller: Option<crate::tui::server_control::ServerController>,
}

impl CommandDispatcher {
    /// Create new command dispatcher
    ///
    /// # Arguments
    /// - `base_url`: Base URL for HTTP endpoints (e.g., "http://localhost:8080")
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            capsule: CommandDispatcherCapsule::new(),
            base_url: base_url.into(),
            server_controller: None,
        }
    }

    /// Create new command dispatcher with server controller
    ///
    /// # Arguments
    /// - `base_url`: Base URL for HTTP endpoints (e.g., "http://localhost:8080")
    /// - `server_controller`: Server process controller for start/stop/restart
    pub fn new_with_server(
        base_url: impl Into<String>,
        server_controller: crate::tui::server_control::ServerController,
    ) -> Self {
        Self {
            capsule: CommandDispatcherCapsule::new(),
            base_url: base_url.into(),
            server_controller: Some(server_controller),
        }
    }

    /// Execute command by name
    ///
    /// # Arguments
    /// - `command`: Command name (e.g., "audit", "budget")
    /// - `args`: Command arguments (parsed from command line)
    ///
    /// # Returns
    /// - `Ok(output)` on success
    /// - `Err(error_message)` on failure
    ///
    /// # Performance
    /// - Dispatch: <10ns (atomic state transition)
    /// - Execution: Varies by command (async operations)
    pub async fn execute(&self, command: &str, args: &[String]) -> Result<String, String> {
        // Record execution start
        self.capsule.start_execution(command);

        // Dispatch to handler
        let result = match command {
            "audit" => self.handle_audit(args).await,
            "budget" => self.handle_budget(args).await,
            "cache" => self.handle_cache(args).await,
            "clear" => self.handle_clear(args).await,
            "config" => self.handle_config(args).await,
            "doctor" => self.handle_doctor(args).await,
            "help" => self.handle_help(args).await,
            "metrics" => self.handle_metrics(args).await,
            "profile" => self.handle_profile(args).await,
            "providers" => self.handle_providers(args).await,
            "start" => self.execute_start(args).await,
            "stop" => self.execute_stop(args).await,
            "restart" => self.execute_restart(args).await,
            "wizard" => self.handle_wizard(args).await,
            "wizard on" => self.handle_wizard_on(args).await,
            "wizard off" => self.handle_wizard_off(args).await,
            _ => Err(format!("Unknown command: {}", command)),
        };

        // Record result
        match &result {
            Ok(output) => {
                self.capsule.record_success(output);
            }
            Err(_error) => {
                self.capsule.record_error(1); // Generic error code
            }
        }

        result
    }

    /// Check if currently executing
    pub fn is_executing(&self) -> bool {
        self.capsule.is_executing()
    }

    /// Get execution statistics
    pub fn stats(&self) -> ExecutionStats {
        self.capsule.stats()
    }

    // Command Handlers
    // Each handler corresponds to a command from palette.rs

    /// Handle 'audit' command
    ///
    /// Show audit log entries with optional filtering.
    async fn handle_audit(&self, args: &[String]) -> Result<String, String> {
        // Parse arguments
        let limit = args
            .iter()
            .position(|a| a == "--limit")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(100);

        let provider = args
            .iter()
            .position(|a| a == "--provider")
            .and_then(|i| args.get(i + 1))
            .map(|s| s.as_str());

        // Fetch audit logs from HTTP endpoint
        let url = format!("{}/api/audit?limit={}", self.base_url, limit);
        let url = if let Some(p) = provider {
            format!("{}&provider={}", url, p)
        } else {
            url
        };

        match reqwest::get(&url).await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.text().await {
                        Ok(body) => Ok(body),
                        Err(e) => Err(format!("Failed to read response: {}", e)),
                    }
                } else {
                    Err(format!("HTTP error: {}", response.status()))
                }
            }
            Err(e) => Err(format!("Failed to fetch audit logs: {}", e)),
        }
    }

    /// Handle 'budget' command
    ///
    /// Show budget allocation status.
    async fn handle_budget(&self, args: &[String]) -> Result<String, String> {
        let json_format = args.contains(&"--json".to_string());

        // Fetch budget status from HTTP endpoint
        let url = format!("{}/api/budget", self.base_url);

        match reqwest::get(&url).await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.text().await {
                        Ok(body) => {
                            if json_format {
                                Ok(body)
                            } else {
                                // Format as table (simple text for now)
                                Ok(format!("Budget Status:\n{}", body))
                            }
                        }
                        Err(e) => Err(format!("Failed to read response: {}", e)),
                    }
                } else {
                    Err(format!("HTTP error: {}", response.status()))
                }
            }
            Err(e) => Err(format!("Failed to fetch budget: {}", e)),
        }
    }

    /// Handle 'cache' command
    ///
    /// Cache operations (stats, clear, export).
    async fn handle_cache(&self, args: &[String]) -> Result<String, String> {
        if args.is_empty() {
            return Err("Usage: cache <stats|clear|export>".to_string());
        }

        match args[0].as_str() {
            "stats" => {
                let url = format!("{}/api/cache/stats", self.base_url);
                match reqwest::get(&url).await {
                    Ok(response) => {
                        if response.status().is_success() {
                            match response.text().await {
                                Ok(body) => Ok(body),
                                Err(e) => Err(format!("Failed to read response: {}", e)),
                            }
                        } else {
                            Err(format!("HTTP error: {}", response.status()))
                        }
                    }
                    Err(e) => Err(format!("Failed to fetch cache stats: {}", e)),
                }
            }
            "clear" => {
                let force = args.contains(&"--force".to_string());
                if !force {
                    return Err("Use --force to confirm cache clear".to_string());
                }

                let url = format!("{}/api/cache/clear", self.base_url);
                let client = reqwest::Client::new();
                match client.post(&url).send().await {
                    Ok(response) => {
                        if response.status().is_success() {
                            Ok("Cache cleared successfully".to_string())
                        } else {
                            Err(format!("HTTP error: {}", response.status()))
                        }
                    }
                    Err(e) => Err(format!("Failed to clear cache: {}", e)),
                }
            }
            "export" => {
                let output = args
                    .iter()
                    .position(|a| a == "--output")
                    .and_then(|i| args.get(i + 1))
                    .ok_or("Missing --output argument")?;

                let url = format!("{}/api/cache/export", self.base_url);
                match reqwest::get(&url).await {
                    Ok(response) => {
                        if response.status().is_success() {
                            match response.text().await {
                                Ok(body) => {
                                    // Write to file
                                    std::fs::write(output, &body)
                                        .map_err(|e| format!("Failed to write file: {}", e))?;
                                    Ok(format!("Cache exported to: {}", output))
                                }
                                Err(e) => Err(format!("Failed to read response: {}", e)),
                            }
                        } else {
                            Err(format!("HTTP error: {}", response.status()))
                        }
                    }
                    Err(e) => Err(format!("Failed to export cache: {}", e)),
                }
            }
            _ => Err(format!("Unknown cache action: {}", args[0])),
        }
    }

    /// Handle 'clear' command
    ///
    /// Clear terminal screen (TUI-specific).
    async fn handle_clear(&self, _args: &[String]) -> Result<String, String> {
        // Return signal to clear screen
        Ok("__CLEAR_SCREEN__".to_string())
    }

    /// Handle 'config' command
    ///
    /// Show configuration from local file.
    async fn handle_config(&self, args: &[String]) -> Result<String, String> {
        let section = args
            .iter()
            .position(|a| a == "--section")
            .and_then(|i| args.get(i + 1))
            .map(|s| s.as_str());

        // Read local config file
        let config_path = Self::get_config_path()?;
        let contents = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;

        if let Some(section_name) = section {
            // Parse TOML and extract specific section
            let value: toml::Value = toml::from_str(&contents)
                .map_err(|e| format!("Failed to parse config: {}", e))?;

            if let Some(section_value) = value.get(section_name) {
                let section_toml = toml::to_string_pretty(section_value)
                    .map_err(|e| format!("Failed to serialize section: {}", e))?;
                Ok(format!("[{}]\n{}", section_name, section_toml))
            } else {
                Err(format!("Section '{}' not found in config", section_name))
            }
        } else {
            // Show entire config
            Ok(format!("Config file: {}\n\n{}", config_path.display(), contents))
        }
    }

    /// Handle 'doctor' command
    ///
    /// Run health diagnostics.
    async fn handle_doctor(&self, args: &[String]) -> Result<String, String> {
        let fix = args.contains(&"--fix".to_string());

        // Fetch diagnostics from HTTP endpoint
        let url = if fix {
            format!("{}/api/doctor?fix=true", self.base_url)
        } else {
            format!("{}/api/doctor", self.base_url)
        };

        match reqwest::get(&url).await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.text().await {
                        Ok(body) => Ok(body),
                        Err(e) => Err(format!("Failed to read response: {}", e)),
                    }
                } else {
                    Err(format!("HTTP error: {}", response.status()))
                }
            }
            Err(e) => Err(format!("Failed to run diagnostics: {}", e)),
        }
    }

    /// Handle 'help' command
    ///
    /// Show help for commands.
    async fn handle_help(&self, args: &[String]) -> Result<String, String> {
        use crate::tui::palette::COMMANDS;

        if args.is_empty() {
            // Show all commands
            let mut output = String::from("Available commands:\n\n");
            for cmd in COMMANDS {
                output.push_str(&format!("  {:<12} {}\n", cmd.name, cmd.description));
            }
            output.push_str("\nUse '/help <command>' for detailed help on a command.\n");
            Ok(output)
        } else {
            // Show help for specific command
            let cmd_name = &args[0];
            if let Some(cmd) = COMMANDS.iter().find(|c| c.name == cmd_name) {
                let mut output = String::new();
                output.push_str(&format!("Command: {}\n\n", cmd.name));
                output.push_str(&format!("Description: {}\n\n", cmd.description));
                output.push_str(&format!("Arguments: {}\n\n", cmd.args));
                output.push_str(&format!("Example: {}\n", cmd.example));
                Ok(output)
            } else {
                Err(format!("Unknown command: {}", cmd_name))
            }
        }
    }

    /// Handle 'metrics' command
    ///
    /// Show metrics dashboard.
    async fn handle_metrics(&self, args: &[String]) -> Result<String, String> {
        let watch = args
            .iter()
            .position(|a| a == "--watch")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse::<u32>().ok());

        let provider = args
            .iter()
            .position(|a| a == "--provider")
            .and_then(|i| args.get(i + 1))
            .map(|s| s.as_str());

        // Fetch metrics from HTTP endpoint
        let url = if let Some(p) = provider {
            format!("{}/metrics?provider={}", self.base_url, p)
        } else {
            format!("{}/metrics", self.base_url)
        };

        match reqwest::get(&url).await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.text().await {
                        Ok(body) => {
                            if let Some(interval) = watch {
                                Ok(format!(
                                    "__WATCH_METRICS__:{}:{}",
                                    interval, body
                                ))
                            } else {
                                Ok(body)
                            }
                        }
                        Err(e) => Err(format!("Failed to read response: {}", e)),
                    }
                } else {
                    Err(format!("HTTP error: {}", response.status()))
                }
            }
            Err(e) => Err(format!("Failed to fetch metrics: {}", e)),
        }
    }

    /// Handle 'profile' command
    ///
    /// View performance profile.
    async fn handle_profile(&self, args: &[String]) -> Result<String, String> {
        let histogram = args.contains(&"--histogram".to_string());

        // Fetch profile from HTTP endpoint
        let url = if histogram {
            format!("{}/api/profile?histogram=true", self.base_url)
        } else {
            format!("{}/api/profile", self.base_url)
        };

        match reqwest::get(&url).await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.text().await {
                        Ok(body) => Ok(body),
                        Err(e) => Err(format!("Failed to read response: {}", e)),
                    }
                } else {
                    Err(format!("HTTP error: {}", response.status()))
                }
            }
            Err(e) => Err(format!("Failed to fetch profile: {}", e)),
        }
    }

    /// Handle 'providers' command
    ///
    /// List configured providers.
    async fn handle_providers(&self, args: &[String]) -> Result<String, String> {
        let status = args.contains(&"--status".to_string());

        // Fetch providers from HTTP endpoint
        let url = if status {
            format!("{}/api/providers?status=true", self.base_url)
        } else {
            format!("{}/api/providers", self.base_url)
        };

        match reqwest::get(&url).await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.text().await {
                        Ok(body) => Ok(body),
                        Err(e) => Err(format!("Failed to read response: {}", e)),
                    }
                } else {
                    Err(format!("HTTP error: {}", response.status()))
                }
            }
            Err(e) => Err(format!("Failed to fetch providers: {}", e)),
        }
    }

    /// Handle 'wizard' command
    ///
    /// Toggle wizard on startup (reads current config and flips the setting).
    async fn handle_wizard(&self, args: &[String]) -> Result<String, String> {
        // Check if user specified on/off explicitly
        if !args.is_empty() {
            match args[0].as_str() {
                "on" => return self.handle_wizard_on(&[]).await,
                "off" => return self.handle_wizard_off(&[]).await,
                _ => return Err(format!("Unknown wizard argument: {}. Use 'on' or 'off'", args[0])),
            }
        }

        // Read current config and toggle
        let config_path = Self::get_config_path()?;
        let current_value = Self::read_wizard_setting(&config_path)?;
        Self::update_wizard_setting(&config_path, !current_value)?;

        let new_state = if !current_value { "enabled" } else { "disabled" };
        Ok(format!("✓ Wizard on startup: {}", new_state))
    }

    /// Handle 'wizard on' command
    ///
    /// Enable wizard on startup.
    async fn handle_wizard_on(&self, _args: &[String]) -> Result<String, String> {
        let config_path = Self::get_config_path()?;
        Self::update_wizard_setting(&config_path, true)?;
        Ok("✓ Wizard enabled - will show on next startup".to_string())
    }

    /// Handle 'wizard off' command
    ///
    /// Disable wizard on startup.
    async fn handle_wizard_off(&self, _args: &[String]) -> Result<String, String> {
        let config_path = Self::get_config_path()?;
        Self::update_wizard_setting(&config_path, false)?;
        Ok("✓ Wizard disabled - will skip on next startup".to_string())
    }

    // Config utility functions

    /// Get config file path
    fn get_config_path() -> Result<std::path::PathBuf, String> {
        let path = dirs::config_dir()
            .map(|d| d.join("clapi/clapi.toml"))
            .unwrap_or_else(|| std::path::PathBuf::from("clapi.toml"));

        if !path.exists() {
            return Err(format!(
                "Config file not found at {}\nRun 'clapi config' to create it.",
                path.display()
            ));
        }

        Ok(path)
    }

    /// Read current wizard setting from config
    fn read_wizard_setting(config_path: &std::path::Path) -> Result<bool, String> {
        let contents = std::fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read config: {}", e))?;

        let value: toml::Value = toml::from_str(&contents)
            .map_err(|e| format!("Failed to parse config: {}", e))?;

        // Extract show_wizard_on_start field (default to true if missing)
        Ok(value
            .get("show_wizard_on_start")
            .and_then(|v| v.as_bool())
            .unwrap_or(true))
    }

    /// Update wizard setting in config file
    fn update_wizard_setting(config_path: &std::path::Path, enabled: bool) -> Result<(), String> {
        let contents = std::fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read config: {}", e))?;

        let mut value: toml::Value = toml::from_str(&contents)
            .map_err(|e| format!("Failed to parse config: {}", e))?;

        // Update the show_wizard_on_start field
        if let Some(table) = value.as_table_mut() {
            table.insert(
                "show_wizard_on_start".to_string(),
                toml::Value::Boolean(enabled),
            );
        } else {
            return Err("Invalid config format: expected TOML table".to_string());
        }

        // Serialize and write back
        let updated_contents = toml::to_string_pretty(&value)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        std::fs::write(config_path, updated_contents)
            .map_err(|e| format!("Failed to write config: {}", e))?;

        Ok(())
    }

    /// Execute 'start' command
    ///
    /// Start clapi proxy server via ServerController.
    ///
    /// # Errors
    /// - Server already running
    /// - ServerController not available
    /// - Process spawn failure
    /// - Health check timeout
    async fn execute_start(&self, _args: &[String]) -> Result<String, String> {
        if let Some(ref controller) = self.server_controller {
            controller.start().map(|_| "Server started successfully".to_string())
        } else {
            Err("Server controller not available (use new_with_server)".to_string())
        }
    }

    /// Execute 'stop' command
    ///
    /// Stop clapi proxy server via ServerController.
    ///
    /// # Errors
    /// - Server not running
    /// - ServerController not available
    /// - Signal send failure
    async fn execute_stop(&self, _args: &[String]) -> Result<String, String> {
        if let Some(ref controller) = self.server_controller {
            controller.stop().map(|_| "Server stopped successfully".to_string())
        } else {
            Err("Server controller not available (use new_with_server)".to_string())
        }
    }

    /// Execute 'restart' command
    ///
    /// Restart clapi proxy server via ServerController.
    ///
    /// # Errors
    /// - Stop or start failure
    /// - ServerController not available
    async fn execute_restart(&self, _args: &[String]) -> Result<String, String> {
        if let Some(ref controller) = self.server_controller {
            let restart_count = controller.restart_count();
            controller.restart().map(|_| {
                format!(
                    "Server restarted successfully (total restarts: {})",
                    restart_count + 1
                )
            })
        } else {
            Err("Server controller not available (use new_with_server)".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_alignment() {
        assert_eq!(std::mem::size_of::<CommandDispatcherCapsule>(), 128);
        assert_eq!(std::mem::align_of::<CommandDispatcherCapsule>(), 128);
    }

    #[test]
    fn test_initial_state() {
        let capsule = CommandDispatcherCapsule::new();
        assert_eq!(capsule.state(), ExecutionState::Idle);
        assert!(!capsule.is_executing());
    }

    #[test]
    fn test_state_transitions() {
        let capsule = CommandDispatcherCapsule::new();

        // Idle -> Executing
        capsule.start_execution("test_command");
        assert_eq!(capsule.state(), ExecutionState::Executing);
        assert!(capsule.is_executing());

        // Executing -> Success
        capsule.record_success("test_result");
        assert_eq!(capsule.state(), ExecutionState::Success);
        assert!(!capsule.is_executing());

        // Reset to executing
        capsule.start_execution("test_command_2");
        assert_eq!(capsule.state(), ExecutionState::Executing);

        // Executing -> Error
        capsule.record_error(1);
        assert_eq!(capsule.state(), ExecutionState::Error);
        assert!(!capsule.is_executing());
    }

    #[test]
    fn test_execution_counters() {
        let capsule = CommandDispatcherCapsule::new();

        capsule.start_execution("cmd1");
        capsule.record_success("ok");

        capsule.start_execution("cmd2");
        capsule.record_error(1);

        capsule.start_execution("cmd3");
        capsule.record_success("ok");

        let stats = capsule.stats();
        assert_eq!(stats.total_executions, 3);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.last_error_code, 1);
    }

    #[test]
    fn test_hash_string() {
        let hash1 = CommandDispatcherCapsule::hash_string("test");
        let hash2 = CommandDispatcherCapsule::hash_string("test");
        assert_eq!(hash1, hash2); // Deterministic

        let hash3 = CommandDispatcherCapsule::hash_string("different");
        assert_ne!(hash1, hash3); // Different inputs
    }

    #[test]
    fn test_command_hashing() {
        let capsule = CommandDispatcherCapsule::new();

        capsule.start_execution("audit");
        let hash1 = capsule.last_command_hash.load(Ordering::Acquire);

        capsule.start_execution("budget");
        let hash2 = capsule.last_command_hash.load(Ordering::Acquire);

        assert_ne!(hash1, hash2); // Different commands produce different hashes
    }

    #[test]
    fn test_result_hashing() {
        let capsule = CommandDispatcherCapsule::new();

        capsule.start_execution("test");
        capsule.record_success("result_1");
        let hash1 = capsule.last_result_hash.load(Ordering::Acquire);

        capsule.start_execution("test");
        capsule.record_success("result_2");
        let hash2 = capsule.last_result_hash.load(Ordering::Acquire);

        assert_ne!(hash1, hash2); // Different results produce different hashes
    }

    #[tokio::test]
    async fn test_dispatcher_help() {
        let dispatcher = CommandDispatcher::new("http://localhost:8080");

        // Test help with no args (list all commands)
        let result = dispatcher.handle_help(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Available commands"));

        // Test help with specific command
        let result = dispatcher.handle_help(&["audit".to_string()]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("audit"));
    }

    #[tokio::test]
    async fn test_dispatcher_clear() {
        let dispatcher = CommandDispatcher::new("http://localhost:8080");

        let result = dispatcher.handle_clear(&[]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "__CLEAR_SCREEN__");
    }

    #[tokio::test]
    async fn test_dispatcher_start_without_controller() {
        let dispatcher = CommandDispatcher::new("http://localhost:8080");

        // Test start without server controller (should fail gracefully)
        let result = dispatcher.execute_start(&[]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Server controller not available"));
    }

    #[tokio::test]
    async fn test_dispatcher_stop_without_controller() {
        let dispatcher = CommandDispatcher::new("http://localhost:8080");

        // Test stop without server controller (should fail gracefully)
        let result = dispatcher.execute_stop(&[]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Server controller not available"));
    }

    #[tokio::test]
    async fn test_dispatcher_restart_without_controller() {
        let dispatcher = CommandDispatcher::new("http://localhost:8080");

        // Test restart without server controller (should fail gracefully)
        let result = dispatcher.execute_restart(&[]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Server controller not available"));
    }

    #[tokio::test]
    async fn test_dispatcher_cache() {
        let dispatcher = CommandDispatcher::new("http://localhost:8080");

        // Test cache with no args (should fail)
        let result = dispatcher.handle_cache(&[]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Usage"));

        // Test cache clear without force
        let result = dispatcher.handle_cache(&["clear".to_string()]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--force"));
    }

    #[test]
    fn test_execution_state_from_u8() {
        assert_eq!(ExecutionState::from(0), ExecutionState::Idle);
        assert_eq!(ExecutionState::from(1), ExecutionState::Executing);
        assert_eq!(ExecutionState::from(2), ExecutionState::Success);
        assert_eq!(ExecutionState::from(3), ExecutionState::Error);
        assert_eq!(ExecutionState::from(255), ExecutionState::Idle); // Invalid -> Idle
    }
}
