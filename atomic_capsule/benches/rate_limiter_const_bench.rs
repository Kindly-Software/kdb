//! Benchmark: RateLimiterConst - Const Generic Rate Limiter
//!
//! **Performance Target**: 3-10× speedup vs RateLimiterCapsule (0ns allocation)
//!
//! ## Baseline Comparison
//! - RateLimiterCapsule (runtime): Single rate/burst per instance, heap allocation
//! - RateLimiterConst (compile-time): Multiple const variants, zero allocation
//!
//! ## Benchmark Groups
//! 1. Single try_acquire(): 1000 attempts at 1 kHz rate
//! 2. Burst handling: Acquire 100 tokens, refill, retry
//! 3. Concurrent: 4 threads acquiring at different rates
//! 4. Realistic: 1M attempts simulating real workload

#![allow(missing_docs)]

#[cfg(all(test, feature = "nightly-const-streaming"))]
mod benches {
    use atomic_capsule::patterns::RateLimiterConst;
    use std::sync::Arc;
    use std::thread;
    use std::time::Instant;

    // ========== GROUP 1: Single try_acquire() Benchmark ==========

    #[test]
    fn bench_try_acquire_1khz_100_attempts() {
        let limiter: RateLimiterConst<1000, 100> = RateLimiterConst::new();

        let start = Instant::now();
        for _ in 0..100 {
            let _ = limiter.try_acquire(1);
        }
        let elapsed = start.elapsed();

        println!(
            "try_acquire (1kHz, 100 attempts): {:.2} μs total, {:.0} ns/attempt",
            elapsed.as_micros(),
            elapsed.as_nanos() as f64 / 100.0
        );
    }

    #[test]
    fn bench_try_acquire_10khz_1000_attempts() {
        let limiter: RateLimiterConst<10000, 500> = RateLimiterConst::new();

        let start = Instant::now();
        for _ in 0..1000 {
            let _ = limiter.try_acquire(1);
        }
        let elapsed = start.elapsed();

        println!(
            "try_acquire (10kHz, 1000 attempts): {:.2} ms total, {:.0} ns/attempt",
            elapsed.as_micros() as f64 / 1000.0,
            elapsed.as_nanos() as f64 / 1000.0
        );
    }

    #[test]
    fn bench_try_acquire_100khz_10000_attempts() {
        let limiter: RateLimiterConst<100000, 1000> = RateLimiterConst::new();

        let start = Instant::now();
        for _ in 0..10000 {
            let _ = limiter.try_acquire(1);
        }
        let elapsed = start.elapsed();

        println!(
            "try_acquire (100kHz, 10k attempts): {:.2} ms total, {:.0} ns/attempt",
            elapsed.as_micros() as f64 / 1000.0,
            elapsed.as_nanos() as f64 / 10000.0
        );
    }

    // ========== GROUP 2: Burst Handling Benchmark ==========

    #[test]
    fn bench_burst_exhaust_and_refill() {
        let limiter: RateLimiterConst<100, 5> = RateLimiterConst::new();

        let start = Instant::now();
        for _ in 0..100 {
            // Try to acquire until we hit the limit
            let mut acquired = 0;
            while acquired < 5 && limiter.try_acquire(1) {
                acquired += 1;
            }
            // Now we wait or retry (in real scenario, would wait for refill)
        }
        let elapsed = start.elapsed();

        println!(
            "Burst exhaust+retry (100Hz, 100 cycles): {:.2} ms total, {:.0} μs/cycle",
            elapsed.as_micros() as f64 / 1000.0,
            elapsed.as_micros() as f64 / 100.0
        );
    }

    // ========== GROUP 3: Concurrent Access Benchmark ==========

    #[test]
    fn bench_concurrent_4threads() {
        let limiter = Arc::new(RateLimiterConst::<10000, 50>::new());
        let mut handles = vec![];

        let start = Instant::now();

        for _ in 0..4 {
            let limiter_clone = Arc::clone(&limiter);
            let handle = thread::spawn(move || {
                let mut acquired = 0;
                for _ in 0..250 {
                    if limiter_clone.try_acquire(1) {
                        acquired += 1;
                    }
                }
                acquired
            });
            handles.push(handle);
        }

        let mut total_acquired = 0;
        for handle in handles {
            total_acquired += handle.join().unwrap();
        }

        let elapsed = start.elapsed();

        println!(
            "Concurrent 4 threads (10kHz, 1000 total attempts): {:.2} ms, {} acquired, {:.0} ns/attempt",
            elapsed.as_micros() as f64 / 1000.0,
            total_acquired,
            elapsed.as_nanos() as f64 / 1000.0
        );
    }

    // ========== GROUP 4: Realistic Production Benchmark ==========

    #[test]
    fn bench_realistic_1m_requests() {
        let limiter: RateLimiterConst<1000, 100> = RateLimiterConst::new();

        let start = Instant::now();
        for _ in 0..1000000 {
            let _ = limiter.try_acquire(1);
        }
        let elapsed = start.elapsed();

        println!(
            "Realistic 1M requests (1kHz, 100 burst): {:.2} ms total, {:.2} ns/request",
            elapsed.as_millis(),
            elapsed.as_nanos() as f64 / 1_000_000.0
        );
    }

    // ========== HELPER: Available Tokens Benchmark ==========

    #[test]
    fn bench_available_tokens_lookup() {
        let limiter: RateLimiterConst<1000, 100> = RateLimiterConst::new();

        let start = Instant::now();
        for _ in 0..10000 {
            let _ = limiter.available_tokens();
        }
        let elapsed = start.elapsed();

        println!(
            "available_tokens (10k lookups): {:.2} μs total, {:.0} ns/lookup",
            elapsed.as_micros(),
            elapsed.as_nanos() as f64 / 10000.0
        );
    }

    // ========== MEMORY FOOTPRINT TEST ==========

    #[test]
    fn test_memory_layout() {
        use std::mem;

        let limiter: RateLimiterConst<100, 5> = RateLimiterConst::new();

        println!(
            "RateLimiterConst layout: size={} bytes, align={} bytes",
            mem::size_of_val(&limiter),
            mem::align_of_val(&limiter)
        );

        // Verify it's exactly 64 bytes
        assert_eq!(mem::size_of_val(&limiter), 64);
        // Verify it's 64-byte aligned
        assert_eq!(mem::align_of_val(&limiter), 64);
    }
}
