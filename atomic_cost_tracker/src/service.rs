use crate::estimator::{
    EstimationInputs, FillFeedback, LatencyTicket, OrderIntent, Route, SlipFeeSurface,
    VenueSnapshot,
};
use crate::gate::GateOutcome;
use crate::manager::{ActEngineManager, ManagerError};
use crate::telemetry::TelemetryReport;
use atomic_slip_fee_surface::AsfSnapshot;

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::time::{Duration, Instant};

/// Sink that consumes telemetry reports for persistence/auditing.
pub trait TelemetrySink {
    fn publish(&mut self, report: &TelemetryReport) -> Result<(), Box<dyn StdError + Send + Sync>>;
}

/// No-op telemetry sink for tests and benchmarks.
#[derive(Default)]
pub struct NoopTelemetrySink;

impl TelemetrySink for NoopTelemetrySink {
    fn publish(
        &mut self,
        _report: &TelemetryReport,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        Ok(())
    }
}

#[derive(Debug)]
pub enum ServiceError {
    Manager(ManagerError),
    MissingSurface { symbol: String, route: Route },
    MissingVenue { symbol: String, route: Route },
    MissingLatency { symbol: String, route: Route },
    Telemetry(Box<dyn StdError + Send + Sync>),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceError::Manager(err) => write!(f, "manager error: {}", err),
            ServiceError::MissingSurface { symbol, route } => {
                write!(f, "missing surface for {} {:?}", symbol, route)
            }
            ServiceError::MissingVenue { symbol, route } => {
                write!(f, "missing venue for {} {:?}", symbol, route)
            }
            ServiceError::MissingLatency { symbol, route } => {
                write!(f, "missing latency for {} {:?}", symbol, route)
            }
            ServiceError::Telemetry(err) => write!(f, "telemetry sink error: {}", err),
        }
    }
}

impl std::error::Error for ServiceError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            ServiceError::Manager(err) => Some(err),
            ServiceError::Telemetry(err) => Some(&**err),
            _ => None,
        }
    }
}

impl From<ManagerError> for ServiceError {
    fn from(value: ManagerError) -> Self {
        Self::Manager(value)
    }
}

impl From<Box<dyn StdError + Send + Sync>> for ServiceError {
    fn from(value: Box<dyn StdError + Send + Sync>) -> Self {
        Self::Telemetry(value)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct CapsuleState {
    surface: Option<SlipFeeSurface>,
    venue: Option<VenueSnapshot>,
    latency: Option<LatencyTicket>,
    last_publish: Option<Instant>,
}

impl CapsuleState {
    fn ensure_surface(&self, symbol: &str, route: Route) -> Result<SlipFeeSurface, ServiceError> {
        self.surface.ok_or_else(|| ServiceError::MissingSurface {
            symbol: symbol.to_string(),
            route,
        })
    }

    fn ensure_venue(&self, symbol: &str, route: Route) -> Result<VenueSnapshot, ServiceError> {
        self.venue.ok_or_else(|| ServiceError::MissingVenue {
            symbol: symbol.to_string(),
            route,
        })
    }

    fn ensure_latency(&self, symbol: &str, route: Route) -> Result<LatencyTicket, ServiceError> {
        self.latency.ok_or_else(|| ServiceError::MissingLatency {
            symbol: symbol.to_string(),
            route,
        })
    }

    fn update_surface(&mut self, surface: SlipFeeSurface) {
        self.surface = Some(surface);
    }

    fn update_venue(&mut self, venue: VenueSnapshot) {
        self.venue = Some(venue);
    }

    fn update_latency(&mut self, latency: LatencyTicket) {
        self.latency = Some(latency);
    }

    fn mark_published(&mut self) {
        self.last_publish = Some(Instant::now());
    }

    fn age_ms(&self) -> u32 {
        self.last_publish
            .map(|t| t.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0))
            .as_millis()
            .min(u32::MAX as u128) as u32
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct SymbolRouteKey {
    symbol: String,
    route: Route,
}

impl SymbolRouteKey {
    fn new(symbol: &str, route: Route) -> Self {
        Self {
            symbol: symbol.to_string(),
            route,
        }
    }
}

/// High-level service that ingests ASF/AVS/ALT capsules, publishes ACT snapshots,
/// and periodically flushes telemetry.
pub struct ActService<S: TelemetrySink> {
    manager: ActEngineManager,
    capsules: HashMap<SymbolRouteKey, CapsuleState>,
    sink: S,
}

impl<S: TelemetrySink> ActService<S> {
    pub fn new(manager: ActEngineManager, sink: S) -> Self {
        Self {
            manager,
            capsules: HashMap::new(),
            sink,
        }
    }

    pub fn manager(&self) -> &ActEngineManager {
        &self.manager
    }

    pub fn manager_mut(&mut self) -> &mut ActEngineManager {
        &mut self.manager
    }

    fn capsule_entry_mut(&mut self, symbol: &str, route: Route) -> &mut CapsuleState {
        self.capsules
            .entry(SymbolRouteKey::new(symbol, route))
            .or_default()
    }

    pub fn update_surface(
        &mut self,
        symbol: &str,
        route: Route,
        surface: SlipFeeSurface,
    ) -> Result<(), ServiceError> {
        let entry = self.capsule_entry_mut(symbol, route);
        entry.update_surface(surface);
        self.manager.set_surface(symbol, route, surface)?;
        Ok(())
    }

    pub fn update_surface_from_asf(
        &mut self,
        symbol: &str,
        route: Route,
        snapshot: &AsfSnapshot,
    ) -> Result<(), ServiceError> {
        let jitter_weight = self.manager.latency_jitter_weight(symbol, route)?;
        let surface = SlipFeeSurface::from_asf_snapshot(snapshot, route, jitter_weight);
        self.update_surface(symbol, route, surface)
    }

    pub fn update_venue(
        &mut self,
        symbol: &str,
        route: Route,
        venue: VenueSnapshot,
    ) -> Result<(), ServiceError> {
        let entry = self.capsule_entry_mut(symbol, route);
        entry.update_venue(venue);
        Ok(())
    }

    pub fn update_latency(
        &mut self,
        symbol: &str,
        route: Route,
        latency: LatencyTicket,
    ) -> Result<(), ServiceError> {
        let entry = self.capsule_entry_mut(symbol, route);
        entry.update_latency(latency);
        Ok(())
    }

    pub fn publish_intent(
        &mut self,
        symbol: &str,
        route: Route,
        size_bucket: u8,
        intent: OrderIntent,
    ) -> Result<(), ServiceError> {
        let entry = self
            .capsules
            .get_mut(&SymbolRouteKey::new(symbol, route))
            .ok_or_else(|| ServiceError::MissingSurface {
                symbol: symbol.to_string(),
                route,
            })?;

        let surface = entry.ensure_surface(symbol, route)?;
        let venue = entry.ensure_venue(symbol, route)?;
        let latency = entry.ensure_latency(symbol, route)?;
        let age_ms = entry.age_ms();

        self.manager.publish_snapshot(
            symbol,
            route,
            size_bucket,
            EstimationInputs {
                surface,
                venue,
                latency,
                intent,
                age_ms,
            },
        )?;

        entry.mark_published();
        Ok(())
    }

    pub fn record_fill(
        &mut self,
        symbol: &str,
        route: Route,
        size_bucket: u8,
        feedback: FillFeedback,
    ) -> Result<(), ServiceError> {
        self.manager
            .record_fill(symbol, route, size_bucket, feedback)?;
        Ok(())
    }

    pub fn evaluate(&self, symbol: &str, route: Route) -> Result<GateOutcome, ServiceError> {
        Ok(self.manager.evaluate(symbol, route)?)
    }

    pub fn evaluate_acquire(
        &self,
        symbol: &str,
        route: Route,
    ) -> Result<GateOutcome, ServiceError> {
        Ok(self.manager.evaluate_acquire(symbol, route)?)
    }

    pub fn flush_telemetry(&mut self) -> Result<(), ServiceError> {
        let report = self.manager.telemetry_report();
        self.sink.publish(&report)?;
        Ok(())
    }

    pub fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ActEngine;
    use crate::estimator::{
        ActEstimator, EstimatorConfig, FeeSchedule, LatencyTicket, Side, SlipCoefficients,
        VenueSnapshot,
    };
    use crate::gate::GateConfig;
    use crate::telemetry::{ActTelemetry, SnapshotStats};
    use atomic_slip_fee_surface::{flag, AsfPacked, AsfSnapshotBuilder, LanePublish};

    #[derive(Default)]
    struct TestSink {
        reports: Vec<TelemetryReport>,
    }

    impl TelemetrySink for TestSink {
        fn publish(
            &mut self,
            report: &TelemetryReport,
        ) -> Result<(), Box<dyn super::StdError + Send + Sync>> {
            self.reports.push(report.clone());
            Ok(())
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

    fn intent(route: Route) -> OrderIntent {
        OrderIntent {
            side: Side::Buy,
            route,
            size: 1.0,
            size_normalizer: 1.0,
            price: 4_000.0,
            tick_size: 0.25,
            gross_edge_signal_bp: Some(3.0),
        }
    }

    #[test]
    fn publishes_using_cached_capsules() {
        let mut manager = ActEngineManager::new();
        manager
            .register_engine(engine("MES", Route::Maker))
            .expect("register");

        let sink = TestSink::default();
        let mut service = ActService::new(manager, sink);

        service
            .update_surface("MES", Route::Maker, surface())
            .expect("surface");
        service
            .update_venue(
                "MES",
                Route::Maker,
                VenueSnapshot {
                    spread_ticks: 1.0,
                    microprice_offset_ticks: 0.5,
                    short_horizon_vol_bp: 0.3,
                },
            )
            .expect("venue");
        service
            .update_latency(
                "MES",
                Route::Maker,
                LatencyTicket {
                    rtt_ms: 2.5,
                    jitter_ms: 0.8,
                },
            )
            .expect("latency");

        service
            .publish_intent("MES", Route::Maker, 0, intent(Route::Maker))
            .expect("publish");

        let report = service.manager().telemetry_report();
        assert_eq!(report.snapshots.len(), 1);
        let SnapshotStats { samples, .. } = report.snapshots[0].stats;
        assert_eq!(samples, 1);
    }

    #[test]
    fn flushes_telemetry_to_sink() {
        let mut manager = ActEngineManager::new();
        manager
            .register_engine(engine("MES", Route::Maker))
            .expect("register");

        let sink = TestSink::default();
        let mut service = ActService::new(manager, sink);
        service
            .sink_mut()
            .publish(&TelemetryReport {
                snapshots: vec![],
                fills: vec![],
            })
            .expect("direct publish");

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
            .publish_intent("MES", Route::Maker, 0, intent(Route::Maker))
            .expect("publish");

        service.flush_telemetry().expect("flush");
        assert!(!service.sink_mut().reports.is_empty());
    }

    #[test]
    fn update_surface_from_asf_applies_snapshot() {
        let mut manager = ActEngineManager::new();
        manager
            .register_engine(engine("MES", Route::Maker))
            .expect("register");

        let sink = TestSink::default();
        let mut service = ActService::new(manager, sink);

        let snapshot_publish = AsfSnapshotBuilder::builder()
            .with_size_scale(0.5)
            .with_maker_fee_bp(0.1)
            .with_taker_fee_bp(0.2)
            .with_misc_fee_bp(0.05)
            .with_shared_vol_coeff(0.4)
            .with_shared_spread_coeff(0.25)
            .with_flags(flag::HAS_DATA_M)
            .with_maker_lane(|_| LanePublish {
                intercept_bp: 0.3,
                size_linear_bp: 0.12,
                size_quadratic_bp: 0.02,
                latency_coeff_bp: 0.05,
                slip_cap_bp: 7.0,
            })
            .with_taker_lane(|_| LanePublish {
                intercept_bp: 0.35,
                size_linear_bp: 0.1,
                size_quadratic_bp: 0.01,
                latency_coeff_bp: 0.06,
                slip_cap_bp: 8.0,
            })
            .build();
        let snapshot = AsfPacked::from_publish(&snapshot_publish).snapshot();

        service
            .update_surface_from_asf("MES", Route::Maker, &snapshot)
            .expect("surface asf");
        let venue = VenueSnapshot {
            spread_ticks: 2.0,
            microprice_offset_ticks: 0.0,
            short_horizon_vol_bp: 1.5,
        };
        service
            .update_venue("MES", Route::Maker, venue)
            .expect("venue");
        let latency = LatencyTicket {
            rtt_ms: 4.0,
            jitter_ms: 1.0,
        };
        service
            .update_latency("MES", Route::Maker, latency)
            .expect("latency");

        service
            .publish_intent("MES", Route::Maker, 0, intent(Route::Maker))
            .expect("publish");

        let report = service.manager().telemetry_report();
        let entry = report
            .snapshots
            .iter()
            .find(|entry| entry.key.symbol == "MES" && entry.key.route == Route::Maker)
            .expect("telemetry entry");
        assert_eq!(entry.stats.samples, 1);
        let avg_slip_bp = entry.stats.slip_bp_sum / entry.stats.samples as f64;

        let expected_surface = SlipFeeSurface::from_asf_snapshot(&snapshot, Route::Maker, 0.5);
        let expected_slip_bp = expected_surface.slip.predict(
            intent(Route::Maker).size_k(),
            venue.short_horizon_vol_bp,
            venue.spread_ticks,
            &latency,
        );

        assert!((avg_slip_bp - expected_slip_bp).abs() < 1e-3);
    }

    #[test]
    fn missing_capsule_data_errors() {
        let mut manager = ActEngineManager::new();
        manager
            .register_engine(engine("MES", Route::Maker))
            .expect("register");
        let mut service = ActService::new(manager, NoopTelemetrySink::default());

        let err = service
            .publish_intent("MES", Route::Maker, 0, intent(Route::Maker))
            .expect_err("expected missing surface");
        assert!(matches!(err, ServiceError::MissingSurface { .. }));
    }
}
