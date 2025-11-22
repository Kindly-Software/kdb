use atomic_capsule::serialize::HexDecoderCapsule;

fn main() {
    println!("Testing HexDecoderCapsule (T2 SIMD)...\n");

    // Test 1: Basic decoding
    let hex = "deadbeef";
    match HexDecoderCapsule::decode(hex) {
        Ok(bytes) => {
            println!("✓ Test 1: Decoded '{}' to {:?}", hex, bytes);
            assert_eq!(bytes, vec![0xde, 0xad, 0xbe, 0xef]);
            println!("  Values match expected output\n");
        }
        Err(e) => {
            eprintln!("✗ Test 1 failed: {}", e);
            std::process::exit(1);
        }
    }

    // Test 2: Uppercase
    let hex_upper = "DEADBEEF";
    match HexDecoderCapsule::decode(hex_upper) {
        Ok(bytes) => {
            println!("✓ Test 2: Uppercase decoding: {:?}", bytes);
            assert_eq!(bytes, vec![0xde, 0xad, 0xbe, 0xef]);
            println!("  Values match expected output\n");
        }
        Err(e) => {
            eprintln!("✗ Test 2 failed: {}", e);
            std::process::exit(1);
        }
    }

    // Test 3: Mixed case
    let hex_mixed = "DeAdBeEf";
    match HexDecoderCapsule::decode(hex_mixed) {
        Ok(bytes) => {
            println!("✓ Test 3: Mixed case decoding: {:?}", bytes);
            assert_eq!(bytes, vec![0xde, 0xad, 0xbe, 0xef]);
            println!("  Values match expected output\n");
        }
        Err(e) => {
            eprintln!("✗ Test 3 failed: {}", e);
            std::process::exit(1);
        }
    }

    // Test 4: Error on odd length
    let hex_odd = "123";
    match HexDecoderCapsule::decode(hex_odd) {
        Ok(_) => {
            eprintln!("✗ Test 4 failed: Should have failed on odd length");
            std::process::exit(1);
        }
        Err(e) => {
            println!("✓ Test 4: Correctly rejected odd length");
            println!("  Error: {}\n", e);
        }
    }

    // Test 5: Error on invalid char
    let hex_invalid = "xyz";
    match HexDecoderCapsule::decode(hex_invalid) {
        Ok(_) => {
            eprintln!("✗ Test 5 failed: Should have failed on invalid chars");
            std::process::exit(1);
        }
        Err(e) => {
            println!("✓ Test 5: Correctly rejected invalid chars");
            println!("  Error: {}\n", e);
        }
    }

    // Test 6: Empty string
    match HexDecoderCapsule::decode("") {
        Ok(bytes) => {
            println!("✓ Test 6: Empty string decoding: {:?}", bytes);
            assert_eq!(bytes.len(), 0);
            println!("  Correctly produced empty vector\n");
        }
        Err(e) => {
            eprintln!("✗ Test 6 failed: {}", e);
            std::process::exit(1);
        }
    }

    // Test 7: Large input
    let hex_large = "ab".repeat(256);
    match HexDecoderCapsule::decode(&hex_large) {
        Ok(bytes) => {
            println!("✓ Test 7: Large input (512 hex chars)");
            println!("  Decoded to {} bytes", bytes.len());
            assert_eq!(bytes.len(), 256);
            assert!(bytes.iter().all(|b| *b == 0xab));
            println!("  All values are 0xab as expected\n");
        }
        Err(e) => {
            eprintln!("✗ Test 7 failed: {}", e);
            std::process::exit(1);
        }
    }

    println!("═══════════════════════════════════════════════════════════");
    println!("All tests passed! HexDecoderCapsule is working correctly.");
    println!("═══════════════════════════════════════════════════════════");
}
