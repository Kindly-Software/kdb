//! Command Definition and Parsing (T0 Auditable)
//!
//! Enum and parsing for 8 core debugger commands.

use std::str::FromStr;

/// 10+ debugger commands
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Attach to process: attach <pid>
    Attach(u32),

    /// Set breakpoint: break <address|symbol>
    Break(String),

    /// Resume execution: continue
    Continue,

    /// Single step forward: step
    Step,

    /// Time-travel step backward: back
    Back,

    /// Capture snapshot: snapshot
    Snapshot,

    /// Show stack trace: stack
    Stack,

    /// Info subcommands: info breakpoints
    Info(String),

    /// Examine memory: x <address> [len]
    Examine(String),

    /// Exit debugger: quit
    Quit,

    /// Help: help [command]
    Help(Option<String>),

    /// Invalid command
    Invalid(String),
}

impl Command {
    /// Parse command string into Command enum
    pub fn parse(input: &str) -> Self {
        let trimmed = input.trim();

        if trimmed.is_empty() {
            return Command::Invalid("Empty command".to_string());
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let cmd = parts[0].to_lowercase();

        match cmd.as_str() {
            "attach" => {
                if parts.len() < 2 {
                    return Command::Invalid("attach requires <pid>".to_string());
                }
                match parts[1].parse::<u32>() {
                    Ok(pid) => Command::Attach(pid),
                    Err(_) => Command::Invalid(format!("Invalid PID: {}", parts[1])),
                }
            }
            "break" | "b" => {
                if parts.len() < 2 {
                    return Command::Invalid("break requires <address|symbol>".to_string());
                }
                Command::Break(parts[1..].join(" "))
            }
            "continue" | "c" => Command::Continue,
            "step" | "s" => Command::Step,
            "back" => Command::Back,
            "snapshot" | "snap" => Command::Snapshot,
            "stack" | "bt" => Command::Stack,
            "info" => {
                if parts.len() < 2 {
                    return Command::Invalid("info requires <subcommand> (e.g., info breakpoints)".to_string());
                }
                Command::Info(parts[1..].join(" "))
            }
            "x" => {
                if parts.len() < 2 {
                    return Command::Invalid("x requires <address> [length]".to_string());
                }
                Command::Examine(parts[1..].join(" "))
            }
            "quit" | "q" | "exit" => Command::Quit,
            "help" | "h" | "?" => {
                let topic = if parts.len() > 1 {
                    Some(parts[1].to_string())
                } else {
                    None
                };
                Command::Help(topic)
            }
            _ => Command::Invalid(format!("Unknown command: {}", cmd)),
        }
    }

    /// Get help text for command
    pub fn help_text(&self) -> String {
        match self {
            Command::Attach(_) => {
                "attach <pid>        Attach to process (e.g., 'attach 12345')".to_string()
            }
            Command::Break(_) => {
                "break <addr|symbol> Set breakpoint (e.g., 'break main' or 'break 0x401234')".to_string()
            }
            Command::Continue => {
                "continue (c)        Resume execution until next breakpoint".to_string()
            }
            Command::Step => "step (s)             Single step forward one instruction".to_string(),
            Command::Back => "back                 Time-travel step backward one snapshot".to_string(),
            Command::Snapshot => "snapshot             Capture time-travel snapshot".to_string(),
            Command::Stack => "stack (bt)           Show stack trace (backtrace)".to_string(),
            Command::Info(_) => {
                "info <subcommand>   Show info (e.g., 'info breakpoints')".to_string()
            }
            Command::Examine(_) => {
                "x <addr> [len]      Examine memory (e.g., 'x 0x400000 64')".to_string()
            }
            Command::Quit => "quit (q)             Exit debugger and detach".to_string(),
            Command::Help(_) => {
                "help [command]      Show help (e.g., 'help attach')".to_string()
            }
            Command::Invalid(msg) => format!("Invalid command: {}", msg),
        }
    }

    /// Get general help text
    pub fn general_help() -> String {
        "KDB - The Kindly Debugger v0.1.0\n\
         Commands:\n\
         \n\
         ATTACHMENT:\n\
           attach <pid>        Attach to process\n\
         \n\
         BREAKPOINTS:\n\
           break <addr|symbol> Set breakpoint at address or symbol\n\
         \n\
         EXECUTION:\n\
           continue (c)        Resume execution\n\
           step (s)            Single step forward\n\
           back                Time-travel step backward\n\
         \n\
         SNAPSHOTS:\n\
           snapshot            Capture time-travel snapshot\n\
         \n\
         INSPECTION:\n\
           stack (bt)          Show stack trace\n\
         \n\
         OTHER:\n\
           help [cmd]          Show help\n\
           quit (q)            Exit\n\
         \n\
         Examples:\n\
           kdb> attach 12345\n\
           kdb> break main\n\
           kdb> continue\n\
           kdb> stack\n\
           kdb> quit\n"
            .to_string()
    }
}

impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Command::parse(s) {
            Command::Invalid(err) => Err(err),
            cmd => Ok(cmd),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_attach() {
        let cmd = Command::parse("attach 12345");
        assert_eq!(cmd, Command::Attach(12345));
    }

    #[test]
    fn test_parse_break() {
        let cmd = Command::parse("break main");
        assert_eq!(cmd, Command::Break("main".to_string()));

        let cmd = Command::parse("b 0x401234");
        assert_eq!(cmd, Command::Break("0x401234".to_string()));
    }

    #[test]
    fn test_parse_continue() {
        let cmd = Command::parse("continue");
        assert_eq!(cmd, Command::Continue);

        let cmd = Command::parse("c");
        assert_eq!(cmd, Command::Continue);
    }

    #[test]
    fn test_parse_step() {
        let cmd = Command::parse("step");
        assert_eq!(cmd, Command::Step);

        let cmd = Command::parse("s");
        assert_eq!(cmd, Command::Step);
    }

    #[test]
    fn test_parse_back() {
        let cmd = Command::parse("back");
        assert_eq!(cmd, Command::Back);
    }

    #[test]
    fn test_parse_snapshot() {
        let cmd = Command::parse("snapshot");
        assert_eq!(cmd, Command::Snapshot);

        let cmd = Command::parse("snap");
        assert_eq!(cmd, Command::Snapshot);
    }

    #[test]
    fn test_parse_stack() {
        let cmd = Command::parse("stack");
        assert_eq!(cmd, Command::Stack);

        let cmd = Command::parse("bt");
        assert_eq!(cmd, Command::Stack);
    }

    #[test]
    fn test_parse_quit() {
        let cmd = Command::parse("quit");
        assert_eq!(cmd, Command::Quit);

        let cmd = Command::parse("q");
        assert_eq!(cmd, Command::Quit);

        let cmd = Command::parse("exit");
        assert_eq!(cmd, Command::Quit);
    }

    #[test]
    fn test_parse_help() {
        let cmd = Command::parse("help");
        assert_eq!(cmd, Command::Help(None));

        let cmd = Command::parse("help attach");
        assert_eq!(cmd, Command::Help(Some("attach".to_string())));
    }

    #[test]
    fn test_parse_invalid_attach() {
        let cmd = Command::parse("attach");
        assert_eq!(cmd, Command::Invalid("attach requires <pid>".to_string()));

        let cmd = Command::parse("attach notapid");
        assert!(matches!(cmd, Command::Invalid(_)));
    }

    #[test]
    fn test_parse_invalid_break() {
        let cmd = Command::parse("break");
        assert_eq!(cmd, Command::Invalid("break requires <address|symbol>".to_string()));
    }

    #[test]
    fn test_parse_empty() {
        let cmd = Command::parse("");
        assert_eq!(cmd, Command::Invalid("Empty command".to_string()));
    }

    #[test]
    fn test_parse_unknown() {
        let cmd = Command::parse("unknown");
        assert!(matches!(cmd, Command::Invalid(_)));
    }
}
