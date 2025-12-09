use crate::estimator::{FillFeedback, OrderIntent, Route};
use crate::events::{ActEvent, ActEventResult};
use crate::gate::GateOutcome;
use crate::service::{ActService, ServiceError, TelemetrySink};
use atomic_slip_fee_surface::AsfSnapshot;

use std::time::{Duration, Instant};

/// Order request sent into the ACT pipeline.
#[derive(Clone, Debug)]
pub struct OrderRequest {
    pub symbol: String,
    pub route: Route,
    pub size_bucket: u8,
    pub intent: OrderIntent,
}

impl OrderRequest {
    pub fn new(
        symbol: impl Into<String>,
        route: Route,
        size_bucket: u8,
        intent: OrderIntent,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            route,
            size_bucket,
            intent,
        }
    }
}

/// Router interface that receives gate outcomes for each order request.
pub trait StrategyRouter {
    fn handle_gate(&mut self, request: &OrderRequest, outcome: GateOutcome);
}

/// Coordinates feed updates, order intents, and telemetry flushing.
pub struct ActCoordinator<S: TelemetrySink, R: StrategyRouter> {
    service: ActService<S>,
    router: R,
    flush_interval: Duration,
    last_flush: Instant,
}

impl<S: TelemetrySink, R: StrategyRouter> ActCoordinator<S, R> {
    pub fn new(service: ActService<S>, router: R, flush_interval: Duration) -> Self {
        let now = Instant::now();
        Self {
            service,
            router,
            flush_interval,
            last_flush: now.checked_sub(flush_interval).unwrap_or(now),
        }
    }

    pub fn service(&self) -> &ActService<S> {
        &self.service
    }

    pub fn service_mut(&mut self) -> &mut ActService<S> {
        &mut self.service
    }

    pub fn router(&mut self) -> &mut R {
        &mut self.router
    }

    pub fn on_surface(
        &mut self,
        symbol: &str,
        route: Route,
        surface: crate::SlipFeeSurface,
    ) -> Result<(), ServiceError> {
        self.service.update_surface(symbol, route, surface)
    }

    pub fn on_surface_snapshot(
        &mut self,
        symbol: &str,
        route: Route,
        snapshot: &AsfSnapshot,
    ) -> Result<(), ServiceError> {
        self.service
            .update_surface_from_asf(symbol, route, snapshot)
    }

    pub fn on_venue(
        &mut self,
        symbol: &str,
        route: Route,
        venue: crate::VenueSnapshot,
    ) -> Result<(), ServiceError> {
        self.service.update_venue(symbol, route, venue)
    }

    pub fn on_latency(
        &mut self,
        symbol: &str,
        route: Route,
        latency: crate::LatencyTicket,
    ) -> Result<(), ServiceError> {
        self.service.update_latency(symbol, route, latency)
    }

    pub fn on_fill(
        &mut self,
        symbol: &str,
        route: Route,
        size_bucket: u8,
        feedback: FillFeedback,
    ) -> Result<(), ServiceError> {
        self.service
            .record_fill(symbol, route, size_bucket, feedback)
    }

    pub fn on_order(&mut self, request: OrderRequest) -> Result<GateOutcome, ServiceError> {
        self.service.publish_intent(
            &request.symbol,
            request.route,
            request.size_bucket,
            request.intent,
        )?;
        let outcome = self.service.evaluate(&request.symbol, request.route)?;
        self.router.handle_gate(&request, outcome);
        Ok(outcome)
    }

    pub fn tick(&mut self) -> Result<(), ServiceError> {
        let now = Instant::now();
        self.tick_at(now)
    }

    pub fn tick_at(&mut self, now: Instant) -> Result<(), ServiceError> {
        if now.duration_since(self.last_flush) >= self.flush_interval {
            self.service.flush_telemetry()?;
            self.last_flush = now;
        }
        Ok(())
    }

    pub fn handle_event(&mut self, event: ActEvent) -> Result<ActEventResult, ServiceError> {
        match event {
            ActEvent::Surface {
                symbol,
                route,
                surface,
            } => {
                self.on_surface(&symbol, route, surface)?;
                Ok(ActEventResult::None)
            }
            ActEvent::SurfaceAsf {
                symbol,
                route,
                snapshot,
            } => {
                self.on_surface_snapshot(&symbol, route, &snapshot)?;
                Ok(ActEventResult::None)
            }
            ActEvent::Venue {
                symbol,
                route,
                venue,
            } => {
                self.on_venue(&symbol, route, venue)?;
                Ok(ActEventResult::None)
            }
            ActEvent::Latency {
                symbol,
                route,
                latency,
            } => {
                self.on_latency(&symbol, route, latency)?;
                Ok(ActEventResult::None)
            }
            ActEvent::Order(request) => {
                let outcome = self.on_order(request)?;
                Ok(ActEventResult::Gate(outcome))
            }
            ActEvent::Fill {
                symbol,
                route,
                size_bucket,
                feedback,
            } => {
                self.on_fill(&symbol, route, size_bucket, feedback)?;
                Ok(ActEventResult::None)
            }
            ActEvent::Flush => {
                self.service.flush_telemetry()?;
                Ok(ActEventResult::None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ActEngine;
    use crate::estimator::{
        ActEstimator, EstimatorConfig, FeeSchedule, FillFeedback, LatencyTicket, OrderIntent, Side,
        SlipCoefficients, SlipFeeSurface, VenueSnapshot,
    };
    use crate::events::{ActEvent, ActEventResult};
    use crate::gate::GateConfig;
    use crate::manager::ActEngineManager;
    use crate::service::{ActService, NoopTelemetrySink};
    use crate::telemetry::ActTelemetry;
    use atomic_slip_fee_surface::{flag, AsfPacked, AsfSnapshotBuilder, LanePublish};

    use std::sync::Arc;
    use std::sync::Mutex;

    #[derive(Default, Clone)]
    struct RecordingRouter {
        outcomes: Arc<Mutex<Vec<GateOutcome>>>,
    }

    impl StrategyRouter for RecordingRouter {
        fn handle_gate(&mut self, _request: &OrderRequest, outcome: GateOutcome) {
            self.outcomes.lock().unwrap().push(outcome);
        }
    }

    fn surface() -> SlipFeeSurface {
        SlipFeeSurface {
            fees: FeeSchedule {
                maker_fee_bp: 0.05,
                taker_fee_bp: 0.25,
                exchange_misc_bp: 0.05,
            },
            slip: SlipCoefficients {
                a0: 0.15,
                a1: 0.04,
                a2: 0.01,
                b1: 0.2,
                b2: 0.1,
                c1: 0.01,
                c2: 0.02,
                size_scale: 1.0,
                clip_min_bp: 0.0,
                clip_max_bp: 10.0,
            },
        }
    }

    fn estimator(route: Route) -> ActEstimator {
        let _ = route;
        ActEstimator::new(
            crate::writer::ActSlot::default(),
            surface(),
            EstimatorConfig {
                safety_buffer_bp: 0.3,
                sigma_alpha: 0.2,
                sigma_init_bp: 0.4,
                sigma_clip_bp: 5.0,
                slip_alpha: 0.1,
                version: 1,
                age_bucket_ms: 100,
                high_jitter_cutoff_ms: 10.0,
                wide_spread_cutoff: 5.0,
                ok_sigma_k: Some(0.5),
                latency_jitter_weight: 0.5,
            },
        )
    }

    fn engine(symbol: &str, route: Route) -> ActEngine {
        ActEngine::new(
            symbol,
            route,
            estimator(route),
            GateConfig::default(),
            ActTelemetry::default(),
        )
    }

    fn order(route: Route) -> OrderRequest {
        OrderRequest::new(
            "MES",
            route,
            0,
            OrderIntent {
                side: Side::Buy,
                route,
                size: 1.0,
                size_normalizer: 1.0,
                price: 4_000.0,
                tick_size: 0.25,
                gross_edge_signal_bp: Some(3.0),
            },
        )
    }

    fn service() -> ActService<NoopTelemetrySink> {
        let mut manager = ActEngineManager::new();
        manager
            .register_engine(engine("MES", Route::Maker))
            .expect("register");
        let mut service = ActService::new(manager, NoopTelemetrySink::default());
        service
            .update_surface("MES", Route::Maker, surface())
            .expect("surface");
        service
            .update_venue("MES", Route::Maker, VenueSnapshot::default())
            .expect("venue");
        service
            .update_latency("MES", Route::Maker, LatencyTicket::default())
            .expect("latency");
        service
    }

    #[test]
    fn routes_gate_outcomes_to_router() {
        let service = service();
        let router = RecordingRouter::default();
        let mut coordinator = ActCoordinator::new(service, router.clone(), Duration::from_secs(60));

        coordinator.on_order(order(Route::Maker)).expect("order");

        let outcomes = router.outcomes.lock().unwrap();
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0], GateOutcome::Allow(_)));
    }

    #[test]
    fn tick_flushes_telemetry_when_due() {
        let service = service();
        let router = RecordingRouter::default();
        let mut coordinator = ActCoordinator::new(service, router, Duration::from_millis(1));

        let now = Instant::now();
        coordinator.tick_at(now).expect("tick");
        let later = now + Duration::from_millis(5);
        coordinator.tick_at(later).expect("tick");
    }

    #[test]
    fn handle_event_dispatches() {
        let mut coordinator = ActCoordinator::new(
            service(),
            RecordingRouter::default(),
            Duration::from_secs(1),
        );

        let surface_event = ActEvent::Surface {
            symbol: "MES".into(),
            route: Route::Maker,
            surface: surface(),
        };
        coordinator.handle_event(surface_event).expect("surface");

        let snapshot_publish = AsfSnapshotBuilder::builder()
            .with_size_scale(1.0)
            .with_maker_fee_bp(0.05)
            .with_taker_fee_bp(0.25)
            .with_misc_fee_bp(0.05)
            .with_shared_vol_coeff(0.3)
            .with_shared_spread_coeff(0.2)
            .with_flags(flag::HAS_DATA_M | flag::HAS_DATA_T)
            .with_maker_lane(|_| LanePublish {
                intercept_bp: 0.3,
                size_linear_bp: 0.1,
                size_quadratic_bp: 0.02,
                latency_coeff_bp: 0.05,
                slip_cap_bp: 8.0,
            })
            .with_taker_lane(|_| LanePublish {
                intercept_bp: 0.4,
                size_linear_bp: 0.12,
                size_quadratic_bp: 0.03,
                latency_coeff_bp: 0.06,
                slip_cap_bp: 9.0,
            })
            .build();
        let snapshot = AsfPacked::from_publish(&snapshot_publish).snapshot();
        let surface_asf_event = ActEvent::SurfaceAsf {
            symbol: "MES".into(),
            route: Route::Maker,
            snapshot,
        };
        coordinator
            .handle_event(surface_asf_event)
            .expect("surface_asf");

        let venue_event = ActEvent::Venue {
            symbol: "MES".into(),
            route: Route::Maker,
            venue: VenueSnapshot::default(),
        };
        coordinator.handle_event(venue_event).expect("venue");

        let latency_event = ActEvent::Latency {
            symbol: "MES".into(),
            route: Route::Maker,
            latency: LatencyTicket::default(),
        };
        coordinator.handle_event(latency_event).expect("latency");

        let order_event = ActEvent::Order(order(Route::Maker));
        let outcome = coordinator.handle_event(order_event).expect("order");
        assert!(matches!(
            outcome,
            ActEventResult::Gate(GateOutcome::Allow(_))
        ));

        let fill_event = ActEvent::Fill {
            symbol: "MES".into(),
            route: Route::Maker,
            size_bucket: 0,
            feedback: FillFeedback {
                surface_override: None,
                realized_slip_bp: 1.0,
                size: 1.0,
                size_normalizer: 1.0,
                vol_bp: 0.3,
                spread_ticks: 1.0,
                latency: LatencyTicket::default(),
            },
        };
        coordinator.handle_event(fill_event).expect("fill");

        let flush_event = ActEvent::Flush;
        coordinator.handle_event(flush_event).expect("flush");
    }
}
