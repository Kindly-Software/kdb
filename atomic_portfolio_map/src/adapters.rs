use crate::feed::{ActEdge, ActFeed, ApcFeed, ApcSnapshot, AvsFeed, AvsSnapshot, EcoFeed};
use atomic_cost_tracker::ActSlot;
use atomic_event_lockout_map::EcoSnapshot;
use atomic_position_capsule::AtomicPositionCapsule;
use atomic_venue_snapshot::Avs128;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Thread-safe store of APC snapshots keyed by `sym_id`.
#[derive(Clone, Default)]
pub struct SharedApcFeed {
    inner: Arc<RwLock<HashMap<u16, ApcSnapshot>>>,
}

impl SharedApcFeed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, sym_id: u16, snapshot: ApcSnapshot) {
        self.inner.write().unwrap().insert(sym_id, snapshot);
    }
}

impl ApcFeed for SharedApcFeed {
    fn snapshot(&self, sym_id: u16) -> Option<ApcSnapshot> {
        self.inner.read().unwrap().get(&sym_id).cloned()
    }
}

/// Thread-safe store of ACT edge data keyed by `sym_id`.
#[derive(Clone, Default)]
pub struct SharedActFeed {
    inner: Arc<RwLock<HashMap<u16, ActEdge>>>,
}

impl SharedActFeed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, sym_id: u16, edge: ActEdge) {
        self.inner.write().unwrap().insert(sym_id, edge);
    }
}

impl ActFeed for SharedActFeed {
    fn edge(&self, sym_id: u16) -> Option<ActEdge> {
        self.inner.read().unwrap().get(&sym_id).cloned()
    }
}

/// Thread-safe store of AVS venue data keyed by `sym_id`.
#[derive(Clone, Default)]
pub struct SharedAvsFeed {
    inner: Arc<RwLock<HashMap<u16, AvsSnapshot>>>,
}

impl SharedAvsFeed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, sym_id: u16, snapshot: AvsSnapshot) {
        self.inner.write().unwrap().insert(sym_id, snapshot);
    }
}

impl AvsFeed for SharedAvsFeed {
    fn snapshot(&self, sym_id: u16) -> Option<AvsSnapshot> {
        self.inner.read().unwrap().get(&sym_id).cloned()
    }
}

/// Thread-safe store of ECO snapshots.
#[derive(Clone, Default)]
pub struct SharedEcoFeed {
    inner: Arc<RwLock<Option<EcoSnapshot>>>,
}

impl SharedEcoFeed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish(&self, snapshot: EcoSnapshot) {
        *self.inner.write().unwrap() = Some(snapshot);
    }
}

impl EcoFeed for SharedEcoFeed {
    fn snapshot(&self) -> Option<EcoSnapshot> {
        self.inner.read().unwrap().clone()
    }
}

/// Feed wrapper over live `AtomicPositionCapsule` instances keyed by `sym_id`.
#[derive(Clone, Default)]
pub struct CapsuleApcFeed {
    capsules: Arc<RwLock<HashMap<u16, Arc<AtomicPositionCapsule>>>>,
}

impl CapsuleApcFeed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, sym_id: u16, capsule: Arc<AtomicPositionCapsule>) {
        self.capsules.write().unwrap().insert(sym_id, capsule);
    }

    pub fn unregister(&self, sym_id: u16) -> Option<Arc<AtomicPositionCapsule>> {
        self.capsules.write().unwrap().remove(&sym_id)
    }
}

impl ApcFeed for CapsuleApcFeed {
    fn snapshot(&self, sym_id: u16) -> Option<ApcSnapshot> {
        let capsule = self.capsules.read().unwrap().get(&sym_id)?.clone();
        let snapshot = capsule.load()?;
        Some(ApcSnapshot::from_capsule(&snapshot))
    }
}

/// Feed wrapper over live AVS-128 capsules keyed by `sym_id`.
#[derive(Clone, Default)]
pub struct CapsuleAvsFeed {
    venues: Arc<RwLock<HashMap<u16, Arc<Avs128>>>>,
}

impl CapsuleAvsFeed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, sym_id: u16, venue: Arc<Avs128>) {
        self.venues.write().unwrap().insert(sym_id, venue);
    }

    pub fn unregister(&self, sym_id: u16) -> Option<Arc<Avs128>> {
        self.venues.write().unwrap().remove(&sym_id)
    }
}

impl AvsFeed for CapsuleAvsFeed {
    fn snapshot(&self, sym_id: u16) -> Option<AvsSnapshot> {
        let venue = self.venues.read().unwrap().get(&sym_id)?.clone();
        let snapshot = venue.load_relaxed().unpack();
        Some(AvsSnapshot::from_capsule(&snapshot))
    }
}

/// Feed wrapper over live ACT slots keyed by `sym_id`.
#[derive(Clone, Default)]
pub struct ActSlotFeed {
    slots: Arc<RwLock<HashMap<u16, Arc<ActSlot>>>>,
}

impl ActSlotFeed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, sym_id: u16, slot: Arc<ActSlot>) {
        self.slots.write().unwrap().insert(sym_id, slot);
    }

    pub fn unregister(&self, sym_id: u16) -> Option<Arc<ActSlot>> {
        self.slots.write().unwrap().remove(&sym_id)
    }
}

impl ActFeed for ActSlotFeed {
    fn edge(&self, sym_id: u16) -> Option<ActEdge> {
        let slot = self.slots.read().unwrap().get(&sym_id)?.clone();
        let word = slot.load_acquire();
        let snapshot = word.unpack();
        Some(ActEdge::from_act_snapshot(&snapshot))
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use crate::layout::BreakerLevel;
    use atomic_cost_tracker::{ActSnapshot, FixedQ8_8};
    use atomic_event_lockout_map::{
        AccountScope as EcoAccountScope, BuildRequest, EcoWriter, GlobalFlag,
    };
    use atomic_position_capsule::{
        CapsuleDraft, EquityWord, PositionHeadWord, SessionWord, TailWord,
    };
    use atomic_venue_snapshot::Avs128Snapshot;

    #[test]
    fn shared_feeds_store_and_retrieve() {
        let apc_feed = SharedApcFeed::new();
        apc_feed.insert(
            1,
            ApcSnapshot {
                position: 4,
                unreal_cents: 15_000,
                realized_cents: 9_000,
                rem_daily_loss_cents: 180_000,
                breaker_level: BreakerLevel::L1,
            },
        );
        assert!(apc_feed.snapshot(2).is_none());
        assert_eq!(apc_feed.snapshot(1).unwrap().position, 4);

        let act_feed = SharedActFeed::new();
        act_feed.insert(
            5,
            ActEdge {
                edge_surplus_bp: 12,
            },
        );
        assert_eq!(act_feed.edge(5).unwrap().edge_surplus_bp, 12);

        let avs_feed = SharedAvsFeed::new();
        avs_feed.insert(
            7,
            AvsSnapshot {
                spread_ticks: 3,
                vol_band: 2,
            },
        );
        assert_eq!(avs_feed.snapshot(7).unwrap().spread_ticks, 3);

        let eco_feed = SharedEcoFeed::new();
        let mut writer = EcoWriter::new(EcoAccountScope::new(7, 0))
            .with_origin_minute(480)
            .with_baseline_window(480, 600);
        let snapshot = writer.build_and_publish(BuildRequest {
            now_min_ct: 500,
            age_8ms: 0,
            created_ms_coarse: 1_000,
            events: &[],
            global_flags: GlobalFlag::empty(),
            manual_pause: false,
            day_of_week: 2,
            holiday_flag: false,
        });
        eco_feed.publish(snapshot);
        assert!(eco_feed.snapshot().is_some());
    }

    #[test]
    fn capsule_avs_feed_produces_snapshot() {
        let feed = CapsuleAvsFeed::new();
        let venue = Arc::new(Avs128::new());
        venue.publish(Avs128Snapshot {
            spread_ticks: 2,
            obi_q1_10: 64,
            micro_off_ticks: 1,
            sum_bid_l1_3: 150,
            sum_ask_l1_3: 120,
            vol_bp_q8_8: 512,
            sweep_flag: false,
            trend_200ms_ticks: -1,
            ts_coarse_ms: 100,
            version: 1,
            sequence: 3,
        });
        feed.register(42, venue.clone());

        let snapshot = feed.snapshot(42).expect("avs snapshot");
        assert_eq!(snapshot.spread_ticks, 2);
        assert_eq!(snapshot.vol_band, 0);
        assert!(feed.snapshot(7).is_none());
    }

    #[test]
    fn capsule_feed_exposes_live_capsule() {
        let feed = CapsuleApcFeed::new();
        let capsule = Arc::new(AtomicPositionCapsule::new());
        let mut draft = CapsuleDraft::new();
        draft
            .set_head(PositionHeadWord {
                position_qty: 3,
                avg_px_ticks: 0,
                remaining_daily_loss_cents: 150_000,
                flags: 0,
            })
            .set_equity(EquityWord {
                realized_cents: 120_000,
                unrealized_cents: 25_000,
                peak_equity_cents: 150_000,
                trailing_draw_cents: 8_000,
            })
            .set_session(SessionWord {
                now_min_ct: 905,
                forbid_after_min_ct: 910,
                eod_flat_min_ct: 915,
                open_since_ms: 0,
                max_open_ms: 0,
                max_contracts: 10,
                max_per_trade_cents: 0,
                risk_flags: 0,
                reserved_bits: 0,
            })
            .set_tail(TailWord {
                symbol_id: 42,
                account_id: 1,
                last_exec_id: 0,
                breaker_level: 2,
                alt_health: 0,
                violation_bits: 0,
            });
        capsule.publish_draft(&draft);
        feed.register(42, capsule);

        let snapshot = feed.snapshot(42).expect("capsule snapshot available");
        assert_eq!(snapshot.position, 3);
        assert_eq!(snapshot.unreal_cents, 25_000);
        assert_eq!(snapshot.breaker_level, BreakerLevel::from_u8(2));
        assert!(feed.snapshot(99).is_none());
    }

    #[test]
    fn act_slot_feed_reports_surplus() {
        let feed = ActSlotFeed::new();
        let slot = Arc::new(ActSlot::default());
        let mut snapshot = ActSnapshot::empty();
        snapshot.net = FixedQ8_8::saturating_from_bp(3.5);
        snapshot.min_required = FixedQ8_8::saturating_from_bp(1.5);
        slot.publish(&snapshot);
        feed.register(7, slot.clone());

        let edge = feed.edge(7).expect("act edge present");
        assert_eq!(edge.edge_surplus_bp, 2);
        assert!(feed.edge(0).is_none());
    }
}
