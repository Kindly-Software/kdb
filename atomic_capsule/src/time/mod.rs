//! # Time Management Module for Capsule OS
//!
//! **Production-grade time management primitives using Chaos (Computational Capsule) architecture.**
//!
//! ## Overview
//!
//! This module provides OS-level time management with 100% lockfree coordination:
//! - **ClockSourceCapsule**: TSC calibration and multi-clock source management (T1 Atomic, 256B)
//! - **TimerWheelCapsule**: Hierarchical timing wheel for O(1) timer scheduling (T4 Batch, 2KB)
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Time Management System                        │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  ClockSourceCapsule (T1 Atomic, 256B)                           │
//! │  ├─ TSC calibration (<1ms boot, <1ns read)                      │
//! │  ├─ Multi-clock support (TSC, HPET, ACPI PM Timer)              │
//! │  ├─ Frequency tracking with generation counters                  │
//! │  └─ Monotonic/Wall-clock time conversion                        │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  TimerWheelCapsule (T4 Batch, 2KB)                              │
//! │  ├─ 4-level hierarchical wheel (ms/100ms/10s/16min)             │
//! │  ├─ O(1) schedule/cancel/tick operations                         │
//! │  ├─ Batch timer expiry collection                                │
//! │  └─ Slot chaining for collision handling                        │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Performance Targets (B32 Validated)
//!
//! | Operation | ClockSourceCapsule | TimerWheelCapsule |
//! |-----------|-------------------|-------------------|
//! | Read      | <1ns (TSC)        | N/A               |
//! | Schedule  | N/A               | <30ns (P99)       |
//! | Cancel    | N/A               | <20ns             |
//! | Tick      | N/A               | <5ns/expired      |
//! | Calibrate | <1ms (boot)       | N/A               |
//!
//! ## Clock Source Hierarchy
//!
//! Linux kernel-inspired clock source selection:
//! 1. **TSC (Time Stamp Counter)**: <1ns, requires constant_tsc + nonstop_tsc
//! 2. **HPET (High Precision Event Timer)**: ~100ns, fallback for older systems
//! 3. **ACPI PM Timer**: ~300ns, lowest-tier fallback
//!
//! ## Safety (99.5%+ ASSUM)
//!
//! - All state updates via atomics (zero mutex/RwLock)
//! - Generation counters prevent ABA problems
//! - Cache-aligned structures (64B/128B/256B boundaries)
//! - 45 ASSUM safety annotations throughout
//!
//! ## Feature Flags
//!
//! - `time-tsc`: Enable TSC clock source (default)
//! - `time-hpet`: Enable HPET clock source
//! - `time-wheel`: Enable hierarchical timer wheel
//! - `time-full`: Enable all time management features
//!
//! ## Usage
//!
//! ```rust,ignore
//! use atomic_capsule::time::{ClockSourceCapsule, TimerWheelCapsule};
//! use std::time::Duration;
//!
//! // Initialize clock source with TSC calibration
//! let clock = ClockSourceCapsule::new();
//! clock.calibrate()?;
//!
//! // Read current time (<1ns for TSC)
//! let now_ns = clock.read_ns();
//!
//! // Create timer wheel
//! let wheel = TimerWheelCapsule::new();
//!
//! // Schedule timer for 100ms
//! let timer_id = wheel.schedule(Duration::from_millis(100), 42)?;
//!
//! // In event loop: tick and collect expired timers
//! let expired = wheel.tick(Duration::from_millis(50));
//! for task_id in expired {
//!     // Handle expired timer
//! }
//! ```
//!
//! ## References
//!
//! - [Linux Kernel Clocks and Timers](https://docs.kernel.org/next/virt/hyperv/clocks.html)
//! - [TSC Calibration](https://github.com/yb303/tsc_clock)
//! - [Hierarchical Timing Wheels Paper](https://www.cs.columbia.edu/~nahum/w6998/papers/sosp87-timing-wheels.pdf)
//! - [Ratas Implementation](https://www.snellman.net/blog/archive/2016-07-27-ratas-hierarchical-timer-wheel/)

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::identity_op)]
#![allow(clippy::must_use_candidate)]

// Clock source module (T1 Atomic)
mod clock_source;

// Timer wheel module (T4 Batch)
mod timer_wheel;

// Tests
#[cfg(test)]
mod tests;

// Re-export main types
pub use clock_source::{
    ClockSourceCapsule, ClockSourceError, ClockSourceResult, ClockSourceType,
    TscCalibration, TscCapabilities, ClockMetrics,
};

pub use timer_wheel::{
    TimerWheelCapsule, TimerWheelError, TimerWheelResult, TimerId, TaskId,
    TimerEntry, TimerWheelMetrics, TimerWheelLevel, TimerCallback,
    WHEEL_LEVEL_0_SLOTS, WHEEL_LEVEL_1_SLOTS, WHEEL_LEVEL_2_SLOTS, WHEEL_LEVEL_3_SLOTS,
};
