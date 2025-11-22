//! Shell Config Parser - Multi-Shell Support
//!
//! Supports parsing and generating shell config files for:
//! - Bash (.bashrc)
//! - Zsh (.zshrc)
//! - Fish (.config/fish/config.fish)

use super::error::{AliasError, AliasResult};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

/// Supported shell types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellType {
    /// Bash shell (.bashrc)
    Bash,
    /// Zsh shell (.zshrc)
    Zsh,
    /// Fish shell (.config/fish/config.fish)
    Fish,
    /// Unknown or unsupported shell
    Unknown,
}

impl ShellType {
    /// Detect current shell from SHELL environment variable
    ///
    /// # Examples
    /// ```ignore
    /// let shell = ShellType::detect();
    /// match shell {
    ///     ShellType::Bash => println!("Using Bash"),
    ///     ShellType::Zsh => println!("Using Zsh"),
    ///     ShellType::Fish => println!("Using Fish"),
    ///     ShellType::Unknown => println!("Unknown shell"),
    /// }
    /// ```
    pub fn detect() -> Self {
        if let Ok(shell_path) = env::var("SHELL") {
            if shell_path.contains("bash") {
                ShellType::Bash
            } else if shell_path.contains("zsh") {
                ShellType::Zsh
            } else if shell_path.contains("fish") {
                ShellType::Fish
            } else {
                ShellType::Unknown
            }
        } else {
            ShellType::Unknown
        }
    }

    /// Get config file path for this shell type
    ///
    /// # Returns
    /// PathBuf to the shell config file
    ///
    /// # Errors
    /// - HOME environment variable not set
    pub fn config_path(&self) -> AliasResult<PathBuf> {
        let home = env::var("HOME").map_err(|_| AliasError::IoError {
            message: "HOME environment variable not set".to_string(),
        })?;

        let path = match self {
            ShellType::Bash => PathBuf::from(home).join(".bashrc"),
            ShellType::Zsh => PathBuf::from(home).join(".zshrc"),
            ShellType::Fish => PathBuf::from(home).join(".config/fish/config.fish"),
            ShellType::Unknown => {
                return Err(AliasError::UnsupportedShell {
                    shell: "Unknown".to_string(),
                });
            }
        };

        Ok(path)
    }

    /// Get shell name as string
    pub fn as_str(&self) -> &'static str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::Unknown => "unknown",
        }
    }
}

/// Single alias entry
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    /// Alias name (e.g., "g", "git", "ll")
    pub name: String,
    /// Target command (e.g., "git-coordinated-v2", "ls -la")
    pub command: String,
    /// Shell this alias belongs to
    pub shell: ShellType,
    /// PID that added this alias (for audit trail)
    pub added_by_pid: u32,
    /// Timestamp when added (nanoseconds since epoch)
    pub timestamp_ns: u64,
}

impl Alias {
    /// Create new alias
    pub fn new(name: String, command: String, shell: ShellType) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        Self {
            name,
            command,
            shell,
            added_by_pid: std::process::id(),
            timestamp_ns,
        }
    }
}

/// Parsed shell configuration
#[derive(Debug, Clone)]
pub struct ShellConfig {
    /// Aliases found in config (name -> command)
    pub aliases: HashMap<String, String>,
    /// All other lines (comments, exports, functions, etc.)
    pub other_lines: Vec<String>,
    /// Shell type this config is for
    pub shell_type: ShellType,
}

impl ShellConfig {
    /// Create new empty shell config
    pub fn new(shell_type: ShellType) -> Self {
        Self {
            aliases: HashMap::new(),
            other_lines: Vec::new(),
            shell_type,
        }
    }

    /// Parse shell config from file content
    ///
    /// # Arguments
    /// - `content`: Raw file content from shell config
    /// - `shell_type`: Which shell syntax to use for parsing
    ///
    /// # Returns
    /// Parsed ShellConfig with aliases extracted
    pub fn parse(content: &str, shell_type: ShellType) -> Self {
        match shell_type {
            ShellType::Bash | ShellType::Zsh => Self::parse_bash_zsh(content, shell_type),
            ShellType::Fish => Self::parse_fish(content, shell_type),
            ShellType::Unknown => Self::new(shell_type),
        }
    }

    /// Parse Bash/Zsh config (same syntax)
    ///
    /// Bash/Zsh alias format: `alias name="command"` or `alias name='command'`
    fn parse_bash_zsh(content: &str, shell_type: ShellType) -> Self {
        let mut aliases = HashMap::new();
        let mut other_lines = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Check if line starts with "alias "
            if trimmed.starts_with("alias ") {
                // Extract alias definition
                if let Some(definition) = trimmed.strip_prefix("alias ") {
                    if let Some((name, command)) = Self::parse_alias_definition(definition) {
                        aliases.insert(name, command);
                        continue;
                    }
                }
            }

            // Not an alias, preserve line as-is
            other_lines.push(line.to_string());
        }

        Self {
            aliases,
            other_lines,
            shell_type,
        }
    }

    /// Parse Fish config
    ///
    /// Fish alias format: `alias name "command"` or `alias name 'command'`
    fn parse_fish(content: &str, shell_type: ShellType) -> Self {
        let mut aliases = HashMap::new();
        let mut other_lines = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Check if line starts with "alias "
            if trimmed.starts_with("alias ") {
                // Extract alias definition
                if let Some(definition) = trimmed.strip_prefix("alias ") {
                    // Fish uses space instead of =
                    if let Some((name, command)) = Self::parse_fish_alias(definition) {
                        aliases.insert(name, command);
                        continue;
                    }
                }
            }

            // Not an alias, preserve line as-is
            other_lines.push(line.to_string());
        }

        Self {
            aliases,
            other_lines,
            shell_type,
        }
    }

    /// Parse alias definition: name="command" or name='command'
    ///
    /// # Returns
    /// Some((name, command)) if valid, None otherwise
    fn parse_alias_definition(definition: &str) -> Option<(String, String)> {
        // Find the = sign
        if let Some(eq_pos) = definition.find('=') {
            let name = definition[..eq_pos].trim().to_string();
            let command_part = definition[eq_pos + 1..].trim();

            // Remove quotes from command
            let command = if (command_part.starts_with('"') && command_part.ends_with('"'))
                || (command_part.starts_with('\'') && command_part.ends_with('\''))
            {
                command_part[1..command_part.len() - 1].to_string()
            } else {
                command_part.to_string()
            };

            return Some((name, command));
        }

        None
    }

    /// Parse Fish alias definition: name "command"
    fn parse_fish_alias(definition: &str) -> Option<(String, String)> {
        // Find first space
        if let Some(space_pos) = definition.find(' ') {
            let name = definition[..space_pos].trim().to_string();
            let command_part = definition[space_pos + 1..].trim();

            // Remove quotes from command
            let command = if (command_part.starts_with('"') && command_part.ends_with('"'))
                || (command_part.starts_with('\'') && command_part.ends_with('\''))
            {
                command_part[1..command_part.len() - 1].to_string()
            } else {
                command_part.to_string()
            };

            return Some((name, command));
        }

        None
    }

    /// Check if alias exists
    pub fn has_alias(&self, name: &str) -> bool {
        self.aliases.contains_key(name)
    }

    /// Add alias to config
    ///
    /// # Returns
    /// New ShellConfig with alias added
    pub fn with_alias(mut self, alias: Alias) -> Self {
        self.aliases.insert(alias.name, alias.command);
        self
    }

    /// Remove alias from config
    ///
    /// # Returns
    /// New ShellConfig with alias removed
    pub fn without_alias(mut self, name: &str) -> Self {
        self.aliases.remove(name);
        self
    }

    /// Generate shell config file content
    ///
    /// # Returns
    /// String ready to write to shell config file
    pub fn generate(&self) -> String {
        let mut lines = Vec::new();

        // Add all non-alias lines first
        lines.extend(self.other_lines.iter().cloned());

        // Add aliases
        let mut alias_names: Vec<_> = self.aliases.keys().collect();
        alias_names.sort(); // Deterministic ordering

        for name in alias_names {
            if let Some(command) = self.aliases.get(name) {
                let alias_line = match self.shell_type {
                    ShellType::Bash | ShellType::Zsh => {
                        format!("alias {}=\"{}\"", name, command)
                    }
                    ShellType::Fish => {
                        format!("alias {} \"{}\"", name, command)
                    }
                    ShellType::Unknown => {
                        // Fallback to Bash syntax
                        format!("alias {}=\"{}\"", name, command)
                    }
                };
                lines.push(alias_line);
            }
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_type_detect() {
        // Just verify the function doesn't panic
        let _shell = ShellType::detect();
    }

    #[test]
    fn test_shell_type_as_str() {
        assert_eq!(ShellType::Bash.as_str(), "bash");
        assert_eq!(ShellType::Zsh.as_str(), "zsh");
        assert_eq!(ShellType::Fish.as_str(), "fish");
        assert_eq!(ShellType::Unknown.as_str(), "unknown");
    }

    #[test]
    fn test_parse_bash_simple() {
        let content = r#"
# My bashrc
alias ll="ls -la"
alias g="git"
export PATH=/usr/local/bin:$PATH
"#;

        let config = ShellConfig::parse(content, ShellType::Bash);
        assert_eq!(config.aliases.len(), 2);
        assert_eq!(config.aliases.get("ll"), Some(&"ls -la".to_string()));
        assert_eq!(config.aliases.get("g"), Some(&"git".to_string()));
        assert!(config.other_lines.len() >= 2); // Comment + export
    }

    #[test]
    fn test_parse_zsh_simple() {
        let content = r#"
alias gco="git checkout"
alias gst="git status"
"#;

        let config = ShellConfig::parse(content, ShellType::Zsh);
        assert_eq!(config.aliases.len(), 2);
        assert_eq!(config.aliases.get("gco"), Some(&"git checkout".to_string()));
        assert_eq!(config.aliases.get("gst"), Some(&"git status".to_string()));
    }

    #[test]
    fn test_parse_fish_simple() {
        let content = r#"
# Fish config
alias ll "ls -la"
alias g "git"
"#;

        let config = ShellConfig::parse(content, ShellType::Fish);
        assert_eq!(config.aliases.len(), 2);
        assert_eq!(config.aliases.get("ll"), Some(&"ls -la".to_string()));
        assert_eq!(config.aliases.get("g"), Some(&"git".to_string()));
    }

    #[test]
    fn test_has_alias() {
        let mut config = ShellConfig::new(ShellType::Bash);
        config.aliases.insert("test".to_string(), "echo test".to_string());

        assert!(config.has_alias("test"));
        assert!(!config.has_alias("missing"));
    }

    #[test]
    fn test_with_alias() {
        let config = ShellConfig::new(ShellType::Bash);
        let alias = Alias::new("test".to_string(), "echo test".to_string(), ShellType::Bash);

        let config = config.with_alias(alias);
        assert_eq!(config.aliases.get("test"), Some(&"echo test".to_string()));
    }

    #[test]
    fn test_without_alias() {
        let mut config = ShellConfig::new(ShellType::Bash);
        config.aliases.insert("test".to_string(), "echo test".to_string());

        let config = config.without_alias("test");
        assert!(!config.has_alias("test"));
    }

    #[test]
    fn test_generate_bash() {
        let mut config = ShellConfig::new(ShellType::Bash);
        config.aliases.insert("ll".to_string(), "ls -la".to_string());
        config.aliases.insert("g".to_string(), "git".to_string());

        let output = config.generate();
        assert!(output.contains("alias g=\"git\""));
        assert!(output.contains("alias ll=\"ls -la\""));
    }

    #[test]
    fn test_generate_fish() {
        let mut config = ShellConfig::new(ShellType::Fish);
        config.aliases.insert("ll".to_string(), "ls -la".to_string());

        let output = config.generate();
        assert!(output.contains("alias ll \"ls -la\""));
    }

    #[test]
    fn test_alias_creation() {
        let alias = Alias::new("test".to_string(), "echo test".to_string(), ShellType::Bash);
        assert_eq!(alias.name, "test");
        assert_eq!(alias.command, "echo test");
        assert_eq!(alias.shell, ShellType::Bash);
        assert!(alias.timestamp_ns > 0);
        assert!(alias.added_by_pid > 0);
    }
}
