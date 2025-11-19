//! CSV capsule demonstration - RFC 4180 compliant CSV writer/reader.
//!
//! **Tier**: T5 Streaming (O(1) per field)
//! **Performance**: <50ns per field
//! **Features**: RFC 4180 compliance, quote escaping, delimiter customization

use atomic_capsule::serialize::{CsvWriterCapsule, CsvReaderCapsule};

fn main() {
    println!("=== CSV Capsule Demo (T5 Streaming) ===\n");

    // Example 1: Simple CSV writing
    println!("1. Writing CSV with simple data:");
    let mut writer = CsvWriterCapsule::new();
    writer.write_header(&["Name", "Age", "City"]).unwrap();
    writer.write_row(&["Alice", "30", "NYC"]).unwrap();
    writer.write_row(&["Bob", "25", "LA"]).unwrap();
    let csv1 = writer.finalize().unwrap();
    println!("{}\n", csv1);

    // Example 2: CSV with special characters (quotes, commas, newlines)
    println!("2. Writing CSV with special characters (auto-quoted):");
    let mut writer = CsvWriterCapsule::new();
    writer.write_row(&["Name", "Quote"]).unwrap();
    writer.write_row(&["Alice", "She said \"hello\""]).unwrap();
    writer.write_row(&["Bob", "Line1\nLine2"]).unwrap();
    writer.write_row(&["Carol", "Smith, Jr."]).unwrap();
    let csv2 = writer.finalize().unwrap();
    println!("{}\n", csv2);

    // Example 3: Reading CSV
    println!("3. Reading CSV data:");
    let csv_input = "Name,Age,City\nAlice,30,NYC\nBob,25,LA\n";
    let mut reader = CsvReaderCapsule::new(csv_input);
    let headers = reader.parse_row().unwrap();
    println!("Headers: {:?}", headers);

    while let Ok(row) = reader.parse_row() {
        if row.is_empty() { break; }
        println!("Row: {:?}", row);
    }
    println!();

    // Example 4: Custom delimiter (semicolon-separated)
    println!("4. Using custom delimiter (;):");
    let mut writer = CsvWriterCapsule::new().with_delimiter(b';');
    writer.write_row(&["A", "B", "C"]).unwrap();
    writer.write_row(&["1", "2", "3"]).unwrap();
    let csv4 = writer.finalize().unwrap();
    println!("{}\n", csv4);

    // Example 5: Roundtrip test (write → read → verify)
    println!("5. Roundtrip test (write → read → verify):");
    let original_rows = vec![
        vec!["John", "28", "Seattle"],
        vec!["Jane", "32", "Portland"],
    ];

    let mut writer = CsvWriterCapsule::new();
    for row in &original_rows {
        writer.write_row(row).unwrap();
    }
    let csv_data = writer.finalize().unwrap();
    println!("Generated CSV:\n{}", csv_data);

    let mut reader = CsvReaderCapsule::new(&csv_data);
    for (i, original) in original_rows.iter().enumerate() {
        let parsed = reader.parse_row().unwrap();
        let matches = original == &parsed.iter().map(|s| s.as_str()).collect::<Vec<_>>();
        println!("Row {} matches: {}", i, matches);
    }
    println!();

    // Example 6: Performance note
    println!("6. Performance characteristics (B32 Framework):");
    println!("  - write_field(): <50ns (escape + write)");
    println!("  - write_row(4 fields): <200ns (4×50ns fields)");
    println!("  - parse_row(): <200ns (sequential scan + allocation)");
    println!("  - finalize(): O(n) where n = bytes written");
    println!("\n✅ All examples completed successfully!");
}
