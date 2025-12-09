//! Minimal UCE34-compliant driver sketch: build shard contexts with terrain/ballistics/profile,
//! inject known target IDs for artillery, and tick deterministically.

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
    // Core capsules (preallocated, lock-free).
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

    // Formations and pathing (two formations so we can target by ID).
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

    // Build shard context with terrain + ballistics + fire profile wired in.
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

    // Issue an artillery order from formation 0 targeting formation 1.
    let payload_a = pack_fire_payload(Q16_16::from_f64(50.0).to_raw() as u32, 0);
    let payload_b = pack_fire_meta_extended(12, 0, 0, false);
    orders
        .push_order(OrderKind::ArtilleryFire, 0, payload_a, payload_b)
        .expect("queue push");

    // Formation-aware fire-control using explicit shooter/target IDs.
    let outcome = apply_fire_control_for_ids(
        &orders.pop_order().expect("order exists"),
        &terrain,
        &ballistics,
        None,
        &telemetry,
        &fire_profile,
        &formations,
        0,
        Some(1),
    );
    if let Some(outcome) = outcome {
        println!(
            "Fire-control outcome: range {:.1} m, expected casualties {}",
            outcome.effective_range_m, outcome.expected_casualties
        );
    }

    // Re-queue the order so the tick path can process it deterministically.
    orders
        .push_order(OrderKind::ArtilleryFire, 0, payload_a, payload_b)
        .expect("queue push");

    // Tick once; in a real loop you'd reseed deterministically and iterate.
    let stats = tick_world::<16>(0, &[shard]);
    println!("Tick processed orders: {}", stats[0].processed_orders);
}
