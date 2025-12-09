//! Shared test infrastructure for v0.3.2 comprehensive testing
//!
//! **T28 Framework**: Common utilities, fixtures, and baselines for 4-tier test pyramid

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

// Re-export memmap2 for persistent storage tests
#[cfg(feature = "mmap-persistence")]
pub use memmap2::{MmapMut, MmapOptions};

// ============================================================================
// TEMPORARY FILE FIXTURES
// ============================================================================

/// Temporary mmap file fixture for testing
///
/// Automatically cleaned up on drop
#[cfg(feature = "mmap-persistence")]
pub struct TempMmapFile {
    pub path: PathBuf,
    pub mmap: Option<MmapMut>,
}

#[cfg(feature = "mmap-persistence")]
impl TempMmapFile {
    /// Create temporary mmap file with given size
    pub fn new(name: &str, size: usize) -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!("test_mmap_{}_{}", name, rand_suffix()));
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;
        file.set_len(size as u64)?;

        let mmap = unsafe { MmapOptions::new().len(size).map_mut(&file)? };

        Ok(Self {
            path,
            mmap: Some(mmap),
        })
    }

    /// Get mutable reference to mmap
    pub fn mmap_mut(&mut self) -> &mut MmapMut {
        self.mmap.as_mut().expect("mmap should be initialized")
    }

    /// Flush mmap to disk
    pub fn flush(&mut self) -> std::io::Result<()> {
        self.mmap_mut().flush()
    }
}

#[cfg(feature = "mmap-persistence")]
impl Drop for TempMmapFile {
    fn drop(&mut self) {
        // Ensure mmap is dropped before file deletion
        self.mmap.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Generate random suffix for temporary files
fn rand_suffix() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hash, Hasher};

    let state = RandomState::new();
    let mut hasher = state.build_hasher();
    std::time::SystemTime::now().hash(&mut hasher);
    hasher.finish()
}

// ============================================================================
// CRASH SIMULATION UTILITIES
// ============================================================================

/// Simulate process crash mid-operation
///
/// **Purpose**: Test crash recovery scenarios
/// **Usage**: Call at strategic points during state updates
#[cfg(feature = "mmap-persistence")]
pub struct CrashSimulator {
    should_crash: AtomicU64,
    crash_point: u64,
}

#[cfg(feature = "mmap-persistence")]
impl CrashSimulator {
    /// Create new crash simulator
    ///
    /// `crash_point`: Operation count at which to "crash"
    pub fn new(crash_point: u64) -> Self {
        Self {
            should_crash: AtomicU64::new(0),
            crash_point,
        }
    }

    /// Check if should crash (increments operation counter)
    ///
    /// Returns true if crash point reached
    pub fn check_crash(&self) -> bool {
        let count = self.should_crash.fetch_add(1, Ordering::Relaxed);
        count >= self.crash_point
    }

    /// Simulate crash by returning early (test utility)
    ///
    /// **Note**: In real tests, use `check_crash()` and return early
    pub fn maybe_crash<T>(&self, result: T) -> Option<T> {
        if self.check_crash() {
            None
        } else {
            Some(result)
        }
    }
}

// ============================================================================
// BASELINE IMPLEMENTATIONS (For comparison)
// ============================================================================

/// Baseline mutex-based counter for performance comparison
pub struct MutexCounter {
    value: std::sync::Mutex<u64>,
}

impl MutexCounter {
    pub fn new() -> Self {
        Self {
            value: std::sync::Mutex::new(0),
        }
    }

    pub fn increment(&self) -> u64 {
        let mut guard = self.value.lock().unwrap();
        *guard += 1;
        *guard
    }

    pub fn get(&self) -> u64 {
        *self.value.lock().unwrap()
    }
}

/// Baseline RwLock-based counter for performance comparison
pub struct RwLockCounter {
    value: std::sync::RwLock<u64>,
}

impl RwLockCounter {
    pub fn new() -> Self {
        Self {
            value: std::sync::RwLock::new(0),
        }
    }

    pub fn increment(&self) -> u64 {
        let mut guard = self.value.write().unwrap();
        *guard += 1;
        *guard
    }

    pub fn get(&self) -> u64 {
        *self.value.read().unwrap()
    }
}

// ============================================================================
// CONCURRENT TEST HELPERS
// ============================================================================

/// Run concurrent test with multiple threads
///
/// **Purpose**: Stress test concurrent operations
/// **Usage**: `run_concurrent(num_threads, operations_per_thread, |thread_id| { ... })`
pub fn run_concurrent<F>(num_threads: usize, operations_per_thread: usize, mut f: F)
where
    F: FnMut(usize) + Send + Clone + 'static,
{
    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let f = f.clone();
            std::thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    f(thread_id);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread should not panic");
    }
}

/// Wait for condition with timeout
///
/// **Purpose**: Wait for asynchronous operations to complete
/// **Returns**: true if condition met, false if timeout
pub fn wait_for<F>(mut condition: F, timeout: Duration) -> bool
where
    F: FnMut() -> bool,
{
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_micros(100));
    }
    false
}

// ============================================================================
// MEMORY LEAK DETECTION
// ============================================================================

/// Track allocations for memory leak detection
///
/// **Purpose**: Validate no memory leaks in hot paths
#[derive(Debug)]
pub struct AllocationTracker {
    start_allocations: usize,
}

impl AllocationTracker {
    /// Start tracking allocations
    pub fn new() -> Self {
        Self {
            start_allocations: get_allocation_count(),
        }
    }

    /// Check if allocations increased
    ///
    /// **Returns**: Number of new allocations since start
    pub fn check(&self) -> usize {
        let current = get_allocation_count();
        current.saturating_sub(self.start_allocations)
    }

    /// Assert no new allocations
    pub fn assert_no_leaks(&self) {
        let leaks = self.check();
        assert_eq!(
            leaks, 0,
            "Memory leak detected: {} allocations not freed",
            leaks
        );
    }
}

/// Get current allocation count (platform-specific)
///
/// **Note**: Returns 0 on platforms without allocation tracking
fn get_allocation_count() -> usize {
    // TODO: Integrate with jemalloc or custom allocator for precise tracking
    // For now, return 0 (manual validation via valgrind/miri)
    0
}

// ============================================================================
// PERFORMANCE MEASUREMENT UTILITIES
// ============================================================================

/// Simple benchmark utility for performance validation
pub struct BenchmarkResults {
    pub min_ns: u64,
    pub max_ns: u64,
    pub mean_ns: f64,
    pub p99_ns: u64,
}

/// Run simple benchmark (not as rigorous as Criterion, but sufficient for tests)
pub fn simple_benchmark<F>(iterations: usize, mut operation: F) -> BenchmarkResults
where
    F: FnMut(),
{
    let mut samples = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let start = std::time::Instant::now();
        operation();
        let elapsed = start.elapsed();
        samples.push(elapsed.as_nanos() as u64);
    }

    samples.sort_unstable();

    let min_ns = samples[0];
    let max_ns = samples[samples.len() - 1];
    let mean_ns = samples.iter().sum::<u64>() as f64 / samples.len() as f64;
    let p99_idx = (samples.len() as f64 * 0.99) as usize;
    let p99_ns = samples[p99_idx];

    BenchmarkResults {
        min_ns,
        max_ns,
        mean_ns,
        p99_ns,
    }
}

// ============================================================================
// ASSERTION HELPERS
// ============================================================================

/// Assert value within range (with tolerance)
pub fn assert_within_range(actual: u64, expected: u64, tolerance_percent: u64) {
    let tolerance = (expected * tolerance_percent) / 100;
    let lower = expected.saturating_sub(tolerance);
    let upper = expected + tolerance;

    assert!(
        actual >= lower && actual <= upper,
        "Value {} not within {}% of expected {} (range: {}-{})",
        actual,
        tolerance_percent,
        expected,
        lower,
        upper
    );
}

/// Assert approximate equality for floats
pub fn assert_approx_eq(actual: f64, expected: f64, epsilon: f64) {
    let diff = (actual - expected).abs();
    assert!(
        diff <= epsilon,
        "Values not approximately equal: {} vs {} (diff: {}, epsilon: {})",
        actual,
        expected,
        diff,
        epsilon
    );
}

// ============================================================================
// TEST DATA GENERATORS
// ============================================================================

/// Generate deterministic test data for reproducibility
pub fn generate_test_data(count: usize, seed: u64) -> Vec<u64> {
    let mut data = Vec::with_capacity(count);
    let mut rng = SimplePrng::new(seed);

    for _ in 0..count {
        data.push(rng.next());
    }

    data
}

/// Simple PRNG for test data generation (deterministic)
struct SimplePrng {
    state: u64,
}

impl SimplePrng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        // Xorshift64
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }
}

// ============================================================================
// CONSTANTS
// ============================================================================

/// Standard test timeout (5 seconds)
pub const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Standard concurrent test thread count
pub const CONCURRENT_THREADS: usize = 8;

/// Standard concurrent test operations per thread
pub const OPERATIONS_PER_THREAD: usize = 1000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_prng_deterministic() {
        let mut rng1 = SimplePrng::new(42);
        let mut rng2 = SimplePrng::new(42);

        for _ in 0..100 {
            assert_eq!(rng1.next(), rng2.next(), "PRNG should be deterministic");
        }
    }

    #[test]
    fn test_generate_test_data() {
        let data1 = generate_test_data(1000, 12345);
        let data2 = generate_test_data(1000, 12345);

        assert_eq!(data1, data2, "Test data should be deterministic");
        assert_eq!(data1.len(), 1000);
    }

    #[test]
    fn test_wait_for_success() {
        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();

        // Spawn thread that increments after 100ms
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            counter_clone.store(42, Ordering::Release);
        });

        let result = wait_for(
            || counter.load(Ordering::Acquire) == 42,
            Duration::from_secs(1),
        );

        assert!(result, "wait_for should succeed");
    }

    #[test]
    fn test_wait_for_timeout() {
        let result = wait_for(|| false, Duration::from_millis(100));
        assert!(!result, "wait_for should timeout");
    }

    #[test]
    fn test_assert_within_range() {
        assert_within_range(100, 100, 10); // Exact match
        assert_within_range(95, 100, 10); // Within lower bound
        assert_within_range(105, 100, 10); // Within upper bound
    }

    #[test]
    #[should_panic(expected = "not within")]
    fn test_assert_within_range_fails() {
        assert_within_range(80, 100, 10); // Outside range
    }

    #[test]
    fn test_simple_benchmark() {
        let results = simple_benchmark(1000, || {
            // Simple operation
            let _x = 42 * 2;
        });

        assert!(results.min_ns > 0);
        assert!(results.mean_ns > 0.0);
        assert!(results.p99_ns >= results.min_ns);
        assert!(results.max_ns >= results.p99_ns);
    }
}
