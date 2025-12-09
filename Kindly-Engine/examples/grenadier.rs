//! Minimal grenadier example: throws a grenade at a target formation, logs replay payload,
//! and prints decoded grenade timeline.

use kindly_engine::grenade::GrenadeCapsule;
use kindly_engine::order::{pack_grenade_meta, pack_grenade_payload, OrderKind};
use kindly_engine::replay::{decode_replay_payload, encode_grenade_replay_payload};
use kindly_engine::terrain::{TerrainGridCapsule, TerrainSnapshot};

fn main() {
    // Flat terrain stub.
    let grid = TerrainGridCapsule::new(
        4,
        4,
        TerrainSnapshot {
            height_mm: 0,
            slope_q16: 0,
            cover_q16: 1_000,
            mud_q16: 500,
            material: 0,
        },
    );

    // Simple grenade profile.
    let grenade = GrenadeCapsule::new(30 << 16, 1200, 48, 50_000, 2 << 16, 0xDEADBEEF);

    // Shooter at (1,1), target at (2,2) in Q16.16.
    let shooter_x = 1 << 16;
    let shooter_z = 1 << 16;
    let target_x = 2 << 16;
    let target_z = 2 << 16;

    let payload = pack_grenade_payload(target_x, target_z);
    let meta = pack_grenade_meta(1200, 48);

    // Execute throw (no structures in this demo).
    let outcome = grenade.throw(
        shooter_x,
        shooter_z,
        target_x,
        target_z,
        &grid,
        None,
        Some(1200),
        Some(48),
        0,
    );

    // Encode + decode replay payload to show how tooling reads it.
    let replay = encode_grenade_replay_payload(
        outcome.expected_casualties,
        outcome.avg_cover_q16,
        outcome.detonation_ms,
    );
    match decode_replay_payload(replay) {
        kindly_engine::replay::ReplayRecord::Grenade {
            casualties,
            avg_cover_q16,
            detonation_ms,
        } => {
            println!(
                "Grenade order {:?} meta {:?}: casualties={}, cover_q16={}, detonation_ms={}",
                OrderKind::Grenade,
                (payload, meta),
                casualties,
                avg_cover_q16,
                detonation_ms
            );
        }
        other => {
            println!("Unexpected replay record: {:?}", other);
        }
    }
}
