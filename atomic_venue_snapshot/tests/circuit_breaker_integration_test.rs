use atomic_venue_snapshot::{Avs128Snapshot, AtomicVenueSnapshotWithBreaker, MarketQualityThresholds};
use atomic_breaker::breaker::State as BreakerState;

#[test]
fn test_circuit_breaker_basic_functionality() {
    let avs_with_breaker = AtomicVenueSnapshotWithBreaker::new();

    // Initial state should be closed
    assert_eq!(avs_with_breaker.breaker_state(), BreakerState::Closed);

    // Test with healthy market data
    let healthy_snapshot = Avs128Snapshot {
        spread_ticks: 2,        // Low spread
        vol_bp_q8_8: 100,       // Low volatility
        obi_q1_10: 100,         // Balanced order book
        trend_200ms_ticks: 5,   // Small trend
        sweep_flag: false,      // No sweep
        ..Default::default()
    };

    let thresholds = MarketQualityThresholds::default();
    avs_with_breaker.publish_with_validation(healthy_snapshot, thresholds);

    // Should remain closed with healthy data
    assert_eq!(avs_with_breaker.breaker_state(), BreakerState::Closed);

    // Load snapshot and verify it's published
    let (loaded_snapshot, is_breaker_open) = avs_with_breaker.load_snapshot_with_breaker();
    assert_eq!(loaded_snapshot.spread_ticks, 2);
    assert!(!is_breaker_open);
}

#[test]
fn test_circuit_breaker_triggers_on_bad_market() {
    let avs_with_breaker = AtomicVenueSnapshotWithBreaker::new();

    // Create market data that violates multiple thresholds
    let bad_snapshot = Avs128Snapshot {
        spread_ticks: 100,      // High spread (> 50 default)
        vol_bp_q8_8: 10000,     // High volatility (> 5000 default)
        obi_q1_10: 1000,        // Extreme imbalance (> 900 default)
        trend_200ms_ticks: 200, // Large trend (> 100 default)
        sweep_flag: true,       // Sweep detected
        ..Default::default()
    };

    let thresholds = MarketQualityThresholds::default();
    avs_with_breaker.publish_with_validation(bad_snapshot, thresholds);

    // Should trip breaker with multiple violations
    assert_eq!(avs_with_breaker.breaker_state(), BreakerState::Open);

    // Load snapshot and verify breaker state
    let (loaded_snapshot, is_breaker_open) = avs_with_breaker.load_snapshot_with_breaker();
    assert_eq!(loaded_snapshot.spread_ticks, 100);
    assert!(is_breaker_open);
}

#[test]
fn test_force_breaker_open() {
    let avs_with_breaker = AtomicVenueSnapshotWithBreaker::new();

    // Force breaker open
    avs_with_breaker.force_breaker_open();
    assert_eq!(avs_with_breaker.breaker_state(), BreakerState::ForcedOpen);

    // Should report as open
    let (_, is_breaker_open) = avs_with_breaker.load_snapshot_with_breaker();
    assert!(is_breaker_open);
}

#[test]
fn test_breaker_recovery() {
    let avs_with_breaker = AtomicVenueSnapshotWithBreaker::new();

    // Trip breaker first
    avs_with_breaker.force_breaker_open();
    assert_eq!(avs_with_breaker.breaker_state(), BreakerState::ForcedOpen);

    // Manually close breaker
    avs_with_breaker.close_breaker();
    assert_eq!(avs_with_breaker.breaker_state(), BreakerState::Closed);
}

#[test]
fn test_custom_thresholds() {
    let avs_with_breaker = AtomicVenueSnapshotWithBreaker::new();

    // Create restrictive thresholds
    let strict_thresholds = MarketQualityThresholds {
        max_volatility_bp_q8_8: 100,   // Very low volatility threshold
        max_spread_ticks: 5,           // Very low spread threshold
        max_obi_abs_q1_10: 50,         // Very low imbalance threshold
        max_trend_spike_ticks: 10,     // Very low trend threshold
    };

    // Test with data that would be OK under default thresholds
    let moderate_snapshot = Avs128Snapshot {
        spread_ticks: 10,      // OK under default, bad under strict
        vol_bp_q8_8: 200,      // OK under default, bad under strict
        obi_q1_10: 0,          // Always OK
        trend_200ms_ticks: 5,  // Always OK
        sweep_flag: false,     // Always OK
        ..Default::default()
    };

    avs_with_breaker.publish_with_validation(moderate_snapshot, strict_thresholds);

    // Should trip under strict thresholds
    assert_eq!(avs_with_breaker.breaker_state(), BreakerState::Open);
}

#[test]
fn test_with_breaker_state_constructor() {
    let avs_with_breaker = AtomicVenueSnapshotWithBreaker::with_breaker_state(BreakerState::HalfOpen);

    // Should start in specified state
    assert_eq!(avs_with_breaker.breaker_state(), BreakerState::HalfOpen);
}