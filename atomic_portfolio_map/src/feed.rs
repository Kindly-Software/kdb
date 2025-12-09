use crate::inputs::SymbolInputs;
use crate::layout::BreakerLevel;
use atomic_event_lockout_map::{EcoSnapshot, EventAction, GlobalFlag};
use std::sync::Arc;

/// Provider capable of supplying the latest APC snapshot for a symbol.
pub trait ApcFeed {
    fn snapshot(&self, sym_id: u16) -> Option<ApcSnapshot>;
}

/// Provider capable of supplying ACT-derived edge surplus information.
pub trait ActFeed {
    fn edge(&self, sym_id: u16) -> Option<ActEdge>;
}

/// Provider capable of supplying venue state metrics for a symbol.
pub trait AvsFeed {
    fn snapshot(&self, sym_id: u16) -> Option<AvsSnapshot>;
}

/// Snapshot of the Atomic Position Capsule (APC-512) reduced to the fields needed by APM.
#[derive(Clone, Debug)]
pub struct ApcSnapshot {
    pub position: i32,
    pub unreal_cents: i32,
    pub realized_cents: i32,
    pub rem_daily_loss_cents: u32,
    pub breaker_level: BreakerLevel,
}

/// Snapshot of Atomic Cost Tracker-derived surplus edge information.
#[derive(Clone, Debug)]
pub struct ActEdge {
    /// Net edge minus min-required bp (already in basis points).
    pub edge_surplus_bp: i16,
}

/// Venue state snapshot carrying spread and volatility band heuristics.
#[derive(Clone, Debug)]
pub struct AvsSnapshot {
    pub spread_ticks: u8,
    pub vol_band: u8,
}

impl AvsSnapshot {
    pub fn from_capsule(snapshot: &atomic_venue_snapshot::Avs128Snapshot) -> Self {
        Self {
            spread_ticks: snapshot.spread_ticks,
            vol_band: Self::quantize_vol(snapshot.vol_bp_q8_8),
        }
    }

    fn quantize_vol(vol_bp_q8_8: u16) -> u8 {
        let bp = ((u32::from(vol_bp_q8_8) + 128) >> 8) as u16;
        match bp {
            0..=25 => 0,
            26..=60 => 1,
            61..=120 => 2,
            _ => 3,
        }
    }
}

/// Static policy/configuration for a single symbol route.
#[derive(Clone, Debug)]
pub struct SymbolPolicy {
    pub sym_id: u16,
    pub max_abs_position: i32,
    pub forbid_after_min_ct: Option<u16>,
    pub eod_flat_min_ct: Option<u16>,
    pub priority_offset: i16,
}

/// Dynamic flags surfaced by surrounding systems (news locks, manual overrides).
#[derive(Clone, Debug, Default)]
pub struct SymbolGates {
    pub news_lockout: bool,
    pub eco_lockout: bool,
    pub manual_lockout: bool,
    pub force_reduce_only: bool,
}

impl SymbolGates {
    /// Derive gate flags from an ECO-1024 snapshot.
    pub fn from_eco(snapshot: &EcoSnapshot) -> Self {
        let head = snapshot.head();
        let tail = snapshot.tail();
        let allowed = snapshot.is_allowed_now();

        let news_lockout = head.global_flags.contains(GlobalFlag::NEWS_LOCKOUT);
        let manual_lockout = head
            .global_flags
            .intersects(GlobalFlag::PAUSED | GlobalFlag::MANUAL);
        let eco_lockout = !allowed
            || tail.active_action.at_least(EventAction::ForbidNew)
            || head.global_flags.contains(GlobalFlag::AT_EOD);
        let force_reduce_only = head.global_flags.contains(GlobalFlag::REDUCE_ONLY)
            || tail.active_action.at_least(EventAction::Degrade)
            || !allowed;

        SymbolGates {
            news_lockout,
            eco_lockout,
            manual_lockout,
            force_reduce_only,
        }
    }

    /// Overlay manual toggles from a supervisory system.
    pub fn with_manual_overrides(mut self, manual_lockout: bool, force_reduce_only: bool) -> Self {
        if manual_lockout {
            self.manual_lockout = true;
            self.eco_lockout = true;
        }
        if force_reduce_only {
            self.force_reduce_only = true;
        }
        self
    }
}

/// Bundle of live feeds required to construct a `SymbolInputs` instance.
#[derive(Clone, Debug)]
pub struct FeedSnapshot {
    pub policy: SymbolPolicy,
    pub apc: ApcSnapshot,
    pub act: Option<ActEdge>,
    pub avs: Option<AvsSnapshot>,
    pub gates: SymbolGates,
}

/// Provider capable of supplying the latest ECO-1024 snapshot.
pub trait EcoFeed {
    fn snapshot(&self) -> Option<EcoSnapshot>;
}

/// Convert live feed data into a `SymbolInputs` suitable for aggregation.
pub fn build_symbol_inputs(feed: &FeedSnapshot) -> SymbolInputs {
    let edge_surplus_bp = feed.act.as_ref().map_or(0, |act| act.edge_surplus_bp);
    let (spread_ticks, vol_band) = feed
        .avs
        .as_ref()
        .map(|avs| (avs.spread_ticks, avs.vol_band))
        .unwrap_or((0, 0));

    SymbolInputs {
        sym_id: feed.policy.sym_id,
        position: feed.apc.position,
        unreal_cents: feed.apc.unreal_cents,
        realized_cents: feed.apc.realized_cents,
        rem_daily_loss_cents: feed.apc.rem_daily_loss_cents,
        breaker_level: feed.apc.breaker_level,
        spread_ticks,
        vol_band,
        edge_surplus_bp,
        priority_offset: feed.policy.priority_offset,
        max_abs_position: feed.policy.max_abs_position,
        forbid_after_min_ct: feed.policy.forbid_after_min_ct,
        eod_flat_min_ct: feed.policy.eod_flat_min_ct,
        news_lockout: feed.gates.news_lockout,
        eco_lockout: feed.gates.eco_lockout,
        manual_lockout: feed.gates.manual_lockout,
        force_reduce_only: feed.gates.force_reduce_only,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{SharedActFeed, SharedApcFeed, SharedAvsFeed};
    use atomic_cost_tracker::{ActSnapshot, FixedQ8_8};
    use atomic_event_lockout_map::{
        AccountScope, BuildRequest, EcoWriter, EventAction, EventSeverity, EventWindow, GlobalFlag,
    };
    use atomic_position_capsule::{
        AtomicPositionCapsule, CapsuleDraft, EquityWord, PositionHeadWord, SessionWord, TailWord,
    };
    use std::sync::Arc;

    #[test]
    fn symbol_gates_reflect_eco_snapshot() {
        let mut writer = EcoWriter::new(AccountScope::new(1, 0))
            .with_origin_minute(480)
            .with_mask_length(512)
            .with_baseline_window(480, 540);

        let events = [EventWindow::econ(
            500,
            510,
            EventSeverity::Medium,
            EventAction::Degrade,
        )];

        let snapshot = writer.build_and_publish(BuildRequest {
            now_min_ct: 505,
            age_8ms: 0,
            created_ms_coarse: 10,
            events: &events,
            global_flags: GlobalFlag::empty(),
            manual_pause: false,
            day_of_week: 2,
            holiday_flag: false,
        });

        let gates = SymbolGates::from_eco(&snapshot);
        assert!(gates.force_reduce_only);
        assert!(!gates.eco_lockout);

        let paused_snapshot = writer.build_and_publish(BuildRequest {
            now_min_ct: 505,
            age_8ms: 0,
            created_ms_coarse: 20,
            events: &[],
            global_flags: GlobalFlag::empty(),
            manual_pause: true,
            day_of_week: 2,
            holiday_flag: false,
        });

        let paused_gates = SymbolGates::from_eco(&paused_snapshot);
        assert!(paused_gates.manual_lockout);
        assert!(paused_gates.eco_lockout);
    }

    #[test]
    fn build_symbol_inputs_applies_overrides() {
        let feed = FeedSnapshot {
            policy: SymbolPolicy {
                sym_id: 12,
                max_abs_position: 6,
                forbid_after_min_ct: Some(900),
                eod_flat_min_ct: Some(910),
                priority_offset: 5,
            },
            apc: ApcSnapshot {
                position: 4,
                unreal_cents: 25_000,
                realized_cents: 8_000,
                rem_daily_loss_cents: 120_000,
                breaker_level: BreakerLevel::L1,
            },
            act: Some(ActEdge {
                edge_surplus_bp: 12,
            }),
            avs: Some(AvsSnapshot {
                spread_ticks: 3,
                vol_band: 2,
            }),
            gates: SymbolGates {
                news_lockout: true,
                eco_lockout: false,
                manual_lockout: false,
                force_reduce_only: false,
            },
        };

        let inputs = build_symbol_inputs(&feed);
        assert_eq!(inputs.sym_id, 12);
        assert_eq!(inputs.edge_surplus_bp, 12);
        assert_eq!(inputs.spread_ticks, 3);
        assert!(inputs.news_lockout);
    }

    #[test]
    fn assembler_yields_snapshot_only_when_apc_present() {
        let apc_feed = SharedApcFeed::new();
        let act_feed = SharedActFeed::new();
        let avs_feed = SharedAvsFeed::new();
        let assembler = FeedAssembler::new(
            Arc::new(apc_feed.clone()),
            Some(Arc::new(act_feed.clone())),
            Some(Arc::new(avs_feed.clone())),
        );

        let policy = SymbolPolicy {
            sym_id: 99,
            max_abs_position: 6,
            forbid_after_min_ct: None,
            eod_flat_min_ct: None,
            priority_offset: 0,
        };

        assert!(
            assembler
                .assemble(&policy, SymbolGates::default())
                .is_none()
        );

        apc_feed.insert(
            99,
            ApcSnapshot {
                position: 5,
                unreal_cents: 11_000,
                realized_cents: 6_000,
                rem_daily_loss_cents: 140_000,
                breaker_level: BreakerLevel::L0,
            },
        );
        act_feed.insert(99, ActEdge { edge_surplus_bp: 9 });
        avs_feed.insert(
            99,
            AvsSnapshot {
                spread_ticks: 3,
                vol_band: 2,
            },
        );

        let snapshot = assembler
            .assemble(&policy, SymbolGates::default())
            .expect("should build snapshot");
        assert_eq!(snapshot.policy.sym_id, 99);
        assert!(snapshot.act.is_some());
        assert!(snapshot.avs.is_some());
    }

    #[test]
    fn capsule_conversion_matches_capsule_snapshot() {
        let capsule = AtomicPositionCapsule::new();
        let mut draft = CapsuleDraft::new();
        draft
            .set_head(PositionHeadWord {
                position_qty: -2,
                avg_px_ticks: 0,
                remaining_daily_loss_cents: 175_000,
                flags: 0,
            })
            .set_equity(EquityWord {
                realized_cents: 90_000,
                unrealized_cents: -12_500,
                peak_equity_cents: 120_000,
                trailing_draw_cents: 5_000,
            })
            .set_session(SessionWord {
                now_min_ct: 900,
                forbid_after_min_ct: 905,
                eod_flat_min_ct: 910,
                open_since_ms: 0,
                max_open_ms: 0,
                max_contracts: 8,
                max_per_trade_cents: 0,
                risk_flags: 0,
                reserved_bits: 0,
            })
            .set_tail(TailWord {
                symbol_id: 77,
                account_id: 1,
                last_exec_id: 0,
                breaker_level: 3,
                alt_health: 0,
                violation_bits: 0,
            });
        let snapshot = capsule.publish_draft(&draft);
        let converted = ApcSnapshot::from_capsule(&snapshot);
        assert_eq!(converted.position, -2);
        assert_eq!(converted.unreal_cents, -12_500);
        assert_eq!(converted.breaker_level, BreakerLevel::from_u8(3));
    }

    #[test]
    fn act_edge_conversion_uses_net_minus_floor() {
        let mut snapshot = ActSnapshot::empty();
        snapshot.net = FixedQ8_8::saturating_from_bp(4.25);
        snapshot.min_required = FixedQ8_8::saturating_from_bp(1.5);
        let edge = ActEdge::from_act_snapshot(&snapshot);
        assert_eq!(edge.edge_surplus_bp, 3);

        snapshot.net = FixedQ8_8::saturating_from_bp(-2.0);
        snapshot.min_required = FixedQ8_8::saturating_from_bp(0.5);
        let edge_neg = ActEdge::from_act_snapshot(&snapshot);
        assert_eq!(edge_neg.edge_surplus_bp, -3);
    }
}

/// Collects live feed sources and materialises [`FeedSnapshot`] instances.
#[derive(Clone)]
pub struct FeedAssembler {
    apc: Arc<dyn ApcFeed + Send + Sync>,
    act: Option<Arc<dyn ActFeed + Send + Sync>>,
    avs: Option<Arc<dyn AvsFeed + Send + Sync>>,
}

impl FeedAssembler {
    pub fn new(
        apc: Arc<dyn ApcFeed + Send + Sync>,
        act: Option<Arc<dyn ActFeed + Send + Sync>>,
        avs: Option<Arc<dyn AvsFeed + Send + Sync>>,
    ) -> Self {
        Self { apc, act, avs }
    }

    /// Build a snapshot for the provided policy and runtime gates.
    pub fn assemble(&self, policy: &SymbolPolicy, gates: SymbolGates) -> Option<FeedSnapshot> {
        let apc = self.apc.snapshot(policy.sym_id)?;
        let act = self.act.as_ref().and_then(|feed| feed.edge(policy.sym_id));
        let avs = self
            .avs
            .as_ref()
            .and_then(|feed| feed.snapshot(policy.sym_id));

        Some(FeedSnapshot {
            policy: policy.clone(),
            apc,
            act,
            avs,
            gates,
        })
    }

    pub fn apc_feed(&self) -> Arc<dyn ApcFeed + Send + Sync> {
        Arc::clone(&self.apc)
    }

    pub fn act_feed(&self) -> Option<Arc<dyn ActFeed + Send + Sync>> {
        self.act.as_ref().map(Arc::clone)
    }

    pub fn avs_feed(&self) -> Option<Arc<dyn AvsFeed + Send + Sync>> {
        self.avs.as_ref().map(Arc::clone)
    }
}

impl ApcSnapshot {
    pub fn from_capsule(snapshot: &atomic_position_capsule::Snapshot) -> Self {
        let head = snapshot.head();
        let equity = snapshot.equity();
        let tail = snapshot.tail();
        Self {
            position: head.position_qty,
            unreal_cents: equity.unrealized_cents,
            realized_cents: equity.realized_cents,
            rem_daily_loss_cents: head.remaining_daily_loss_cents,
            breaker_level: BreakerLevel::from_u8(tail.breaker_level),
        }
    }
}

impl ActEdge {
    pub fn from_act_snapshot(snapshot: &atomic_cost_tracker::ActSnapshot) -> Self {
        let diff = snapshot.net.to_bp() - snapshot.min_required.to_bp();
        let rounded = diff.round().clamp(i16::MIN as f64, i16::MAX as f64);
        Self {
            edge_surplus_bp: rounded as i16,
        }
    }
}
