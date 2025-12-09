//! T1 Atomic example: Circuit Breaker (Atomic vs RwLock)
//!
//! Demonstrates benchmarking a T1 Atomic capsule against a fair RwLock baseline.

use kindly_bench::{BenchmarkConfig, run_benchmark};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

/// Circuit breaker states
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Closed = 0,
    Open = 1,
    HalfOpen = 2,
}

/// Optimized: T1 Atomic circuit breaker
struct AtomicCircuitBreaker {
    state: AtomicU64,
}

impl AtomicCircuitBreaker {
    fn new(initial: State) -> Self {
        Self {
            state: AtomicU64::new(initial as u64),
        }
    }

    fn transition(&self, new_state: State) {
        self.state.store(new_state as u64, Ordering::Release);
    }

    fn get_state(&self) -> State {
        match self.state.load(Ordering::Acquire) {
            0 => State::Closed,
            1 => State::Open,
            2 => State::HalfOpen,
            _ => State::Closed,
        }
    }
}

/// Fair baseline: RwLock-based circuit breaker
struct RwLockCircuitBreaker {
    state: RwLock<u64>,
}

impl RwLockCircuitBreaker {
    fn new(initial: State) -> Self {
        Self {
            state: RwLock::new(initial as u64),
        }
    }

    fn transition(&self, new_state: State) {
        *self.state.write().unwrap() = new_state as u64;
    }

    fn get_state(&self) -> State {
        match *self.state.read().unwrap() {
            0 => State::Closed,
            1 => State::Open,
            2 => State::HalfOpen,
            _ => State::Closed,
        }
    }
}

fn main() {
    println!("T1 Atomic Example: Circuit Breaker");
    println!("Comparing lockfree atomic vs RwLock implementation");

    // Create instances
    let atomic_breaker = AtomicCircuitBreaker::new(State::Closed);
    let rwlock_breaker = RwLockCircuitBreaker::new(State::Closed);

    // Configure benchmark
    let config = BenchmarkConfig::new(
        "CircuitBreaker::transition",
        "T1-Atomic",
        "RwLock"
    )
    .iterations(10_000)
    .warmup(100);

    // Run benchmark
    run_benchmark(
        config,
        || {
            // Optimized: Atomic operations
            atomic_breaker.transition(State::Open);
            atomic_breaker.transition(State::HalfOpen);
            atomic_breaker.transition(State::Closed);
            let _state = atomic_breaker.get_state();
        },
        || {
            // Baseline: RwLock operations
            rwlock_breaker.transition(State::Open);
            rwlock_breaker.transition(State::HalfOpen);
            rwlock_breaker.transition(State::Closed);
            let _state = rwlock_breaker.get_state();
        },
    );
}
