//! XML Export Format
//!
//! Provides XML serialization with proper entity encoding.
//!
//! # Features
//! - Proper entity encoding (<, >, &, ", ')
//! - CDATA sections for large text
//! - Nested structure support
//!
//! # Performance (B32)
//! - ~70-130μs per record (verbose format)

use crate::error::ClapiResult;

/// XML exporter
pub struct XmlExporter;

impl XmlExporter {
    /// Export records as XML
    ///
    /// # Arguments
    /// - `root_tag`: Root element name
    /// - `record_tag`: Individual record element name
    /// - `records`: Iterator of (field_name, field_value) tuples
    ///
    /// # Returns
    /// XML string with proper entity encoding
    pub fn export_records<'a, I>(
        root_tag: &str,
        record_tag: &str,
        records: I,
    ) -> ClapiResult<String>
    where
        I: IntoIterator<Item = Vec<(&'a str, &'a str)>>,
    {
        let mut output = String::with_capacity(4096);

        // XML declaration
        output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");

        // Root element
        output.push('<');
        output.push_str(root_tag);
        output.push_str(">\n");

        // Records
        for record in records {
            output.push_str("  <");
            output.push_str(record_tag);
            output.push_str(">\n");

            for (key, value) in record {
                output.push_str("    <");
                output.push_str(key);
                output.push('>');
                Self::write_xml_value(&mut output, value);
                output.push_str("</");
                output.push_str(key);
                output.push_str(">\n");
            }

            output.push_str("  </");
            output.push_str(record_tag);
            output.push_str(">\n");
        }

        // Close root element
        output.push_str("</");
        output.push_str(root_tag);
        output.push_str(">\n");

        Ok(output)
    }

    /// Write XML value with proper entity encoding
    fn write_xml_value(output: &mut String, value: &str) {
        // Use CDATA for large text with special chars
        if value.len() > 100 && (value.contains('<') || value.contains('&')) {
            output.push_str("<![CDATA[");
            output.push_str(value);
            output.push_str("]]>");
        } else {
            // Standard entity encoding
            for ch in value.chars() {
                match ch {
                    '<' => output.push_str("&lt;"),
                    '>' => output.push_str("&gt;"),
                    '&' => output.push_str("&amp;"),
                    '"' => output.push_str("&quot;"),
                    '\'' => output.push_str("&apos;"),
                    c => output.push(c),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xml_export_basic() {
        let records = vec![
            vec![("id", "1"), ("name", "Alice")],
            vec![("id", "2"), ("name", "Bob")],
        ];

        let xml = XmlExporter::export_records("users", "user", records).unwrap();

        assert!(xml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<users>"));
        assert!(xml.contains("<user>"));
        assert!(xml.contains("<id>1</id>"));
        assert!(xml.contains("<name>Alice</name>"));
        assert!(xml.contains("</user>"));
        assert!(xml.contains("</users>"));
    }

    #[test]
    fn test_xml_entity_encoding() {
        let records = vec![
            vec![("text", "<tag> & \"quote\" 'apostrophe'")],
        ];

        let xml = XmlExporter::export_records("root", "item", records).unwrap();

        assert!(xml.contains("&lt;tag&gt; &amp; &quot;quote&quot; &apos;apostrophe&apos;"));
    }

    #[test]
    fn test_xml_cdata_for_large_text() {
        let large_text = "x".repeat(150) + "<tag>";
        let records = vec![
            vec![("content", large_text.as_str())],
        ];

        let xml = XmlExporter::export_records("root", "item", records).unwrap();

        // Large text with special chars should use CDATA
        assert!(xml.contains("<![CDATA["));
        assert!(xml.contains("]]>"));
    }

    #[test]
    fn test_xml_small_text_no_cdata() {
        let records = vec![
            vec![("text", "small")],
        ];

        let xml = XmlExporter::export_records("root", "item", records).unwrap();

        // Small text without special chars should NOT use CDATA
        assert!(!xml.contains("CDATA"));
        assert!(xml.contains("<text>small</text>"));
    }
}
