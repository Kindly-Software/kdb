//! Integration tests for atomic capsules
//! Tests all 5 Tier 1 atomic capsules in isolation

use kindly_web::state::{
    AppStateCapsule, BudgetViewCapsule, ThemeCapsule,
    WebSocketStateCapsule, WebSocketState, MetricsCapsule,
};

#[test]
fn test_app_state_capsule() {
    let capsule = AppStateCapsule::new();

    // Test theme
    assert_eq!(capsule.get_theme(), 0);
    capsule.set_theme(2).unwrap();
    assert_eq!(capsule.get_theme(), 2);

    // Test dark mode
    assert_eq!(capsule.get_dark_mode(), false);
    capsule.set_dark_mode(true);
    assert_eq!(capsule.get_dark_mode(), true);

    // Test snapshot
    let (theme, dark, user_id, _gen) = capsule.snapshot();
    assert_eq!(theme, 2);
    assert_eq!(dark, true);
    assert_eq!(user_id, 0);
}

#[test]
fn test_budget_view_capsule() {
    let capsule = BudgetViewCapsule::new(1000_00); // $1000.00

    // Test deduct
    let remaining = capsule.try_deduct(250_00).unwrap();
    assert_eq!(remaining, 750_00);
    assert_eq!(capsule.get_budget(), 750_00);
    assert_eq!(capsule.get_spent(), 250_00);

    // Test credit
    let new_budget = capsule.credit(100_00).unwrap();
    assert_eq!(new_budget, 850_00);

    // Test snapshot
    let (budget, spent, count, _gen) = capsule.snapshot();
    assert_eq!(budget, 850_00);
    assert_eq!(spent, 250_00);
    assert_eq!(count, 2); // 1 deduct + 1 credit
}

#[test]
fn test_theme_capsule() {
    let capsule = ThemeCapsule::new();

    // Test color indices
    capsule.set_color_index(5).unwrap();
    capsule.set_accent_index(10).unwrap();
    assert_eq!(capsule.get_color_index(), 5);
    assert_eq!(capsule.get_accent_index(), 10);

    // Test dark mode toggle
    assert_eq!(capsule.get_dark_mode(), false);
    capsule.toggle_dark_mode();
    assert_eq!(capsule.get_dark_mode(), true);

    // Test snapshot
    let (color, accent, dark) = capsule.snapshot();
    assert_eq!(color, 5);
    assert_eq!(accent, 10);
    assert_eq!(dark, true);
}

#[test]
fn test_websocket_state_capsule() {
    let capsule = WebSocketStateCapsule::new();

    // Test initial state
    assert_eq!(capsule.get_state().unwrap(), WebSocketState::Disconnected);
    assert_eq!(capsule.is_connected(), false);

    // Test state transition
    capsule.update_state(WebSocketState::Connecting).unwrap();
    assert_eq!(capsule.get_state().unwrap(), WebSocketState::Connecting);

    capsule.update_state(WebSocketState::Connected).unwrap();
    assert_eq!(capsule.get_state().unwrap(), WebSocketState::Connected);
    assert_eq!(capsule.is_connected(), true);

    // Test ping
    capsule.ping(123456789);
    assert_eq!(capsule.get_last_ping_ns(), 123456789);

    // Test message tracking
    capsule.record_message();
    capsule.record_message();
    assert_eq!(capsule.get_message_count(), 2);
}

#[test]
fn test_metrics_capsule() {
    let capsule = MetricsCapsule::new();

    // Test page views
    capsule.record_page_view();
    capsule.record_page_view();
    assert_eq!(capsule.get_page_views(), 2);

    // Test clicks
    capsule.record_click();
    assert_eq!(capsule.get_clicks(), 1);

    // Test submissions
    capsule.record_submission();
    assert_eq!(capsule.get_submissions(), 1);

    // Test performance
    capsule.update_performance_p99(150).unwrap();
    assert_eq!(capsule.get_performance_p99_ms(), 150);

    // Test snapshot
    let (views, clicks, subs, perf) = capsule.snapshot();
    assert_eq!(views, 2);
    assert_eq!(clicks, 1);
    assert_eq!(subs, 1);
    assert_eq!(perf, 150);

    // Test derived metrics
    assert!((capsule.click_through_rate() - 50.0).abs() < 0.01); // 1/2 = 50%
    assert!((capsule.submission_rate() - 50.0).abs() < 0.01); // 1/2 = 50%
}

#[test]
fn test_alignment_and_size() {
    use std::mem::{align_of, size_of};

    // Verify all capsules meet alignment requirements
    assert_eq!(align_of::<AppStateCapsule>(), 64);
    assert_eq!(size_of::<AppStateCapsule>(), 64);

    assert_eq!(align_of::<BudgetViewCapsule>(), 128);
    assert_eq!(size_of::<BudgetViewCapsule>(), 128);

    assert_eq!(align_of::<ThemeCapsule>(), 64);
    assert_eq!(size_of::<ThemeCapsule>(), 64);

    assert_eq!(align_of::<WebSocketStateCapsule>(), 128);
    assert_eq!(size_of::<WebSocketStateCapsule>(), 128);

    assert_eq!(align_of::<MetricsCapsule>(), 64);
    assert_eq!(size_of::<MetricsCapsule>(), 64);
}
