//! Minimal wiring example: build shard contexts with terrain/ballistics/profile and use
//! `apply_fire_control_for_ids` when you know the target formation.

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

fn main() {
    // Shared capsules
    let telemetry = TelemetryCapsule::new();
    let orders = OrderQueueCapsule::new();
    let ballistics = BallisticsCapsule::new(
        (400.0 * 65_536.0) as u32, // max range q16
        (6.0 * 65_536.0) as u32,   // dispersion q16
        0,
        0,
        1,
        0,
    );
    let profile = FireControlProfileCapsule::default();
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

    // Formations and pathing
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

    // Issue an artillery order from formation 0 targeting formation 1.
    let payload_a = pack_fire_payload(Q16_16::from_f64(50.0).to_raw() as u32, 0);
    let payload_b = pack_fire_meta_extended(12, 0, 0, false);
    orders
        .push_order(OrderKind::ArtilleryFire, 0, payload_a, payload_b)
        .expect("queue push");

    // Formation-aware fire-control: use both shooter/target snapshots for density/variance scaling.
    let outcome = apply_fire_control_for_ids(
        &orders.pop_order().expect("order exists"), // pop for demonstration
        &terrain,
        &ballistics,
        None,
        &telemetry,
        &profile,
        &formations,
        0,
        Some(1),
    );
    if let Some(outcome) = outcome {
        println!(
            "Fire-control outcome: range {:.2} m, expected casualties {}",
            outcome.effective_range_m, outcome.expected_casualties
        );
    }

    // Re-push the order so the tick path can process it too.
    orders
        .push_order(OrderKind::ArtilleryFire, 0, payload_a, payload_b)
        .expect("queue push");

    // Build a shard context with terrain/ballistics/profile wired in.
    let shard = make_shard_context(
        0,
        &orders,
        &formations,
        &pathings,
        &telemetry,
        None,
        Some(&ballistics),
        Some(&profile),
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

    // Run a single tick; artillery orders will use terrain + shooter snapshot and the
    // nearest-target fallback if no explicit target is supplied.
    let stats = tick_world::<16>(0, &[shard]);
    println!("Processed orders: {}", stats[0].processed_orders);
}
