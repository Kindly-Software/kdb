use atomic_capsule::serialize::{CsvWriterCapsule, CsvReaderCapsule};

fn main() {
    // Test writing
    let mut writer = CsvWriterCapsule::new();
    writer.write_row(&["Alice", "30"]).unwrap();
    writer.write_row(&["Bob", "25"]).unwrap();
    let csv = writer.finalize().unwrap();
    
    println!("CSV Output:\n{}", csv);
    
    // Test reading
    let mut reader = CsvReaderCapsule::new(&csv);
    let row1 = reader.parse_row().unwrap();
    let row2 = reader.parse_row().unwrap();
    
    println!("\nRow 1: {:?}", row1);
    println!("Row 2: {:?}", row2);
    
    assert_eq!(row1, vec!["Alice", "30"]);
    assert_eq!(row2, vec!["Bob", "25"]);
    
    println!("\nAll tests passed!");
}
