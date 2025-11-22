//! T2 SIMD HTTP Header Parsing Benchmarks
//!
//! **UCE34 Q33**: Empirical validation with B32 framework
//! **Target**: 7× speedup for multi-header parsing (10+ headers)
//! **Baseline**: Scalar byte-by-byte search
//! **SIMD**: u8x32 vectorized search (AVX2, 32 bytes/op)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(feature = "http-simd")]
use atomic_capsule::http::{find_colon_simd, find_crlf_simd, parse_headers_simd};

/// Scalar fallback (baseline for comparison)
fn find_colon_scalar(haystack: &[u8]) -> Option<usize> {
    haystack.iter().position(|&b| b == b':')
}

/// Scalar CRLF search (baseline)
fn find_crlf_scalar(haystack: &[u8]) -> Option<usize> {
    haystack.windows(2).position(|window| window == b"\r\n")
}

/// Benchmark: Find ':' separator (scalar vs SIMD)
fn bench_find_colon(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_colon");

    // Test different sizes (8 bytes to 1024 bytes)
    for size in [8, 16, 32, 64, 128, 256, 512, 1024].iter() {
        let mut input = vec![b'x'; *size];
        input[*size - 1] = b':'; // Put ':' at end (worst case)

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            b.iter(|| find_colon_scalar(black_box(&input)))
        });

        #[cfg(feature = "http-simd")]
        group.bench_with_input(BenchmarkId::new("simd", size), size, |b, _| {
            b.iter(|| find_colon_simd(black_box(&input)))
        });
    }

    group.finish();
}

/// Benchmark: Find '\r\n' line ending (scalar vs SIMD)
fn bench_find_crlf(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_crlf");

    for size in [8, 16, 32, 64, 128, 256, 512, 1024].iter() {
        let mut input = vec![b'x'; *size];
        if *size >= 2 {
            input[*size - 2] = b'\r';
            input[*size - 1] = b'\n';
        }

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            b.iter(|| find_crlf_scalar(black_box(&input)))
        });

        #[cfg(feature = "http-simd")]
        group.bench_with_input(BenchmarkId::new("simd", size), size, |b, _| {
            b.iter(|| find_crlf_simd(black_box(&input)))
        });
    }

    group.finish();
}

/// Parse headers with scalar search
fn parse_headers_scalar(input: &str) -> Vec<(&str, &str)> {
    let bytes = input.as_bytes();
    let mut headers = Vec::new();
    let mut pos = 0;

    while pos < bytes.len() {
        // Find line ending
        let line_end = match find_crlf_scalar(&bytes[pos..]) {
            Some(offset) => pos + offset,
            None => break,
        };

        if line_end == pos {
            break; // Empty line
        }

        let line = &bytes[pos..line_end];

        // Find ':'
        if let Some(colon_pos) = find_colon_scalar(line) {
            let name_bytes = &line[..colon_pos];
            let value_bytes = &line[colon_pos + 1..];

            let value_bytes = value_bytes
                .iter()
                .position(|&b| b != b' ')
                .map(|i| &value_bytes[i..])
                .unwrap_or(b"");

            if let (Ok(name), Ok(value)) = (
                core::str::from_utf8(name_bytes),
                core::str::from_utf8(value_bytes),
            ) {
                headers.push((name, value));
            }
        }

        pos = line_end + 2;
    }

    headers
}

/// Benchmark: Multi-header parsing (scalar vs SIMD)
fn bench_parse_headers(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_headers");

    // Test with different header counts
    for count in [1, 5, 10, 20, 50].iter() {
        let mut input = String::new();
        for i in 0..*count {
            input.push_str(&format!("Header-{}: Value-{}\r\n", i, i));
        }
        input.push_str("\r\n");

        group.bench_with_input(BenchmarkId::new("scalar", count), count, |b, _| {
            b.iter(|| parse_headers_scalar(black_box(&input)))
        });

        #[cfg(feature = "http-simd")]
        group.bench_with_input(BenchmarkId::new("simd", count), count, |b, _| {
            b.iter(|| parse_headers_simd(black_box(&input)).unwrap())
        });
    }

    group.finish();
}

/// Benchmark: Real-world HTTP request headers
fn bench_real_world_headers(c: &mut Criterion) {
    let input = concat!(
        "Host: example.com\r\n",
        "User-Agent: Mozilla/5.0\r\n",
        "Accept: text/html,application/xhtml+xml\r\n",
        "Accept-Language: en-US,en;q=0.9\r\n",
        "Accept-Encoding: gzip, deflate, br\r\n",
        "Connection: keep-alive\r\n",
        "Upgrade-Insecure-Requests: 1\r\n",
        "Cache-Control: max-age=0\r\n",
        "Content-Type: application/json\r\n",
        "Content-Length: 1234\r\n",
        "\r\n"
    );

    let mut group = c.benchmark_group("real_world_headers");

    group.bench_function("scalar", |b| {
        b.iter(|| parse_headers_scalar(black_box(input)))
    });

    #[cfg(feature = "http-simd")]
    group.bench_function("simd", |b| {
        b.iter(|| parse_headers_simd(black_box(input)).unwrap())
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_find_colon,
    bench_find_crlf,
    bench_parse_headers,
    bench_real_world_headers
);
criterion_main!(benches);
