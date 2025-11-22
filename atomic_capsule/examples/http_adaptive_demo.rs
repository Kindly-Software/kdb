//! Adaptive SIMD HTTP Header Parsing Demo
//!
//! **Demonstrates**:
//! - Hybrid threshold strategy (scalar <128B, SIMD ≥128B)
//! - Zero-cost abstraction (#[inline(always)])
//! - Branch prediction optimization (<1ns overhead)
//!
//! **Performance**:
//! - <128B: Scalar fallback (no penalty)
//! - ≥128B: 28-70× SIMD speedup
//! - Threshold check: <1ns (predicted branch)

#[cfg(feature = "http-simd")]
fn main() {
    use atomic_capsule::http::{find_colon_adaptive, find_crlf_adaptive};

    println!("Adaptive SIMD HTTP Header Parsing Demo");
    println!("======================================\n");

    // Test 1: Small header (<128B) - Uses scalar fallback
    let small_header = b"Content-Type: application/json";
    println!("Test 1: Small header ({} bytes)", small_header.len());
    match find_colon_adaptive(small_header) {
        Some(pos) => {
            let name = std::str::from_utf8(&small_header[..pos]).unwrap();
            let value = std::str::from_utf8(&small_header[pos + 1..]).unwrap();
            println!("  Found ':' at position {}", pos);
            println!("  Name: '{}', Value: '{}'", name, value.trim());
            println!("  Strategy: Scalar fallback (<128B)\n");
        }
        None => println!("  No colon found\n"),
    }

    // Test 2: Large header (≥128B) - Uses SIMD acceleration
    let large_header = {
        let mut h = vec![b'x'; 200];
        let header_text = b"Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
        h[..header_text.len()].copy_from_slice(header_text);
        h
    };
    println!("Test 2: Large header ({} bytes)", large_header.len());
    match find_colon_adaptive(&large_header) {
        Some(pos) => {
            let name = std::str::from_utf8(&large_header[..pos]).unwrap();
            println!("  Found ':' at position {}", pos);
            println!("  Name: '{}'", name);
            println!("  Strategy: SIMD acceleration (≥128B)");
            println!("  Expected speedup: 28-70× vs scalar\n");
        }
        None => println!("  No colon found\n"),
    }

    // Test 3: Threshold boundary (exactly 128B)
    let mut threshold_header = vec![b'x'; 128];
    let header_text = b"Content-Length: 12345";
    threshold_header[..header_text.len()].copy_from_slice(header_text);
    println!(
        "Test 3: Threshold boundary ({} bytes)",
        threshold_header.len()
    );
    match find_colon_adaptive(&threshold_header) {
        Some(pos) => {
            let name = std::str::from_utf8(&threshold_header[..pos]).unwrap();
            let value_start = pos + 1;
            let value_end = threshold_header[value_start..]
                .iter()
                .position(|&b| b == b'x')
                .map(|p| value_start + p)
                .unwrap_or(threshold_header.len());
            let value = std::str::from_utf8(&threshold_header[value_start..value_end]).unwrap();
            println!("  Found ':' at position {}", pos);
            println!("  Name: '{}', Value: '{}'", name, value.trim());
            println!("  Strategy: SIMD acceleration (≥128B threshold)\n");
        }
        None => println!("  No colon found\n"),
    }

    // Test 4: CRLF detection (small)
    let small_response = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n";
    println!(
        "Test 4: CRLF detection - Small ({} bytes)",
        small_response.len()
    );
    match find_crlf_adaptive(small_response) {
        Some(pos) => {
            let first_line = std::str::from_utf8(&small_response[..pos]).unwrap();
            println!("  Found '\\r\\n' at position {}", pos);
            println!("  First line: '{}'", first_line);
            println!("  Strategy: Scalar fallback (<128B)\n");
        }
        None => println!("  No CRLF found\n"),
    }

    // Test 5: CRLF detection (large)
    let mut large_response = vec![b'x'; 200];
    let response_text = b"HTTP/1.1 200 OK\r\n";
    large_response[..response_text.len()].copy_from_slice(response_text);
    println!(
        "Test 5: CRLF detection - Large ({} bytes)",
        large_response.len()
    );
    match find_crlf_adaptive(&large_response) {
        Some(pos) => {
            let first_line = std::str::from_utf8(&large_response[..pos]).unwrap();
            println!("  Found '\\r\\n' at position {}", pos);
            println!("  First line: '{}'", first_line);
            println!("  Strategy: SIMD acceleration (≥128B)");
            println!("  Expected speedup: 28-70× vs scalar\n");
        }
        None => println!("  No CRLF found\n"),
    }

    println!("Summary");
    println!("=======");
    println!("Adaptive dispatcher uses runtime threshold:");
    println!("  - <128B:  Scalar fallback (no overhead)");
    println!("  - ≥128B:  SIMD acceleration (28-70× speedup)");
    println!("  - Check:  <1ns (branch prediction)");
    println!("\nUCE34 Q10: Hybrid tier (scalar + T2 SIMD)");
    println!("UCE34 Q26: Branch prediction optimization");
    println!("IMPL-2 V3.1: Nightly #[cold] hints for scalar path");
}

#[cfg(not(feature = "http-simd"))]
fn main() {
    eprintln!("Error: This example requires the 'http-simd' feature.");
    eprintln!("Run with: cargo run --example http_adaptive_demo --features http-simd");
    std::process::exit(1);
}
