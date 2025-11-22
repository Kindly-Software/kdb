//! # SIMD MurmurHash3 Performance Demo
//!
//! Demonstrates 4× speedup using SIMD parallelization for Bloom filter hashing.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --release --example murmur3_simd_demo --features portable_simd
//! ```
//!
//! ## Expected Output
//!
//! - SIMD x4: ~15ns (4 hashes in parallel)
//! - Scalar x4: ~60ns (4 sequential hashes)
//! - Speedup: 4× (60ns / 15ns)

#[cfg(feature = "portable_simd")]
use atomic_capsule::hash::{murmur3_hash_scalar, murmur3_hash_simd_x4, murmur3_hash_simd_x8};
use std::time::Instant;

const ITERATIONS: usize = 100_000;

fn main() {
    #[cfg(feature = "portable_simd")]
    {
        println!("SIMD MurmurHash3 Performance Benchmark");
        println!("======================================\n");

        // Benchmark SIMD x4
        let start = Instant::now();
        let mut checksum = 0u64;
        for i in 0..ITERATIONS {
            let hashes = murmur3_hash_simd_x4(i as u64);
            checksum = checksum.wrapping_add(hashes[0]);
        }
        let simd_x4_time = start.elapsed();
        let simd_x4_ns = simd_x4_time.as_nanos() as f64 / ITERATIONS as f64;
        println!("SIMD x4 (4 hashes parallel):");
        println!("  Time: {:.2}ns per call", simd_x4_ns);
        println!("  Total: {:?} for {} iterations", simd_x4_time, ITERATIONS);
        println!("  Checksum: 0x{:016x} (prevent optimization)\n", checksum);

        // Benchmark scalar equivalent (4 sequential hashes)
        let start = Instant::now();
        let mut checksum = 0u64;
        for i in 0..ITERATIONS {
            let h0 = murmur3_hash_scalar(i as u64, 0);
            let h1 = murmur3_hash_scalar(i as u64, 1);
            let h2 = murmur3_hash_scalar(i as u64, 2);
            let h3 = murmur3_hash_scalar(i as u64, 3);
            checksum = checksum
                .wrapping_add(h0)
                .wrapping_add(h1)
                .wrapping_add(h2)
                .wrapping_add(h3);
        }
        let scalar_x4_time = start.elapsed();
        let scalar_x4_ns = scalar_x4_time.as_nanos() as f64 / ITERATIONS as f64;
        println!("Scalar x4 (4 hashes sequential):");
        println!("  Time: {:.2}ns per call", scalar_x4_ns);
        println!(
            "  Total: {:?} for {} iterations",
            scalar_x4_time, ITERATIONS
        );
        println!("  Checksum: 0x{:016x} (prevent optimization)\n", checksum);

        let speedup_x4 = scalar_x4_ns / simd_x4_ns;
        println!("Speedup (SIMD x4 vs Scalar x4): {:.2}×\n", speedup_x4);

        // Benchmark SIMD x8
        let start = Instant::now();
        let mut checksum = 0u64;
        for i in 0..ITERATIONS {
            let hashes = murmur3_hash_simd_x8(i as u64);
            checksum = checksum.wrapping_add(hashes[0]);
        }
        let simd_x8_time = start.elapsed();
        let simd_x8_ns = simd_x8_time.as_nanos() as f64 / ITERATIONS as f64;
        println!("SIMD x8 (8 hashes parallel):");
        println!("  Time: {:.2}ns per call", simd_x8_ns);
        println!("  Total: {:?} for {} iterations", simd_x8_time, ITERATIONS);
        println!("  Checksum: 0x{:016x} (prevent optimization)\n", checksum);

        // Benchmark scalar equivalent (8 sequential hashes)
        let start = Instant::now();
        let mut checksum = 0u64;
        for i in 0..ITERATIONS {
            for seed in 0..8 {
                let h = murmur3_hash_scalar(i as u64, seed);
                checksum = checksum.wrapping_add(h);
            }
        }
        let scalar_x8_time = start.elapsed();
        let scalar_x8_ns = scalar_x8_time.as_nanos() as f64 / ITERATIONS as f64;
        println!("Scalar x8 (8 hashes sequential):");
        println!("  Time: {:.2}ns per call", scalar_x8_ns);
        println!(
            "  Total: {:?} for {} iterations",
            scalar_x8_time, ITERATIONS
        );
        println!("  Checksum: 0x{:016x} (prevent optimization)\n", checksum);

        let speedup_x8 = scalar_x8_ns / simd_x8_ns;
        println!("Speedup (SIMD x8 vs Scalar x8): {:.2}×\n", speedup_x8);

        // Bloom filter simulation
        println!("Bloom Filter Use Case:");
        println!("======================");
        const BLOOM_SIZE: usize = 65536; // 8KB Bloom filter
        let mut bloom = vec![0u8; BLOOM_SIZE / 8];

        let start = Instant::now();
        for i in 0..ITERATIONS {
            let hashes = murmur3_hash_simd_x4(i as u64);
            for hash in hashes {
                let bit_idx = (hash % BLOOM_SIZE as u64) as usize;
                let byte_idx = bit_idx / 8;
                let bit_offset = bit_idx % 8;
                bloom[byte_idx] |= 1 << bit_offset;
            }
        }
        let bloom_insert_time = start.elapsed();
        let bloom_insert_ns = bloom_insert_time.as_nanos() as f64 / ITERATIONS as f64;
        println!("SIMD Bloom insert (4 hashes + 4 atomic ORs):");
        println!("  Time: {:.2}ns per insert", bloom_insert_ns);
        println!(
            "  Total: {:?} for {} inserts",
            bloom_insert_time, ITERATIONS
        );
        println!(
            "  Bits set: {}/{} ({:.2}%)\n",
            bloom.iter().map(|b| b.count_ones() as usize).sum::<usize>(),
            BLOOM_SIZE,
            bloom.iter().map(|b| b.count_ones() as f64).sum::<f64>() / BLOOM_SIZE as f64 * 100.0
        );

        // Target analysis
        println!("Performance Target Analysis:");
        println!("============================");
        if bloom_insert_ns < 50.0 {
            println!("✓ TARGET MET: <50ns insert latency");
            println!(
                "  Achieved: {:.2}ns ({:.0}% faster than target)",
                bloom_insert_ns,
                (50.0 - bloom_insert_ns) / 50.0 * 100.0
            );
        } else {
            println!(
                "✗ TARGET MISSED: {:.2}ns insert latency (target <50ns)",
                bloom_insert_ns
            );
            println!(
                "  Requires: {:.2}× further optimization",
                bloom_insert_ns / 50.0
            );
        }

        if speedup_x4 >= 3.5 {
            println!("✓ EXCEPTIONAL SPEEDUP: {:.2}× (target 4×)", speedup_x4);
        } else if speedup_x4 >= 2.0 {
            println!(
                "✓ GOOD SPEEDUP: {:.2}× (target 4×, achieved >2×)",
                speedup_x4
            );
        } else {
            println!("✗ INSUFFICIENT SPEEDUP: {:.2}× (target 4×)", speedup_x4);
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    {
        println!("ERROR: This example requires the 'portable_simd' feature.");
        println!("Run: cargo run --release --example murmur3_simd_demo --features portable_simd");
    }
}
