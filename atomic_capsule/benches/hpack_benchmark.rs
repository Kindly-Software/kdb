//! HPACK Benchmark Suite - RFC 7541 Performance Validation (B32 Framework)
//!
//! **Performance Targets** (B32 validation):
//! - Encode: <2μs per header (including Huffman)
//! - Decode: <3μs per header (including Huffman)
//! - Compression: 30-50% ratio typical (static table matching)
//! - Static lookup: <100ns (T1 Atomic performance)
//! - Dynamic lookup: <500ns (with table management)

use atomic_capsule::http::hpack::*;
use core::sync::atomic::Ordering;

fn main() {
    println!("HPACK Header Compression Benchmarks (RFC 7541)\n");
    println!("=".repeat(70));

    // Benchmark 1: Static table lookup performance
    bench_static_table_lookup();

    // Benchmark 2: Encoding common headers
    bench_encode_common_headers();

    // Benchmark 3: Decoding indexed headers
    bench_decode_indexed_headers();

    // Benchmark 4: Literal header encoding
    bench_encode_literal_headers();

    // Benchmark 5: Compression ratio analysis
    bench_compression_ratios();

    // Benchmark 6: Multi-header encoding throughput
    bench_multi_header_throughput();

    // Benchmark 7: Concurrent encoding (thread safety)
    bench_concurrent_encoding();

    println!("\n{}", "=".repeat(70));
    println!("All benchmarks completed successfully!");
}

/// Benchmark 1: Static table lookup performance
fn bench_static_table_lookup() {
    println!("\n[BENCH 1] Static Table Lookup (<100ns target, T1 Atomic)");
    println!("-".repeat(70));

    let encoder = HpackEncoderCapsule::new();
    let iterations = 100_000;

    // Lookup common headers
    let test_cases = vec![
        (b":method" as &[u8], b"GET" as &[u8]),
        (b":path", b"/"),
        (b":scheme", b"https"),
        (b":status", b"200"),
        (b"content-type", b"application/json"),
    ];

    for (name, value) in test_cases {
        let start = std::time::Instant::now();

        for _ in 0..iterations {
            let _ = encoder.lookup_static_table(name, value);
        }

        let elapsed = start.elapsed();
        let per_op = elapsed.as_nanos() / iterations as u128;

        println!(
            "  {:30} {:20}: {} ns/op ({:.2}% of 100ns target)",
            String::from_utf8_lossy(name),
            String::from_utf8_lossy(value),
            per_op,
            (per_op as f64 / 100.0) * 100.0
        );
    }
}

/// Benchmark 2: Encoding common headers
fn bench_encode_common_headers() {
    println!("\n[BENCH 2] Encoding Common Headers (<2μs target, including Huffman)");
    println!("-".repeat(70));

    let encoder = HpackEncoderCapsule::new();
    let iterations = 10_000;

    let test_cases = vec![
        ("GET", (b":method" as &[u8], b"GET" as &[u8])),
        ("POST", (b":method", b"POST")),
        ("200 OK", (b":status", b"200")),
        ("404 NOT FOUND", (b":status", b"404")),
        ("Path /", (b":path", b"/")),
        ("HTTPS", (b":scheme", b"https")),
    ];

    for (label, (name, value)) in test_cases {
        let start = std::time::Instant::now();

        for _ in 0..iterations {
            let _ = encoder.encode_header(name, value, false);
        }

        let elapsed = start.elapsed();
        let per_op = elapsed.as_micros() as f64 / iterations as f64;

        println!(
            "  {:30}: {:.3} μs/op ({:.0}% of 2μs target)",
            label,
            per_op,
            (per_op / 2.0) * 100.0
        );
    }
}

/// Benchmark 3: Decoding indexed headers
fn bench_decode_indexed_headers() {
    println!("\n[BENCH 3] Decoding Indexed Headers (<3μs target)");
    println!("-".repeat(70));

    let decoder = HpackDecoderCapsule::new();
    let iterations = 10_000;

    let test_cases = vec![
        ("Method GET", [0x82_u8] as &[u8]),   // Index 2
        ("Status 200", [0x88_u8] as &[u8]),   // Index 8
        ("Status 404", [0x8d_u8] as &[u8]),   // Index 13
        ("Scheme HTTPS", [0x87_u8] as &[u8]), // Index 7
        ("Path /", [0x84_u8] as &[u8]),       // Index 4
    ];

    for (label, buffer) in test_cases {
        let start = std::time::Instant::now();

        for _ in 0..iterations {
            let _ = decoder.decode_header(buffer);
        }

        let elapsed = start.elapsed();
        let per_op = elapsed.as_micros() as f64 / iterations as f64;

        println!(
            "  {:30}: {:.3} μs/op ({:.0}% of 3μs target)",
            label,
            per_op,
            (per_op / 3.0) * 100.0
        );
    }
}

/// Benchmark 4: Literal header encoding
fn bench_encode_literal_headers() {
    println!("\n[BENCH 4] Literal Header Encoding (custom headers)");
    println!("-".repeat(70));

    let encoder = HpackEncoderCapsule::new();
    let iterations = 5_000;

    let test_cases = vec![
        ("Short custom", (b"x-id" as &[u8], b"123" as &[u8])),
        (
            "Medium custom",
            (b"x-request-id", b"550e8400-e29b-41d4-a716-446655440000"),
        ),
        (
            "Long custom",
            (
                b"x-trace-path",
                b"service1->service2->service3->service4->service5",
            ),
        ),
    ];

    for (label, (name, value)) in test_cases {
        let start = std::time::Instant::now();

        for _ in 0..iterations {
            let _ = encoder.encode_header(name, value, false);
        }

        let elapsed = start.elapsed();
        let per_op = elapsed.as_micros() as f64 / iterations as f64;

        println!(
            "  {:30}: {:.3} μs/op (value len: {})",
            label,
            per_op,
            value.len()
        );
    }
}

/// Benchmark 5: Compression ratio analysis
fn bench_compression_ratios() {
    println!("\n[BENCH 5] Compression Ratio Analysis (30-50% target)");
    println!("-".repeat(70));

    let encoder = HpackEncoderCapsule::new();

    // Common HTTP/2 request headers
    let headers = vec![
        (b":method" as &[u8], b"GET" as &[u8]),
        (b":scheme", b"https"),
        (b":authority", b"www.example.com"),
        (b":path", b"/api/users/123"),
        (b"user-agent", b"Mozilla/5.0"),
        (b"accept", b"application/json"),
        (b"accept-encoding", b"gzip, deflate, br"),
    ];

    for (name, value) in headers {
        let _ = encoder.encode_header(name, value, false);
    }

    let metrics = encoder.metrics();
    let ratio = metrics.compression_ratio();

    println!("  Headers encoded:     {}", metrics.headers_encoded);
    println!("  Bytes before:        {}", metrics.bytes_before);
    println!("  Bytes after:         {}", metrics.bytes_after);
    println!(
        "  Compression ratio:   {:.2}% ({:.0}% improvement)",
        ratio * 100.0,
        (1.0 - ratio) * 100.0
    );
    println!("  Indexed lookups:     {}", metrics.indexed_lookups);
    println!("  Literal encodings:   {}", metrics.literal_encodings);
}

/// Benchmark 6: Multi-header throughput
fn bench_multi_header_throughput() {
    println!("\n[BENCH 6] Multi-Header Throughput (realistic requests)");
    println!("-".repeat(70));

    let encoder = HpackEncoderCapsule::new();
    let iterations = 1_000;

    // Realistic HTTP/2 request with 7 headers
    let headers = vec![
        (b":method".to_vec(), b"POST".to_vec()),
        (b":scheme".to_vec(), b"https".to_vec()),
        (b":authority".to_vec(), b"api.example.com".to_vec()),
        (b":path".to_vec(), b"/v1/users".to_vec()),
        (b"content-type".to_vec(), b"application/json".to_vec()),
        (b"authorization".to_vec(), b"Bearer token123".to_vec()),
        (b"x-request-id".to_vec(), b"req-12345".to_vec()),
    ];

    let start = std::time::Instant::now();

    for _ in 0..iterations {
        let _ = encoder.encode_headers(&headers);
    }

    let elapsed = start.elapsed();
    let total_headers = headers.len() * iterations;
    let per_header = elapsed.as_micros() as f64 / total_headers as f64;
    let throughput = (1_000_000 / elapsed.as_micros()) * total_headers as u128;

    println!("  Request headers:     {}", headers.len());
    println!("  Iterations:          {}", iterations);
    println!("  Total headers:       {}", total_headers);
    println!("  Per-header latency:  {:.3} μs", per_header);
    println!("  Throughput:          {} headers/sec", throughput);
}

/// Benchmark 7: Concurrent encoding stress test
fn bench_concurrent_encoding() {
    println!("\n[BENCH 7] Concurrent Encoding (8 threads, thread-safety validation)");
    println!("-".repeat(70));

    use std::sync::Arc;
    use std::thread;

    let encoder = Arc::new(HpackEncoderCapsule::new());
    let iterations_per_thread = 10_000;
    let num_threads = 8;

    let start = std::time::Instant::now();

    let mut handles = vec![];
    for _ in 0..num_threads {
        let enc = Arc::clone(&encoder);
        let handle = thread::spawn(move || {
            for _ in 0..iterations_per_thread {
                let _ = enc.encode_header(b":method", b"GET", false);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = num_threads * iterations_per_thread;
    let per_op = elapsed.as_nanos() / total_ops as u128;
    let throughput = (1_000_000_000 / elapsed.as_nanos()) * total_ops as u128;

    println!("  Threads:             {}", num_threads);
    println!("  Iterations/thread:   {}", iterations_per_thread);
    println!("  Total operations:    {}", total_ops);
    println!("  Per-operation:       {} ns", per_op);
    println!("  Throughput:          {} ops/sec", throughput);

    let metrics = encoder.metrics();
    println!(
        "  Headers encoded:     {} (all threads combined)",
        metrics.headers_encoded
    );
}
