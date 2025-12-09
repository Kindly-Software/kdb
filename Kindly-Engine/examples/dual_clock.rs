//! Dual-clock sketch: fixed-step simulation tick plus a coarser strategic tick.
//! Demonstrates how to keep deterministic sim ticks while running grand-strategy
//! updates (logistics/AI/diplomacy) on a slower cadence.

use kindly_engine::ballistics::{
    apply_fire_control_for_ids, BallisticsCapsule, FireControlProfileCapsule,
};
use kindly_engine::formation::FormationCapsule;
use kindly_engine::math::Q16_16;
use kindly_engine::order::{
    pack_fire_meta_extended, pack_fire_payload, OrderKind, OrderQueueCapsule,
};
use kindly_engine::pathing::PathingCapsule;
use kindly_engine::physics::PhysicsPreset;
use kindly_engine::telemetry::TelemetryCapsule;
use kindly_engine::terrain::{TerrainGridCapsule, TerrainSnapshot};
use kindly_engine::tick::{make_shard_context, tick_world};

/// Coarse clock for strategic layers (runs every `interval` sim ticks).
struct StrategicClock {
    interval: u64,
    next_tick: u64,
}

impl StrategicClock {
    fn new(interval: u64) -> Self {
        Self {
            interval,
            next_tick: interval,
        }
    }

    fn should_fire(&self, sim_tick: u64) -> bool {
        sim_tick >= self.next_tick
    }

    fn advance(&mut self) {
        self.next_tick = self.next_tick.saturating_add(self.interval);
    }
}

fn main() {
    let telemetry = TelemetryCapsule::new();
    let orders = OrderQueueCapsule::new();
    let ballistics = BallisticsCapsule::new(
        (400.0 * 65_536.0) as u32,
        (6.0 * 65_536.0) as u32,
        0,
        0,
        1,
        0,
    );
    let fire_profile = FireControlProfileCapsule::default();
    let terrain = TerrainGridCapsule::new(
        8,
        8,
        TerrainSnapshot {
            height_mm: 0,
            slope_q16: 0,
            cover_q16: 2_000,
            mud_q16: 1_000,
            material: 0,
        },
    );

    let formations = vec![
        FormationCapsule::new_with_preset(
            0,
            0,
            0,
            40_000,
            8_000,
            50_000,
            120,
            0,
            0,
            0,
            PhysicsPreset::LineInfantry,
        ),
        FormationCapsule::new_with_preset(
            1,
            0,
            0,
            45_000,
            7_000,
            52_000,
            120,
            Q16_16::from_f64(50.0).to_raw() as u32,
            0,
            0,
            PhysicsPreset::OldGuard,
        ),
    ];
    let pathings = vec![PathingCapsule::new(16, 0, 8), PathingCapsule::new(16, 0, 8)];

    let payload_a = pack_fire_payload(Q16_16::from_f64(50.0).to_raw() as u32, 0);
    let payload_b = pack_fire_meta_extended(12, 0, 0, false);

    // Strategic clock fires every 5 sim ticks.
    let mut strat_clock = StrategicClock::new(5);

    for sim_tick in 0..10 {
        // Example strategic layer: inject an artillery order on strategic ticks.
        if strat_clock.should_fire(sim_tick) {
            orders
                .push_order(OrderKind::ArtilleryFire, 0, payload_a, payload_b)
                .expect("queue push");
            strat_clock.advance();
        }

        // If you know the target ID, resolve fire control deterministically before the tick.
        if let Some(order) = orders.pop_order() {
            let _outcome = apply_fire_control_for_ids(
                &order,
                &terrain,
                &ballistics,
                None,
                &telemetry,
                &fire_profile,
                &formations,
                0,
                Some(1),
            );
            // Requeue for the tick so formation state updates still flow through telemetry.
            orders
                .push_order(
                    order.kind,
                    order.formation_id,
                    order.payload_a,
                    order.payload_b,
                )
                .expect("queue push");
        }

        let shard = make_shard_context(
            0,
            &orders,
            &formations,
            &pathings,
            &telemetry,
            None,
            Some(&ballistics),
            Some(&fire_profile),
            Some(&terrain),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
        let stats = tick_world::<16>(dual.now(), &[shard]);
        println!(
            "sim_tick {} processed {}",
            sim_tick, stats[0].processed_orders
        );
    }
}
