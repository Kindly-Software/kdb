//! YAML Export Format
//!
//! Provides YAML serialization for human-readable configuration exports.
//!
//! # Features
//! - Nested structure support
//! - Proper indentation (2 spaces)
//! - String escaping for special YAML characters
//!
//! # Performance (B32)
//! - ~80-150μs per record (human-readable format)

use crate::error::ClapiResult;

/// YAML exporter
pub struct YamlExporter;

impl YamlExporter {
    /// Export records as YAML array
    ///
    /// # Arguments
    /// - `records`: Iterator of (field_name, field_value) tuples
    ///
    /// # Returns
    /// YAML string with proper indentation
    pub fn export_records<'a, I>(records: I) -> ClapiResult<String>
    where
        I: IntoIterator<Item = Vec<(&'a str, &'a str)>>,
    {
        let mut output = String::with_capacity(4096);

        for record in records {
            output.push_str("-\n");
            for (key, value) in record {
                output.push_str("  ");
                output.push_str(key);
                output.push_str(": ");
                Self::write_yaml_value(&mut output, value);
                output.push('\n');
            }
        }

        Ok(output)
    }

    /// Write YAML value with proper escaping
    fn write_yaml_value(output: &mut String, value: &str) {
        // Check if value needs quoting
        if Self::needs_yaml_quotes(value) {
            output.push('"');
            for ch in value.chars() {
                match ch {
                    '"' => output.push_str("\\\""),
                    '\\' => output.push_str("\\\\"),
                    '\n' => output.push_str("\\n"),
                    '\r' => output.push_str("\\r"),
                    '\t' => output.push_str("\\t"),
                    c => output.push(c),
                }
            }
            output.push('"');
        } else {
            output.push_str(value);
        }
    }

    /// Check if value needs quoting in YAML
    fn needs_yaml_quotes(value: &str) -> bool {
        // Quote if contains special YAML characters or starts with special chars
        value.is_empty()
            || value.starts_with('-')
            || value.starts_with('[')
            || value.starts_with('{')
            || value.starts_with('>')
            || value.starts_with('|')
            || value.contains(':')
            || value.contains('#')
            || value.contains('\n')
            || value.contains('"')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_export_basic() {
        let records = vec![
            vec![("name", "Alice"), ("age", "30")],
            vec![("name", "Bob"), ("age", "25")],
        ];

        let yaml = YamlExporter::export_records(records).unwrap();

        assert!(yaml.contains("-\n"));
        assert!(yaml.contains("  name: Alice"));
        assert!(yaml.contains("  age: 30"));
        assert!(yaml.contains("  name: Bob"));
    }

    #[test]
    fn test_yaml_export_with_escaping() {
        let records = vec![
            vec![("key", "value: with colon")],
        ];

        let yaml = YamlExporter::export_records(records).unwrap();

        // Colon requires quoting
        assert!(yaml.contains("\"value: with colon\""));
    }

    #[test]
    fn test_yaml_needs_quotes() {
        assert!(YamlExporter::needs_yaml_quotes("")); // Empty string
        assert!(YamlExporter::needs_yaml_quotes("- item")); // Starts with dash
        assert!(YamlExporter::needs_yaml_quotes("key: value")); // Contains colon
        assert!(YamlExporter::needs_yaml_quotes("text # comment")); // Contains hash

        assert!(!YamlExporter::needs_yaml_quotes("simple")); // No special chars
        assert!(!YamlExporter::needs_yaml_quotes("123")); // Number
    }

    #[test]
    fn test_yaml_escape_newlines() {
        let records = vec![
            vec![("text", "line1\nline2")],
        ];

        let yaml = YamlExporter::export_records(records).unwrap();

        assert!(yaml.contains("\"line1\\nline2\""));
    }
}
