use crate::gate::{evaluate_gate, GateConfig, GateOutcome};
use crate::writer::ActSlot;

/// Lightweight helper exposing the hot-path gate for readers.
pub struct StrategyGate<'a> {
    slot: &'a ActSlot,
    config: GateConfig,
}

impl<'a> StrategyGate<'a> {
    pub fn new(slot: &'a ActSlot, config: GateConfig) -> Self {
        Self { slot, config }
    }

    pub fn config(&self) -> &GateConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut GateConfig {
        &mut self.config
    }

    /// Evaluate the ACT snapshot using relaxed semantics.
    pub fn evaluate(&self) -> GateOutcome {
        evaluate_gate(self.slot.load_relaxed(), &self.config)
    }

    /// Evaluate the ACT snapshot using acquire ordering.
    pub fn evaluate_acquire(&self) -> GateOutcome {
        evaluate_gate(self.slot.load_acquire(), &self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::estimator::{
        ActEstimator, EstimationInputs, EstimatorConfig, FeeSchedule, LatencyTicket, OrderIntent,
        Route, Side, SlipCoefficients, SlipFeeSurface, VenueSnapshot,
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

    fn estimator() -> ActEstimator {
        ActEstimator::new(
            ActSlot::default(),
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

    #[test]
    fn gate_wrapper_delegates_to_evaluate_gate() {
        let mut estimator = estimator();
        estimator.publish_snapshot(EstimationInputs {
            surface: surface(),
            venue: VenueSnapshot {
                spread_ticks: 1.0,
                microprice_offset_ticks: 0.75,
                short_horizon_vol_bp: 0.3,
            },
            latency: LatencyTicket::default(),
            intent: OrderIntent {
                side: Side::Buy,
                route: Route::Maker,
                size: 1.0,
                size_normalizer: 1.0,
                price: 4_000.0,
                tick_size: 0.25,
                gross_edge_signal_bp: Some(2.5),
            },
            age_ms: 42,
        });

        let mut gate = StrategyGate::new(
            estimator.slot(),
            GateConfig {
                sigma_k: Some(0.5),
                reject_high_jitter: false,
                reject_wide_spread: false,
                reject_emergency_buffer: false,
            },
        );

        let outcome = gate.evaluate();
        match outcome {
            GateOutcome::Allow(snapshot) => assert!(snapshot.flags.contains(ActFlags::OK)),
            GateOutcome::Deny(reason) => panic!("unexpected denial: {:?}", reason),
        }

        gate.config_mut().reject_high_jitter = true;
        let outcome_acquire = gate.evaluate_acquire();
        assert!(matches!(outcome_acquire, GateOutcome::Allow(_)));
    }
}
