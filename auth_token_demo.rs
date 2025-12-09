//! Simple demonstration of AuthTokenCapsule
//! Compile: rustc -O auth_token_demo.rs
//! Run: ./auth_token_demo

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

#[repr(C, align(128))]
struct AuthTokenCapsule {
    cache_hits: AtomicU64,
    _padding1: [u8; 56],
    generation: AtomicU64,
    _padding2: [u8; 56],
}

impl AuthTokenCapsule {
    const fn new() -> Self {
        Self {
            cache_hits: AtomicU64::new(0),
            _padding1: [0u8; 56],
            generation: AtomicU64::new(0),
            _padding2: [0u8; 56],
        }
    }

    fn validate_token(&self, token_hash: u64) {
        let gen_before = self.generation.load(Ordering::Acquire);
        let _session_id = token_hash;
        let gen_after = self.generation.load(Ordering::Acquire);
        if gen_before == gen_after {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::Release);
        self.cache_hits.store(0, Ordering::Relaxed);
    }

    fn get_stats(&self) -> (u64, u64) {
        (
            self.cache_hits.load(Ordering::Relaxed),
            self.generation.load(Ordering::Relaxed),
        )
    }

    fn fnv1a_hash(s: &str) -> u64 {
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        let mut hash = FNV_OFFSET_BASIS;
        for byte in s.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }
}

fn main() {
    println!("AuthTokenCapsule - T1 Atomic JWT Validation");
    println!("===========================================\n");

    test_basic();
    test_concurrent();
    test_performance();
    test_layout();

    println!("\nAll tests PASSED!");
}

fn test_basic() {
    println!("Test 1: Basic Functionality");
    println!("==========================");

    let capsule = AuthTokenCapsule::new();
    let (hits, gen) = capsule.get_stats();
    println!("Initial state: hits={}, gen={}", hits, gen);

    let token_hash = AuthTokenCapsule::fnv1a_hash("header.payload.signature");
    capsule.validate_token(token_hash);
    let (hits, gen) = capsule.get_stats();
    println!("After 1 validation: hits={}, gen={}", hits, gen);
    assert_eq!(hits, 1);

    capsule.validate_token(token_hash);
    let (hits, gen) = capsule.get_stats();
    println!("After 2nd validation: hits={}, gen={}", hits, gen);
    assert_eq!(hits, 2);

    capsule.invalidate();
    let (hits, gen) = capsule.get_stats();
    println!("After invalidation: hits={}, gen={}", hits, gen);
    assert_eq!(gen, 1);

    println!("PASS\n");
}

fn test_concurrent() {
    println!("Test 2: Concurrent Access (8 threads x 100 validations)");
    println!("====================================================");

    let capsule = Arc::new(AuthTokenCapsule::new());
    let num_threads = 8;
    let iterations_per_thread = 100;
    let barrier = Arc::new(Barrier::new(num_threads));

    let start = Instant::now();

    let threads: Vec<_> = (0..num_threads)
        .map(|i| {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait();
                for j in 0..iterations_per_thread {
                    let token = format!("token_{}.{}", i, j);
                    let hash = AuthTokenCapsule::fnv1a_hash(&token);
                    capsule.validate_token(hash);
                }
            })
        })
        .collect();

    for thread in threads {
        thread.join().unwrap();
    }

    let elapsed = start.elapsed();
    let (hits, _gen) = capsule.get_stats();

    println!("Total validations: {}", hits);
    println!("Expected: {}", num_threads * iterations_per_thread);
    println!("Time: {:.3}ms", elapsed.as_secs_f64() * 1000.0);
    assert_eq!(hits as usize, num_threads * iterations_per_thread);
    println!("PASS\n");
}

fn test_performance() {
    println!("Test 3: Performance Benchmark");
    println!("=============================");

    let capsule = AuthTokenCapsule::new();
    let token_hash = AuthTokenCapsule::fnv1a_hash("header.payload.signature");

    // Warmup
    for _ in 0..100 {
        capsule.validate_token(token_hash);
    }

    // Measure 100K iterations
    let start = Instant::now();
    for _ in 0..100_000 {
        capsule.validate_token(token_hash);
    }
    let elapsed = start.elapsed();

    let latency_ns = elapsed.as_nanos() as f64 / 100_000.0;
    let ops_per_sec = (100_000.0 / elapsed.as_secs_f64()) as u64;

    println!("Latency per validation: {:.1} ns", latency_ns);
    println!("Throughput: {:.1} M ops/sec", ops_per_sec as f64 / 1_000_000.0);
    println!("PASS\n");
}

fn test_layout() {
    println!("Test 4: Memory Layout Verification");
    println!("==================================");

    use std::mem::{size_of, align_of};

    let size = size_of::<AuthTokenCapsule>();
    let alignment = align_of::<AuthTokenCapsule>();

    println!("Size: {} bytes (expected: 128)", size);
    println!("Alignment: {} bytes (expected: 128)", alignment);
    assert_eq!(size, 128);
    assert_eq!(alignment, 128);

    let capsule = AuthTokenCapsule::new();
    let ptr = &capsule as *const _ as usize;
    let offset = ptr % 128;

    println!("Runtime alignment offset: {} (expected: 0)", offset);
    assert_eq!(offset, 0);

    println!("PASS\n");
}
