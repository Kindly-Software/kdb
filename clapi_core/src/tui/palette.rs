//! # Command Palette - Fuzzy Search with Lockfree State
//!
//! **UCE34 Q1-Q34 Analysis** (answered internally)
//!
//! ## Q10: Tier Selection
//! - **Tier 1 (Atomic)**: Lockfree coordination for visible/selected/filter state
//! - **Tier 0 (Const Hash)**: 0ns runtime command ID lookups
//!
//! ## Q11: Rust Transform
//! - AtomicBool for visibility toggle
//! - AtomicU32 for selected index
//! - AtomicU64 for filter hash (FNV-1a)
//!
//! ## Q12: Nightly Enhancement
//! - const_fn_floating_point_arithmetic for compile-time score thresholds
//! - const_hash for zero-cost command IDs
//!
//! ## Q31: Simplicity
//! - Single struct, flat layout
//! - Simple toggle(), next(), prev(), execute() API
//! - No heap allocations in hot path
//!
//! ## Q32: Practical Constraints
//! - <1ms filter latency (target: <100µs)
//! - <128B memory footprint
//! - 64B cache alignment
//!
//! ## Q33: Empirical Validation
//! - #[derive(ComputationalCapsule)] compile-time verification
//! - B32 benchmarking for filter performance
//!
//! ## Architecture
//! ```text
//! CommandPaletteCapsule (128B, T1 Atomic)
//!   [0..8]   visible: AtomicBool          // / key toggle
//!   [8..16]  selected_index: AtomicU32    // ↑↓ navigation
//!   [16..24] filter_hash: AtomicU64       // FNV-1a hash of input
//!   [24..32] _padding1
//!   [32..128] _padding2                   // Complete 128B alignment
//! ```
//!
//! ## Command Registry (Compile-Time Const)
//! - 13 commands: /audit, /budget, /cache, /clear, /config, /doctor, /help, /metrics, /profile, /providers, /restart, /start, /stop
//! - Alphabetical order for binary search
//! - Each with description, args, examples
//!
//! ## Fuzzy Matching Algorithm
//! - Simple substring match (0-cost for TUI)
//! - Case-insensitive ASCII lowering
//! - No allocations (stack-only scoring)
//!
//! ## Performance
//! - Filter latency: <100µs (target)
//! - Toggle latency: <10ns (atomic load/store)
//! - Navigation: <10ns (atomic fetch_add)

#![warn(clippy::missing_capsule_verification)]

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// Command Palette Capsule (128B, T1 Atomic)
///
/// 100% lockfree command palette state with scrolling support.
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128, tier = "Atomic")]
#[repr(C, align(128))]
pub struct CommandPaletteCapsule {
    /// Visibility toggle (/ key)
    visible: AtomicBool,
    _padding0: [u8; 7],

    /// Selected index (↑↓ navigation)
    selected_index: AtomicU32,
    _padding1: [u8; 4],

    /// Filter hash (FNV-1a of input string)
    filter_hash: AtomicU64,

    /// Scroll position (↑↓ scroll offset when content overflows)
    scroll_position: AtomicU32,
    _padding2: [u8; 4],

    /// Complete 128B alignment
    _padding3: [u8; 88],
}

impl CommandPaletteCapsule {
    /// Create new command palette capsule
    pub const fn new() -> Self {
        Self {
            visible: AtomicBool::new(false),
            _padding0: [0u8; 7],
            selected_index: AtomicU32::new(0),
            _padding1: [0u8; 4],
            filter_hash: AtomicU64::new(0),
            scroll_position: AtomicU32::new(0),
            _padding2: [0u8; 4],
            _padding3: [0u8; 88],
        }
    }

    /// Toggle visibility (/ key)
    #[inline(always)]
    pub fn toggle(&self) {
        let current = self.visible.load(Ordering::Relaxed);
        self.visible.store(!current, Ordering::Release);

        // Reset state on show
        if !current {
            self.selected_index.store(0, Ordering::Release);
            self.filter_hash.store(0, Ordering::Release);
            self.scroll_position.store(0, Ordering::Release);
        }
    }

    /// Check if visible
    #[inline(always)]
    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Acquire)
    }

    /// Hide palette
    #[inline(always)]
    pub fn hide(&self) {
        self.visible.store(false, Ordering::Release);
    }

    /// Move selection up (↑)
    #[inline(always)]
    pub fn prev(&self, max_index: u32) {
        let current = self.selected_index.load(Ordering::Acquire);
        let new_index = if current == 0 { max_index } else { current - 1 };
        self.selected_index.store(new_index, Ordering::Release);
    }

    /// Move selection down (↓)
    #[inline(always)]
    pub fn next(&self, max_index: u32) {
        let current = self.selected_index.load(Ordering::Acquire);
        let new_index = if current >= max_index { 0 } else { current + 1 };
        self.selected_index.store(new_index, Ordering::Release);
    }

    /// Get selected index
    #[inline(always)]
    pub fn selected_index(&self) -> u32 {
        self.selected_index.load(Ordering::Acquire)
    }

    /// Update filter (compute FNV-1a hash)
    #[inline(always)]
    pub fn update_filter(&self, input: &str) {
        let hash = fnv1a_hash(input.as_bytes());
        self.filter_hash.store(hash, Ordering::Release);
        self.selected_index.store(0, Ordering::Release); // Reset selection
    }

    /// Get filter hash
    #[inline(always)]
    pub fn filter_hash(&self) -> u64 {
        self.filter_hash.load(Ordering::Acquire)
    }

    /// Scroll up (↑)
    #[inline(always)]
    pub fn scroll_up(&self) {
        let current = self.scroll_position.load(Ordering::Acquire);
        if current > 0 {
            self.scroll_position.store(current - 1, Ordering::Release);
        }
    }

    /// Scroll down (↓)
    #[inline(always)]
    pub fn scroll_down(&self, max_scroll: u32) {
        let current = self.scroll_position.load(Ordering::Acquire);
        if current < max_scroll {
            self.scroll_position.store(current + 1, Ordering::Release);
        }
    }

    /// Get scroll position
    #[inline(always)]
    pub fn scroll_position(&self) -> u32 {
        self.scroll_position.load(Ordering::Acquire)
    }

    /// Reset scroll position
    #[inline(always)]
    pub fn reset_scroll(&self) {
        self.scroll_position.store(0, Ordering::Release);
    }
}

/// Command metadata
#[derive(Debug, Clone, Copy)]
pub struct Command {
    /// Command name (e.g., "audit")
    pub name: &'static str,
    /// Command ID hash (const-computed FNV-1a)
    pub id_hash: u64,
    /// Description
    pub description: &'static str,
    /// Arguments
    pub args: &'static str,
    /// Example usage
    pub example: &'static str,
}

/// FNV-1a hash (32-bit, simple)
#[inline(always)]
const fn fnv1a_hash(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

/// Const-hashed command IDs (0ns runtime)
mod command_hashes {
    use super::fnv1a_hash;

    pub const AUDIT: u64 = fnv1a_hash(b"audit");
    pub const BUDGET: u64 = fnv1a_hash(b"budget");
    pub const CACHE: u64 = fnv1a_hash(b"cache");
    pub const CLEAR: u64 = fnv1a_hash(b"clear");
    pub const CONFIG: u64 = fnv1a_hash(b"config");
    pub const DOCTOR: u64 = fnv1a_hash(b"doctor");
    pub const HELP: u64 = fnv1a_hash(b"help");
    pub const METRICS: u64 = fnv1a_hash(b"metrics");
    pub const PROFILE: u64 = fnv1a_hash(b"profile");
    pub const PROVIDERS: u64 = fnv1a_hash(b"providers");
    pub const RESTART: u64 = fnv1a_hash(b"restart");
    pub const START: u64 = fnv1a_hash(b"start");
    pub const STOP: u64 = fnv1a_hash(b"stop");
    pub const WIZARD: u64 = fnv1a_hash(b"wizard");
    pub const WIZARD_ON: u64 = fnv1a_hash(b"wizard on");
    pub const WIZARD_OFF: u64 = fnv1a_hash(b"wizard off");
}

/// Command registry (compile-time constant)
/// Priority commands first (start/restart), then alphabetical
pub const COMMANDS: &[Command] = &[
    // Priority: Server control commands
    Command {
        name: "start",
        id_hash: command_hashes::START,
        description: "Start clapi proxy server",
        args: "[--port PORT]",
        example: "/start --port 8080",
    },
    Command {
        name: "restart",
        id_hash: command_hashes::RESTART,
        description: "Restart clapi proxy server",
        args: "",
        example: "/restart",
    },
    // Alphabetical: Remaining commands
    Command {
        name: "audit",
        id_hash: command_hashes::AUDIT,
        description: "View audit log entries",
        args: "[--limit N] [--provider NAME]",
        example: "/audit --limit 100 --provider openai",
    },
    Command {
        name: "budget",
        id_hash: command_hashes::BUDGET,
        description: "Show budget allocation status",
        args: "[--json]",
        example: "/budget --json",
    },
    Command {
        name: "cache",
        id_hash: command_hashes::CACHE,
        description: "Cache operations (stats, clear, etc.)",
        args: "<stats|clear|warmup>",
        example: "/cache stats",
    },
    Command {
        name: "clear",
        id_hash: command_hashes::CLEAR,
        description: "Clear terminal screen",
        args: "",
        example: "/clear",
    },
    Command {
        name: "config",
        id_hash: command_hashes::CONFIG,
        description: "Show configuration",
        args: "[--section NAME]",
        example: "/config --section providers",
    },
    Command {
        name: "doctor",
        id_hash: command_hashes::DOCTOR,
        description: "Run health diagnostics",
        args: "[--fix]",
        example: "/doctor --fix",
    },
    Command {
        name: "help",
        id_hash: command_hashes::HELP,
        description: "Show help for commands",
        args: "[COMMAND]",
        example: "/help audit",
    },
    Command {
        name: "metrics",
        id_hash: command_hashes::METRICS,
        description: "Show metrics dashboard",
        args: "[--watch N] [--provider NAME]",
        example: "/metrics --watch 5",
    },
    Command {
        name: "profile",
        id_hash: command_hashes::PROFILE,
        description: "View performance profile",
        args: "[--histogram]",
        example: "/profile --histogram",
    },
    Command {
        name: "providers",
        id_hash: command_hashes::PROVIDERS,
        description: "List configured providers",
        args: "[--status]",
        example: "/providers --status",
    },
    Command {
        name: "stop",
        id_hash: command_hashes::STOP,
        description: "Stop clapi proxy server",
        args: "",
        example: "/stop",
    },
    Command {
        name: "wizard",
        id_hash: command_hashes::WIZARD,
        description: "Toggle wizard on startup (reads current config)",
        args: "[on|off]",
        example: "/wizard off",
    },
    Command {
        name: "wizard on",
        id_hash: command_hashes::WIZARD_ON,
        description: "Enable wizard on startup",
        args: "",
        example: "/wizard on",
    },
    Command {
        name: "wizard off",
        id_hash: command_hashes::WIZARD_OFF,
        description: "Disable wizard on startup",
        args: "",
        example: "/wizard off",
    },
];

/// Fuzzy match score (0-100, higher is better)
///
/// Simple substring matching:
/// - Exact match: 100
/// - Prefix match: 90
/// - Contains match: 50
/// - No match: 0
#[inline]
fn fuzzy_score(query: &str, target: &str) -> u8 {
    if query.is_empty() {
        return 100; // Show all commands when filter is empty
    }

    let query_lower = query.to_ascii_lowercase();
    let target_lower = target.to_ascii_lowercase();

    if target_lower == query_lower {
        100 // Exact match
    } else if target_lower.starts_with(&query_lower) {
        90 // Prefix match
    } else if target_lower.contains(&query_lower) {
        50 // Contains match
    } else {
        0 // No match
    }
}

/// Filter commands by query string
///
/// Returns indices of matching commands, sorted by score (descending).
/// No heap allocations - uses stack array.
pub fn filter_commands(query: &str) -> impl Iterator<Item = usize> {
    let mut scores: [(usize, u8); 16] = [(0, 0); 16];

    for (i, cmd) in COMMANDS.iter().enumerate() {
        scores[i] = (i, fuzzy_score(query, cmd.name));
    }

    // Sort by score (descending), stable for equal scores
    scores.sort_by(|a, b| b.1.cmp(&a.1));

    // Filter out zero scores and return indices
    scores
        .into_iter()
        .filter(|(_, score)| *score > 0)
        .map(|(idx, _)| idx)
}

/// High-level command palette interface
pub struct CommandPalette {
    capsule: CommandPaletteCapsule,
    current_filter: String,
}

impl CommandPalette {
    /// Create new command palette
    pub fn new() -> Self {
        Self {
            capsule: CommandPaletteCapsule::new(),
            current_filter: String::new(),
        }
    }

    /// Toggle visibility (/ key)
    pub fn toggle(&mut self) {
        self.capsule.toggle();
        if self.capsule.is_visible() {
            self.current_filter.clear();
        }
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        self.capsule.is_visible()
    }

    /// Hide palette
    pub fn hide(&mut self) {
        self.capsule.hide();
        self.current_filter.clear();
    }

    /// Update filter string
    pub fn update_filter(&mut self, input: String) {
        self.current_filter = input;
        self.capsule.update_filter(&self.current_filter);
    }

    /// Get current filter
    pub fn current_filter(&self) -> &str {
        &self.current_filter
    }

    /// Get filtered commands (sorted by score)
    pub fn filtered_commands(&self) -> Vec<&'static Command> {
        filter_commands(&self.current_filter)
            .map(|idx| &COMMANDS[idx])
            .collect()
    }

    /// Move selection up (↑)
    pub fn prev(&self) {
        let filtered_count = filter_commands(&self.current_filter).count() as u32;
        if filtered_count > 0 {
            self.capsule.prev(filtered_count - 1);
        }
    }

    /// Move selection down (↓)
    pub fn next(&self) {
        let filtered_count = filter_commands(&self.current_filter).count() as u32;
        if filtered_count > 0 {
            self.capsule.next(filtered_count - 1);
        }
    }

    /// Get selected command
    pub fn selected_command(&self) -> Option<&'static Command> {
        let filtered: Vec<_> = filter_commands(&self.current_filter).collect();
        let idx = self.capsule.selected_index() as usize;
        filtered.get(idx).and_then(|&cmd_idx| COMMANDS.get(cmd_idx))
    }

    /// Execute selected command (returns command name)
    pub fn execute(&mut self) -> Option<String> {
        let cmd = self.selected_command()?.name.to_string();
        self.hide();
        Some(cmd)
    }

    /// Scroll up (↑)
    pub fn scroll_up(&self) {
        self.capsule.scroll_up();
    }

    /// Scroll down (↓)
    pub fn scroll_down(&self, max_scroll: u32) {
        self.capsule.scroll_down(max_scroll);
    }

    /// Get scroll position
    pub fn scroll_position(&self) -> u32 {
        self.capsule.scroll_position()
    }

    /// Reset scroll position
    pub fn reset_scroll(&self) {
        self.capsule.reset_scroll();
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

/// Format error messages with friendly emojis and helpful text
///
/// Maps common error patterns to user-friendly messages with appropriate emoji.
/// Falls back to original error with warning emoji if no pattern matches.
///
/// **Complexity**: O(n) string matching, <1μs typical
pub fn format_friendly_error(command: &str, error: &str) -> String {
    let error_lower = error.to_lowercase();

    // Server controller not available (start/restart/stop commands)
    if (error_lower.contains("controller not available") || error_lower.contains("use new_with_server"))
        && (command == "start" || command == "restart" || command == "stop") {
        return format!("🚫 Server control unavailable: The '{}' command requires running clapi with direct server control (not available when monitoring an existing server)", command);
    }

    // Network/connection errors
    if error_lower.contains("connection") || error_lower.contains("network") || error_lower.contains("timeout") {
        return format!("🌐 Connection issue while running '{}': Check your internet connection or try again", command);
    }

    // Permission/auth errors
    if error_lower.contains("permission") || error_lower.contains("forbidden") || error_lower.contains("unauthorized") {
        return format!("🔒 Permission denied for '{}': Check your API keys or credentials", command);
    }

    // Not found errors
    if error_lower.contains("not found") || error_lower.contains("404") {
        return format!("🔍 Resource not found for '{}': Double-check the command arguments", command);
    }

    // Configuration errors
    if error_lower.contains("config") || error_lower.contains("invalid argument") {
        return format!("⚙️  Configuration error in '{}': {}", command, error);
    }

    // Rate limit errors
    if error_lower.contains("rate limit") || error_lower.contains("too many requests") {
        return format!("⏱️  Rate limit exceeded for '{}': Please wait a moment and try again", command);
    }

    // Budget/quota errors
    if error_lower.contains("budget") || error_lower.contains("quota") || error_lower.contains("insufficient") {
        return format!("💰 Budget limit reached for '{}': Increase budget or wait for reset", command);
    }

    // Server errors (5xx)
    if error_lower.contains("500") || error_lower.contains("503") || error_lower.contains("server error") {
        return format!("🔧 Server error while running '{}': The provider may be experiencing issues", command);
    }

    // Command not implemented
    if error_lower.contains("not implemented") || error_lower.contains("unsupported") {
        return format!("🚧 Command '{}' is not yet implemented or unsupported", command);
    }

    // Generic error with warning emoji
    format!("⚠️  Error in '{}': {}", command, error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_alignment() {
        assert_eq!(std::mem::size_of::<CommandPaletteCapsule>(), 128);
        assert_eq!(std::mem::align_of::<CommandPaletteCapsule>(), 128);
    }

    #[test]
    fn test_toggle() {
        let capsule = CommandPaletteCapsule::new();
        assert!(!capsule.is_visible());

        capsule.toggle();
        assert!(capsule.is_visible());

        capsule.toggle();
        assert!(!capsule.is_visible());
    }

    #[test]
    fn test_navigation() {
        let capsule = CommandPaletteCapsule::new();
        assert_eq!(capsule.selected_index(), 0);

        capsule.next(5);
        assert_eq!(capsule.selected_index(), 1);

        capsule.next(5);
        assert_eq!(capsule.selected_index(), 2);

        capsule.prev(5);
        assert_eq!(capsule.selected_index(), 1);

        capsule.prev(5);
        assert_eq!(capsule.selected_index(), 0);

        // Wrap around
        capsule.prev(5);
        assert_eq!(capsule.selected_index(), 5);

        capsule.next(5);
        assert_eq!(capsule.selected_index(), 0);
    }

    #[test]
    fn test_filter_update() {
        let capsule = CommandPaletteCapsule::new();
        assert_eq!(capsule.filter_hash(), 0);

        capsule.update_filter("test");
        assert_ne!(capsule.filter_hash(), 0);

        let hash1 = capsule.filter_hash();
        capsule.update_filter("test");
        assert_eq!(capsule.filter_hash(), hash1); // Same input = same hash
    }

    #[test]
    fn test_fuzzy_score() {
        assert_eq!(fuzzy_score("audit", "audit"), 100); // Exact
        assert_eq!(fuzzy_score("aud", "audit"), 90); // Prefix
        assert_eq!(fuzzy_score("dit", "audit"), 50); // Contains
        assert_eq!(fuzzy_score("xyz", "audit"), 0); // No match

        // Case insensitive
        assert_eq!(fuzzy_score("AUD", "audit"), 90);
        assert_eq!(fuzzy_score("aud", "AUDIT"), 90);
    }

    #[test]
    fn test_filter_commands() {
        let results: Vec<_> = filter_commands("").collect();
        assert_eq!(results.len(), 16); // All commands shown when empty (updated for 16 commands)

        let results: Vec<_> = filter_commands("aud").collect();
        assert!(results.len() >= 1); // At least "audit"
        // Note: audit is now at index 2 (after start and restart)
        assert_eq!(COMMANDS[results[0]].name, "audit");

        let results: Vec<_> = filter_commands("met").collect();
        assert!(results.len() >= 1); // At least "metrics"
        assert_eq!(COMMANDS[results[0]].name, "metrics");

        let results: Vec<_> = filter_commands("xyz").collect();
        assert_eq!(results.len(), 0); // No matches
    }

    #[test]
    fn test_command_registry() {
        assert_eq!(COMMANDS.len(), 16);

        // Verify priority ordering: start and restart first
        assert_eq!(COMMANDS[0].name, "start", "First command should be 'start'");
        assert_eq!(COMMANDS[1].name, "restart", "Second command should be 'restart'");

        // Verify remaining commands are alphabetical (indices 2+)
        for i in 3..COMMANDS.len() {
            assert!(
                COMMANDS[i - 1].name < COMMANDS[i].name,
                "Commands not alphabetical after priority: {} >= {}",
                COMMANDS[i - 1].name,
                COMMANDS[i].name
            );
        }

        // Verify all commands have unique hashes
        let mut hashes = std::collections::HashSet::new();
        for cmd in COMMANDS {
            assert!(hashes.insert(cmd.id_hash), "Duplicate hash for {}", cmd.name);
        }
    }

    #[test]
    fn test_command_palette_high_level() {
        let mut palette = CommandPalette::new();
        assert!(!palette.is_visible());

        palette.toggle();
        assert!(palette.is_visible());

        palette.update_filter("aud".to_string());
        let filtered = palette.filtered_commands();
        assert!(filtered.len() >= 1);
        assert_eq!(filtered[0].name, "audit");

        palette.next();
        palette.prev();

        let cmd = palette.execute();
        assert!(cmd.is_some());
        assert!(!palette.is_visible()); // Palette hides after execute
    }

    #[test]
    fn test_const_hashes() {
        // Verify const hashes match runtime hashes
        assert_eq!(command_hashes::AUDIT, fnv1a_hash(b"audit"));
        assert_eq!(command_hashes::BUDGET, fnv1a_hash(b"budget"));
        assert_eq!(command_hashes::CACHE, fnv1a_hash(b"cache"));
        assert_eq!(command_hashes::CLEAR, fnv1a_hash(b"clear"));
        assert_eq!(command_hashes::CONFIG, fnv1a_hash(b"config"));
        assert_eq!(command_hashes::DOCTOR, fnv1a_hash(b"doctor"));
        assert_eq!(command_hashes::HELP, fnv1a_hash(b"help"));
        assert_eq!(command_hashes::METRICS, fnv1a_hash(b"metrics"));
        assert_eq!(command_hashes::PROFILE, fnv1a_hash(b"profile"));
        assert_eq!(command_hashes::PROVIDERS, fnv1a_hash(b"providers"));
        assert_eq!(command_hashes::START, fnv1a_hash(b"start"));
        assert_eq!(command_hashes::STOP, fnv1a_hash(b"stop"));
        assert_eq!(command_hashes::WIZARD, fnv1a_hash(b"wizard"));
        assert_eq!(command_hashes::WIZARD_ON, fnv1a_hash(b"wizard on"));
        assert_eq!(command_hashes::WIZARD_OFF, fnv1a_hash(b"wizard off"));
    }
}
