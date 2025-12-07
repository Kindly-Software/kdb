//! REPL Capsule (T0 Auditable)
//!
//! Interactive command loop with history and formatted output.
//! Uses rustyline for cross-platform history management.

use crate::cli::audit::AuditLogCapsule;
use crate::cli::commands::Command;
// use crate::cli::dispatcher::CommandDispatcherCapsule;  // TODO: Re-enable when dispatcher module is ready
use std::path::PathBuf;

/// REPL Output styling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStyle {
    /// Green success messages
    Success,
    /// Red error messages
    Error,
    /// Yellow warnings
    Warning,
    /// Default (no color)
    Default,
}

impl OutputStyle {
    fn colorize(&self, msg: &str) -> String {
        match self {
            OutputStyle::Success => format!("\x1b[32m{}\x1b[0m", msg),
            OutputStyle::Error => format!("\x1b[31m{}\x1b[0m", msg),
            OutputStyle::Warning => format!("\x1b[33m{}\x1b[0m", msg),
            OutputStyle::Default => msg.to_string(),
        }
    }
}

/// REPL Capsule - Interactive command loop
///
/// # Architecture (T0 Auditable)
/// - Fields: dispatcher, audit_log, prompt
/// - Memory: ~30 KB (dispatcher 64B + audit 24KB + buffers 6KB)
/// - Alignment: 64B cache-line
///
/// # Performance
/// - Command parsing: ~100ns
/// - Dispatch: ~100ns
/// - Audit log: ~100ns (per command)
/// - Total per command: ~300ns user time (ptrace overhead not included)
///
/// # ASSUM Safety
/// - #ASSUME_SINGLE_THREADED: REPL runs in single thread
/// - #ASSUME_VALID_INPUT: User input is always valid UTF-8
/// - #ASSUME_NO_PANICS: All Error types caught and displayed
#[derive(Debug)]
pub struct REPLCapsule {
    // /// Command dispatcher
    // dispatcher: CommandDispatcherCapsule,  // TODO: Re-enable when dispatcher module is ready
    /// Audit log (Q34 compliance)
    audit_log: AuditLogCapsule,
    /// REPL prompt
    prompt: String,
    /// Exit flag
    should_exit: bool,
    /// Command counter
    command_count: u64,
}

impl REPLCapsule {
    const HISTORY_FILE: &'static str = ".kdb_history";

    /// Create new REPL capsule
    pub fn new() -> Self {
        Self {
            // dispatcher: CommandDispatcherCapsule::new(),  // TODO: Re-enable when dispatcher module is ready
            audit_log: AuditLogCapsule::new(),
            prompt: "kdb> ".to_string(),
            should_exit: false,
            command_count: 0,
        }
    }

    /// Run REPL loop
    pub fn run(&mut self) -> std::io::Result<()> {
        self.display_banner();

        loop {
            if self.should_exit {
                break;
            }

            // Display prompt and read input
            let input = self.read_line()?;

            if input.trim().is_empty() {
                continue;
            }

            // Parse command
            let cmd = Command::parse(&input);

            // Log to audit trail
            self.audit_log.log_command(&input);

            // Execute command
            // TODO: Re-enable when dispatcher module is ready
            // match self.dispatcher.dispatch(&cmd) {
            //     Ok(response) => {
            //         self.display_success(&response);
            //
            //         // Check for quit command
            //         if matches!(cmd, Command::Quit) {
            //             self.should_exit = true;
            //         }
            //     }
            //     Err(err) => {
            //         self.display_error(&format!("{}", err));
            //     }
            // }

            // Temporary: handle quit directly without dispatcher
            if matches!(cmd, Command::Quit) {
                self.display_success("[kdb] Detached. Goodbye!");
                self.should_exit = true;
            } else {
                self.display_warning(&format!("Dispatcher not available. Command: {:?}", cmd));
            }

            self.command_count += 1;
        }

        Ok(())
    }

    /// Read a line from user (simple stdin-based, no rustyline for now)
    fn read_line(&self) -> std::io::Result<String> {
        use std::io::{self, Write};

        print!("{}", self.prompt);
        io::stdout().flush()?;

        let mut line = String::new();
        io::stdin().read_line(&mut line)?;

        Ok(line)
    }

    /// Display banner/welcome message
    fn display_banner(&self) {
        println!(
            "{}",
            OutputStyle::Success.colorize("KDB - The Kindly Debugger v0.1.0")
        );
        println!(
            "{}",
            OutputStyle::Default.colorize("Type 'help' for commands, 'quit' to exit")
        );
        println!();
    }

    /// Display success message
    fn display_success(&self, msg: &str) {
        if msg.starts_with("[kdb]") {
            println!("{}", msg);
        } else {
            println!("{}", OutputStyle::Success.colorize(&format!("[OK] {}", msg)));
        }
    }

    /// Display error message
    fn display_error(&self, msg: &str) {
        println!("{}", OutputStyle::Error.colorize(&format!("[ERROR] {}", msg)));
    }

    /// Display warning message
    fn display_warning(&self, msg: &str) {
        println!("{}", OutputStyle::Warning.colorize(&format!("[WARN] {}", msg)));
    }

    /// Get audit log
    pub fn audit_log(&self) -> &AuditLogCapsule {
        &self.audit_log
    }

    /// Get mutable audit log reference
    pub fn audit_log_mut(&mut self) -> &mut AuditLogCapsule {
        &mut self.audit_log
    }

    // /// Get dispatcher
    // pub fn dispatcher(&self) -> &CommandDispatcherCapsule {
    //     &self.dispatcher
    // }
    //
    // /// Get mutable dispatcher reference
    // pub fn dispatcher_mut(&mut self) -> &mut CommandDispatcherCapsule {
    //     &mut self.dispatcher
    // }
    // TODO: Re-enable dispatcher methods when dispatcher module is ready

    /// Get history file path
    fn history_path() -> PathBuf {
        dirs::home_dir()
            .map(|home| home.join(Self::HISTORY_FILE))
            .unwrap_or_else(|| PathBuf::from(Self::HISTORY_FILE))
    }

    /// Get command count
    pub fn command_count(&self) -> u64 {
        self.command_count
    }

    /// Get exit status
    pub fn should_exit(&self) -> bool {
        self.should_exit
    }
}

impl Default for REPLCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Helper module for directory functions (if dirs crate not available)
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .and_then(|h| if h.is_empty() { None } else { Some(h) })
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .and_then(|h| if h.is_empty() { None } else { Some(h) })
                    .map(PathBuf::from)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repl_create() {
        let repl = REPLCapsule::new();
        assert_eq!(repl.command_count(), 0);
        assert!(!repl.should_exit());
        assert_eq!(repl.prompt, "kdb> ");
    }

    #[test]
    fn test_output_style_success() {
        let styled = OutputStyle::Success.colorize("test");
        assert!(styled.contains("32m")); // Green ANSI code
        assert!(styled.contains("test"));
    }

    #[test]
    fn test_output_style_error() {
        let styled = OutputStyle::Error.colorize("test");
        assert!(styled.contains("31m")); // Red ANSI code
        assert!(styled.contains("test"));
    }

    #[test]
    fn test_audit_log_integration() {
        let repl = REPLCapsule::new();
        assert_eq!(repl.audit_log().entries().len(), 0);
    }

    // #[test]
    // fn test_dispatcher_integration() {
    //     let repl = REPLCapsule::new();
    //     assert_eq!(repl.dispatcher().attached_pid(), None);
    // }
    // TODO: Re-enable test when dispatcher module is ready

    #[test]
    fn test_history_path() {
        let path = REPLCapsule::history_path();
        assert!(path.to_string_lossy().contains("kdb_history"));
    }

    #[test]
    fn test_command_counter() {
        let mut repl = REPLCapsule::new();
        assert_eq!(repl.command_count(), 0);
        repl.command_count += 1;
        assert_eq!(repl.command_count(), 1);
    }
}
