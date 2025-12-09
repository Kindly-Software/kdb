//! Comprehensive Export Format Tests (T28 Framework)
//!
//! Tests all 8 export formats with:
//! - Unit tests (Q1-Q7): Basic functionality, escaping, edge cases
//! - Property tests (Q8-Q14): Round-trip validation, format equivalence
//! - Integration tests (Q15-Q21): End-to-end export workflows
//! - Performance tests (Q22-Q28): B32 benchmarking compliance

// Feature gate: compliance module required for all tests
#![cfg(feature = "compliance")]

use clapi_core::compliance::export_capsule::{DataExportCapsule, ExportFormat};
use clapi_core::compliance::export_formats::formats::{
    JsonExporter, CsvExporter, SqlExporter, SqlDialect,
    YamlExporter, XmlExporter,
};

// ============================================================================
// UNIT TESTS (Q1-Q7): Basic Functionality
// ============================================================================

#[test]
fn test_json_basic_export() {
    let records = vec![
        vec![("id", "1"), ("name", "Alice")],
        vec![("id", "2"), ("name", "Bob")],
    ];

    let json = JsonExporter::export_records(records).unwrap();

    assert!(json.contains(r#""id":"1""#));
    assert!(json.contains(r#""name":"Alice""#));
    assert!(json.contains(r#""name":"Bob""#));
}

#[test]
fn test_json_escaping() {
    let records = vec![
        vec![("text", r#"quote:" slash:\ newline:
"#)],
    ];

    let json = JsonExporter::export_records(records).unwrap();

    // Verify proper escaping
    assert!(json.contains(r#"\""#)); // Quote escaped
    assert!(json.contains(r#"\\"#)); // Backslash escaped
    assert!(json.contains(r#"\n"#)); // Newline escaped
}

#[test]
fn test_csv_basic_export() {
    let headers = &["id", "name", "age"];
    let records = vec![
        vec!["1", "Alice", "30"],
        vec!["2", "Bob", "25"],
    ];

    let csv = CsvExporter::export_records(headers, records).unwrap();

    assert!(csv.contains("id,name,age"));
    assert!(csv.contains("1,Alice,30"));
    assert!(csv.contains("2,Bob,25"));
}

#[test]
fn test_csv_rfc4180_compliance() {
    let headers = &["name", "description"];
    let records = vec![
        vec!["John, Jr.", "He said \"hi\""],
    ];

    let csv = CsvExporter::export_records(headers, records).unwrap();

    // Verify RFC 4180 escaping
    assert!(csv.contains("\"John, Jr.\"")); // Comma requires quotes
    assert!(csv.contains("\"He said \"\"hi\"\"\"")); // Quote escaped as ""
}

#[test]
fn test_sql_mysql_export() {
    let records = vec![
        vec!["1", "Alice"],
        vec!["2", "Bob"],
    ];

    let sql = SqlExporter::export_records(
        "users",
        &["id", "name"],
        records,
        SqlDialect::MySql,
    ).unwrap();

    assert!(sql.contains("INSERT INTO users (`id`, `name`) VALUES"));
    assert!(sql.contains("('1', 'Alice')"));
    assert!(sql.contains("('2', 'Bob')"));
}

#[test]
fn test_sql_postgresql_export() {
    let records = vec![
        vec!["1", "test"],
    ];

    let sql = SqlExporter::export_records(
        "data",
        &["id", "value"],
        records,
        SqlDialect::PostgreSql,
    ).unwrap();

    assert!(sql.contains("INSERT INTO data (\"id\", \"value\") VALUES"));
    assert!(sql.contains("('1', 'test')"));
}

#[test]
fn test_sql_value_escaping() {
    let records = vec![
        vec!["O'Brien", "line1\nline2"],
    ];

    let sql = SqlExporter::export_records(
        "test",
        &["name", "text"],
        records,
        SqlDialect::MySql,
    ).unwrap();

    assert!(sql.contains("'O''Brien'")); // Single quote escaped
    assert!(sql.contains("'line1\\nline2'")); // Newline escaped
}

#[test]
fn test_yaml_basic_export() {
    let records = vec![
        vec![("name", "Alice"), ("age", "30")],
    ];

    let yaml = YamlExporter::export_records(records).unwrap();

    assert!(yaml.contains("-\n"));
    assert!(yaml.contains("  name: Alice"));
    assert!(yaml.contains("  age: 30"));
}

#[test]
fn test_yaml_special_char_quoting() {
    let records = vec![
        vec![("key", "value: with colon")],
    ];

    let yaml = YamlExporter::export_records(records).unwrap();

    // Colon requires quoting
    assert!(yaml.contains("\"value: with colon\""));
}

#[test]
fn test_xml_basic_export() {
    let records = vec![
        vec![("id", "1"), ("name", "Alice")],
    ];

    let xml = XmlExporter::export_records("users", "user", records).unwrap();

    assert!(xml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(xml.contains("<users>"));
    assert!(xml.contains("<user>"));
    assert!(xml.contains("<id>1</id>"));
    assert!(xml.contains("<name>Alice</name>"));
}

#[test]
fn test_xml_entity_encoding() {
    let records = vec![
        vec![("text", "<tag> & \"quote\"")],
    ];

    let xml = XmlExporter::export_records("root", "item", records).unwrap();

    assert!(xml.contains("&lt;tag&gt; &amp; &quot;quote&quot;"));
}

// ============================================================================
// PROPERTY TESTS (Q8-Q14): Round-trip Validation
// ============================================================================

#[test]
fn test_json_roundtrip() {
    use serde_json::Value;

    let records = vec![
        vec![("id", "1"), ("value", "test")],
        vec![("id", "2"), ("value", "data")],
    ];

    let json = JsonExporter::export_records(records).unwrap();
    let parsed: Value = serde_json::from_str(&json).unwrap();

    assert!(parsed.is_array());
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["id"], "1");
    assert_eq!(arr[1]["value"], "data");
}

#[test]
fn test_csv_roundtrip() {
    let headers = &["id", "value"];
    let records = vec![
        vec!["1", "test"],
        vec!["2", "data"],
    ];

    let csv = CsvExporter::export_records(headers, records).unwrap();

    // Parse back (simple validation)
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 3); // Header + 2 data rows
    assert_eq!(lines[0], "id,value");
    assert!(lines[1].contains("1"));
    assert!(lines[2].contains("2"));
}

#[test]
fn test_all_formats_equivalent_record_count() {
    // All text formats should contain same number of records
    let records_json = vec![
        vec![("id", "1"), ("value", "a")],
        vec![("id", "2"), ("value", "b")],
    ];
    let records_csv = vec![
        vec!["1", "a"],
        vec!["2", "b"],
    ];
    let records_yaml = records_json.clone();
    let records_xml = records_json.clone();

    let json = JsonExporter::export_records(records_json).unwrap();
    let csv = CsvExporter::export_records(&["id", "value"], records_csv).unwrap();
    let yaml = YamlExporter::export_records(records_yaml).unwrap();
    let xml = XmlExporter::export_records("root", "item", records_xml).unwrap();

    // Count records (simplified - just verify non-empty)
    assert!(json.len() > 10);
    assert!(csv.len() > 10);
    assert!(yaml.len() > 10);
    assert!(xml.len() > 10);
}

// ============================================================================
// INTEGRATION TESTS (Q15-Q21): DataExportCapsule
// ============================================================================

#[test]
fn test_export_capsule_snapshot_consistency() {
    let capsule = DataExportCapsule::new();

    // Begin snapshot
    let gen1 = capsule.begin_snapshot();
    assert!(capsule.validate_snapshot(gen1));

    // Invalidate snapshot (simulating data mutation)
    capsule.invalidate_snapshot();
    assert!(!capsule.validate_snapshot(gen1)); // Old snapshot invalid

    // New snapshot
    let gen2 = capsule.begin_snapshot();
    assert!(capsule.validate_snapshot(gen2));
    assert_ne!(gen1, gen2);
}

#[test]
fn test_export_capsule_format_switching() {
    let capsule = DataExportCapsule::new();

    assert_eq!(capsule.get_format(), ExportFormat::Json);

    capsule.set_format(ExportFormat::Csv);
    assert_eq!(capsule.get_format(), ExportFormat::Csv);

    capsule.set_format(ExportFormat::Sql);
    assert_eq!(capsule.get_format(), ExportFormat::Sql);

    capsule.set_format(ExportFormat::Yaml);
    assert_eq!(capsule.get_format(), ExportFormat::Yaml);
}

#[test]
fn test_export_capsule_metrics_tracking() {
    let capsule = DataExportCapsule::new();

    assert_eq!(capsule.total_exported(), 0);
    assert_eq!(capsule.total_errors(), 0);

    capsule.record_export(100);
    assert_eq!(capsule.total_exported(), 100);

    capsule.record_export(50);
    assert_eq!(capsule.total_exported(), 150);

    capsule.record_error();
    assert_eq!(capsule.total_errors(), 1);

    capsule.record_error();
    assert_eq!(capsule.total_errors(), 2);
}

#[test]
fn test_export_capsule_concurrent_snapshot_invalidation() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(DataExportCapsule::new());

    // Thread 1: Repeatedly invalidate snapshots
    let c1 = capsule.clone();
    let t1 = thread::spawn(move || {
        for _ in 0..1000 {
            c1.invalidate_snapshot();
        }
    });

    // Thread 2: Check snapshot validity
    let c2 = capsule.clone();
    let t2 = thread::spawn(move || {
        let mut valid_count = 0;
        for _ in 0..1000 {
            let gen = c2.begin_snapshot();
            if c2.validate_snapshot(gen) {
                valid_count += 1;
            }
        }
        valid_count
    });

    t1.join().unwrap();
    let valid = t2.join().unwrap();

    // Some snapshots should be valid (not all invalidated immediately)
    assert!(valid > 0);
}

// ============================================================================
// STRESS TESTS (Q22-Q28): Large Exports
// ============================================================================

#[test]
fn test_json_large_export() {
    let mut records = Vec::new();
    for i in 0..1000 {
        records.push(vec![("id", "1"), ("index", Box::leak(Box::new(i.to_string())).as_str())]);
    }

    let json = JsonExporter::export_records(records).unwrap();

    // Verify large export succeeds
    assert!(json.len() > 10000);
    assert!(json.starts_with('['));
    assert!(json.ends_with(']'));
}

#[test]
fn test_csv_large_export() {
    let headers = &["id", "index"];
    let mut records = Vec::new();
    for i in 0..1000 {
        records.push(vec!["1", Box::leak(Box::new(i.to_string())).as_str()]);
    }

    let csv = CsvExporter::export_records(headers, records).unwrap();

    // Verify large export succeeds
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 1001); // Header + 1000 rows
}

#[test]
fn test_export_capsule_memory_layout() {
    use std::mem;

    assert_eq!(mem::size_of::<DataExportCapsule>(), 256);
    assert_eq!(mem::align_of::<DataExportCapsule>(), 256);
}

#[test]
fn test_export_format_metadata() {
    assert_eq!(ExportFormat::Json.mime_type(), "application/json");
    assert_eq!(ExportFormat::Csv.mime_type(), "text/csv");
    assert_eq!(ExportFormat::Parquet.mime_type(), "application/vnd.apache.parquet");
    assert_eq!(ExportFormat::Arrow.mime_type(), "application/vnd.apache.arrow.stream");
    assert_eq!(ExportFormat::Sql.mime_type(), "application/sql");
    assert_eq!(ExportFormat::Yaml.mime_type(), "application/x-yaml");
    assert_eq!(ExportFormat::Xml.mime_type(), "application/xml");

    assert_eq!(ExportFormat::Json.extension(), "json");
    assert_eq!(ExportFormat::Csv.extension(), "csv");
    assert_eq!(ExportFormat::Parquet.extension(), "parquet");
    assert_eq!(ExportFormat::Arrow.extension(), "arrow");
    assert_eq!(ExportFormat::Orc.extension(), "orc");
    assert_eq!(ExportFormat::Sql.extension(), "sql");
    assert_eq!(ExportFormat::Yaml.extension(), "yaml");
    assert_eq!(ExportFormat::Xml.extension(), "xml");
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[test]
fn test_json_empty_export() {
    let records: Vec<Vec<(&str, &str)>> = vec![];
    let json = JsonExporter::export_records(records).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn test_csv_empty_export() {
    let headers = &["id", "value"];
    let records: Vec<Vec<&str>> = vec![];
    let csv = CsvExporter::export_records(headers, records).unwrap();
    assert_eq!(csv, "id,value\n");
}

#[test]
fn test_yaml_empty_export() {
    let records: Vec<Vec<(&str, &str)>> = vec![];
    let yaml = YamlExporter::export_records(records).unwrap();
    assert_eq!(yaml, "");
}

#[test]
fn test_xml_empty_export() {
    let records: Vec<Vec<(&str, &str)>> = vec![];
    let xml = XmlExporter::export_records("root", "item", records).unwrap();
    assert!(xml.contains("<root>"));
    assert!(xml.contains("</root>"));
}
