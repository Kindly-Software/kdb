use atomic_cost_tracker::coordinator::{ActCoordinator, OrderRequest, StrategyRouter};
use atomic_cost_tracker::engine::ActEngine;
use atomic_cost_tracker::estimator::{
    ActEstimator, EstimatorConfig, FeeSchedule, FillFeedback, LatencyTicket, OrderIntent, Route,
    Side, SlipCoefficients, SlipFeeSurface, VenueSnapshot,
};
use atomic_cost_tracker::gate::{GateConfig, GateOutcome};
use atomic_cost_tracker::manager::ActEngineManager;
use atomic_cost_tracker::service::{ActService, NoopTelemetrySink};
use atomic_cost_tracker::telemetry::ActTelemetry;
use atomic_cost_tracker::ServiceError;

use std::time::Duration;

#[derive(Default)]
struct PrintRouter;

impl StrategyRouter for PrintRouter {
    fn handle_gate(&mut self, request: &OrderRequest, outcome: GateOutcome) {
        println!(
            "ACT decision for {} {:?}: {:?}",
            request.symbol, request.route, outcome
        );
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
        atomic_cost_tracker::ActSlot::default(),
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

fn main() -> Result<(), ServiceError> {
    let mut manager = ActEngineManager::new();
    let engine = ActEngine::new(
        "MES",
        Route::Maker,
        estimator(Route::Maker),
        GateConfig::default(),
        ActTelemetry::default(),
    );
    manager
        .register_engine(engine)
        .map_err(ServiceError::from)?;

    let mut service = ActService::new(manager, NoopTelemetrySink::default());
    service.update_surface("MES", Route::Maker, surface())?;
    service.update_venue(
        "MES",
        Route::Maker,
        VenueSnapshot {
            spread_ticks: 1.0,
            microprice_offset_ticks: 0.5,
            short_horizon_vol_bp: 0.3,
        },
    )?;
    service.update_latency(
        "MES",
        Route::Maker,
        LatencyTicket {
            rtt_ms: 2.5,
            jitter_ms: 0.8,
        },
    )?;

    let router = PrintRouter::default();
    let mut coordinator = ActCoordinator::new(service, router, Duration::from_secs(5));

    let request = OrderRequest::new(
        "MES",
        Route::Maker,
        0,
        OrderIntent {
            side: Side::Buy,
            route: Route::Maker,
            size: 1.0,
            size_normalizer: 1.0,
            price: 4_000.0,
            tick_size: 0.25,
            gross_edge_signal_bp: Some(3.0),
        },
    );

    coordinator.on_order(request)?;

    coordinator.service_mut().record_fill(
        "MES",
        Route::Maker,
        0,
        FillFeedback {
            surface_override: None,
            realized_slip_bp: 1.0,
            size: 1.0,
            size_normalizer: 1.0,
            vol_bp: 0.4,
            spread_ticks: 1.0,
            latency: LatencyTicket {
                rtt_ms: 3.0,
                jitter_ms: 1.0,
            },
        },
    )?;

    coordinator.tick()?;

    Ok(())
}
