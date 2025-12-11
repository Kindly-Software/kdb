//! DotenvParserCapsule - T1 Atomic .env File Parser (256B)
//!
//! Standard dotenv file parser with atomic statistics tracking.
//! Implements standard dotenv conventions with security-conscious design.
//!
//! **Tier**: T1 Atomic (lockfree parsing with statistics)
//! **Size**: 256 bytes (64-byte aligned)
//! **Latency**: <100us for typical .env file
//!
//! ## Supported Syntax
//!
//! ```text
//! # Comment
//! KEY=value                    # Unquoted
//! KEY="value with spaces"      # Double-quoted (escape sequences: \n, \t, \", \\)
//! KEY='literal value'          # Single-quoted (no escaping)
//! export KEY=value             # Export prefix (optional)
//! KEY=${OTHER_VAR}             # Variable expansion
//! KEY=${VAR:-default}          # Expansion with default
//! ```
//!
//! ## Security
//!
//! **NOT SUPPORTED** (shell injection risk):
//! ```text
//! KEY=`command`                # Command substitution BLOCKED
//! KEY=$(command)               # Command expansion BLOCKED
//! ```
//!
//! ## UCE35 Compliance
//! - Q10: T1 Atomic tier (lockfree statistics)
//! - Q22: Packed atomic fields (cache-aligned)
//! - Q23: 100% lockfree (AtomicU64 for all state)
//! - Q33: 64B cache-aligned
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kdb_mcp::configure::env::DotenvParserCapsule;
//!
//! let parser = DotenvParserCapsule::new();
//!
//! let content = r#"
//! # Database config
//! DB_HOST=localhost
//! DB_PORT=5432
//! DB_PASSWORD="secret123"
//! "#;
//!
//! let result = parser.parse(content, ".env");
//! for (key, value) in &result.variables {
//!     println!("{} = {}", key, value);
//! }
//! ```

use core::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;

// ============================================================================
// Error Types
// ============================================================================

/// Parse error with location information
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// Line number (1-indexed)
    pub line: u32,
    /// Column number (1-indexed)
    pub column: u32,
    /// Error message
    pub message: String,
    /// Error severity
    pub severity: ErrorSeverity,
}

impl ParseError {
    /// Create a new parse error
    #[inline]
    pub fn new(line: u32, column: u32, message: impl Into<String>, severity: ErrorSeverity) -> Self {
        Self {
            line,
            column,
            message: message.into(),
            severity,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}] line {}:{}: {}",
            self.severity, self.line, self.column, self.message
        )
    }
}

impl std::error::Error for ParseError {}

/// Error severity levels
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ErrorSeverity {
    /// Recoverable warning (e.g., trailing whitespace)
    Warning = 0,
    /// Parse failed for this line (e.g., unterminated quote)
    Error = 1,
    /// Cannot continue (e.g., I/O error)
    Fatal = 2,
}

// ============================================================================
// Parsed Result
// ============================================================================

/// Result of parsing a .env file
#[derive(Clone, Debug, Default)]
pub struct ParsedEnvFile {
    /// Successfully parsed variables (key, value)
    pub variables: Vec<(String, String)>,
    /// Parse errors encountered
    pub errors: Vec<ParseError>,
    /// Source file path
    pub source_path: String,
    /// Total lines processed
    pub lines_processed: u32,
}

impl ParsedEnvFile {
    /// Create a new empty result
    #[inline]
    pub fn new(source_path: impl Into<String>) -> Self {
        Self {
            variables: Vec::new(),
            errors: Vec::new(),
            source_path: source_path.into(),
            lines_processed: 0,
        }
    }

    /// Check if parsing was successful (no errors)
    #[inline]
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Check if there are any errors
    #[inline]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get error count by severity
    pub fn error_count_by_severity(&self, severity: ErrorSeverity) -> usize {
        self.errors.iter().filter(|e| e.severity == severity).count()
    }

    /// Convert to HashMap for easy lookups
    pub fn to_hashmap(&self) -> HashMap<String, String> {
        self.variables.iter().cloned().collect()
    }
}

// ============================================================================
// DotenvParserCapsule (T1 Atomic, 256B)
// ============================================================================

/// T1 Atomic Dotenv Parser Capsule (256 bytes)
///
/// Lockfree .env file parser with atomic statistics tracking.
/// Implements standard dotenv conventions with security protections.
///
/// ## Memory Layout (256 bytes)
///
/// ```text
/// +----------------+----------------+----------------+----------------+
/// | Cache Line 1 (64B): Statistics                                   |
/// | lines_parsed   | vars_extracted | parse_errors   | last_parse_ns |
/// | (8B AtomicU64) | (8B AtomicU64) | (8B AtomicU64) | (8B AtomicU64)|
/// | _padding[224]                                                    |
/// +------------------------------------------------------------------+
/// ```
#[repr(C, align(64))]
pub struct DotenvParserCapsule {
    /// Total lines parsed across all calls
    lines_parsed: AtomicU64,
    /// Total variables successfully extracted
    variables_extracted: AtomicU64,
    /// Total parse errors encountered
    parse_errors: AtomicU64,
    /// Last parse duration in nanoseconds
    last_parse_ns: AtomicU64,
    /// Padding to 256B cache-aligned boundary
    _padding: [u8; 224],
}

// Compile-time size/alignment verification (Q33)
const _: () = {
    assert!(core::mem::size_of::<DotenvParserCapsule>() == 256);
    assert!(core::mem::align_of::<DotenvParserCapsule>() == 64);
};

impl DotenvParserCapsule {
    /// Create a new parser capsule
    ///
    /// # Performance
    /// - <1ns (const initialization)
    pub const fn new() -> Self {
        Self {
            lines_parsed: AtomicU64::new(0),
            variables_extracted: AtomicU64::new(0),
            parse_errors: AtomicU64::new(0),
            last_parse_ns: AtomicU64::new(0),
            _padding: [0u8; 224],
        }
    }

    /// Parse .env file content
    ///
    /// # Performance
    /// - <100us for typical .env file (50 lines)
    ///
    /// # Arguments
    /// - `content`: Raw file content
    /// - `source_path`: Path for error reporting
    ///
    /// # Returns
    /// ParsedEnvFile with variables and any errors
    pub fn parse(&self, content: &str, source_path: &str) -> ParsedEnvFile {
        let start = std::time::Instant::now();

        let mut result = ParsedEnvFile::new(source_path);
        let mut line_num: u32 = 0;

        for line in content.lines() {
            line_num += 1;
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Check for dangerous patterns (security)
            if self.contains_command_substitution(trimmed) {
                result.errors.push(ParseError::new(
                    line_num,
                    1,
                    "Command substitution not allowed (security)",
                    ErrorSeverity::Error,
                ));
                continue;
            }

            // Parse KEY=value line
            match self.parse_line(trimmed, line_num) {
                Ok((key, value)) => result.variables.push((key, value)),
                Err(err) => result.errors.push(err),
            }
        }

        result.lines_processed = line_num;

        // Update atomic statistics
        self.lines_parsed.fetch_add(line_num as u64, Ordering::Relaxed);
        self.variables_extracted
            .fetch_add(result.variables.len() as u64, Ordering::Relaxed);
        self.parse_errors
            .fetch_add(result.errors.len() as u64, Ordering::Relaxed);
        self.last_parse_ns
            .store(start.elapsed().as_nanos() as u64, Ordering::Relaxed);

        result
    }

    /// Parse with variable expansion
    ///
    /// Expands ${VAR} and ${VAR:-default} references using the provided
    /// existing variables or environment.
    ///
    /// # Arguments
    /// - `content`: Raw file content
    /// - `source_path`: Path for error reporting
    /// - `existing`: Existing variables for expansion
    pub fn parse_with_expansion(
        &self,
        content: &str,
        source_path: &str,
        existing: &HashMap<String, String>,
    ) -> ParsedEnvFile {
        let parsed = self.parse(content, source_path);

        // Build combined variables for expansion (existing + newly parsed)
        let mut all_vars = existing.clone();
        for (k, v) in &parsed.variables {
            all_vars.insert(k.clone(), v.clone());
        }

        // Expand variables
        let expanded_vars: Vec<_> = parsed
            .variables
            .iter()
            .map(|(k, v)| {
                let expanded = self.expand_variables(v, &all_vars);
                (k.clone(), expanded)
            })
            .collect();

        ParsedEnvFile {
            variables: expanded_vars,
            errors: parsed.errors,
            source_path: parsed.source_path,
            lines_processed: parsed.lines_processed,
        }
    }

    /// Parse a single KEY=value line
    fn parse_line(&self, line: &str, line_num: u32) -> Result<(String, String), ParseError> {
        // Remove 'export ' prefix if present
        let line = line.strip_prefix("export ").unwrap_or(line).trim();

        // Find = separator
        let eq_pos = line.find('=').ok_or_else(|| ParseError {
            line: line_num,
            column: 1,
            message: "Missing '=' separator".to_string(),
            severity: ErrorSeverity::Error,
        })?;

        let key = line[..eq_pos].trim();
        let value = line[eq_pos + 1..].trim();

        // Validate key (must be valid identifier)
        if !is_valid_key(key) {
            return Err(ParseError {
                line: line_num,
                column: 1,
                message: format!("Invalid key: '{}' (must be alphanumeric or _)", key),
                severity: ErrorSeverity::Error,
            });
        }

        // Parse value (handle quotes)
        let parsed_value = self.parse_value(value, line_num)?;

        Ok((key.to_string(), parsed_value))
    }

    /// Parse value with quote handling
    fn parse_value(&self, value: &str, line_num: u32) -> Result<String, ParseError> {
        if value.is_empty() {
            return Ok(String::new());
        }

        // Double-quoted: "value" (supports escape sequences)
        if value.starts_with('"') {
            return self.parse_double_quoted(value, line_num);
        }

        // Single-quoted: 'value' (literal, no escaping)
        if value.starts_with('\'') {
            return self.parse_single_quoted(value, line_num);
        }

        // Unquoted: trim trailing comment
        let unquoted = value.split('#').next().unwrap_or(value).trim();
        Ok(unquoted.to_string())
    }

    /// Parse double-quoted string with escape sequences
    fn parse_double_quoted(&self, value: &str, line_num: u32) -> Result<String, ParseError> {
        // Find closing quote (accounting for escaped quotes)
        let content = &value[1..]; // Skip opening quote

        let mut end_pos = None;
        let mut i = 0;
        let chars: Vec<char> = content.chars().collect();

        while i < chars.len() {
            if chars[i] == '\\' && i + 1 < chars.len() {
                i += 2; // Skip escaped character
                continue;
            }
            if chars[i] == '"' {
                end_pos = Some(i);
                break;
            }
            i += 1;
        }

        let end = end_pos.ok_or_else(|| ParseError {
            line: line_num,
            column: 1,
            message: "Unterminated double quote".to_string(),
            severity: ErrorSeverity::Error,
        })?;

        let inner: String = chars[..end].iter().collect();
        Ok(self.unescape(&inner))
    }

    /// Parse single-quoted string (literal, no escaping)
    fn parse_single_quoted(&self, value: &str, line_num: u32) -> Result<String, ParseError> {
        // Find closing quote
        let content = &value[1..]; // Skip opening quote

        let end_pos = content.find('\'').ok_or_else(|| ParseError {
            line: line_num,
            column: 1,
            message: "Unterminated single quote".to_string(),
            severity: ErrorSeverity::Error,
        })?;

        Ok(content[..end_pos].to_string())
    }

    /// Unescape double-quoted string (\n, \t, \r, \", \\)
    fn unescape(&self, s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\\' {
                match chars.next() {
                    Some('n') => result.push('\n'),
                    Some('t') => result.push('\t'),
                    Some('r') => result.push('\r'),
                    Some('"') => result.push('"'),
                    Some('\\') => result.push('\\'),
                    Some(other) => {
                        result.push('\\');
                        result.push(other);
                    }
                    None => result.push('\\'),
                }
            } else {
                result.push(ch);
            }
        }

        result
    }

    /// Expand ${VAR} and ${VAR:-default} references
    fn expand_variables(&self, value: &str, vars: &HashMap<String, String>) -> String {
        let mut result = value.to_string();

        // Expand ${VAR} and ${VAR:-default} patterns
        while let Some(start) = result.find("${") {
            if let Some(rel_end) = result[start..].find('}') {
                let end = start + rel_end;
                let var_expr = &result[start + 2..end];

                // Check for default syntax ${VAR:-default}
                let (var_name, default) = if let Some(colon_pos) = var_expr.find(":-") {
                    (&var_expr[..colon_pos], Some(&var_expr[colon_pos + 2..]))
                } else {
                    (var_expr, None)
                };

                // Resolve variable
                let resolved = vars
                    .get(var_name)
                    .cloned()
                    .or_else(|| std::env::var(var_name).ok())
                    .or_else(|| default.map(|d| d.to_string()))
                    .unwrap_or_default();

                result.replace_range(start..=end, &resolved);
            } else {
                // No closing brace, stop expanding
                break;
            }
        }

        result
    }

    /// Check for dangerous command substitution patterns
    #[inline]
    fn contains_command_substitution(&self, line: &str) -> bool {
        // Check for backtick command substitution: `command`
        if line.contains('`') {
            return true;
        }

        // Check for $() command expansion (but not ${} variable expansion)
        // We need to differentiate $(command) from ${VAR}
        let mut chars = line.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '$' {
                if let Some(&next) = chars.peek() {
                    if next == '(' {
                        return true;
                    }
                }
            }
        }

        false
    }

    // ========== Statistics Methods ==========

    /// Get total lines parsed
    #[inline]
    pub fn lines_parsed(&self) -> u64 {
        self.lines_parsed.load(Ordering::Relaxed)
    }

    /// Get total variables extracted
    #[inline]
    pub fn variables_extracted(&self) -> u64 {
        self.variables_extracted.load(Ordering::Relaxed)
    }

    /// Get total parse errors
    #[inline]
    pub fn parse_errors(&self) -> u64 {
        self.parse_errors.load(Ordering::Relaxed)
    }

    /// Get last parse duration in nanoseconds
    #[inline]
    pub fn last_parse_ns(&self) -> u64 {
        self.last_parse_ns.load(Ordering::Relaxed)
    }

    /// Reset statistics (for testing)
    #[cfg(test)]
    pub fn reset_stats(&self) {
        self.lines_parsed.store(0, Ordering::Relaxed);
        self.variables_extracted.store(0, Ordering::Relaxed);
        self.parse_errors.store(0, Ordering::Relaxed);
        self.last_parse_ns.store(0, Ordering::Relaxed);
    }
}

impl Default for DotenvParserCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Check if a key is a valid environment variable identifier
///
/// Valid keys must:
/// - Be non-empty
/// - Start with letter or underscore
/// - Contain only alphanumeric characters or underscores
#[inline]
fn is_valid_key(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }

    let mut chars = key.chars();

    // First character must be letter or underscore
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }

    // Rest must be alphanumeric or underscore
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ============================================================================
// Tests (T28 Q1-Q7 Unit Tests)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    // ========== Q1-Q2: Size and Alignment ==========

    #[test]
    fn test_dotenv_parser_size() {
        assert_eq!(
            size_of::<DotenvParserCapsule>(),
            256,
            "DotenvParserCapsule must be exactly 256 bytes"
        );
    }

    #[test]
    fn test_dotenv_parser_alignment() {
        assert_eq!(
            align_of::<DotenvParserCapsule>(),
            64,
            "DotenvParserCapsule must be 64-byte aligned"
        );
    }

    // ========== Q3: Basic Parsing ==========

    #[test]
    fn test_parse_simple() {
        let parser = DotenvParserCapsule::new();
        let result = parser.parse("KEY=value", ".env");

        assert!(result.is_ok());
        assert_eq!(result.variables.len(), 1);
        assert_eq!(result.variables[0], ("KEY".to_string(), "value".to_string()));
    }

    #[test]
    fn test_parse_with_comments() {
        let parser = DotenvParserCapsule::new();
        let content = r#"
# This is a comment
KEY=value
# Another comment
OTHER=stuff  # inline comment
"#;
        let result = parser.parse(content, ".env");

        assert!(result.is_ok());
        assert_eq!(result.variables.len(), 2);
        assert_eq!(result.variables[0], ("KEY".to_string(), "value".to_string()));
        assert_eq!(result.variables[1], ("OTHER".to_string(), "stuff".to_string()));
    }

    #[test]
    fn test_parse_double_quoted() {
        let parser = DotenvParserCapsule::new();
        let result = parser.parse(r#"KEY="value with spaces""#, ".env");

        assert!(result.is_ok());
        assert_eq!(result.variables.len(), 1);
        assert_eq!(
            result.variables[0],
            ("KEY".to_string(), "value with spaces".to_string())
        );
    }

    #[test]
    fn test_parse_single_quoted() {
        let parser = DotenvParserCapsule::new();
        let result = parser.parse("KEY='literal value'", ".env");

        assert!(result.is_ok());
        assert_eq!(result.variables.len(), 1);
        assert_eq!(
            result.variables[0],
            ("KEY".to_string(), "literal value".to_string())
        );
    }

    #[test]
    fn test_parse_escape_sequences() {
        let parser = DotenvParserCapsule::new();
        let result = parser.parse(r#"KEY="line1\nline2\ttab""#, ".env");

        assert!(result.is_ok());
        assert_eq!(result.variables.len(), 1);
        assert_eq!(
            result.variables[0],
            ("KEY".to_string(), "line1\nline2\ttab".to_string())
        );
    }

    #[test]
    fn test_parse_export_prefix() {
        let parser = DotenvParserCapsule::new();
        let result = parser.parse("export KEY=value", ".env");

        assert!(result.is_ok());
        assert_eq!(result.variables.len(), 1);
        assert_eq!(result.variables[0], ("KEY".to_string(), "value".to_string()));
    }

    #[test]
    fn test_parse_variable_expansion() {
        let parser = DotenvParserCapsule::new();
        let mut existing = HashMap::new();
        existing.insert("OTHER".to_string(), "expanded".to_string());

        let result = parser.parse_with_expansion("KEY=${OTHER}", ".env", &existing);

        assert!(result.is_ok());
        assert_eq!(result.variables.len(), 1);
        assert_eq!(
            result.variables[0],
            ("KEY".to_string(), "expanded".to_string())
        );
    }

    #[test]
    fn test_parse_expansion_with_default() {
        let parser = DotenvParserCapsule::new();
        let existing = HashMap::new(); // Empty - MISSING not defined

        let result = parser.parse_with_expansion("KEY=${MISSING:-default}", ".env", &existing);

        assert!(result.is_ok());
        assert_eq!(result.variables.len(), 1);
        assert_eq!(
            result.variables[0],
            ("KEY".to_string(), "default".to_string())
        );
    }

    #[test]
    fn test_parse_empty_value() {
        let parser = DotenvParserCapsule::new();
        let result = parser.parse("KEY=", ".env");

        assert!(result.is_ok());
        assert_eq!(result.variables.len(), 1);
        assert_eq!(result.variables[0], ("KEY".to_string(), "".to_string()));
    }

    // ========== Q4: Error Handling ==========

    #[test]
    fn test_parse_unterminated_quote() {
        let parser = DotenvParserCapsule::new();
        let result = parser.parse(r#"KEY="unterminated"#, ".env");

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].severity, ErrorSeverity::Error);
        assert!(result.errors[0].message.contains("Unterminated"));
    }

    #[test]
    fn test_parse_invalid_key() {
        let parser = DotenvParserCapsule::new();
        let result = parser.parse("123KEY=value", ".env");

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].severity, ErrorSeverity::Error);
        assert!(result.errors[0].message.contains("Invalid key"));
    }

    // ========== Q5: Multi-line Files ==========

    #[test]
    fn test_parse_multiline_file() {
        let parser = DotenvParserCapsule::new();
        let content = r#"
# Database configuration
DB_HOST=localhost
DB_PORT=5432
DB_USER=admin
DB_PASSWORD="secret123"

# Application settings
APP_NAME='My App'
APP_DEBUG=true
"#;
        let result = parser.parse(content, ".env");

        assert!(result.is_ok());
        assert_eq!(result.variables.len(), 6);

        let map = result.to_hashmap();
        assert_eq!(map.get("DB_HOST"), Some(&"localhost".to_string()));
        assert_eq!(map.get("DB_PORT"), Some(&"5432".to_string()));
        assert_eq!(map.get("DB_USER"), Some(&"admin".to_string()));
        assert_eq!(map.get("DB_PASSWORD"), Some(&"secret123".to_string()));
        assert_eq!(map.get("APP_NAME"), Some(&"My App".to_string()));
        assert_eq!(map.get("APP_DEBUG"), Some(&"true".to_string()));
    }

    // ========== Q6: Statistics ==========

    #[test]
    fn test_statistics() {
        let parser = DotenvParserCapsule::new();
        parser.reset_stats();

        // Parse a file with some variables and errors
        let content = r#"
VALID1=value1
VALID2=value2
123INVALID=value
VALID3=value3
"#;
        let _result = parser.parse(content, ".env");

        // Check statistics updated
        assert!(parser.lines_parsed() > 0);
        assert_eq!(parser.variables_extracted(), 3);
        assert_eq!(parser.parse_errors(), 1);
        assert!(parser.last_parse_ns() > 0);
    }

    // ========== Q7: Security ==========

    #[test]
    fn test_command_substitution_blocked() {
        let parser = DotenvParserCapsule::new();

        // Backtick command substitution should be blocked
        let result = parser.parse("KEY=`whoami`", ".env");
        assert!(result.has_errors());
        assert!(result.errors[0].message.contains("Command substitution"));

        // $() command expansion should be blocked
        let result = parser.parse("KEY=$(whoami)", ".env");
        assert!(result.has_errors());
        assert!(result.errors[0].message.contains("Command substitution"));
    }

    #[test]
    fn test_variable_expansion_allowed() {
        let parser = DotenvParserCapsule::new();

        // ${VAR} should NOT be blocked (it's variable expansion, not command)
        let result = parser.parse("KEY=${OTHER}", ".env");
        assert!(result.is_ok());
        assert_eq!(result.variables.len(), 1);
    }

    // ========== Additional Edge Cases ==========

    #[test]
    fn test_const_new() {
        // Verify const construction works
        static PARSER: DotenvParserCapsule = DotenvParserCapsule::new();
        assert_eq!(PARSER.lines_parsed(), 0);
    }

    #[test]
    fn test_is_valid_key() {
        // Valid keys
        assert!(is_valid_key("KEY"));
        assert!(is_valid_key("_KEY"));
        assert!(is_valid_key("KEY_NAME"));
        assert!(is_valid_key("key123"));
        assert!(is_valid_key("_123"));

        // Invalid keys
        assert!(!is_valid_key("")); // Empty
        assert!(!is_valid_key("123KEY")); // Starts with number
        assert!(!is_valid_key("KEY-NAME")); // Contains dash
        assert!(!is_valid_key("KEY.NAME")); // Contains dot
        assert!(!is_valid_key("KEY NAME")); // Contains space
    }

    #[test]
    fn test_escaped_quote_in_double_quoted() {
        let parser = DotenvParserCapsule::new();
        let result = parser.parse(r#"KEY="value with \"escaped\" quotes""#, ".env");

        assert!(result.is_ok());
        assert_eq!(result.variables.len(), 1);
        assert_eq!(
            result.variables[0],
            ("KEY".to_string(), "value with \"escaped\" quotes".to_string())
        );
    }

    #[test]
    fn test_backslash_in_double_quoted() {
        let parser = DotenvParserCapsule::new();
        let result = parser.parse(r#"KEY="path\\to\\file""#, ".env");

        assert!(result.is_ok());
        assert_eq!(result.variables.len(), 1);
        assert_eq!(
            result.variables[0],
            ("KEY".to_string(), "path\\to\\file".to_string())
        );
    }
}
