use crate::estimator::{ActEstimator, EstimationInputs, FillFeedback, Route, SlipFeeSurface};
use crate::gate::{evaluate_gate, GateConfig, GateOutcome};
use crate::telemetry::{ActTelemetry, TelemetryKey, TelemetryReport};

/// Coordinates estimator publishing, gate evaluation, and telemetry for one symbol/route.
pub struct ActEngine {
    symbol: String,
    route: Route,
    estimator: ActEstimator,
    gate_config: GateConfig,
    telemetry: ActTelemetry,
}

impl ActEngine {
    pub fn new(
        symbol: impl Into<String>,
        route: Route,
        estimator: ActEstimator,
        gate_config: GateConfig,
        telemetry: ActTelemetry,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            route,
            estimator,
            gate_config,
            telemetry,
        }
    }

    pub fn gate_config(&self) -> &GateConfig {
        &self.gate_config
    }

    pub fn gate_config_mut(&mut self) -> &mut GateConfig {
        &mut self.gate_config
    }

    pub fn telemetry(&self) -> &ActTelemetry {
        &self.telemetry
    }

    pub fn telemetry_mut(&mut self) -> &mut ActTelemetry {
        &mut self.telemetry
    }

    pub fn telemetry_report(&self) -> TelemetryReport {
        self.telemetry.to_report()
    }

    pub fn telemetry_json_pretty(&self) -> serde_json::Result<String> {
        self.telemetry.to_json_pretty()
    }

    pub fn estimator(&self) -> &ActEstimator {
        &self.estimator
    }

    pub fn estimator_mut(&mut self) -> &mut ActEstimator {
        &mut self.estimator
    }

    pub fn latency_jitter_weight(&self) -> f64 {
        self.estimator.latency_jitter_weight()
    }

    /// Publish a new ACT snapshot and update telemetry.
    pub fn publish_snapshot(&mut self, size_bucket: u8, inputs: EstimationInputs) {
        debug_assert_eq!(inputs.intent.route, self.route);
        let snapshot = self.estimator.publish_snapshot(inputs);
        let key = TelemetryKey::new(&self.symbol, self.route, size_bucket);
        self.telemetry.record_snapshot(key, &snapshot);
    }

    /// Evaluate the gate on the latest snapshot.
    pub fn evaluate(&self) -> GateOutcome {
        evaluate_gate(self.estimator.slot().load_relaxed(), &self.gate_config)
    }

    /// Evaluate the gate with acquire semantics.
    pub fn evaluate_acquire(&self) -> GateOutcome {
        evaluate_gate(self.estimator.slot().load_acquire(), &self.gate_config)
    }

    /// Record a realized fill, updating the estimator coefficients and telemetry.
    pub fn record_fill(&mut self, size_bucket: u8, feedback: FillFeedback) {
        let key = TelemetryKey::new(&self.symbol, self.route, size_bucket);
        let predicted_slip_bp = self.estimator.surface().slip.predict(
            feedback.size_k(),
            feedback.vol_bp,
            feedback.spread_ticks,
            &feedback.latency,
        );
        let realized_slip_bp = feedback.realized_slip_bp;
        self.estimator.record_fill(feedback);
        self.telemetry
            .record_fill(key, predicted_slip_bp, realized_slip_bp);
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn route(&self) -> Route {
        self.route
    }

    pub fn set_surface(&mut self, surface: SlipFeeSurface) {
        self.estimator.set_surface(surface);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::estimator::{
        EstimatorConfig, FeeSchedule, LatencyTicket, OrderIntent, Side, SlipCoefficients,
        VenueSnapshot,
    };
    use crate::layout::ActFlags;

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

    #[test]
    fn publish_updates_snapshot_telemetry() {
        let mut engine = ActEngine::new(
            "MES",
            Route::Maker,
            estimator(Route::Maker),
            GateConfig::default(),
            ActTelemetry::default(),
        );

        engine.publish_snapshot(
            0,
            EstimationInputs {
                surface: surface(),
                venue: VenueSnapshot {
                    spread_ticks: 1.0,
                    microprice_offset_ticks: 0.5,
                    short_horizon_vol_bp: 0.3,
                },
                latency: LatencyTicket::default(),
                intent: intent(Route::Maker),
                age_ms: 25,
            },
        );

        let key = TelemetryKey::new("MES", Route::Maker, 0);
        let stats = engine.telemetry().snapshot_stats().get(&key).unwrap();
        assert_eq!(stats.samples, 1);
    }

    #[test]
    fn record_fill_updates_estimator_and_telemetry() {
        let mut engine = ActEngine::new(
            "MES",
            Route::Taker,
            estimator(Route::Taker),
            GateConfig::default(),
            ActTelemetry::default(),
        );

        engine.publish_snapshot(
            0,
            EstimationInputs {
                surface: surface(),
                venue: VenueSnapshot::default(),
                latency: LatencyTicket::default(),
                intent: intent(Route::Taker),
                age_ms: 0,
            },
        );

        let before = engine.estimator().surface().slip.a0;
        engine.record_fill(
            0,
            FillFeedback {
                surface_override: None,
                realized_slip_bp: 1.2,
                size: 1.0,
                size_normalizer: 1.0,
                vol_bp: 0.4,
                spread_ticks: 1.0,
                latency: LatencyTicket {
                    rtt_ms: 3.0,
                    jitter_ms: 1.0,
                },
            },
        );
        let after = engine.estimator().surface().slip.a0;
        assert_ne!(before, after);

        let key = TelemetryKey::new("MES", Route::Taker, 0);
        let fill_stats = engine.telemetry().fill_stats().get(&key).unwrap();
        assert_eq!(fill_stats.samples, 1);
    }

    #[test]
    fn evaluate_uses_latest_snapshot() {
        let mut engine = ActEngine::new(
            "MES",
            Route::Maker,
            estimator(Route::Maker),
            GateConfig {
                sigma_k: Some(0.5),
                reject_high_jitter: false,
                reject_wide_spread: false,
                reject_emergency_buffer: false,
            },
            ActTelemetry::default(),
        );

        engine.publish_snapshot(
            0,
            EstimationInputs {
                surface: surface(),
                venue: VenueSnapshot {
                    spread_ticks: 1.0,
                    microprice_offset_ticks: 0.75,
                    short_horizon_vol_bp: 0.3,
                },
                latency: LatencyTicket::default(),
                intent: intent(Route::Maker),
                age_ms: 10,
            },
        );

        let outcome = engine.evaluate();
        match outcome {
            GateOutcome::Allow(snapshot) => assert!(snapshot.flags.contains(ActFlags::OK)),
            GateOutcome::Deny(reason) => panic!("unexpected denial: {:?}", reason),
        }
    }
}
