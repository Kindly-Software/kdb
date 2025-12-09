use crate::AggregationResult;
use crate::controller::{AccountSnapshot, PortfolioController};
use crate::feed::{EcoFeed, FeedAssembler, FeedSnapshot, SymbolGates, SymbolPolicy};

/// Simple runtime harness that pulls data from live feeds and publishes snapshots via
/// [`PortfolioController`].
pub struct PortfolioRuntime {
    controller: PortfolioController,
    assembler: FeedAssembler,
    policies: Vec<SymbolPolicy>,
}

impl PortfolioRuntime {
    /// Construct a new runtime harness.
    pub fn new(
        controller: PortfolioController,
        assembler: FeedAssembler,
        policies: Vec<SymbolPolicy>,
    ) -> Self {
        Self {
            controller,
            assembler,
            policies,
        }
    }

    /// Publish a snapshot for the provided account state. Returns `None` if mandatory feeds
    /// are missing for any symbol.
    pub fn publish_cycle<F>(
        &mut self,
        account: &AccountSnapshot,
        now_minute_count: u16,
        now_ms: u64,
        mut gates_fn: F,
    ) -> Option<AggregationResult>
    where
        F: FnMut(u16) -> SymbolGates,
    {
        let mut feeds = Vec::<FeedSnapshot>::with_capacity(self.policies.len());
        for policy in &self.policies {
            let gates = gates_fn(policy.sym_id);
            let feed = self.assembler.assemble(policy, gates)?;
            feeds.push(feed);
        }

        Some(
            self.controller
                .publish(account, &feeds, now_minute_count, now_ms),
        )
    }

    /// Publish a snapshot using ECO-1024 derived gates.
    pub fn publish_cycle_with_eco(
        &mut self,
        account: &AccountSnapshot,
        now_minute_count: u16,
        now_ms: u64,
        eco_feed: &dyn EcoFeed,
    ) -> Option<AggregationResult> {
        self.publish_cycle_with_eco_override(account, now_minute_count, now_ms, eco_feed, |_, g| g)
    }

    /// Publish a snapshot using ECO-derived gates with per-symbol overrides.
    pub fn publish_cycle_with_eco_override<F>(
        &mut self,
        account: &AccountSnapshot,
        now_minute_count: u16,
        now_ms: u64,
        eco_feed: &dyn EcoFeed,
        mut gates_fn: F,
    ) -> Option<AggregationResult>
    where
        F: FnMut(u16, SymbolGates) -> SymbolGates,
    {
        let eco_snapshot = eco_feed.snapshot();
        self.publish_cycle(account, now_minute_count, now_ms, |sym_id| {
            let base = eco_snapshot
                .as_ref()
                .map(SymbolGates::from_eco)
                .unwrap_or_default();
            gates_fn(sym_id, base)
        })
    }

    /// Advance the controller's stale timer.
    pub fn tick(&mut self, now_ms: u64) {
        self.controller.tick(now_ms);
    }

    pub fn controller(&self) -> &PortfolioController {
        &self.controller
    }

    pub fn controller_mut(&mut self) -> &mut PortfolioController {
        &mut self.controller
    }

    pub fn set_policies(&mut self, policies: Vec<SymbolPolicy>) {
        self.policies = policies;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApmSlot;
    use crate::adapters::SharedEcoFeed;
    use crate::feed::{ActEdge, ActFeed, ApcFeed, ApcSnapshot, AvsFeed, AvsSnapshot, SymbolGates};
    use crate::layout::BreakerLevel;
    use crate::writer::PortfolioMapWriter;
    use atomic_event_lockout_map::{
        AccountScope as EcoAccountScope, BuildRequest, EcoWriter, EventAction, EventSeverity,
        EventWindow, GlobalFlag,
    };
    use std::sync::Arc;
    use std::time::Duration;

    #[derive(Clone)]
    struct TestApc(std::sync::Arc<std::sync::RwLock<ApcSnapshot>>);
    impl TestApc {
        fn new(snapshot: ApcSnapshot) -> Self {
            Self(std::sync::Arc::new(std::sync::RwLock::new(snapshot)))
        }
    }

    impl ApcFeed for TestApc {
        fn snapshot(&self, _sym_id: u16) -> Option<ApcSnapshot> {
            Some(self.0.read().unwrap().clone())
        }
    }

    #[derive(Clone)]
    struct TestAct(std::sync::Arc<ActEdge>);
    impl TestAct {
        fn new(edge: ActEdge) -> Self {
            Self(std::sync::Arc::new(edge))
        }
    }

    impl ActFeed for TestAct {
        fn edge(&self, _sym_id: u16) -> Option<ActEdge> {
            Some((*self.0).clone())
        }
    }

    #[derive(Clone)]
    struct TestAvs(std::sync::Arc<AvsSnapshot>);
    impl TestAvs {
        fn new(snapshot: AvsSnapshot) -> Self {
            Self(std::sync::Arc::new(snapshot))
        }
    }

    impl AvsFeed for TestAvs {
        fn snapshot(&self, _sym_id: u16) -> Option<AvsSnapshot> {
            Some((*self.0).clone())
        }
    }

    fn policy(sym_id: u16) -> SymbolPolicy {
        SymbolPolicy {
            sym_id,
            max_abs_position: 8,
            forbid_after_min_ct: Some(900),
            eod_flat_min_ct: Some(910),
            priority_offset: 4,
        }
    }

    fn account() -> AccountSnapshot {
        AccountSnapshot {
            account_id: 999,
            forbid_after_min_ct: 900,
            eod_flat_min_ct: 910,
            rem_daily_loss_total_cents: 700_000,
            trailing_draw_cents: 25_000,
            base_realized_cents: 150_000,
            created_ms_coarse: 30_000,
            portfolio_flags: crate::PortfolioFlags::empty(),
        }
    }

    #[test]
    fn runtime_publishes_and_tracks_stale() {
        let apc = TestApc::new(ApcSnapshot {
            position: 2,
            unreal_cents: 20_000,
            realized_cents: 10_000,
            rem_daily_loss_cents: 180_000,
            breaker_level: BreakerLevel::L0,
        });
        let act = TestAct::new(ActEdge {
            edge_surplus_bp: 10,
        });
        let avs = TestAvs::new(AvsSnapshot {
            spread_ticks: 3,
            vol_band: 1,
        });

        let assembler = FeedAssembler::new(
            std::sync::Arc::new(apc),
            Some(std::sync::Arc::new(act)),
            Some(std::sync::Arc::new(avs)),
        );
        let writer = PortfolioMapWriter::new(ApmSlot::new());
        let controller = PortfolioController::new(writer, Duration::from_millis(2_000));
        let mut runtime = PortfolioRuntime::new(controller, assembler, vec![policy(1)]);

        let result = runtime
            .publish_cycle(&account(), 880, 1_000, |_| SymbolGates::default())
            .expect("publish succeeds");
        assert_eq!(result.snapshot.header.seq, 1);
        assert!(
            runtime
                .controller()
                .writer()
                .slot()
                .load_relaxed()
                .is_some()
        );

        runtime.tick(3_300);
        let header =
            crate::layout::ApmHeader::decode(runtime.controller().writer().slot().raw_words()[0]);
        assert!(!header.commit);
        assert!(header.stale);
    }

    #[test]
    fn runtime_applies_eco_gates() {
        let apc = TestApc::new(ApcSnapshot {
            position: 1,
            unreal_cents: 10_000,
            realized_cents: 5_000,
            rem_daily_loss_cents: 100_000,
            breaker_level: BreakerLevel::L0,
        });
        let assembler = FeedAssembler::new(Arc::new(apc), None, None);
        let writer = PortfolioMapWriter::new(ApmSlot::new());
        let controller = PortfolioController::new(writer, Duration::from_millis(1_000));
        let mut runtime = PortfolioRuntime::new(controller, assembler, vec![policy(1)]);

        let eco_feed = SharedEcoFeed::new();
        let mut eco_writer = EcoWriter::new(EcoAccountScope::new(7, 0))
            .with_origin_minute(480)
            .with_mask_length(512)
            .with_baseline_window(480, 540);
        let events = [EventWindow::econ(
            500,
            510,
            EventSeverity::Medium,
            EventAction::Degrade,
        )];
        let eco_snapshot = eco_writer.build_and_publish(BuildRequest {
            now_min_ct: 505,
            age_8ms: 0,
            created_ms_coarse: 500,
            events: &events,
            global_flags: GlobalFlag::empty(),
            manual_pause: false,
            day_of_week: 1,
            holiday_flag: false,
        });
        eco_feed.publish(eco_snapshot);

        let result = runtime
            .publish_cycle_with_eco(&account(), 505, 1_000, &eco_feed)
            .expect("eco publish succeeds");

        let slice = result.snapshot.slices[0];
        assert!(
            slice
                .flags
                .contains(crate::layout::SymbolFlags::REDUCE_ONLY)
        );
        assert!(!slice.flags.contains(crate::layout::SymbolFlags::LOCKOUT));
    }
}
