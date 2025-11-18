//! Standalone PUF test for 6900HX validation
//!
//! Tests 3-source PUF extraction on AMD Ryzen 9 6900HX

#[cfg(target_arch = "x86_64")]
fn main() {
    println!("=== PUF Extraction Test (AMD Ryzen 9 6900HX) ===\n");

    // Test 1: RDRAND timing
    println!("Source 1: RDRAND Timing");
    let rdrand = extract_rdrand_timing();
    print_entropy_stats("RDRAND", &rdrand);

    // Test 2: Cache latency
    println!("\nSource 2: Cache Latency");
    let cache = extract_cache_latency();
    print_entropy_stats("Cache", &cache);

    // Test 3: Memory row timing
    println!("\nSource 3: Memory Row Timing");
    let memory = extract_memory_row();
    print_entropy_stats("Memory", &memory);

    // Test 4: Combined (XOR)
    println!("\nCombined (XOR):");
    let mut combined = [0u8; 32];
    for i in 0..32 {
        combined[i] = rdrand[i] ^ cache[i] ^ memory[i];
    }
    print_entropy_stats("Combined", &combined);

    // Test 5: Stability (10 extractions)
    println!("\n=== Stability Test (10 extractions) ===");
    test_stability();
}

#[cfg(not(target_arch = "x86_64"))]
fn main() {
    eprintln!("This test requires x86-64 architecture");
    std::process::exit(1);
}

fn extract_rdrand_timing() -> [u8; 32] {
    let mut entropy = [0u8; 32];

    for i in 0..256 {
        unsafe {
            let start = std::arch::x86_64::_rdtsc();
            let mut rand_val = 0u64;
            std::arch::x86_64::_rdrand64_step(&mut rand_val);
            let end = std::arch::x86_64::_rdtsc();

            let latency = end.wrapping_sub(start);

            // Extract bit from different positions to capture variation
            let bit_pos = i % 64;
            let bit = ((latency >> bit_pos) & 1) as u8;
            entropy[i / 8] |= bit << (i % 8);
        }
    }

    entropy
}

fn extract_cache_latency() -> [u8; 32] {
    let mut entropy = [0u8; 32];
    let cache_lines = vec![[0u64; 8]; 256];

    for i in 0..256 {
        unsafe {
            let ptr = cache_lines[i].as_ptr() as *const u8;
            std::arch::x86_64::_mm_clflush(ptr);

            let start = std::arch::x86_64::_rdtsc();
            let _ = cache_lines[i][0];
            let end = std::arch::x86_64::_rdtsc();

            let latency = end.wrapping_sub(start);

            let bit_pos = i % 64;
            let bit = ((latency >> bit_pos) & 1) as u8;
            entropy[i / 8] |= bit << (i % 8);
        }
    }

    entropy
}

fn extract_memory_row() -> [u8; 32] {
    let mut entropy = [0u8; 32];
    let memory_rows = vec![[0u64; 1024]; 256];

    for i in 0..256 {
        unsafe {
            let start = std::arch::x86_64::_rdtsc();
            let _ = memory_rows[i][0];
            let end = std::arch::x86_64::_rdtsc();

            let latency = end.wrapping_sub(start);

            let bit_pos = i % 64;
            let bit = ((latency >> bit_pos) & 1) as u8;
            entropy[i / 8] |= bit << (i % 8);
        }
    }

    entropy
}

fn print_entropy_stats(name: &str, entropy: &[u8; 32]) {
    let ones = entropy.iter().map(|b| b.count_ones()).sum::<u32>();
    let zeros = 256 - ones;
    let percentage = (ones as f64 / 256.0) * 100.0;

    println!("  {} bits set: {} ({}%)", name, ones, percentage);
    println!("  0 bits set: {} ({}%)", zeros, 100.0 - percentage);

    if ones == 0 {
        println!("  ⚠️  WARNING: All zeros (timing too stable)");
    } else if ones == 256 {
        println!("  ⚠️  WARNING: All ones (timing inverted)");
    } else if (ones as f64 / 256.0) > 0.45 && (ones as f64 / 256.0) < 0.55 {
        println!("  ✓ Good distribution (near 50/50)");
    }
}

fn test_stability() {
    let mut extractions = Vec::new();

    for i in 0..10 {
        let rdrand = extract_rdrand_timing();
        let cache = extract_cache_latency();
        let memory = extract_memory_row();

        let mut combined = [0u8; 32];
        for j in 0..32 {
            combined[j] = rdrand[j] ^ cache[j] ^ memory[j];
        }

        extractions.push(combined);
        println!("  Extraction {}: {} bits set", i + 1, combined.iter().map(|b| b.count_ones()).sum::<u32>());
    }

    // Measure drift between first and each subsequent extraction
    let baseline = extractions[0];
    println!("\nDrift analysis (vs extraction 1):");

    for (i, extraction) in extractions.iter().enumerate().skip(1) {
        let mut drift = 0;
        for (b1, b2) in baseline.iter().zip(extraction.iter()) {
            drift += (b1 ^ b2).count_ones() as usize;
        }

        let drift_pct = (drift as f64 / 256.0) * 100.0;
        println!("  Extraction {}: {} bits drift ({:.2}%)", i + 1, drift, drift_pct);

        if drift <= 26 {
            println!("    ✓ Excellent (≤10% drift)");
        } else if drift <= 51 {
            println!("    ⚠️  Acceptable (≤20% drift)");
        } else {
            println!("    ❌ Too unstable (>20% drift)");
        }
    }
}
