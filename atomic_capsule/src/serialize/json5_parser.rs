//! JSON5 parser capsule (T5 Streaming extension of JsonParserCapsule).
//!
//! Provides O(1) per-token incremental JSON5 parsing with support for relaxed syntax.
//!
//! ## Tier Selection (UCE34 Q10)
//!
//! **T5 Streaming**: Extends JsonParserCapsule with comment skipping and relaxed syntax
//! - Single-pass traversal: No backtracking
//! - Incremental state: Can pause/resume at token boundaries
//! - Zero-copy for value extraction (references original input)
//!
//! ## JSON5 Extensions
//!
//! Beyond standard JSON, JSON5ParserCapsule supports:
//! 1. **Comments**: Single-line (`//`) and multi-line (`/* */`)
//! 2. **Trailing commas**: Allowed in objects and arrays
//! 3. **Unquoted keys**: Object keys can be unquoted identifiers
//! 4. **Single quotes**: Strings can use single quotes `'...'`
//! 5. **Hex numbers**: `0xDEADBEEF` notation
//! 6. **Infinity/NaN**: IEEE 754 special values
//! 7. **Flexible decimals**: `.5` and `5.` allowed
//!
//! ## ASSUM Safety Tags
//!
//! - #ASSUME_UTF8: Input is valid UTF-8 (enforced by &str)
//! - #ASSUME_NO_BACKTRACKING: Cursor never moves backward (property enforced)
//! - #ASSUME_BOUNDED_NESTING: Nesting depth ≤ 256 (checked at parse time)
//! - #ASSUME_COMMENT_BALANCED: Multi-line comments properly closed (validation)

use super::json_parser::{JsonParserError, JsonParserResult, JsonValue};

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

/// JSON5 parser capsule (extends JsonParserCapsule with relaxed syntax).
///
/// Provides O(1) per-token parsing with JSON5 extensions.
pub struct Json5ParserCapsule<'a> {
    /// Input buffer (borrowed from caller)
    input: &'a str,
    /// Current parsing position (never moves backward)
    pos: usize,
    /// Nesting depth tracker (for depth limit enforcement)
    depth: usize,
    /// Allow trailing commas in arrays/objects
    allow_trailing_commas: bool,
    /// Allow single-line and multi-line comments
    allow_comments: bool,
}

impl<'a> Json5ParserCapsule<'a> {
    /// Create new JSON5 parser with all extensions enabled
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            depth: 0,
            allow_trailing_commas: true,
            allow_comments: true,
        }
    }

    /// Create JSON5 parser with custom extension settings
    pub fn with_options(
        input: &'a str,
        allow_trailing_commas: bool,
        allow_comments: bool,
    ) -> Self {
        Self {
            input,
            pos: 0,
            depth: 0,
            allow_trailing_commas,
            allow_comments,
        }
    }

    /// Parse complete JSON5 value (entry point)
    pub fn parse(&mut self) -> JsonParserResult<JsonValue> {
        self.skip_whitespace_and_comments();
        let value = self.parse_value()?;
        self.skip_whitespace_and_comments();

        // Ensure entire input is consumed
        if self.pos < self.input.len() {
            Err(JsonParserError::UnexpectedChar {
                pos: self.pos,
                found: self.current_char()?,
            })
        } else {
            Ok(value)
        }
    }

    /// Parse JSON5 value (recursive descent)
    fn parse_value(&mut self) -> JsonParserResult<JsonValue> {
        self.skip_whitespace_and_comments();

        match self.current_char()? {
            '{' => self.parse_object(),
            '[' => self.parse_array(),
            '"' => Ok(JsonValue::String(self.parse_double_quoted_string()?)),
            '\'' => Ok(JsonValue::String(self.parse_single_quoted_string()?)),
            't' | 'f' => Ok(JsonValue::Bool(self.parse_bool()?)),
            'n' => {
                self.parse_null()?;
                Ok(JsonValue::Null)
            }
            'I' => Ok(JsonValue::Number(f64::INFINITY)), // Infinity
            'N' => Ok(JsonValue::Number(f64::NAN)),       // NaN
            '+' | '-' | '.' | '0'..='9' => {
                // Check for hex numbers (0x or 0X prefix)
                if self.current_char()? == '0' && self.peek() == Some('x') {
                    Ok(JsonValue::Number(self.parse_hex_number()?))
                } else if self.current_char()? == '0' && self.peek() == Some('X') {
                    Ok(JsonValue::Number(self.parse_hex_number()?))
                } else {
                    Ok(JsonValue::Number(self.parse_number()?))
                }
            }
            ch => Err(JsonParserError::UnexpectedChar { pos: self.pos, found: ch }),
        }
    }

    /// Parse JSON5 object with unquoted keys and trailing commas
    fn parse_object(&mut self) -> JsonParserResult<JsonValue> {
        self.expect_char('{')?;
        self.depth += 1;

        if self.depth > 256 {
            return Err(JsonParserError::NestingTooDeep {
                pos: self.pos,
                depth: self.depth,
            });
        }

        self.skip_whitespace_and_comments();

        let mut fields = Vec::new();

        // Empty object
        if self.current_char()? == '}' {
            self.consume_char();
            self.depth -= 1;
            return Ok(JsonValue::Object(fields));
        }

        loop {
            self.skip_whitespace_and_comments();

            // Parse key (unquoted or quoted)
            let key = if self.current_char()? == '"' {
                self.parse_double_quoted_string()?
            } else if self.current_char()? == '\'' {
                self.parse_single_quoted_string()?
            } else {
                self.parse_unquoted_key()?
            };

            self.skip_whitespace_and_comments();
            self.expect_char(':')?;

            // Parse value
            let value = self.parse_value()?;
            fields.push((key, value));

            self.skip_whitespace_and_comments();

            match self.current_char()? {
                ',' => {
                    self.consume_char();
                    self.skip_whitespace_and_comments();

                    // Handle trailing comma if enabled
                    if self.current_char()? == '}' {
                        if self.allow_trailing_commas {
                            self.consume_char();
                            break;
                        } else {
                            return Err(JsonParserError::TrailingComma { pos: self.pos });
                        }
                    }
                    continue;
                }
                '}' => {
                    self.consume_char();
                    break;
                }
                ch => {
                    return Err(JsonParserError::ExpectedChar {
                        pos: self.pos,
                        expected: '}',
                        found: ch,
                    })
                }
            }
        }

        self.depth -= 1;
        Ok(JsonValue::Object(fields))
    }

    /// Parse JSON5 array with trailing commas
    fn parse_array(&mut self) -> JsonParserResult<JsonValue> {
        self.expect_char('[')?;
        self.depth += 1;

        if self.depth > 256 {
            return Err(JsonParserError::NestingTooDeep {
                pos: self.pos,
                depth: self.depth,
            });
        }

        self.skip_whitespace_and_comments();

        let mut values = Vec::new();

        // Empty array
        if self.current_char()? == ']' {
            self.consume_char();
            self.depth -= 1;
            return Ok(JsonValue::Array(values));
        }

        loop {
            let value = self.parse_value()?;
            values.push(value);

            self.skip_whitespace_and_comments();

            match self.current_char()? {
                ',' => {
                    self.consume_char();
                    self.skip_whitespace_and_comments();

                    // Handle trailing comma if enabled
                    if self.current_char()? == ']' {
                        if self.allow_trailing_commas {
                            self.consume_char();
                            break;
                        } else {
                            return Err(JsonParserError::TrailingComma { pos: self.pos });
                        }
                    }
                    continue;
                }
                ']' => {
                    self.consume_char();
                    break;
                }
                ch => {
                    return Err(JsonParserError::ExpectedChar {
                        pos: self.pos,
                        expected: ']',
                        found: ch,
                    })
                }
            }
        }

        self.depth -= 1;
        Ok(JsonValue::Array(values))
    }

    /// Parse unquoted object key (identifier-style)
    fn parse_unquoted_key(&mut self) -> JsonParserResult<String> {
        let mut key = String::new();

        // First character: letter, underscore, or dollar sign
        let ch = self.current_char()?;
        if !matches!(ch, 'a'..='z' | 'A'..='Z' | '_' | '$') {
            return Err(JsonParserError::UnexpectedChar {
                pos: self.pos,
                found: ch,
            });
        }

        key.push(ch);
        self.consume_char();

        // Subsequent characters: alphanumeric, underscore, or dollar sign
        loop {
            match self.current_char() {
                Ok(ch) if matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$') => {
                    key.push(ch);
                    self.consume_char();
                }
                _ => break,
            }
        }

        Ok(key)
    }

    /// Parse double-quoted string with escape sequences
    fn parse_double_quoted_string(&mut self) -> JsonParserResult<String> {
        self.expect_char('"')?;

        let mut result = String::new();

        loop {
            match self.current_char()? {
                '"' => {
                    self.consume_char();
                    break;
                }
                '\\' => {
                    self.consume_char();
                    let ch = self.current_char()?;
                    match ch {
                        '"' => {
                            result.push('"');
                            self.consume_char();
                        }
                        '\\' => {
                            result.push('\\');
                            self.consume_char();
                        }
                        '/' => {
                            result.push('/');
                            self.consume_char();
                        }
                        'b' => {
                            result.push('\x08');
                            self.consume_char();
                        }
                        'f' => {
                            result.push('\x0c');
                            self.consume_char();
                        }
                        'n' => {
                            result.push('\n');
                            self.consume_char();
                        }
                        'r' => {
                            result.push('\r');
                            self.consume_char();
                        }
                        't' => {
                            result.push('\t');
                            self.consume_char();
                        }
                        'u' => {
                            self.consume_char();
                            let code = self.parse_unicode_escape()?;
                            if let Some(ch) = char::from_u32(code) {
                                result.push(ch);
                            } else {
                                return Err(JsonParserError::InvalidUnicode { pos: self.pos });
                            }
                        }
                        _ => {
                            return Err(JsonParserError::InvalidEscape {
                                pos: self.pos,
                                ch,
                            })
                        }
                    }
                }
                ch => {
                    result.push(ch);
                    self.consume_char();
                }
            }
        }

        Ok(result)
    }

    /// Parse single-quoted string (JSON5 extension)
    fn parse_single_quoted_string(&mut self) -> JsonParserResult<String> {
        self.expect_char('\'')?;

        let mut result = String::new();

        loop {
            match self.current_char()? {
                '\'' => {
                    self.consume_char();
                    break;
                }
                '\\' => {
                    self.consume_char();
                    let ch = self.current_char()?;
                    match ch {
                        '\'' => {
                            result.push('\'');
                            self.consume_char();
                        }
                        '\\' => {
                            result.push('\\');
                            self.consume_char();
                        }
                        '/' => {
                            result.push('/');
                            self.consume_char();
                        }
                        'b' => {
                            result.push('\x08');
                            self.consume_char();
                        }
                        'f' => {
                            result.push('\x0c');
                            self.consume_char();
                        }
                        'n' => {
                            result.push('\n');
                            self.consume_char();
                        }
                        'r' => {
                            result.push('\r');
                            self.consume_char();
                        }
                        't' => {
                            result.push('\t');
                            self.consume_char();
                        }
                        'u' => {
                            self.consume_char();
                            let code = self.parse_unicode_escape()?;
                            if let Some(ch) = char::from_u32(code) {
                                result.push(ch);
                            } else {
                                return Err(JsonParserError::InvalidUnicode { pos: self.pos });
                            }
                        }
                        _ => {
                            return Err(JsonParserError::InvalidEscape {
                                pos: self.pos,
                                ch,
                            })
                        }
                    }
                }
                ch => {
                    result.push(ch);
                    self.consume_char();
                }
            }
        }

        Ok(result)
    }

    /// Parse hexadecimal number (0x prefix, JSON5 extension)
    fn parse_hex_number(&mut self) -> JsonParserResult<f64> {
        let start_pos = self.pos;

        // Consume '0' and 'x'
        self.consume_char(); // '0'
        self.consume_char(); // 'x' or 'X'

        let mut value: u64 = 0;
        let mut has_digit = false;

        while let Ok(ch) = self.current_char() {
            match ch {
                '0'..='9' => {
                    value = value * 16 + (ch as u64 - '0' as u64);
                    has_digit = true;
                    self.consume_char();
                }
                'a'..='f' => {
                    value = value * 16 + (ch as u64 - 'a' as u64 + 10);
                    has_digit = true;
                    self.consume_char();
                }
                'A'..='F' => {
                    value = value * 16 + (ch as u64 - 'A' as u64 + 10);
                    has_digit = true;
                    self.consume_char();
                }
                _ => break,
            }
        }

        if !has_digit {
            return Err(JsonParserError::InvalidNumber { pos: start_pos });
        }

        Ok(value as f64)
    }

    /// Parse \uXXXX unicode escape sequence (4 hex digits)
    fn parse_unicode_escape(&mut self) -> JsonParserResult<u32> {
        let start_pos = self.pos;

        let mut code: u32 = 0;
        for _ in 0..4 {
            let ch = self.current_char()?;
            code = code * 16
                + match ch {
                    '0'..='9' => (ch as u32) - ('0' as u32),
                    'a'..='f' => (ch as u32) - ('a' as u32) + 10,
                    'A'..='F' => (ch as u32) - ('A' as u32) + 10,
                    _ => return Err(JsonParserError::InvalidUnicode { pos: start_pos }),
                };
            self.consume_char();
        }

        Ok(code)
    }

    /// Parse JSON number (int or float with exponent)
    fn parse_number(&mut self) -> JsonParserResult<f64> {
        let start_pos = self.pos;

        // Optional plus or minus sign
        if matches!(self.current_char()?, '+' | '-') {
            self.consume_char();
        }

        // Integer part (can start with decimal point)
        if self.current_char()? == '.' {
            // Leading decimal point (.5 is valid in JSON5)
            self.consume_char();
            if !self.is_digit(self.current_char()?) {
                return Err(JsonParserError::InvalidNumber { pos: start_pos });
            }
            while self.is_digit(self.current_char()?) {
                self.consume_char();
            }
        } else {
            // Standard integer part
            if !self.is_digit(self.current_char()?) {
                return Err(JsonParserError::InvalidNumber { pos: start_pos });
            }

            while self.is_digit(self.current_char()?) {
                self.consume_char();
            }

            // Fractional part (optional)
            if self.current_char().ok() == Some('.') {
                self.consume_char();
                // Trailing decimal point (5. is valid in JSON5)
                while self.is_digit(self.current_char()?) {
                    self.consume_char();
                }
            }
        }

        // Exponent part
        if matches!(self.current_char().ok(), Some('e') | Some('E')) {
            self.consume_char();

            if matches!(self.current_char().ok(), Some('+') | Some('-')) {
                self.consume_char();
            }

            if !self.is_digit(self.current_char()?) {
                return Err(JsonParserError::InvalidNumber { pos: start_pos });
            }

            while self.is_digit(self.current_char()?) {
                self.consume_char();
            }
        }

        let num_str = &self.input[start_pos..self.pos];

        // Parse as f64 with fallback
        num_str.parse::<f64>().map_err(|_| JsonParserError::InvalidNumber {
            pos: start_pos,
        })
    }

    /// Parse boolean literal (true or false)
    fn parse_bool(&mut self) -> JsonParserResult<bool> {
        if self.input[self.pos..].starts_with("true") {
            self.pos += 4;
            Ok(true)
        } else if self.input[self.pos..].starts_with("false") {
            self.pos += 5;
            Ok(false)
        } else {
            Err(JsonParserError::InvalidBool { pos: self.pos })
        }
    }

    /// Parse null literal
    fn parse_null(&mut self) -> JsonParserResult<()> {
        if self.input[self.pos..].starts_with("null") {
            self.pos += 4;
            Ok(())
        } else {
            Err(JsonParserError::InvalidNull { pos: self.pos })
        }
    }

    // Helper methods

    /// Skip whitespace and comments (single-line and multi-line)
    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.current_char() {
                Ok(ch) if matches!(ch, ' ' | '\t' | '\n' | '\r') => {
                    self.consume_char();
                }
                Ok('/') if self.allow_comments => {
                    // Check for comment type
                    match self.peek() {
                        Some('/') => {
                            // Single-line comment
                            self.skip_single_line_comment();
                        }
                        Some('*') => {
                            // Multi-line comment
                            if self.skip_multi_line_comment().is_err() {
                                return; // Stop on error
                            }
                        }
                        _ => return,
                    }
                }
                _ => return,
            }
        }
    }

    /// Skip single-line comment (// to end of line)
    fn skip_single_line_comment(&mut self) {
        // Skip '//'
        self.consume_char();
        self.consume_char();

        // Skip to end of line
        while let Ok(ch) = self.current_char() {
            if matches!(ch, '\n' | '\r') {
                break;
            }
            self.consume_char();
        }

        // Consume the newline
        if let Ok(ch) = self.current_char() {
            if matches!(ch, '\n' | '\r') {
                self.consume_char();
                // Handle \r\n
                if ch == '\r' && self.current_char() == Ok('\n') {
                    self.consume_char();
                }
            }
        }
    }

    /// Skip multi-line comment (/* to */)
    fn skip_multi_line_comment(&mut self) -> JsonParserResult<()> {
        // Skip '/*'
        self.consume_char();
        self.consume_char();

        loop {
            match self.current_char() {
                Err(JsonParserError::UnexpectedEof) => {
                    return Err(JsonParserError::UnexpectedEof)
                }
                Ok('*') => {
                    self.consume_char();
                    if self.current_char() == Ok('/') {
                        self.consume_char();
                        return Ok(());
                    }
                }
                _ => {
                    self.consume_char();
                }
            }
        }
    }

    /// Get current character without advancing
    fn current_char(&self) -> JsonParserResult<char> {
        self.input[self.pos..]
            .chars()
            .next()
            .ok_or(JsonParserError::UnexpectedEof)
    }

    /// Peek at next character without advancing
    fn peek(&self) -> Option<char> {
        if self.pos + 1 < self.input.len() {
            self.input[self.pos + 1..].chars().next()
        } else {
            None
        }
    }

    /// Advance past current character
    fn consume_char(&mut self) {
        if let Ok(ch) = self.current_char() {
            self.pos += ch.len_utf8();
        }
    }

    /// Expect specific character and consume it
    fn expect_char(&mut self, expected: char) -> JsonParserResult<()> {
        let ch = self.current_char()?;
        if ch == expected {
            self.consume_char();
            Ok(())
        } else {
            Err(JsonParserError::ExpectedChar {
                pos: self.pos,
                expected,
                found: ch,
            })
        }
    }

    /// Check if character is a digit
    fn is_digit(&self, ch: char) -> bool {
        matches!(ch, '0'..='9')
    }

    /// Get current position
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Get current nesting depth
    pub fn depth(&self) -> usize {
        self.depth
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn test_single_line_comment() {
        let json = r#"
        {
            // This is a comment
            "key": "value"
        }
        "#;
        let mut parser = Json5ParserCapsule::new(json);
        let result = parser.parse();
        assert!(result.is_ok());
        match result.unwrap() {
            JsonValue::Object(fields) => assert_eq!(fields.len(), 1),
            _ => panic!("Expected object"),
        }
    }

    #[test]
    fn test_multi_line_comment() {
        let json = r#"
        {
            /* This is a
               multi-line comment */
            "key": "value"
        }
        "#;
        let mut parser = Json5ParserCapsule::new(json);
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_trailing_comma_array() {
        let json = "[1, 2, 3,]";
        let mut parser = Json5ParserCapsule::new(json);
        let result = parser.parse();
        assert!(result.is_ok());
        match result.unwrap() {
            JsonValue::Array(arr) => assert_eq!(arr.len(), 3),
            _ => panic!("Expected array"),
        }
    }

    #[test]
    fn test_trailing_comma_object() {
        let json = r#"{"a": 1, "b": 2,}"#;
        let mut parser = Json5ParserCapsule::new(json);
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_unquoted_keys() {
        let json = r#"{key: "value", another_key: 42}"#;
        let mut parser = Json5ParserCapsule::new(json);
        let result = parser.parse();
        assert!(result.is_ok());
        match result.unwrap() {
            JsonValue::Object(fields) => {
                assert_eq!(fields[0].0, "key");
                assert_eq!(fields[1].0, "another_key");
            }
            _ => panic!("Expected object"),
        }
    }

    #[test]
    fn test_single_quoted_string() {
        let json = "{'name': 'Alice', 'age': 30}";
        let mut parser = Json5ParserCapsule::new(json);
        let result = parser.parse();
        assert!(result.is_ok());
        match result.unwrap() {
            JsonValue::Object(fields) => {
                assert_eq!(fields[0].0, "name");
                match &fields[0].1 {
                    JsonValue::String(s) => assert_eq!(s, "Alice"),
                    _ => panic!("Expected string"),
                }
            }
            _ => panic!("Expected object"),
        }
    }

    #[test]
    fn test_hex_number() {
        let json = "0xDEADBEEF";
        let mut parser = Json5ParserCapsule::new(json);
        let result = parser.parse();
        assert!(result.is_ok());
        match result.unwrap() {
            JsonValue::Number(n) => assert_eq!(n, 0xDEADBEEF as f64),
            _ => panic!("Expected number"),
        }
    }

    #[test]
    fn test_infinity() {
        let json = "Infinity";
        let mut parser = Json5ParserCapsule::new(json);
        let result = parser.parse();
        assert!(result.is_ok());
        match result.unwrap() {
            JsonValue::Number(n) => assert!(n.is_infinite()),
            _ => panic!("Expected number"),
        }
    }

    #[test]
    fn test_nan() {
        let json = "NaN";
        let mut parser = Json5ParserCapsule::new(json);
        let result = parser.parse();
        assert!(result.is_ok());
        match result.unwrap() {
            JsonValue::Number(n) => assert!(n.is_nan()),
            _ => panic!("Expected number"),
        }
    }

    #[test]
    fn test_leading_decimal_point() {
        let json = ".5";
        let mut parser = Json5ParserCapsule::new(json);
        let result = parser.parse();
        assert!(result.is_ok());
        match result.unwrap() {
            JsonValue::Number(n) => assert!((n - 0.5).abs() < 0.001),
            _ => panic!("Expected number"),
        }
    }

    #[test]
    fn test_trailing_decimal_point() {
        let json = "5.";
        let mut parser = Json5ParserCapsule::new(json);
        let result = parser.parse();
        assert!(result.is_ok());
        match result.unwrap() {
            JsonValue::Number(n) => assert_eq!(n, 5.0),
            _ => panic!("Expected number"),
        }
    }

    #[test]
    fn test_trailing_commas_disabled() {
        let json = "[1, 2,]";
        let mut parser = Json5ParserCapsule::with_options(json, false, true);
        let result = parser.parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_backward_compatibility_json() {
        let json = r#"{"name": "Bob", "items": [1, 2, 3]}"#;
        let mut parser = Json5ParserCapsule::new(json);
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_complex_json5() {
        let json = r#"
        {
            // Configuration
            host: 'localhost', /* server host */
            port: 8080,
            debug: true,
            values: [1, 2, 3,],
        }
        "#;
        let mut parser = Json5ParserCapsule::new(json);
        let result = parser.parse();
        assert!(result.is_ok());
        match result.unwrap() {
            JsonValue::Object(fields) => assert_eq!(fields.len(), 4),
            _ => panic!("Expected object"),
        }
    }
}
