//! GPU Circuit Breaker
//!
//! GPU-specific wrapper around the `atomic_breaker` crate implementing
//! graceful degradation for graphics operations following "degrade, don't die" principle.
//!
//! Quality Levels:
//! - L0 (Closed): Normal operation (full quality, all features enabled)
//! - L1 (HalfOpen): Reduced quality (lower resolution, simplified effects)
//! - L2 (Open): Minimal quality (basic rendering only)
//! - L3 (ForcedOpen): Paused (GPU operations suspended, emergency only)

use atomic_breaker::{AtomicBreakerSWeMR, breaker::State, cause};

/// GPU Circuit Breaker using existing atomic_breaker implementation
///
/// Maps GPU-specific metrics (thermal, memory pressure, errors) to
/// circuit breaker quality levels for graceful degradation.
#[repr(C, align(64))]
pub struct GpuCircuitBreaker {
    /// Underlying atomic breaker (64-bit packed state)
    breaker: AtomicBreakerSWeMR,
    _pad: [u8; 56], // Prevent false sharing
}

impl Default for GpuCircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuCircuitBreaker {
    /// Create new circuit breaker in L0 (normal) state
    pub const fn new() -> Self {
        Self {
            breaker: AtomicBreakerSWeMR::new(State::Closed),
            _pad: [0; 56],
        }
    }

    /// Read current quality level (single atomic load)
    ///
    /// Maps breaker levels to GPU quality levels:
    /// - Level 0 = L0 (normal, full quality)
    /// - Level 1 = L1 (reduced quality)
    /// - Level 2 = L2 (minimal quality)
    /// - Level 3 = L3 (paused)
    pub fn level(&self) -> QualityLevel {
        let level = self.breaker.level();
        QualityLevel::from_level(level)
    }

    /// Read full breaker state
    pub fn read_state(&self) -> BreakerState {
        let word = self.breaker.load_relaxed();
        let guard = atomic_breaker::AtomicBreakerGuard::new(word);

        BreakerState {
            level: QualityLevel::from_state(guard.state()),
            state: guard.state(),
            error_count: guard.err(),
            cause_code: CauseCode::from_bits(guard.cause()),
        }
    }

    /// Check if GPU operations should proceed
    pub fn should_allow_command(&self) -> bool {
        matches!(
            self.breaker.state(),
            State::Closed | State::HalfOpen | State::Open
        )
    }

    /// Get quality multiplier for rendering (1.0 = full, 0.0 = paused)
    pub fn quality_multiplier(&self) -> f32 {
        match self.breaker.level() {
            0 => 1.0,  // L0: Full quality
            1 => 0.75, // L1: Reduced quality
            2 => 0.5,  // L2: Minimal quality
            _ => 0.0,  // L3: Paused
        }
    }

    /// Automatic degradation based on GPU metrics
    ///
    /// # Parameters
    /// - `thermal_mc`: Temperature in millicelsius
    /// - `errors_per_sec`: GPU error rate
    /// - `memory_used_pct`: Memory usage percentage (0-100)
    /// - `util`: GPU utilization (0-100)
    pub fn auto_adjust(&self, thermal_mc: u32, errors_per_sec: u16, memory_used_pct: u8, util: u8) {
        // Map GPU metrics to breaker levels
        let (new_level, cause) = if thermal_mc > 95_000 || errors_per_sec > 100 {
            (3, cause::THERM) // L3: Emergency pause
        } else if thermal_mc > 85_000 || errors_per_sec > 50 || memory_used_pct > 95 {
            (2, cause::CPU) // L2: Minimal quality (reuse CPU cause for error rate)
        } else if thermal_mc > 75_000 || errors_per_sec > 20 || memory_used_pct > 85 {
            (1, cause::LAT) // L1: Reduced quality
        } else {
            (0, 0) // L0: Normal operation
        };

        let current_level = self.breaker.level();

        // Only update if level changed (avoid unnecessary writes)
        if new_level != current_level {
            // Normalize metrics to Q8.8 fixed-point (0.0-1.0 range)
            let mu_q = ((util as u16) << 8) / 100;
            let sg_q = ((memory_used_pct as u16) << 8) / 100;

            self.breaker
                .update_metrics(errors_per_sec, mu_q, sg_q, cause, 0);
            self.breaker.set_level(new_level);

            tracing::info!(
                "GPU circuit breaker: level {} -> {} (thermal: {}°C, errors: {}/s, mem: {}%)",
                current_level,
                new_level,
                thermal_mc / 1000,
                errors_per_sec,
                memory_used_pct
            );
        }
    }

    /// Force breaker to specific level
    pub fn force_level(&self, level: QualityLevel) {
        let state = match level {
            QualityLevel::L0 => State::Closed,
            QualityLevel::L1 => State::HalfOpen,
            QualityLevel::L2 => State::Open,
            QualityLevel::L3 => State::ForcedOpen,
        };
        self.breaker.set_state_level(state, level.to_level());
    }

    /// Reset breaker to normal operation
    pub fn reset(&self) {
        self.breaker.set_state_level(State::Closed, 0);
        self.breaker.clear_error();
    }
}

/// Quality levels for GPU graceful degradation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityLevel {
    /// L0: Normal operation (full quality)
    L0,
    /// L1: Reduced quality (75% quality)
    L1,
    /// L2: Minimal quality (50% quality)
    L2,
    /// L3: Paused (operations suspended)
    L3,
}

impl QualityLevel {
    fn from_state(state: State) -> Self {
        match state {
            State::Closed => Self::L0,
            State::HalfOpen => Self::L1,
            State::Open => Self::L2,
            State::ForcedOpen => Self::L3,
        }
    }

    fn from_level(level: u8) -> Self {
        match level & 0x3 {
            0 => Self::L0,
            1 => Self::L1,
            2 => Self::L2,
            _ => Self::L3,
        }
    }

    fn to_level(self) -> u8 {
        match self {
            Self::L0 => 0,
            Self::L1 => 1,
            Self::L2 => 2,
            Self::L3 => 3,
        }
    }
}

/// Cause codes for GPU breaker activation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CauseCode {
    /// Normal operation
    Normal,
    /// Thermal throttling
    Thermal,
    /// High error rate
    ErrorRate,
    /// Memory pressure
    MemoryPressure,
    /// Latency spike
    Latency,
}

impl CauseCode {
    fn from_bits(bits: u8) -> Self {
        if bits & cause::THERM != 0 {
            Self::Thermal
        } else if bits & cause::CPU != 0 {
            Self::ErrorRate
        } else if bits & cause::LAT != 0 {
            Self::Latency
        } else {
            Self::Normal
        }
    }
}

/// Complete GPU breaker state snapshot
#[derive(Debug, Clone, Copy)]
pub struct BreakerState {
    /// Current quality level
    pub level: QualityLevel,
    /// Underlying breaker state
    pub state: State,
    /// Error count in rolling window
    pub error_count: u16,
    /// Cause of degradation
    pub cause_code: CauseCode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_initial_state() {
        let breaker = GpuCircuitBreaker::new();
        assert_eq!(breaker.level(), QualityLevel::L0);
        assert!(breaker.should_allow_command());
        assert_eq!(breaker.quality_multiplier(), 1.0);
    }

    #[test]
    fn test_quality_multipliers() {
        let breaker = GpuCircuitBreaker::new();
        assert_eq!(breaker.quality_multiplier(), 1.0);

        breaker.force_level(QualityLevel::L1);
        assert_eq!(breaker.quality_multiplier(), 0.75);

        breaker.force_level(QualityLevel::L2);
        assert_eq!(breaker.quality_multiplier(), 0.5);

        breaker.force_level(QualityLevel::L3);
        assert_eq!(breaker.quality_multiplier(), 0.0);
        assert!(!breaker.should_allow_command());
    }

    #[test]
    fn test_auto_adjust_thermal() {
        let breaker = GpuCircuitBreaker::new();

        // Normal thermal
        breaker.auto_adjust(70_000, 0, 50, 50);
        assert_eq!(breaker.level(), QualityLevel::L0);

        // High thermal -> L1
        breaker.auto_adjust(76_000, 0, 50, 50);
        assert_eq!(breaker.level(), QualityLevel::L1);

        // Very high thermal -> L2
        breaker.auto_adjust(86_000, 0, 50, 50);
        assert_eq!(breaker.level(), QualityLevel::L2);

        // Critical thermal -> L3
        breaker.auto_adjust(96_000, 0, 50, 50);
        assert_eq!(breaker.level(), QualityLevel::L3);
    }

    #[test]
    fn test_auto_adjust_errors() {
        let breaker = GpuCircuitBreaker::new();

        // High error rate -> L2
        breaker.auto_adjust(70_000, 60, 50, 50);
        assert_eq!(breaker.level(), QualityLevel::L2);

        let state = breaker.read_state();
        assert_eq!(state.cause_code, CauseCode::ErrorRate);
    }

    #[test]
    fn test_auto_adjust_memory() {
        let breaker = GpuCircuitBreaker::new();

        // High memory pressure -> L2
        breaker.auto_adjust(70_000, 0, 96, 50);
        assert_eq!(breaker.level(), QualityLevel::L2);
    }

    #[test]
    fn test_reset() {
        let breaker = GpuCircuitBreaker::new();

        breaker.force_level(QualityLevel::L3);
        assert_eq!(breaker.level(), QualityLevel::L3);

        breaker.reset();
        assert_eq!(breaker.level(), QualityLevel::L0);
        assert!(breaker.should_allow_command());
    }
}
