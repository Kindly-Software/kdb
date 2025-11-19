//! Serialization helper utilities for atomic_capsule migration.
//!
//! Provides traits and macros to simplify CapsuleSerialize implementations,
//! eliminating the need for serde by offering lightweight alternatives for
//! common serialization patterns.
//!
//! # Overview
//!
//! This module provides:
//! - `WriteJson` trait for types that can serialize to JSON
//! - `ParseJson` trait for types that can deserialize from JSON
//! - Helper functions for struct field serialization
//! - Convenience macros for implementing serialization

use core::fmt;

/// Trait for types that can write themselves to JSON output.
///
/// # Safety
/// Implementations must produce valid JSON that matches the type's semantics.
///
/// # Examples
///
/// ```ignore
/// impl WriteJson for MyStruct {
///     fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), Error> {
///         writer.start_object()?;
///         write_field(writer, "field1", &self.field1, &mut true)?;
///         writer.end_object()
///     }
/// }
/// ```
pub trait WriteJson {
    /// Write this value as JSON to the provided writer.
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError>;
}

/// Trait for types that can parse themselves from JSON.
///
/// # Safety
/// Implementations must validate input and return appropriate errors for invalid data.
pub trait ParseJson: Sized {
    /// Parse a JSON value into this type.
    fn parse_json(value: &JsonValue) -> Result<Self, JsonError>;
}

/// JSON serialization errors.
#[derive(Debug, Clone)]
pub enum JsonError {
    /// Invalid JSON structure
    InvalidJson(String),
    /// Type mismatch (expected type X, got type Y)
    TypeMismatch(String),
    /// Missing required field
    MissingField(String),
    /// Custom error message
    Custom(String),
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonError::InvalidJson(msg) => write!(f, "Invalid JSON: {}", msg),
            JsonError::TypeMismatch(msg) => write!(f, "Type mismatch: {}", msg),
            JsonError::MissingField(name) => write!(f, "Missing field: {}", name),
            JsonError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for JsonError {}

/// Placeholder for JsonWriterCapsule (to be imported from atomic_capsule)
pub struct JsonWriterCapsule {
    buffer: String,
    depth: usize,
}

impl JsonWriterCapsule {
    /// Create a new JSON writer
    pub fn new() -> Self {
        JsonWriterCapsule {
            buffer: String::with_capacity(4096),
            depth: 0,
        }
    }

    /// Start a JSON object
    pub fn start_object(&mut self) -> Result<(), JsonError> {
        self.buffer.push('{');
        self.depth += 1;
        Ok(())
    }

    /// End a JSON object
    pub fn end_object(&mut self) -> Result<(), JsonError> {
        if self.depth == 0 {
            return Err(JsonError::Custom("Depth mismatch".into()));
        }
        self.depth -= 1;
        self.buffer.push('}');
        Ok(())
    }

    /// Start a JSON array
    pub fn start_array(&mut self) -> Result<(), JsonError> {
        self.buffer.push('[');
        self.depth += 1;
        Ok(())
    }

    /// End a JSON array
    pub fn end_array(&mut self) -> Result<(), JsonError> {
        if self.depth == 0 {
            return Err(JsonError::Custom("Depth mismatch".into()));
        }
        self.depth -= 1;
        self.buffer.push(']');
        Ok(())
    }

    /// Write a comma separator
    pub fn write_comma(&mut self) -> Result<(), JsonError> {
        self.buffer.push(',');
        Ok(())
    }

    /// Write a colon separator (for object key-value pairs)
    pub fn write_colon(&mut self) -> Result<(), JsonError> {
        self.buffer.push(':');
        Ok(())
    }

    /// Write a string value (with quotes and escaping)
    pub fn write_string(&mut self, s: &str) -> Result<(), JsonError> {
        self.buffer.push('"');
        for c in s.chars() {
            match c {
                '"' => self.buffer.push_str("\\\""),
                '\\' => self.buffer.push_str("\\\\"),
                '\n' => self.buffer.push_str("\\n"),
                '\r' => self.buffer.push_str("\\r"),
                '\t' => self.buffer.push_str("\\t"),
                _ => self.buffer.push(c),
            }
        }
        self.buffer.push('"');
        Ok(())
    }

    /// Write a numeric value (without quotes)
    pub fn write_number(&mut self, n: f64) -> Result<(), JsonError> {
        use std::fmt::Write as FmtWrite;
        write!(self.buffer, "{}", n).map_err(|_| JsonError::Custom("Failed to write number".into()))
    }

    /// Write a u64 value
    pub fn write_u64(&mut self, n: u64) -> Result<(), JsonError> {
        use std::fmt::Write as FmtWrite;
        write!(self.buffer, "{}", n).map_err(|_| JsonError::Custom("Failed to write u64".into()))
    }

    /// Write an i64 value
    pub fn write_i64(&mut self, n: i64) -> Result<(), JsonError> {
        use std::fmt::Write as FmtWrite;
        write!(self.buffer, "{}", n).map_err(|_| JsonError::Custom("Failed to write i64".into()))
    }

    /// Write a boolean value
    pub fn write_bool(&mut self, b: bool) -> Result<(), JsonError> {
        self.buffer.push_str(if b { "true" } else { "false" });
        Ok(())
    }

    /// Write a null value
    pub fn write_null(&mut self) -> Result<(), JsonError> {
        self.buffer.push_str("null");
        Ok(())
    }

    /// Write a literal (unquoted) value like numbers or keywords
    pub fn write_literal(&mut self, s: &str) -> Result<(), JsonError> {
        self.buffer.push_str(s);
        Ok(())
    }

    /// Finalize and return the JSON string
    pub fn finalize(&self) -> Result<String, JsonError> {
        if self.depth != 0 {
            return Err(JsonError::Custom(format!("Unclosed structure, depth: {}", self.depth)));
        }
        Ok(self.buffer.clone())
    }
}

impl Default for JsonWriterCapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// Placeholder for JsonValue (to be imported from atomic_capsule)
#[derive(Debug, Clone)]
pub enum JsonValue {
    /// Null value
    Null,
    /// Boolean value
    Bool(bool),
    /// Numeric value
    Number(f64),
    /// String value
    String(String),
    /// Array of values
    Array(Vec<JsonValue>),
    /// Object (map of key-value pairs)
    Object(Vec<(String, JsonValue)>),
}

/// Placeholder for JsonParserCapsule (to be imported from atomic_capsule)
pub struct JsonParserCapsule {
    input: String,
    pos: usize,
}

impl JsonParserCapsule {
    /// Create a new JSON parser
    pub fn new(input: &str) -> Self {
        JsonParserCapsule {
            input: input.to_string(),
            pos: 0,
        }
    }

    /// Parse the input JSON
    pub fn parse(&mut self) -> Result<JsonValue, JsonError> {
        self.skip_whitespace();
        self.parse_value()
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            match self.input.as_bytes()[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, JsonError> {
        self.skip_whitespace();
        if self.pos >= self.input.len() {
            return Err(JsonError::InvalidJson("Unexpected EOF".into()));
        }

        match self.input.as_bytes()[self.pos] {
            b'n' => self.parse_null(),
            b't' | b'f' => self.parse_bool(),
            b'"' => self.parse_string().map(JsonValue::String),
            b'[' => self.parse_array(),
            b'{' => self.parse_object(),
            b'-' | b'0'..=b'9' => self.parse_number(),
            c => Err(JsonError::InvalidJson(format!("Unexpected character: {}", c as char))),
        }
    }

    fn parse_null(&mut self) -> Result<JsonValue, JsonError> {
        if self.input[self.pos..].starts_with("null") {
            self.pos += 4;
            Ok(JsonValue::Null)
        } else {
            Err(JsonError::InvalidJson("Expected 'null'".into()))
        }
    }

    fn parse_bool(&mut self) -> Result<JsonValue, JsonError> {
        if self.input[self.pos..].starts_with("true") {
            self.pos += 4;
            Ok(JsonValue::Bool(true))
        } else if self.input[self.pos..].starts_with("false") {
            self.pos += 5;
            Ok(JsonValue::Bool(false))
        } else {
            Err(JsonError::InvalidJson("Expected boolean".into()))
        }
    }

    fn parse_string(&mut self) -> Result<String, JsonError> {
        if self.input.as_bytes()[self.pos] != b'"' {
            return Err(JsonError::InvalidJson("Expected string".into()));
        }
        self.pos += 1;

        let mut result = String::new();
        while self.pos < self.input.len() {
            match self.input.as_bytes()[self.pos] {
                b'"' => {
                    self.pos += 1;
                    return Ok(result);
                }
                b'\\' => {
                    self.pos += 1;
                    if self.pos >= self.input.len() {
                        return Err(JsonError::InvalidJson("Unclosed string".into()));
                    }
                    match self.input.as_bytes()[self.pos] {
                        b'"' => result.push('"'),
                        b'\\' => result.push('\\'),
                        b'/' => result.push('/'),
                        b'b' => result.push('\u{0008}'),
                        b'f' => result.push('\u{000c}'),
                        b'n' => result.push('\n'),
                        b'r' => result.push('\r'),
                        b't' => result.push('\t'),
                        _ => return Err(JsonError::InvalidJson("Invalid escape sequence".into())),
                    }
                    self.pos += 1;
                }
                _ => {
                    result.push(self.input.as_bytes()[self.pos] as char);
                    self.pos += 1;
                }
            }
        }
        Err(JsonError::InvalidJson("Unclosed string".into()))
    }

    fn parse_number(&mut self) -> Result<JsonValue, JsonError> {
        let start = self.pos;
        if self.input.as_bytes()[self.pos] == b'-' {
            self.pos += 1;
        }

        while self.pos < self.input.len() && self.input.as_bytes()[self.pos].is_ascii_digit() {
            self.pos += 1;
        }

        if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'.' {
            self.pos += 1;
            while self.pos < self.input.len() && self.input.as_bytes()[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }

        let num_str = &self.input[start..self.pos];
        num_str.parse::<f64>()
            .map(JsonValue::Number)
            .map_err(|_| JsonError::InvalidJson("Invalid number".into()))
    }

    fn parse_array(&mut self) -> Result<JsonValue, JsonError> {
        self.pos += 1; // Skip '['
        let mut values = Vec::new();

        loop {
            self.skip_whitespace();
            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b']' {
                self.pos += 1;
                return Ok(JsonValue::Array(values));
            }

            values.push(self.parse_value()?);
            self.skip_whitespace();

            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b',' {
                self.pos += 1;
            } else if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b']' {
                self.pos += 1;
                return Ok(JsonValue::Array(values));
            }
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, JsonError> {
        self.pos += 1; // Skip '{'
        let mut fields = Vec::new();

        loop {
            self.skip_whitespace();
            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'}' {
                self.pos += 1;
                return Ok(JsonValue::Object(fields));
            }

            let key = self.parse_string()?;
            self.skip_whitespace();

            if self.pos >= self.input.len() || self.input.as_bytes()[self.pos] != b':' {
                return Err(JsonError::InvalidJson("Expected ':'".into()));
            }
            self.pos += 1;

            let value = self.parse_value()?;
            fields.push((key, value));

            self.skip_whitespace();
            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b',' {
                self.pos += 1;
            } else if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'}' {
                self.pos += 1;
                return Ok(JsonValue::Object(fields));
            }
        }
    }
}

// Implementations of WriteJson for primitive types

impl WriteJson for u64 {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_u64(*self)
    }
}

impl WriteJson for u32 {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_u64(*self as u64)
    }
}

impl WriteJson for u16 {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_u64(*self as u64)
    }
}

impl WriteJson for u8 {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_u64(*self as u64)
    }
}

impl WriteJson for i64 {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_i64(*self)
    }
}

impl WriteJson for i32 {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_i64(*self as i64)
    }
}

impl WriteJson for i16 {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_i64(*self as i64)
    }
}

impl WriteJson for i8 {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_i64(*self as i64)
    }
}

impl WriteJson for usize {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_u64(*self as u64)
    }
}

impl WriteJson for isize {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_i64(*self as i64)
    }
}

impl WriteJson for bool {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_bool(*self)
    }
}

impl WriteJson for f64 {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_number(*self)
    }
}

impl WriteJson for f32 {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_number(*self as f64)
    }
}

impl WriteJson for String {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_string(self)
    }
}

impl WriteJson for &str {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.write_string(self)
    }
}

impl<T: WriteJson> WriteJson for Option<T> {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        match self {
            Some(value) => value.write_json(writer),
            None => writer.write_null(),
        }
    }
}

impl<T: WriteJson> WriteJson for Vec<T> {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.start_array()?;
        for (i, item) in self.iter().enumerate() {
            if i > 0 {
                writer.write_comma()?;
            }
            item.write_json(writer)?;
        }
        writer.end_array()
    }
}

impl<T: WriteJson> WriteJson for [T] {
    fn write_json(&self, writer: &mut JsonWriterCapsule) -> Result<(), JsonError> {
        writer.start_array()?;
        for (i, item) in self.iter().enumerate() {
            if i > 0 {
                writer.write_comma()?;
            }
            item.write_json(writer)?;
        }
        writer.end_array()
    }
}

// Implementations of ParseJson for primitive types

impl ParseJson for u64 {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonError> {
        match value {
            JsonValue::Number(n) => {
                if *n >= 0.0 && n.fract() == 0.0 {
                    Ok(*n as u64)
                } else {
                    Err(JsonError::TypeMismatch("Expected non-negative integer".into()))
                }
            }
            _ => Err(JsonError::TypeMismatch("Expected number".into())),
        }
    }
}

impl ParseJson for i64 {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonError> {
        match value {
            JsonValue::Number(n) => {
                if n.fract() == 0.0 {
                    Ok(*n as i64)
                } else {
                    Err(JsonError::TypeMismatch("Expected integer".into()))
                }
            }
            _ => Err(JsonError::TypeMismatch("Expected number".into())),
        }
    }
}

impl ParseJson for String {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonError> {
        match value {
            JsonValue::String(s) => Ok(s.clone()),
            _ => Err(JsonError::TypeMismatch("Expected string".into())),
        }
    }
}

impl ParseJson for bool {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonError> {
        match value {
            JsonValue::Bool(b) => Ok(*b),
            _ => Err(JsonError::TypeMismatch("Expected bool".into())),
        }
    }
}

impl ParseJson for f64 {
    fn parse_json(value: &JsonValue) -> Result<Self, JsonError> {
        match value {
            JsonValue::Number(n) => Ok(*n),
            _ => Err(JsonError::TypeMismatch("Expected number".into())),
        }
    }
}

// Helper functions

/// Serialize a struct to JSON using a closure
pub fn serialize_struct<F>(f: F) -> Result<String, JsonError>
where
    F: FnOnce(&mut JsonWriterCapsule) -> Result<(), JsonError>,
{
    let mut writer = JsonWriterCapsule::new();
    writer.start_object()?;
    f(&mut writer)?;
    writer.end_object()?;
    writer.finalize()
}

/// Write a struct field to JSON
///
/// This helper manages field separators automatically. Pass `first` by reference
/// and it will be set to false after the first field.
pub fn write_field<T: WriteJson>(
    writer: &mut JsonWriterCapsule,
    name: &str,
    value: &T,
    first: &mut bool,
) -> Result<(), JsonError> {
    if !*first {
        writer.write_comma()?;
    }
    *first = false;
    writer.write_string(name)?;
    writer.write_colon()?;
    value.write_json(writer)
}

/// Get a field from a JSON object
///
/// Returns the value if found, None otherwise.
pub fn get_field<'a>(
    fields: &'a [(String, JsonValue)],
    name: &str,
) -> Option<&'a JsonValue> {
    fields.iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v)
}

/// Get a required field from a JSON object
///
/// Returns an error if the field is not found.
pub fn get_field_required<'a>(
    fields: &'a [(String, JsonValue)],
    name: &str,
) -> Result<&'a JsonValue, JsonError> {
    get_field(fields, name)
        .ok_or_else(|| JsonError::MissingField(name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_primitives() {
        let mut writer = JsonWriterCapsule::new();
        writer.start_object().unwrap();

        let mut first = true;
        write_field(&mut writer, "count", &42u64, &mut first).unwrap();
        write_field(&mut writer, "value", &3.14f64, &mut first).unwrap();
        write_field(&mut writer, "flag", &true, &mut first).unwrap();

        writer.end_object().unwrap();
        let json = writer.finalize().unwrap();

        assert!(json.contains("\"count\":42"));
        assert!(json.contains("\"value\":"));
        assert!(json.contains("\"flag\":true"));
    }

    #[test]
    fn test_parse_primitives() {
        assert_eq!(u64::parse_json(&JsonValue::Number(42.0)).unwrap(), 42u64);
        assert_eq!(bool::parse_json(&JsonValue::Bool(true)).unwrap(), true);
        assert_eq!(
            String::parse_json(&JsonValue::String("hello".into())).unwrap(),
            "hello"
        );
    }

    #[test]
    fn test_json_parser_basic() {
        let mut parser = JsonParserCapsule::new(r#"{"name":"test","count":42}"#);
        let value = parser.parse().unwrap();

        match value {
            JsonValue::Object(fields) => {
                assert_eq!(fields.len(), 2);
                assert_eq!(get_field(&fields, "name"), Some(&JsonValue::String("test".into())));
                assert_eq!(get_field(&fields, "count"), Some(&JsonValue::Number(42.0)));
            }
            _ => panic!("Expected object"),
        }
    }
}
