use crate::layout::{ActFlags, ActSnapshot, FixedQ8_8};
use crate::writer::ActSlot;
use atomic_latency_ticket::AltSnapshot;
use atomic_slip_fee_surface::AsfSnapshot;
use serde::{Deserialize, Serialize};

const EPS: f64 = 1e-6;

/// Static taker/maker fee schedule expressed in basis points.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FeeSchedule {
    pub maker_fee_bp: f64,
    pub taker_fee_bp: f64,
    pub exchange_misc_bp: f64,
}

impl FeeSchedule {
    pub fn fees_bp(self, route: Route) -> f64 {
        let route_fee = match route {
            Route::Maker => self.maker_fee_bp,
            Route::Taker => self.taker_fee_bp,
        };
        route_fee + self.exchange_misc_bp
    }
}

/// Coefficients driving the slip model for ACT-128.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SlipCoefficients {
    pub a0: f64,
    pub a1: f64,
    pub a2: f64,
    pub b1: f64,
    pub b2: f64,
    pub c1: f64,
    pub c2: f64,
    pub size_scale: f64,
    pub clip_min_bp: f64,
    pub clip_max_bp: f64,
}

impl SlipCoefficients {
    pub fn predict(
        &self,
        size_k: f64,
        vol_bp: f64,
        spread_ticks: f64,
        latency: &LatencyTicket,
    ) -> f64 {
        let size_norm = size_k * self.size_scale;
        let size_sq = size_norm * size_norm;
        let mut slip = self.a0
            + self.a1 * size_norm
            + self.a2 * size_sq
            + self.b1 * vol_bp
            + self.b2 * spread_ticks
            + self.c1 * latency.rtt_ms
            + self.c2 * latency.jitter_ms;
        slip = slip.clamp(self.clip_min_bp, self.clip_max_bp);
        slip
    }
}

/// Aggregated fee/slip surface pushed in from ASF-256.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SlipFeeSurface {
    pub fees: FeeSchedule,
    pub slip: SlipCoefficients,
}

impl SlipFeeSurface {
    pub fn from_asf_snapshot(snapshot: &AsfSnapshot, route: Route, jitter_weight: f64) -> Self {
        let fees = FeeSchedule {
            maker_fee_bp: snapshot.maker_fee_bp as f64,
            taker_fee_bp: snapshot.taker_fee_bp as f64,
            exchange_misc_bp: snapshot.misc_fee_bp as f64,
        };

        let lane = match route {
            Route::Maker => snapshot.maker,
            Route::Taker => snapshot.taker,
        };

        let latency_coeff = lane.latency_coeff_bp as f64;
        let jitter_coeff = latency_coeff * jitter_weight;
        let size_scale = if snapshot.size_scale <= 0.0 {
            1.0
        } else {
            snapshot.size_scale as f64
        };
        let clip_min = -64.0;
        let clip_max = (lane.slip_cap_bp.max(0.0) as f64).max(clip_min);

        SlipFeeSurface {
            fees,
            slip: SlipCoefficients {
                a0: lane.intercept_bp as f64,
                a1: lane.size_linear_bp as f64,
                a2: lane.size_quadratic_bp as f64,
                b1: snapshot.shared_vol_coeff_bp as f64,
                b2: snapshot.shared_spread_coeff_bp as f64,
                c1: latency_coeff,
                c2: jitter_coeff,
                size_scale,
                clip_min_bp: clip_min,
                clip_max_bp: clip_max,
            },
        }
    }
}

/// Venue state extracted from AVS-128.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct VenueSnapshot {
    pub spread_ticks: f64,
    pub microprice_offset_ticks: f64,
    pub short_horizon_vol_bp: f64,
}

/// Latency telemetry extracted from ALT-128.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct LatencyTicket {
    pub rtt_ms: f64,
    pub jitter_ms: f64,
}

impl From<AltSnapshot> for LatencyTicket {
    fn from(snapshot: AltSnapshot) -> Self {
        let rtt_us = snapshot.decision_to_ack_us as f64 + snapshot.ack_to_first_fill_us as f64;
        let jitter_ms = snapshot.jitter_us as f64 / 1_000.0;
        Self {
            rtt_ms: rtt_us / 1_000.0,
            jitter_ms,
        }
    }
}

/// Side for the order intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    fn direction(self) -> f64 {
        match self {
            Side::Buy => 1.0,
            Side::Sell => -1.0,
        }
    }
}

/// Route selector (maker / taker).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Route {
    Maker,
    Taker,
}

/// Proposed order parameters.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct OrderIntent {
    pub side: Side,
    pub route: Route,
    pub size: f64,
    pub size_normalizer: f64,
    pub price: f64,
    pub tick_size: f64,
    pub gross_edge_signal_bp: Option<f64>,
}

impl OrderIntent {
    pub fn bp_per_tick(&self) -> f64 {
        if self.price <= EPS {
            0.0
        } else {
            (self.tick_size / self.price) * 10_000.0
        }
    }

    pub fn size_k(&self) -> f64 {
        if self.size_normalizer <= EPS {
            self.size
        } else {
            self.size / self.size_normalizer
        }
    }

    pub fn gross_edge_bp(&self, venue: &VenueSnapshot) -> f64 {
        if let Some(explicit) = self.gross_edge_signal_bp {
            explicit
        } else {
            let base = venue.microprice_offset_ticks * self.bp_per_tick();
            base * self.side.direction()
        }
    }
}

/// Input capsule the estimator consumes for each publish.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct EstimationInputs {
    pub surface: SlipFeeSurface,
    pub venue: VenueSnapshot,
    pub latency: LatencyTicket,
    pub intent: OrderIntent,
    pub age_ms: u32,
}

/// Configuration knobs for the estimator.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct EstimatorConfig {
    pub safety_buffer_bp: f64,
    pub sigma_alpha: f64,
    pub sigma_init_bp: f64,
    pub sigma_clip_bp: f64,
    pub slip_alpha: f64,
    pub version: u8,
    pub age_bucket_ms: u32,
    pub high_jitter_cutoff_ms: f64,
    pub wide_spread_cutoff: f64,
    pub ok_sigma_k: Option<f64>,
    pub latency_jitter_weight: f64,
}

impl Default for EstimatorConfig {
    fn default() -> Self {
        Self {
            safety_buffer_bp: 0.0,
            sigma_alpha: 0.1,
            sigma_init_bp: 0.5,
            sigma_clip_bp: 20.0,
            slip_alpha: 0.05,
            version: 1,
            age_bucket_ms: 100,
            high_jitter_cutoff_ms: f64::INFINITY,
            wide_spread_cutoff: f64::INFINITY,
            ok_sigma_k: None,
            latency_jitter_weight: 0.5,
        }
    }
}

/// Handles cost estimation and publishing for a single (symbol, route) slot.
pub struct ActEstimator {
    slot: ActSlot,
    config: EstimatorConfig,
    surface: SlipFeeSurface,
    sigma_bp: f64,
    seq: u8,
}

impl ActEstimator {
    pub fn new(slot: ActSlot, surface: SlipFeeSurface, config: EstimatorConfig) -> Self {
        Self {
            slot,
            config,
            surface,
            sigma_bp: config.sigma_init_bp.max(0.0),
            seq: 0,
        }
    }

    pub fn slot(&self) -> &ActSlot {
        &self.slot
    }

    pub fn latency_jitter_weight(&self) -> f64 {
        self.config.latency_jitter_weight
    }

    pub fn surface(&self) -> SlipFeeSurface {
        self.surface
    }

    pub fn set_surface(&mut self, surface: SlipFeeSurface) {
        self.surface = surface;
    }

    /// Compute the latest snapshot from the supplied capsules and publish it.
    pub fn publish_snapshot(&mut self, inputs: EstimationInputs) -> ActSnapshot {
        self.surface = inputs.surface;
        let size_k = inputs.intent.size_k();
        let gross_bp = inputs.intent.gross_edge_bp(&inputs.venue);

        let fees_bp = self.surface.fees.fees_bp(inputs.intent.route);
        let slip_est_bp = self.surface.slip.predict(
            size_k,
            inputs.venue.short_horizon_vol_bp,
            inputs.venue.spread_ticks,
            &inputs.latency,
        );
        let net_bp = gross_bp - fees_bp - slip_est_bp;
        let min_required_bp = fees_bp + self.config.safety_buffer_bp;

        let sigma_bp = self.sigma_current();
        let ok_threshold = if let Some(k) = self.config.ok_sigma_k {
            min_required_bp + k * sigma_bp
        } else {
            min_required_bp
        };

        let mut flags = ActFlags::empty();
        match inputs.intent.route {
            Route::Maker => flags |= ActFlags::MAKER,
            Route::Taker => flags |= ActFlags::TAKER,
        }

        if inputs.latency.jitter_ms >= self.config.high_jitter_cutoff_ms {
            flags |= ActFlags::HIGH_JITTER;
        }

        if inputs.venue.spread_ticks >= self.config.wide_spread_cutoff {
            flags |= ActFlags::WIDE_SPREAD;
        }

        if slip_est_bp >= self.surface.slip.clip_max_bp - EPS {
            flags |= ActFlags::EMERG_BUF;
        }

        if net_bp >= ok_threshold {
            flags |= ActFlags::OK;
        }

        let age_bucket = if self.config.age_bucket_ms == 0 {
            0
        } else {
            (inputs.age_ms / self.config.age_bucket_ms).min(255) as u8
        };

        let snapshot = ActSnapshot {
            gross: FixedQ8_8::saturating_from_bp(gross_bp),
            fees: FixedQ8_8::saturating_from_bp(fees_bp),
            slip: FixedQ8_8::saturating_from_bp(slip_est_bp),
            net: FixedQ8_8::saturating_from_bp(net_bp),
            min_required: FixedQ8_8::saturating_from_bp(min_required_bp),
            sigma: FixedQ8_8::saturating_from_bp(sigma_bp),
            flags,
            version: self.config.version,
            seq: self.seq,
            age_ms_bucket: age_bucket,
        };

        self.seq = self.seq.wrapping_add(1);
        self.slot.publish(&snapshot);
        snapshot
    }

    /// Update slip coefficients and sigma from realized fill feedback.
    pub fn record_fill(&mut self, feedback: FillFeedback) {
        let mut surface = feedback.surface_override.unwrap_or(self.surface);

        let old = surface.slip;
        let size_k = feedback.size_k() * surface.slip.size_scale;
        let size_sq = size_k * size_k;
        let realized = feedback.realized_slip_bp;
        let vol = feedback.vol_bp;
        let spread = feedback.spread_ticks;
        let rtt = feedback.latency.rtt_ms;
        let jitter = feedback.latency.jitter_ms;
        let alpha = self.config.slip_alpha.clamp(0.0, 1.0);

        let new_a0 = ewma(
            old.a0,
            realized
                - old.a1 * size_k
                - old.a2 * size_sq
                - old.b1 * vol
                - old.b2 * spread
                - old.c1 * rtt
                - old.c2 * jitter,
            alpha,
        );

        let new_a1 = if size_k.abs() > EPS {
            let remainder = realized
                - old.a0
                - old.a2 * size_sq
                - old.b1 * vol
                - old.b2 * spread
                - old.c1 * rtt
                - old.c2 * jitter;
            ewma(old.a1, remainder / size_k, alpha)
        } else {
            old.a1
        };

        let new_a2 = if size_sq.abs() > EPS {
            let remainder = realized
                - old.a0
                - old.a1 * size_k
                - old.b1 * vol
                - old.b2 * spread
                - old.c1 * rtt
                - old.c2 * jitter;
            ewma(old.a2, remainder / size_sq, alpha)
        } else {
            old.a2
        };

        let new_b1 = if vol.abs() > EPS {
            let remainder = realized
                - old.a0
                - old.a1 * size_k
                - old.a2 * size_sq
                - old.b2 * spread
                - old.c1 * rtt
                - old.c2 * jitter;
            ewma(old.b1, remainder / vol, alpha)
        } else {
            old.b1
        };

        let new_b2 = if spread.abs() > EPS {
            let remainder = realized
                - old.a0
                - old.a1 * size_k
                - old.a2 * size_sq
                - old.b1 * vol
                - old.c1 * rtt
                - old.c2 * jitter;
            ewma(old.b2, remainder / spread, alpha)
        } else {
            old.b2
        };

        let new_c1 = if rtt.abs() > EPS {
            let remainder = realized
                - old.a0
                - old.a1 * size_k
                - old.a2 * size_sq
                - old.b1 * vol
                - old.b2 * spread
                - old.c2 * jitter;
            ewma(old.c1, remainder / rtt, alpha)
        } else {
            old.c1
        };

        let new_c2 = if jitter.abs() > EPS {
            let remainder = realized
                - old.a0
                - old.a1 * size_k
                - old.a2 * size_sq
                - old.b1 * vol
                - old.b2 * spread
                - old.c1 * rtt;
            ewma(old.c2, remainder / jitter, alpha)
        } else {
            old.c2
        };

        surface.slip = SlipCoefficients {
            a0: new_a0,
            a1: new_a1,
            a2: new_a2,
            b1: new_b1,
            b2: new_b2,
            c1: new_c1,
            c2: new_c2,
            size_scale: old.size_scale,
            clip_min_bp: old.clip_min_bp,
            clip_max_bp: old.clip_max_bp,
        };
        self.surface = surface;

        let predicted = self
            .surface
            .slip
            .predict(size_k, vol, spread, &feedback.latency);
        let abs_err = (feedback.realized_slip_bp - predicted).abs();
        let sigma_alpha = self.config.sigma_alpha.clamp(0.0, 1.0);
        self.sigma_bp =
            ewma(self.sigma_bp, abs_err, sigma_alpha).clamp(0.0, self.config.sigma_clip_bp);
    }

    fn sigma_current(&self) -> f64 {
        self.sigma_bp.clamp(0.0, self.config.sigma_clip_bp)
    }
}

fn ewma(current: f64, observation: f64, alpha: f64) -> f64 {
    (1.0 - alpha) * current + alpha * observation
}

/// Fill feedback provided by the router after execution.
#[derive(Clone, Copy, Debug)]
pub struct FillFeedback {
    pub surface_override: Option<SlipFeeSurface>,
    pub realized_slip_bp: f64,
    pub size: f64,
    pub size_normalizer: f64,
    pub vol_bp: f64,
    pub spread_ticks: f64,
    pub latency: LatencyTicket,
}

impl FillFeedback {
    pub fn size_k(&self) -> f64 {
        if self.size_normalizer <= EPS {
            self.size
        } else {
            self.size / self.size_normalizer
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::{evaluate_gate, GateConfig, GateOutcome};

    fn default_surface() -> SlipFeeSurface {
        SlipFeeSurface {
            fees: FeeSchedule {
                maker_fee_bp: 0.1,
                taker_fee_bp: 0.25,
                exchange_misc_bp: 0.05,
            },
            slip: SlipCoefficients {
                a0: 0.2,
                a1: 0.05,
                a2: 0.02,
                b1: 0.3,
                b2: 0.4,
                c1: 0.02,
                c2: 0.03,
                size_scale: 1.0,
                clip_min_bp: 0.0,
                clip_max_bp: 20.0,
            },
        }
    }

    fn estimator() -> ActEstimator {
        let slot = ActSlot::default();
        ActEstimator::new(
            slot,
            default_surface(),
            EstimatorConfig {
                safety_buffer_bp: 0.4,
                sigma_alpha: 0.2,
                sigma_init_bp: 0.6,
                sigma_clip_bp: 5.0,
                slip_alpha: 0.1,
                version: 3,
                age_bucket_ms: 100,
                high_jitter_cutoff_ms: 5.0,
                wide_spread_cutoff: 3.0,
                ok_sigma_k: Some(0.5),
                latency_jitter_weight: 0.5,
            },
        )
    }

    #[test]
    fn publishes_snapshot_and_sets_flags() {
        let mut estimator = estimator();
        let inputs = EstimationInputs {
            surface: default_surface(),
            venue: VenueSnapshot {
                spread_ticks: 4.0,
                microprice_offset_ticks: -1.5,
                short_horizon_vol_bp: 1.2,
            },
            latency: LatencyTicket {
                rtt_ms: 3.0,
                jitter_ms: 6.0,
            },
            intent: OrderIntent {
                side: Side::Sell,
                route: Route::Taker,
                size: 2.0,
                size_normalizer: 1.0,
                price: 4_200.0,
                tick_size: 0.25,
                gross_edge_signal_bp: None,
            },
            age_ms: 340,
        };

        let snapshot = estimator.publish_snapshot(inputs);
        assert!(snapshot.flags.contains(ActFlags::TAKER));
        assert!(snapshot.flags.contains(ActFlags::HIGH_JITTER));
        assert!(snapshot.flags.contains(ActFlags::WIDE_SPREAD));
        assert_eq!(snapshot.version, 3);
        assert_eq!(snapshot.age_ms_bucket, 3); // 300ms bucket with 100ms step.
    }

    #[test]
    fn gate_reads_snapshot_from_slot() {
        let mut estimator = estimator();
        let inputs = EstimationInputs {
            surface: default_surface(),
            venue: VenueSnapshot {
                spread_ticks: 1.0,
                microprice_offset_ticks: 2.0,
                short_horizon_vol_bp: 0.2,
            },
            latency: LatencyTicket::default(),
            intent: OrderIntent {
                side: Side::Buy,
                route: Route::Maker,
                size: 1.0,
                size_normalizer: 1.0,
                price: 4_000.0,
                tick_size: 0.25,
                gross_edge_signal_bp: Some(3.0),
            },
            age_ms: 10,
        };
        estimator.publish_snapshot(inputs);

        let config = GateConfig {
            sigma_k: Some(0.5),
            reject_high_jitter: false,
            reject_wide_spread: false,
            reject_emergency_buffer: false,
        };

        let outcome = evaluate_gate(estimator.slot().load_relaxed(), &config);
        assert!(matches!(outcome, GateOutcome::Allow(_)));
    }

    #[test]
    fn fill_feedback_updates_coefficients_and_sigma() {
        let mut estimator = estimator();
        estimator.publish_snapshot(EstimationInputs {
            surface: default_surface(),
            venue: VenueSnapshot::default(),
            latency: LatencyTicket::default(),
            intent: OrderIntent {
                side: Side::Buy,
                route: Route::Taker,
                size: 2.0,
                size_normalizer: 1.0,
                price: 4_100.0,
                tick_size: 0.25,
                gross_edge_signal_bp: Some(1.0),
            },
            age_ms: 0,
        });

        let before = estimator.surface().slip.a0;
        estimator.record_fill(FillFeedback {
            surface_override: None,
            realized_slip_bp: 1.5,
            size: 2.0,
            size_normalizer: 1.0,
            vol_bp: 0.8,
            spread_ticks: 1.5,
            latency: LatencyTicket {
                rtt_ms: 3.0,
                jitter_ms: 1.0,
            },
        });
        let after = estimator.surface().slip.a0;
        assert!(after != before);
    }

    #[test]
    fn latency_ticket_conversion_from_alt_snapshot() {
        let snapshot = atomic_latency_ticket::AltSample {
            decision_to_ack_us: 4_000,
            ack_to_first_fill_us: 8_000,
            jitter_us: 3_500,
            ..atomic_latency_ticket::AltSample::default()
        };
        let ticket = LatencyTicket::from(snapshot);
        assert!((ticket.rtt_ms - 12.0).abs() < 1e-6);
        assert!((ticket.jitter_ms - 3.5).abs() < 1e-6);
    }
}
