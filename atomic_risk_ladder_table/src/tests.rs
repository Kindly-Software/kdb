use super::*;
use crate::layout::{
    actions::{ActionBases, ActionsWordDraft},
    header::StrategyMask,
    recover_threshold,
};
use proptest::prelude::*;

#[test]
fn header_roundtrip_matches_expectations() {
    let mut header = HeaderWord::ZERO;
    header.set_commit(true);
    header.set_stale(false);
    header.set_version_even(2);
    header.set_seq_head(42);
    header.set_policy_id(0xABCD);
    header.set_strategy_mask(StrategyMask::new(0b111));
    header.set_recover_scale(RecoverScale::new(90));
    header.set_dwell_up_ms(1_000);
    header.set_dwell_down_ms(3_000);
    header.set_created_ms_coarse(123_456);
    header.set_global_flags(0x1234 & ((1 << 14) - 1));
    header.clear_reserved();

    assert!(header.commit());
    assert!(!header.stale());
    assert_eq!(header.version_even(), 2);
    assert_eq!(header.seq_head(), 42);
    assert_eq!(header.policy_id(), 0xABCD);
    assert!(header.strategy_mask().contains(StrategyMask::STRATEGY_A));
    assert!(header.strategy_mask().contains(StrategyMask::STRATEGY_B));
    assert!(header.strategy_mask().contains(StrategyMask::STRATEGY_C));
    assert_eq!(header.recover_scale().raw(), 90);
    assert_eq!(header.dwell_up_ms(), 1_000);
    assert_eq!(header.dwell_down_ms(), 3_000);
    assert_eq!(header.created_ms_coarse(), 123_456 & ((1 << 24) - 1));
    assert_eq!(header.global_flags(), 0x1234 & ((1 << 14) - 1));
    assert_eq!(header.raw() >> 116, 0, "reserved bits must remain zero");
}

#[test]
fn trips_roundtrip_preserves_thresholds() {
    let mut trip_word = TripWord::ZERO;
    trip_word.set_thresholds(TripThresholds::DEFAULT);
    let decoded = trip_word.thresholds();
    assert_eq!(decoded.alt, [640, 896, 1023]);
    assert_eq!(decoded.rej, [150, 300, 600]);
    assert_eq!(decoded.loss, [50, 200, 400]);
    assert_eq!(decoded.vol, [384, 640, 1024]);

    trip_word.clear_spare();
    assert_eq!(trip_word.raw() >> 126, 0, "spare bits must be cleared");
}

#[test]
fn action_roundtrip_matches_defaults() {
    let mut act_word = ActionWord::ZERO;
    act_word.apply_draft(ActionsWordDraft::DEFAULT);
    let draft = act_word.draft();
    assert_eq!(draft.size_q2_6, [64, 32, 16, 0]);
    assert_eq!(draft.slip_q1_7, [128, 109, 90, 90]);
    assert_eq!(draft.latency_q1_7, [128, 109, 90, 64]);
    assert_eq!(draft.route, ActionsWordDraft::DEFAULT.route);
    assert_eq!(draft.dwell_up_ms, 0);
    assert_eq!(draft.dwell_down_ms, 0);
}

#[test]
fn checksum_canonicalises_header_tail() {
    let mut table = Rlt1024::new();
    let mut header = table.header;
    header.set_version_even(2);
    header.set_seq_head(7);
    header.set_policy_id(0x1001);
    header.set_strategy_mask(StrategyMask::new(0b111));
    table.header = header;

    table.strat_a_trips.set_thresholds(TripThresholds::DEFAULT);
    table.strat_a_actions.apply_draft(ActionsWordDraft::DEFAULT);

    let checksum = table.checksum16();
    let mut tail = table.tail;
    tail.set_version(2);
    tail.set_seq_tail(7);
    tail.set_checksum(checksum);
    table.tail = tail;

    assert_eq!(table.tail.checksum(), checksum);
    assert!(layout::validate_snapshot(&table).is_ok());
}

#[test]
fn validate_snapshot_detects_mismatch() {
    let mut table = Rlt1024::new();
    let mut header = table.header;
    header.set_version_even(4);
    header.set_seq_head(10);
    table.header = header;

    let mut tail = table.tail;
    tail.set_version(2);
    tail.set_seq_tail(9);
    tail.set_checksum(0);
    table.tail = tail;

    let err = layout::validate_snapshot(&table).unwrap_err();
    match err {
        layout::CapsuleValidationError::VersionMismatch { header, tail } => {
            assert_eq!(header, 4);
            assert_eq!(tail, 2);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

proptest! {
    #[test]
    fn recover_threshold_matches_fixed_point(trip in 0u16..=1023, scale in 0u8..=254) {
        let scale = RecoverScale::new(scale & !1); // ensure even
        let from_fn = recover_threshold(trip, scale);
        let manual = ((u32::from(trip) * u32::from(scale.raw())) >> 7) as u16;
        prop_assert_eq!(from_fn, manual);
    }
}

#[test]
fn actions_apply_to_scales_bases() {
    let draft = ActionsWordDraft::DEFAULT;
    let applied = draft.apply_to(
        1,
        ActionBases {
            size: 100.0,
            slip_cap: 2.0,
            latency_budget: 1.0,
        },
    );

    assert!((applied.size - 50.0).abs() < 1e-6);
    assert!((applied.slip_cap - 1.703_125).abs() < 1e-6);
    assert!((applied.latency_budget - 0.851_562_5).abs() < 1e-6);
    assert!(matches!(
        applied.route,
        crate::layout::actions::RoutePolicy::MakerPreferred
    ));
}
