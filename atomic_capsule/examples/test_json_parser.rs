use atomic_capsule::serialize::{JsonParserCapsule, JsonValue};

fn main() {
    // Test 1: Simple object
    let json = r#"{"name":"Alice","age":30}"#;
    let mut parser = JsonParserCapsule::new(json);
    let value = parser.parse().unwrap();
    println!("Test 1 (object): OK");

    // Test 2: Array
    let json = "[1,2,3]";
    let mut parser = JsonParserCapsule::new(json);
    let value = parser.parse().unwrap();
    println!("Test 2 (array): OK");

    // Test 3: String with escape
    let json = r#""hello\nworld""#;
    let mut parser = JsonParserCapsule::new(json);
    let value = parser.parse().unwrap();
    println!("Test 3 (string): OK");

    // Test 4: Nested structure
    let json = r#"{"items":[1,2,3],"metadata":{"version":1}}"#;
    let mut parser = JsonParserCapsule::new(json);
    let value = parser.parse().unwrap();
    println!("Test 4 (nested): OK");

    // Test 5: Unicode escape
    let json = r#""hello\u0020world""#;
    let mut parser = JsonParserCapsule::new(json);
    let value = parser.parse().unwrap();
    println!("Test 5 (unicode): OK");

    // Test 6: Error handling
    let json = "[1,2,";
    let mut parser = JsonParserCapsule::new(json);
    match parser.parse() {
        Err(_) => println!("Test 6 (error handling): OK"),
        Ok(_) => panic!("Should have failed"),
    }

    println!("\n✓ All JsonParserCapsule tests passed!");
}
