//! T1 Atomic baseline: Replace atomic capsules with RwLock/Mutex
//!
//! Fair baseline strategy:
//! - Use RwLock (optimized for read-heavy workloads)
//! - Mutex as fallback (more conservative, write-heavy)
//! - Same operations, different synchronization primitive

use std::sync::RwLock;

/// Example: T1 Atomic baseline using RwLock
///
/// For user-defined capsules, developers should follow this pattern:
/// 1. Replace atomic fields with RwLock-protected equivalents
/// 2. Use read() for load operations
/// 3. Use write() for store/update operations
/// 4. Maintain same algorithmic logic
///
/// # Example
///
/// ```rust,ignore
/// // Optimized (T1 Atomic)
/// struct CircuitBreaker {
///     state: AtomicU64,
/// }
///
/// impl CircuitBreaker {
///     fn transition(&self, new_state: State) {
///         self.state.store(new_state as u64, Ordering::Release);
///     }
///
///     fn get_state(&self) -> State {
///         State::from(self.state.load(Ordering::Acquire))
///     }
/// }
///
/// // Baseline (RwLock)
/// struct CircuitBreakerBaseline {
///     state: RwLock<u64>,
/// }
///
/// impl CircuitBreakerBaseline {
///     fn transition(&self, new_state: State) {
///         *self.state.write().unwrap() = new_state as u64;
///     }
///
///     fn get_state(&self) -> State {
///         State::from(*self.state.read().unwrap())
///     }
/// }
/// ```

/// Simple counter example (T1 Atomic)
pub struct AtomicCounter {
    count: std::sync::atomic::AtomicU64,
}

impl AtomicCounter {
    pub fn new(initial: u64) -> Self {
        Self {
            count: std::sync::atomic::AtomicU64::new(initial),
        }
    }

    pub fn increment(&self) {
        self.count.fetch_add(1, std::sync::atomic::Ordering::Release);
    }

    pub fn get(&self) -> u64 {
        self.count.load(std::sync::atomic::Ordering::Acquire)
    }
}

/// Fair RwLock baseline for AtomicCounter
pub struct RwLockCounter {
    count: RwLock<u64>,
}

impl RwLockCounter {
    pub fn new(initial: u64) -> Self {
        Self {
            count: RwLock::new(initial),
        }
    }

    pub fn increment(&self) {
        *self.count.write().unwrap() += 1;
    }

    pub fn get(&self) -> u64 {
        *self.count.read().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_counter() {
        let counter = AtomicCounter::new(0);
        counter.increment();
        counter.increment();
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn test_rwlock_counter() {
        let counter = RwLockCounter::new(0);
        counter.increment();
        counter.increment();
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn test_equivalent_behavior() {
        let atomic = AtomicCounter::new(42);
        let rwlock = RwLockCounter::new(42);

        atomic.increment();
        rwlock.increment();

        assert_eq!(atomic.get(), rwlock.get());
    }
}
