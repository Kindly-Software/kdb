//! # Time Module Tests - T28 5-Tier Testing
//!
//! Comprehensive tests for ClockSourceCapsule and TimerWheelCapsule.
//!
//! ## Test Tiers (T28 Framework)
//!
//! - **Unit Tests (Q1-Q7)**: Basic functionality
//! - **Property Tests (Q8-Q14)**: Invariants and edge cases
//! - **Integration Tests (Q15-Q21)**: Cross-module interaction
//! - **Production Tests (Q22-Q28)**: Real-world scenarios
//! - **Determinism Tests (Q29-Q35)**: Reproducibility
//!
//! ## Coverage Targets
//!
//! - 22 tests total
//! - >90% code coverage
//! - All ASSUM annotations verified

use super::*;
use core::time::Duration;

#[cfg(feature = "std")]
use std::sync::Arc;
#[cfg(feature = "std")]
use std::thread;

// ============================================================================
// Unit Tests - ClockSourceCapsule (Q1-Q7)
// ============================================================================

#[test]
fn test_clock_source_new() {
    let clock = ClockSourceCapsule::new();

    assert!(!clock.is_initialized());
    assert!(!clock.is_calibrated());
    assert!(!clock.has_error());
    assert_eq!(clock.clock_source(), ClockSourceType::None);
}

#[test]
fn test_clock_source_initialize() {
    let clock = ClockSourceCapsule::new();
    let result = clock.initialize();

    assert!(result.is_ok());
    assert!(clock.is_initialized());

    // Should select best clock source
    let source = clock.clock_source();
    assert_ne!(source, ClockSourceType::None);
}

#[test]
fn test_clock_source_calibrate() {
    let clock = ClockSourceCapsule::new();
    clock.initialize().unwrap();

    let calibration = clock.calibrate().unwrap();

    assert!(clock.is_calibrated());
    // Validate frequency is in reasonable range (100MHz - 10GHz)
    assert!(calibration.frequency_hz >= 100_000_000);
    assert!(calibration.frequency_hz <= 10_000_000_000);
}

#[test]
fn test_clock_source_read_ns() {
    let clock = ClockSourceCapsule::new();
    clock.initialize().unwrap();
    clock.calibrate().unwrap();

    let t1 = clock.read_ns();
    // Small delay
    for _ in 0..1000 {
        core::hint::spin_loop();
    }
    let t2 = clock.read_ns();

    // Time should advance
    assert!(t2 >= t1);
}

#[test]
fn test_clock_source_read_tsc() {
    let clock = ClockSourceCapsule::new();

    let tsc1 = clock.read_tsc();
    let tsc2 = clock.read_tsc();

    // TSC should be monotonically increasing
    assert!(tsc2 >= tsc1);
}

#[test]
fn test_clock_source_wall_clock() {
    let clock = ClockSourceCapsule::new();
    clock.initialize().unwrap();

    // Set wall clock to arbitrary Unix epoch time
    let epoch_ns = 1700000000_000_000_000u64; // ~Nov 2023
    clock.set_wall_clock(epoch_ns);

    let wall = clock.read_wall_clock_ns();
    assert!(wall >= epoch_ns);
}

#[test]
fn test_clock_source_generation() {
    let clock = ClockSourceCapsule::new();

    let gen1 = clock.generation();
    clock.initialize().unwrap();
    let gen2 = clock.generation();
    clock.calibrate().unwrap();
    let gen3 = clock.generation();

    // Generation should increase with state changes
    assert!(gen2 > gen1);
    assert!(gen3 > gen2);
}

#[test]
fn test_clock_source_metrics() {
    let clock = ClockSourceCapsule::new();
    clock.initialize().unwrap();
    clock.calibrate().unwrap();

    // Perform some reads
    for _ in 0..10 {
        let _ = clock.read_ns();
    }

    let metrics = clock.metrics();
    assert!(metrics.total_reads >= 10);
    assert!(metrics.calibration_count >= 1);
}

// ============================================================================
// Unit Tests - TimerWheelCapsule (Q1-Q7)
// ============================================================================

#[test]
fn test_timer_wheel_new() {
    let wheel = TimerWheelCapsule::new();

    assert_eq!(wheel.active_count(), 0);
    assert_eq!(wheel.current_time_ns(), 0);
}

#[test]
fn test_timer_wheel_schedule() {
    let wheel = TimerWheelCapsule::new();

    let timer_id = wheel.schedule(Duration::from_millis(100), 42).unwrap();

    assert!(timer_id.raw() > 0);
    assert_eq!(wheel.active_count(), 1);
    assert!(wheel.is_pending(timer_id));
}

#[test]
fn test_timer_wheel_schedule_multiple() {
    let wheel = TimerWheelCapsule::new();

    let t1 = wheel.schedule(Duration::from_millis(10), 1).unwrap();
    let t2 = wheel.schedule(Duration::from_millis(50), 2).unwrap();
    let t3 = wheel.schedule(Duration::from_millis(100), 3).unwrap();

    assert!(t1.raw() != t2.raw());
    assert!(t2.raw() != t3.raw());
    assert_eq!(wheel.active_count(), 3);
}

#[test]
fn test_timer_wheel_cancel() {
    let wheel = TimerWheelCapsule::new();

    let timer_id = wheel.schedule(Duration::from_millis(100), 42).unwrap();
    assert!(wheel.is_pending(timer_id));

    wheel.cancel(timer_id).unwrap();

    assert!(!wheel.is_pending(timer_id));
    assert_eq!(wheel.active_count(), 0);
}

#[test]
fn test_timer_wheel_tick_expiry() {
    let wheel = TimerWheelCapsule::new();

    // Schedule timer for 10ms
    let _timer_id = wheel.schedule(Duration::from_millis(10), 42).unwrap();

    // Tick forward 5ms - should not expire
    let expired = wheel.tick(Duration::from_millis(5));
    #[cfg(feature = "std")]
    assert!(expired.is_empty());
    #[cfg(not(feature = "std"))]
    assert_eq!(expired, 0);

    // Tick forward another 10ms - should expire
    let expired = wheel.tick(Duration::from_millis(10));
    #[cfg(feature = "std")]
    {
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0], 42);
    }
    #[cfg(not(feature = "std"))]
    assert_eq!(expired, 1);
}

#[test]
fn test_timer_wheel_metrics() {
    let wheel = TimerWheelCapsule::new();

    wheel.schedule(Duration::from_millis(10), 1).unwrap();
    wheel.schedule(Duration::from_millis(20), 2).unwrap();

    let timer3 = wheel.schedule(Duration::from_millis(30), 3).unwrap();
    wheel.cancel(timer3).unwrap();

    wheel.tick(Duration::from_millis(25));

    let metrics = wheel.metrics();
    assert_eq!(metrics.scheduled, 3);
    assert_eq!(metrics.cancelled, 1);
    assert!(metrics.fired >= 1);
}

// ============================================================================
// Property Tests (Q8-Q14)
// ============================================================================

#[test]
fn test_timer_id_roundtrip() {
    // Test timer ID encoding/decoding via schedule
    let wheel = TimerWheelCapsule::new();
    let timer_id = wheel.schedule(Duration::from_millis(100), 42).unwrap();

    // Timer ID should have valid structure
    assert!(timer_id.raw() > 0);
    assert!(timer_id.entry_index() < TimerWheelCapsule::POOL_SIZE as u32);
    assert!(timer_id.sequence() > 0);
}

#[test]
fn test_clock_source_type_ordering() {
    // TSC should be highest priority (lowest value)
    assert!(ClockSourceType::Tsc < ClockSourceType::Hpet);
    assert!(ClockSourceType::Hpet < ClockSourceType::AcpiPmTimer);
    assert!(ClockSourceType::AcpiPmTimer < ClockSourceType::Jiffies);
}

#[test]
fn test_tsc_capabilities_bitfield() {
    let caps = TscCapabilities::new(
        TscCapabilities::CONSTANT_TSC
            | TscCapabilities::NONSTOP_TSC
            | TscCapabilities::RDTSCP,
    );

    assert!(caps.has(TscCapabilities::CONSTANT_TSC));
    assert!(caps.has(TscCapabilities::NONSTOP_TSC));
    assert!(caps.has(TscCapabilities::RDTSCP));
    assert!(!caps.has(TscCapabilities::TSC_DEADLINE));
    assert!(caps.is_reliable());
}

#[test]
fn test_timer_entry_state_encoding() {
    let mut entry = TimerEntry::new(42, 1000000, 1);

    assert!(entry.is_pending());
    assert!(!entry.is_free());

    entry.set_position(2, 50);
    assert_eq!(entry.level(), 2);
    assert_eq!(entry.slot(), 50);

    entry.mark_fired();
    assert!(!entry.is_pending());

    entry.mark_free();
    assert!(entry.is_free());
}

// ============================================================================
// Integration Tests (Q15-Q21)
// ============================================================================

#[test]
fn test_clock_and_timer_integration() {
    let clock = ClockSourceCapsule::new();
    clock.initialize().unwrap();
    clock.calibrate().unwrap();

    let wheel = TimerWheelCapsule::new();

    // Sync timer wheel with clock
    let now = clock.read_ns();
    wheel.set_current_time(now);

    // Schedule timer
    let _timer = wheel.schedule(Duration::from_millis(100), 1).unwrap();

    // Simulate elapsed time
    let elapsed = Duration::from_millis(150);
    let expired = wheel.tick(elapsed);

    #[cfg(feature = "std")]
    assert!(!expired.is_empty());
    #[cfg(not(feature = "std"))]
    assert!(expired > 0);
}

#[cfg(feature = "std")]
#[test]
fn test_concurrent_clock_reads() {
    let clock = Arc::new(ClockSourceCapsule::new());
    clock.initialize().unwrap();
    clock.calibrate().unwrap();

    let mut handles = vec![];

    // Spawn multiple reader threads
    for _ in 0..4 {
        let clock_clone = Arc::clone(&clock);
        handles.push(thread::spawn(move || {
            let mut last = 0u64;
            for _ in 0..1000 {
                let now = clock_clone.read_ns();
                assert!(now >= last, "Time went backwards!");
                last = now;
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[cfg(feature = "std")]
#[test]
fn test_concurrent_timer_scheduling() {
    let wheel = Arc::new(TimerWheelCapsule::new());
    let mut handles = vec![];

    // Spawn multiple scheduling threads
    for i in 0..4 {
        let wheel_clone = Arc::clone(&wheel);
        handles.push(thread::spawn(move || {
            for j in 0..8 {
                let delay = Duration::from_millis((i * 10 + j) as u64);
                let task_id = (i * 8 + j) as u64 + 1;
                let _ = wheel_clone.schedule(delay, task_id);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have scheduled most timers (some may have been rejected due to pool size)
    assert!(wheel.active_count() > 0);
}

// ============================================================================
// Production Tests (Q22-Q28)
// ============================================================================

#[test]
fn test_timer_wheel_stress() {
    let wheel = TimerWheelCapsule::new();

    // Schedule many timers
    let mut timer_ids = Vec::new();
    for i in 0..TimerWheelCapsule::POOL_SIZE {
        if let Ok(timer) = wheel.schedule(Duration::from_millis((i * 10) as u64), i as u64 + 1) {
            timer_ids.push(timer);
        }
    }

    // Tick through all of them
    let mut total_expired = 0;
    for _ in 0..100 {
        let expired = wheel.tick(Duration::from_millis(100));
        #[cfg(feature = "std")]
        {
            total_expired += expired.len();
        }
        #[cfg(not(feature = "std"))]
        {
            total_expired += expired as usize;
        }
    }

    assert!(total_expired > 0);
}

#[test]
fn test_clock_calibration_accuracy() {
    let clock = ClockSourceCapsule::new();
    clock.initialize().unwrap();

    let calibration = clock.calibrate().unwrap();

    // Verify frequency is in reasonable range (100MHz - 10GHz)
    assert!(
        calibration.frequency_hz >= 100_000_000,
        "Frequency too low: {}",
        calibration.frequency_hz
    );
    assert!(
        calibration.frequency_hz <= 10_000_000_000,
        "Frequency too high: {}",
        calibration.frequency_hz
    );

    // Accuracy should be reported
    assert!(calibration.accuracy_ppm > 0);
}

// ============================================================================
// Determinism Tests (Q29-Q35)
// ============================================================================

#[test]
fn test_timer_scheduling_deterministic() {
    // Test that scheduling same delay produces consistent behavior
    let delays_ms = [1, 10, 50, 100, 500, 1000];

    for delay_ms in delays_ms {
        let wheel1 = TimerWheelCapsule::new();
        let wheel2 = TimerWheelCapsule::new();

        let delay = Duration::from_millis(delay_ms);

        // Schedule on both wheels
        let t1 = wheel1.schedule(delay, 1).unwrap();
        let t2 = wheel2.schedule(delay, 1).unwrap();

        // Both should be pending
        assert!(wheel1.is_pending(t1));
        assert!(wheel2.is_pending(t2));

        // After same tick, both should have same expiry behavior
        let expired1 = wheel1.tick(Duration::from_millis(delay_ms + 10));
        let expired2 = wheel2.tick(Duration::from_millis(delay_ms + 10));

        #[cfg(feature = "std")]
        {
            assert_eq!(expired1.len(), expired2.len(), "Expiry count not deterministic for {}ms", delay_ms);
        }
    }
}

#[test]
fn test_clock_source_type_conversion_deterministic() {
    for value in 0..=255u64 {
        let type1 = ClockSourceType::from_raw(value);
        let type2 = ClockSourceType::from_raw(value);
        assert_eq!(type1, type2);

        if let Some(t) = type1 {
            assert_eq!(t.to_raw(), value);
        }
    }
}
