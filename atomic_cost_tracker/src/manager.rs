use crate::engine::ActEngine;
use crate::estimator::{EstimationInputs, FillFeedback, Route, SlipFeeSurface};
use crate::gate::GateOutcome;
use crate::telemetry::{ActTelemetry, TelemetryReport};

use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct EngineKey {
    symbol: String,
    route: Route,
}

impl EngineKey {
    fn new(symbol: &str, route: Route) -> Self {
        Self {
            symbol: symbol.to_string(),
            route,
        }
    }
}

#[derive(Debug)]
pub enum ManagerError {
    EngineExists { symbol: String, route: Route },
    EngineMissing { symbol: String, route: Route },
}

impl fmt::Display for ManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManagerError::EngineExists { symbol, route } => {
                write!(f, "engine already registered for {} {:?}", symbol, route)
            }
            ManagerError::EngineMissing { symbol, route } => {
                write!(f, "engine missing for {} {:?}", symbol, route)
            }
        }
    }
}

impl std::error::Error for ManagerError {}

/// Manages multiple ACT engines keyed by symbol/route.
pub struct ActEngineManager {
    engines: HashMap<EngineKey, ActEngine>,
}

impl Default for ActEngineManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ActEngineManager {
    pub fn new() -> Self {
        Self {
            engines: HashMap::new(),
        }
    }

    pub fn register_engine(&mut self, engine: ActEngine) -> Result<(), ManagerError> {
        let key = EngineKey::new(engine.symbol(), engine.route());
        if self.engines.contains_key(&key) {
            return Err(ManagerError::EngineExists {
                symbol: engine.symbol().to_string(),
                route: engine.route(),
            });
        }
        self.engines.insert(key, engine);
        Ok(())
    }

    pub fn publish_snapshot(
        &mut self,
        symbol: &str,
        route: Route,
        size_bucket: u8,
        inputs: EstimationInputs,
    ) -> Result<(), ManagerError> {
        let key = EngineKey::new(symbol, route);
        let engine = self
            .engines
            .get_mut(&key)
            .ok_or_else(|| ManagerError::EngineMissing {
                symbol: symbol.to_string(),
                route,
            })?;
        engine.publish_snapshot(size_bucket, inputs);
        Ok(())
    }

    pub fn record_fill(
        &mut self,
        symbol: &str,
        route: Route,
        size_bucket: u8,
        feedback: FillFeedback,
    ) -> Result<(), ManagerError> {
        let key = EngineKey::new(symbol, route);
        let engine = self
            .engines
            .get_mut(&key)
            .ok_or_else(|| ManagerError::EngineMissing {
                symbol: symbol.to_string(),
                route,
            })?;
        engine.record_fill(size_bucket, feedback);
        Ok(())
    }

    pub fn set_surface(
        &mut self,
        symbol: &str,
        route: Route,
        surface: SlipFeeSurface,
    ) -> Result<(), ManagerError> {
        let key = EngineKey::new(symbol, route);
        let engine = self
            .engines
            .get_mut(&key)
            .ok_or_else(|| ManagerError::EngineMissing {
                symbol: symbol.to_string(),
                route,
            })?;
        engine.set_surface(surface);
        Ok(())
    }

    pub fn latency_jitter_weight(&self, symbol: &str, route: Route) -> Result<f64, ManagerError> {
        let key = EngineKey::new(symbol, route);
        let engine = self
            .engines
            .get(&key)
            .ok_or_else(|| ManagerError::EngineMissing {
                symbol: symbol.to_string(),
                route,
            })?;
        Ok(engine.latency_jitter_weight())
    }

    pub fn evaluate(&self, symbol: &str, route: Route) -> Result<GateOutcome, ManagerError> {
        let key = EngineKey::new(symbol, route);
        let engine = self
            .engines
            .get(&key)
            .ok_or_else(|| ManagerError::EngineMissing {
                symbol: symbol.to_string(),
                route,
            })?;
        Ok(engine.evaluate())
    }

    pub fn evaluate_acquire(
        &self,
        symbol: &str,
        route: Route,
    ) -> Result<GateOutcome, ManagerError> {
        let key = EngineKey::new(symbol, route);
        let engine = self
            .engines
            .get(&key)
            .ok_or_else(|| ManagerError::EngineMissing {
                symbol: symbol.to_string(),
                route,
            })?;
        Ok(engine.evaluate_acquire())
    }

    pub fn telemetry_report(&self) -> TelemetryReport {
        let mut aggregate = ActTelemetry::default();
        for engine in self.engines.values() {
            aggregate.merge(engine.telemetry());
        }
        aggregate.to_report()
    }

    pub fn telemetry_json_pretty(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(&self.telemetry_report())
    }

    pub fn engines(&self) -> impl Iterator<Item = (&str, Route)> {
        self.engines
            .keys()
            .map(|key| (key.symbol.as_str(), key.route))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ActEngine;
    use crate::estimator::{
        ActEstimator, EstimatorConfig, FeeSchedule, LatencyTicket, OrderIntent, Side,
        SlipCoefficients, SlipFeeSurface, VenueSnapshot,
    };
    use crate::gate::GateConfig;

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
        let slot = crate::writer::ActSlot::default();
        ActEstimator::new(
            slot,
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

    fn engine(symbol: &str, route: Route) -> ActEngine {
        ActEngine::new(
            symbol,
            route,
            estimator(route),
            GateConfig::default(),
            ActTelemetry::default(),
        )
    }

    #[test]
    fn registers_and_publishes() {
        let mut manager = ActEngineManager::new();
        manager
            .register_engine(engine("MES", Route::Maker))
            .expect("register");

        manager
            .publish_snapshot(
                "MES",
                Route::Maker,
                0,
                EstimationInputs {
                    surface: surface(),
                    venue: VenueSnapshot::default(),
                    latency: LatencyTicket::default(),
                    intent: intent(Route::Maker),
                    age_ms: 0,
                },
            )
            .expect("publish");

        let report = manager.telemetry_report();
        assert_eq!(report.snapshots.len(), 1);
    }

    #[test]
    fn record_fill_updates_report() {
        let mut manager = ActEngineManager::new();
        manager
            .register_engine(engine("MES", Route::Taker))
            .expect("register");

        manager
            .publish_snapshot(
                "MES",
                Route::Taker,
                0,
                EstimationInputs {
                    surface: surface(),
                    venue: VenueSnapshot::default(),
                    latency: LatencyTicket::default(),
                    intent: intent(Route::Taker),
                    age_ms: 0,
                },
            )
            .expect("publish");

        manager
            .record_fill(
                "MES",
                Route::Taker,
                0,
                FillFeedback {
                    surface_override: None,
                    realized_slip_bp: 1.0,
                    size: 1.0,
                    size_normalizer: 1.0,
                    vol_bp: 0.5,
                    spread_ticks: 1.0,
                    latency: LatencyTicket {
                        rtt_ms: 3.0,
                        jitter_ms: 1.0,
                    },
                },
            )
            .expect("record fill");

        let report = manager.telemetry_report();
        assert_eq!(report.fills.len(), 1);
        assert!(manager.telemetry_json_pretty().unwrap().contains("MES"));
    }

    #[test]
    fn duplicate_registration_fails() {
        let mut manager = ActEngineManager::new();
        manager
            .register_engine(engine("MES", Route::Maker))
            .expect("register");
        let err = manager
            .register_engine(engine("MES", Route::Maker))
            .expect_err("duplicate");
        match err {
            ManagerError::EngineExists { symbol, .. } => assert_eq!(symbol, "MES"),
            _ => panic!("unexpected error"),
        }
    }

    #[test]
    fn missing_engine_yields_error() {
        let mut manager = ActEngineManager::new();
        let err = manager
            .publish_snapshot(
                "MES",
                Route::Maker,
                0,
                EstimationInputs {
                    surface: surface(),
                    venue: VenueSnapshot::default(),
                    latency: LatencyTicket::default(),
                    intent: intent(Route::Maker),
                    age_ms: 0,
                },
            )
            .expect_err("missing");
        match err {
            ManagerError::EngineMissing { symbol, .. } => assert_eq!(symbol, "MES"),
            _ => panic!("unexpected error"),
        }
    }
}
