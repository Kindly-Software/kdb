/// ProcessStateCapsule: T1 Atomic capsule for process state tracking
/// Size: 128B (dual cache line)
/// Performance: <50ns hung detection, <100ns state update

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, Ordering};

/// Process state packed into 64 bits for single-read decisions
/// Layout: pid(20) | cpu_pct(12) | runtime_sec(20) | generation(8) | flags(4)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct ProcessStateCapsule {
    /// Packed atomic state for single-read decisions
    /// Bits 0-19:   PID (max 1,048,576)
    /// Bits 20-31:  CPU percentage * 10 (max 409.5%, handles multi-core)
    /// Bits 32-51:  Runtime in seconds (max 1,048,576 seconds = 12 days)
    /// Bits 52-59:  Generation counter (wraps at 256, TOCTOU prevention)
    /// Bits 60-63:  Flags (is_test, is_bench, is_cargo, whitelisted)
    state: AtomicU64,

    /// Last update timestamp (Unix epoch seconds)
    last_updated: AtomicU64,

    /// Padding to 128B (dual cache line)
    _padding: [u8; 112],
}

// Constants for bit packing
const PID_MASK: u64 = 0xFFFFF;              // 20 bits
const CPU_PCT_SHIFT: u32 = 20;
const CPU_PCT_MASK: u64 = 0xFFF << CPU_PCT_SHIFT;  // 12 bits
const RUNTIME_SHIFT: u32 = 32;
const RUNTIME_MASK: u64 = 0xFFFFF << RUNTIME_SHIFT;  // 20 bits
const GENERATION_SHIFT: u32 = 52;
const GENERATION_MASK: u64 = 0xFF << GENERATION_SHIFT;  // 8 bits
const FLAGS_SHIFT: u32 = 60;
const FLAGS_MASK: u64 = 0xF << FLAGS_SHIFT;  // 4 bits

// Flag bits
const FLAG_IS_TEST: u64 = 1 << FLAGS_SHIFT;
const FLAG_IS_BENCH: u64 = 2 << FLAGS_SHIFT;
const FLAG_IS_CARGO: u64 = 4 << FLAGS_SHIFT;
const FLAG_WHITELISTED: u64 = 8 << FLAGS_SHIFT;

impl ProcessStateCapsule {
    /// Create new process state capsule
    pub fn new(pid: u32) -> Self {
        Self {
            state: AtomicU64::new(pid as u64),
            last_updated: AtomicU64::new(0),
            _padding: [0; 112],
        }
    }

    /// Update process state atomically
    /// Target: <100ns
    pub fn update(
        &self,
        pid: u32,
        cpu_pct: f64,
        runtime_sec: u64,
        is_test: bool,
        is_bench: bool,
        is_cargo: bool,
    ) {
        // CRITICAL-012 FIX: Handle PIDs exceeding 20-bit limit (1,048,575) gracefully
        // This can occur on systems with large PID spaces
        if pid > 0xFFFFF {
            // Log the overflow but don't panic - skip this update
            // Caller (streaming_monitor) should filter these out first
            return;
        }

        // Pack state into 64 bits
        let mut packed = (pid as u64) & PID_MASK;

        // CPU percentage * 10 (supports >100% for multi-core)
        let cpu_scaled = ((cpu_pct * 10.0).min(4095.0) as u64) << CPU_PCT_SHIFT;
        packed |= cpu_scaled & CPU_PCT_MASK;

        // Runtime in seconds
        let runtime = (runtime_sec.min(0xFFFFF)) << RUNTIME_SHIFT;
        packed |= runtime & RUNTIME_MASK;

        // Set flags
        if is_test {
            packed |= FLAG_IS_TEST;
        }
        if is_bench {
            packed |= FLAG_IS_BENCH;
        }
        if is_cargo {
            packed |= FLAG_IS_CARGO;
        }

        // CRITICAL-001 FIX: Atomically increment generation counter (CAS loop)
        // Prevents race condition where PID could be reused during update
        loop {
            let old_state = self.state.load(Ordering::Acquire);  // CRITICAL-002 FIX: Acquire ordering
            let old_gen = (old_state & GENERATION_MASK) >> GENERATION_SHIFT;
            let new_gen = ((old_gen + 1) & 0xFF) << GENERATION_SHIFT;

            // Preserve whitelist flag from old state (set independently via set_whitelisted)
            let old_whitelist = old_state & FLAG_WHITELISTED;
            let new_state = (packed & !GENERATION_MASK & !FLAG_WHITELISTED) | new_gen | old_whitelist;

            match self.state.compare_exchange_weak(
                old_state,
                new_state,
                Ordering::Release,
                Ordering::Acquire,  // CRITICAL-002 FIX: Acquire on failure
            ) {
                Ok(_) => break,
                Err(_) => continue, // Retry on contention
            }
        }

        // CRITICAL-009 FIX: Use unwrap_or to handle clock errors
        self.last_updated.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)
                .as_secs(),
            Ordering::Relaxed,
        );
    }

    /// Check if process is hung (single atomic load)
    /// Target: <50ns
    #[inline(always)]
    pub fn is_hung(&self, cpu_threshold_pct: f64, runtime_threshold_sec: u64) -> bool {
        let state = self.state.load(Ordering::Relaxed);

        // Extract CPU percentage
        let cpu_scaled = (state & CPU_PCT_MASK) >> CPU_PCT_SHIFT;
        let cpu_pct = (cpu_scaled as f64) / 10.0;

        // Extract runtime
        let runtime = (state & RUNTIME_MASK) >> RUNTIME_SHIFT;

        // Extract flags
        let is_whitelisted = (state & FLAG_WHITELISTED) != 0;

        // Hung if: high CPU + long runtime + not whitelisted
        !is_whitelisted && cpu_pct > cpu_threshold_pct && runtime > runtime_threshold_sec
    }

    /// Get PID (for kill action)
    #[inline(always)]
    pub fn pid(&self) -> u32 {
        let state = self.state.load(Ordering::Relaxed);
        (state & PID_MASK) as u32
    }

    /// Get generation counter (TOCTOU prevention)
    #[inline(always)]
    pub fn generation(&self) -> u8 {
        let state = self.state.load(Ordering::Relaxed);
        ((state & GENERATION_MASK) >> GENERATION_SHIFT) as u8
    }

    /// Check if process is a test/benchmark (conservative kill criteria)
    #[inline(always)]
    pub fn is_test_or_bench(&self) -> bool {
        let state = self.state.load(Ordering::Relaxed);
        (state & (FLAG_IS_TEST | FLAG_IS_BENCH)) != 0
    }

    /// Whitelist process (prevent killing)
    pub fn set_whitelisted(&self, whitelisted: bool) {
        loop {
            let state = self.state.load(Ordering::Relaxed);
            let new_state = if whitelisted {
                state | FLAG_WHITELISTED
            } else {
                state & !FLAG_WHITELISTED
            };

            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Get CPU percentage (for logging)
    pub fn cpu_pct(&self) -> f64 {
        let state = self.state.load(Ordering::Relaxed);
        let cpu_scaled = (state & CPU_PCT_MASK) >> CPU_PCT_SHIFT;
        (cpu_scaled as f64) / 10.0
    }

    /// Get runtime in seconds (for logging)
    pub fn runtime_sec(&self) -> u64 {
        let state = self.state.load(Ordering::Relaxed);
        (state & RUNTIME_MASK) >> RUNTIME_SHIFT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(std::mem::align_of::<ProcessStateCapsule>(), 128);
        assert_eq!(std::mem::size_of::<ProcessStateCapsule>(), 128);
    }

    #[test]
    fn test_process_state_update() {
        let capsule = ProcessStateCapsule::new(1234);
        capsule.update(1234, 150.5, 300, true, false, false);

        assert_eq!(capsule.pid(), 1234);
        assert!((capsule.cpu_pct() - 150.5).abs() < 0.1);
        assert_eq!(capsule.runtime_sec(), 300);
        assert!(capsule.is_test_or_bench());
    }

    #[test]
    fn test_hung_detection() {
        let capsule = ProcessStateCapsule::new(5678);

        // Not hung: low CPU
        capsule.update(5678, 50.0, 400, false, false, false);
        assert!(!capsule.is_hung(100.0, 300));

        // Not hung: short runtime
        capsule.update(5678, 150.0, 100, false, false, false);
        assert!(!capsule.is_hung(100.0, 300));

        // Hung: high CPU + long runtime
        capsule.update(5678, 200.0, 400, false, false, false);
        assert!(capsule.is_hung(100.0, 300));

        // Not hung: whitelisted
        capsule.set_whitelisted(true);
        assert!(!capsule.is_hung(100.0, 300));
    }

    #[test]
    fn test_generation_counter() {
        let capsule = ProcessStateCapsule::new(9999);
        let gen1 = capsule.generation();

        capsule.update(9999, 100.0, 200, false, false, false);
        let gen2 = capsule.generation();

        assert_eq!(gen2, (gen1 + 1) & 0xFF);
    }
}
