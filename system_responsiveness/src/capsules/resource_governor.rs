/// ResourceGovernorCapsule: T1 Atomic capsule for resource limit enforcement
/// Size: 64B (single cache line)
/// Performance: <20ns limit check, <50ns kill recording with circuit breaker

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, Ordering};

/// Resource limits and kill tracking packed into atomic state
/// Layout: cpu_limit(16) | mem_limit_mb(24) | active_kills(8) | total_kills(16)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct ResourceGovernorCapsule {
    /// Packed limits and counters
    /// Bits 0-15:   CPU limit percentage * 10 (max 655.3%)
    /// Bits 16-39:  Memory limit in MB (max 16TB)
    /// Bits 40-47:  Active kills in current window (max 255)
    /// Bits 48-63:  Total kills since start (max 65535, wraps)
    limits: AtomicU64,

    /// Circuit breaker state
    /// Bits 0-7:    Circuit state (0=closed, 1=half-open, 2=open)
    /// Bits 8-39:   Last trip timestamp (Unix seconds, 32 bits)
    /// Bits 40-47:  Trip threshold (kills/minute to trip)
    /// Bits 48-63:  Cooldown seconds (time before half-open)
    circuit_breaker: AtomicU64,

    _padding: [u8; 48],
}

// Bit masks for limits
const CPU_LIMIT_MASK: u64 = 0xFFFF;
const MEM_LIMIT_SHIFT: u32 = 16;
const MEM_LIMIT_MASK: u64 = 0xFFFFFF << MEM_LIMIT_SHIFT;
const ACTIVE_KILLS_SHIFT: u32 = 40;
const ACTIVE_KILLS_MASK: u64 = 0xFF << ACTIVE_KILLS_SHIFT;
const TOTAL_KILLS_SHIFT: u32 = 48;
const TOTAL_KILLS_MASK: u64 = 0xFFFF << TOTAL_KILLS_SHIFT;

// Circuit breaker masks
const CIRCUIT_STATE_MASK: u64 = 0xFF;
const CIRCUIT_TIMESTAMP_SHIFT: u32 = 8;
const CIRCUIT_TIMESTAMP_MASK: u64 = 0xFFFFFFFF << CIRCUIT_TIMESTAMP_SHIFT;
const CIRCUIT_THRESHOLD_SHIFT: u32 = 40;
const CIRCUIT_THRESHOLD_MASK: u64 = 0xFF << CIRCUIT_THRESHOLD_SHIFT;
const CIRCUIT_COOLDOWN_SHIFT: u32 = 48;
const CIRCUIT_COOLDOWN_MASK: u64 = 0xFFFF << CIRCUIT_COOLDOWN_SHIFT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed = 0,   // Normal operation
    HalfOpen = 1, // Testing after cooldown
    Open = 2,     // Kills disabled (too many recent kills)
}

impl ResourceGovernorCapsule {
    /// Create new resource governor
    /// cpu_limit_pct: Max CPU % before considered hung (e.g., 100.0)
    /// mem_limit_mb: Max memory in MB before considered hung
    /// kill_threshold: Kills per minute before circuit trips
    /// cooldown_sec: Seconds before circuit allows kills again
    pub fn new(
        cpu_limit_pct: f64,
        mem_limit_mb: u64,
        kill_threshold: u8,
        cooldown_sec: u16,
    ) -> Self {
        let limits = ((cpu_limit_pct * 10.0) as u64 & 0xFFFF)
            | ((mem_limit_mb & 0xFFFFFF) << MEM_LIMIT_SHIFT);

        let circuit = (CircuitState::Closed as u64)
            | ((kill_threshold as u64) << CIRCUIT_THRESHOLD_SHIFT)
            | ((cooldown_sec as u64) << CIRCUIT_COOLDOWN_SHIFT);

        Self {
            limits: AtomicU64::new(limits),
            circuit_breaker: AtomicU64::new(circuit),
            _padding: [0; 48],
        }
    }

    /// Check if kill is allowed (circuit breaker check)
    /// Target: <20ns
    #[inline(always)]
    pub fn can_kill(&self) -> bool {
        let circuit = self.circuit_breaker.load(Ordering::Acquire);  // CRITICAL-003 FIX: Acquire ordering
        let state = (circuit & CIRCUIT_STATE_MASK) as u8;

        match state {
            0 => true, // Closed: kills allowed
            1 => {
                // Half-open: check if cooldown expired
                let last_trip = (circuit & CIRCUIT_TIMESTAMP_MASK) >> CIRCUIT_TIMESTAMP_SHIFT;
                let cooldown = (circuit & CIRCUIT_COOLDOWN_MASK) >> CIRCUIT_COOLDOWN_SHIFT;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(std::time::Duration::ZERO)  // CRITICAL-009 FIX: Handle clock errors
                    .as_secs() as u32;

                now - last_trip as u32 > cooldown as u32
            }
            _ => false, // Open: kills disabled
        }
    }

    /// Record a kill (with circuit breaker check)
    /// Target: <50ns
    pub fn record_kill(&self) -> bool {
        // Check circuit breaker first
        if !self.can_kill() {
            return false;
        }

        // Increment kill counters atomically
        loop {
            let limits = self.limits.load(Ordering::Acquire);  // CRITICAL-004 FIX: Acquire ordering
            let active = ((limits & ACTIVE_KILLS_MASK) >> ACTIVE_KILLS_SHIFT) as u8;
            let total = ((limits & TOTAL_KILLS_MASK) >> TOTAL_KILLS_SHIFT) as u16;

            let new_active = (active + 1) as u64;
            let new_total = ((total.wrapping_add(1)) as u64) << TOTAL_KILLS_SHIFT;

            let new_limits = (limits & !(ACTIVE_KILLS_MASK | TOTAL_KILLS_MASK))
                | (new_active << ACTIVE_KILLS_SHIFT)
                | new_total;

            if self
                .limits
                .compare_exchange_weak(
                    limits,
                    new_limits,
                    Ordering::Release,
                    Ordering::Acquire,  // CRITICAL-004 FIX: Acquire on failure
                )
                .is_ok()
            {
                // Check if we should trip the circuit breaker
                let circuit = self.circuit_breaker.load(Ordering::Acquire);  // CRITICAL-005 FIX: Acquire ordering
                let threshold = ((circuit & CIRCUIT_THRESHOLD_MASK) >> CIRCUIT_THRESHOLD_SHIFT) as u8;

                if new_active > threshold as u64 {
                    self.trip_circuit_breaker();
                }

                return true;
            }
        }
    }

    /// Trip circuit breaker (too many kills)
    fn trip_circuit_breaker(&self) {
        loop {
            let circuit = self.circuit_breaker.load(Ordering::Acquire);  // CRITICAL-003 FIX: Acquire ordering
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)  // CRITICAL-009 FIX: Handle clock errors
                .as_secs() as u32;

            let new_circuit = (CircuitState::Open as u64)
                | ((now as u64) << CIRCUIT_TIMESTAMP_SHIFT)
                | (circuit & (CIRCUIT_THRESHOLD_MASK | CIRCUIT_COOLDOWN_MASK));

            if self
                .circuit_breaker
                .compare_exchange_weak(
                    circuit,
                    new_circuit,
                    Ordering::Release,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
        }
    }

    /// Reset active kill counter (called every minute)
    pub fn reset_active_kills(&self) {
        loop {
            let limits = self.limits.load(Ordering::Acquire);  // CRITICAL-004 FIX: Acquire ordering
            let new_limits = limits & !ACTIVE_KILLS_MASK;

            if self
                .limits
                .compare_exchange_weak(limits, new_limits, Ordering::Release, Ordering::Acquire)  // CRITICAL-004 FIX: Acquire on failure
                .is_ok()
            {
                // Also move circuit to half-open if it was open
                loop {
                    let circuit = self.circuit_breaker.load(Ordering::Acquire);  // CRITICAL-003 FIX: Acquire ordering
                    let state = (circuit & CIRCUIT_STATE_MASK) as u8;

                    if state == CircuitState::Open as u8 {
                        let new_circuit = (CircuitState::HalfOpen as u64)
                            | (circuit & !CIRCUIT_STATE_MASK);

                        if self
                            .circuit_breaker
                            .compare_exchange_weak(
                                circuit,
                                new_circuit,
                                Ordering::Release,
                                Ordering::Acquire,  // CRITICAL-003 FIX: Acquire on failure
                            )
                            .is_ok()
                        {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                break;
            }
        }
    }

    /// Get CPU limit
    pub fn cpu_limit_pct(&self) -> f64 {
        let limits = self.limits.load(Ordering::Relaxed);
        ((limits & CPU_LIMIT_MASK) as f64) / 10.0
    }

    /// Get total kills since start
    pub fn total_kills(&self) -> u16 {
        let limits = self.limits.load(Ordering::Acquire);  // CRITICAL-004 FIX: Acquire ordering
        ((limits & TOTAL_KILLS_MASK) >> TOTAL_KILLS_SHIFT) as u16
    }

    /// Get active kills in current window
    pub fn active_kills(&self) -> u8 {
        let limits = self.limits.load(Ordering::Acquire);  // CRITICAL-004 FIX: Acquire ordering
        ((limits & ACTIVE_KILLS_MASK) >> ACTIVE_KILLS_SHIFT) as u8
    }

    /// Get circuit breaker state
    pub fn circuit_state(&self) -> CircuitState {
        let circuit = self.circuit_breaker.load(Ordering::Acquire);  // CRITICAL-003 FIX: Acquire ordering
        let state = (circuit & CIRCUIT_STATE_MASK) as u8;

        match state {
            1 => CircuitState::HalfOpen,
            2 => CircuitState::Open,
            _ => CircuitState::Closed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(std::mem::align_of::<ResourceGovernorCapsule>(), 64);
        assert_eq!(std::mem::size_of::<ResourceGovernorCapsule>(), 64);
    }

    #[test]
    fn test_circuit_breaker() {
        let governor = ResourceGovernorCapsule::new(100.0, 4096, 5, 60);

        // Initially closed
        assert_eq!(governor.circuit_state(), CircuitState::Closed);
        assert!(governor.can_kill());

        // Record kills (should trip at 6th kill, threshold=5)
        for i in 1..=5 {
            assert!(governor.record_kill());
            assert_eq!(governor.active_kills(), i);
        }

        // 6th kill should trip circuit
        assert!(governor.record_kill());
        assert_eq!(governor.circuit_state(), CircuitState::Open);
        assert!(!governor.can_kill());

        // Reset should move to half-open
        governor.reset_active_kills();
        assert_eq!(governor.circuit_state(), CircuitState::HalfOpen);
        assert_eq!(governor.active_kills(), 0);
    }

    #[test]
    fn test_kill_counting() {
        let governor = ResourceGovernorCapsule::new(100.0, 4096, 100, 60);

        assert!(governor.record_kill());
        assert_eq!(governor.total_kills(), 1);
        assert_eq!(governor.active_kills(), 1);

        assert!(governor.record_kill());
        assert_eq!(governor.total_kills(), 2);
        assert_eq!(governor.active_kills(), 2);

        governor.reset_active_kills();
        assert_eq!(governor.total_kills(), 2); // Persists
        assert_eq!(governor.active_kills(), 0); // Reset
    }
}
