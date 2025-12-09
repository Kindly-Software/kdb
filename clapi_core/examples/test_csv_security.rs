use clapi_core::compliance::export_formats::formats::CsvExporter;

fn main() {
    // Test formula injection prevention
    let dangerous_formulas = vec![
        "=1+1",
        "+1234",
        "-5678",
        "@SUM(A1:A10)",
        "=cmd|'/c calc'!A1",
        "\t=1+1",
        "\r=1+1",
    ];

    println!("Testing CSV formula injection prevention:");
    for formula in dangerous_formulas {
        let escaped = CsvExporter::escape_csv(formula);
        println!("  Input: {:?}", formula);
        println!("  Output: {:?}", escaped);
        assert!(escaped.starts_with("'"), "Formula not sanitized: {}", formula);
    }

    // Test normal values unchanged
    let safe_values = vec![
        "normal text",
        "123",
        "test@example.com",
    ];

    println!("\nTesting safe values unchanged:");
    for value in safe_values {
        let escaped = CsvExporter::escape_csv(value);
        println!("  Input: {:?}", value);
        println!("  Output: {:?}", escaped);
        assert_eq!(escaped, value, "Safe value was modified: {}", value);
    }

    println!("\n✅ All security tests passed!");
}
