//! Tests for TimerWheelCapsule
//!
//! Feature-gated test module for timer wheel functionality

#![cfg(all(test, feature = "queue-unbounded"))]

use super::timer_wheel::*;
use core::time::Duration;

#[test]
fn test_new_wheel() {
    let wheel = TimerWheelCapsule::new();
    assert_eq!(wheel.current_time(), 0);
    let metrics = wheel.metrics();
    assert_eq!(metrics.scheduled, 0);
    assert_eq!(metrics.fired, 0);
}

#[test]
fn test_schedule_and_fire() {
    let wheel = TimerWheelCapsule::new();
    wheel.set_current_time(1_000_000);

    // Schedule timer
    let result = wheel.schedule(Duration::from_millis(10), 42);
    assert!(result.is_ok());

    let metrics = wheel.metrics();
    assert_eq!(metrics.scheduled, 1);

    // Fire timer
    let expired = wheel.tick(Duration::from_millis(10));
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0], 42);

    let metrics = wheel.metrics();
    assert_eq!(metrics.fired, 1);
}

#[test]
fn test_schedule_zero_task_id() {
    let wheel = TimerWheelCapsule::new();
    let result = wheel.schedule(Duration::from_millis(10), 0);
    assert_eq!(result, Err(TimerWheelError::InvalidState));
}

#[test]
fn test_multiple_timers() {
    let wheel = TimerWheelCapsule::new();
    wheel.set_current_time(0);

    // Schedule multiple timers
    let _id1 = wheel.schedule(Duration::from_millis(5), 100).unwrap();
    let _id2 = wheel.schedule(Duration::from_millis(10), 200).unwrap();
    let _id3 = wheel.schedule(Duration::from_millis(15), 300).unwrap();

    let metrics = wheel.metrics();
    assert_eq!(metrics.scheduled, 3);

    // Tick and check firing
    let expired = wheel.tick(Duration::from_millis(5));
    assert_eq!(expired.len(), 1);

    let expired = wheel.tick(Duration::from_millis(5));
    assert_eq!(expired.len(), 1);

    let expired = wheel.tick(Duration::from_millis(5));
    assert_eq!(expired.len(), 1);
}

#[test]
fn test_delay_too_large() {
    let wheel = TimerWheelCapsule::new();
    // Try to schedule a very large delay
    let result = wheel.schedule(Duration::from_secs(3600), 1);
    assert_eq!(result, Err(TimerWheelError::DelayTooLarge));
}

#[test]
fn test_cancel() {
    let wheel = TimerWheelCapsule::new();
    let timer_id = wheel.schedule(Duration::from_millis(100), 42).unwrap();

    let result = wheel.cancel(timer_id);
    assert!(result.is_ok());

    let metrics = wheel.metrics();
    assert_eq!(metrics.cancelled, 1);
}

#[test]
fn test_metric_active() {
    let wheel = TimerWheelCapsule::new();
    wheel.schedule(Duration::from_millis(10), 1).unwrap();
    wheel.schedule(Duration::from_millis(10), 2).unwrap();
    wheel.schedule(Duration::from_millis(10), 3).unwrap();

    let metrics = wheel.metrics();
    assert_eq!(metrics.active(), 3);

    wheel.tick(Duration::from_millis(15));
    let metrics = wheel.metrics();
    assert_eq!(metrics.active(), 0);
}

#[test]
fn test_time_monotonicity() {
    let wheel = TimerWheelCapsule::new();
    wheel.set_current_time(1000);
    assert_eq!(wheel.current_time(), 1000);

    wheel.set_current_time(2000);
    assert_eq!(wheel.current_time(), 2000);
}

#[test]
fn test_no_timers_fired_early() {
    let wheel = TimerWheelCapsule::new();
    wheel.set_current_time(0);

    wheel.schedule(Duration::from_millis(100), 42).unwrap();

    // Only advance 50ms
    let expired = wheel.tick(Duration::from_millis(50));
    assert!(expired.is_empty());

    let metrics = wheel.metrics();
    assert_eq!(metrics.fired, 0);
}

#[test]
fn test_wheel_capacity() {
    let wheel = TimerWheelCapsule::new();
    wheel.set_current_time(0);

    // Fill the wheel (256 total slots available)
    for i in 1..=100 {
        let result = wheel.schedule(Duration::from_millis(1 + i as u64), i);
        assert!(result.is_ok(), "Failed to schedule timer {}", i);
    }

    let metrics = wheel.metrics();
    assert_eq!(metrics.scheduled, 100);
}
