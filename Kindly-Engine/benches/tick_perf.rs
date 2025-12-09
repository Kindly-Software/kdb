#![feature(test)]

//! Coarse perf harness for multi-shard ticks. Bump `PER_SHARD` toward 250_000 to
//! reach ~1M formations and record p99/p999 latencies on kindly-hub.

extern crate test;

use kindly_engine::order::{OrderKind, OrderQueueCapsule};
use kindly_engine::pathing::PathingCapsule;
use kindly_engine::tick::{tick_world, ShardContext};
use kindly_engine::{formation::FormationCapsule, telemetry::TelemetryCapsule};
use test::{black_box, Bencher};

#[bench]
fn tick_world_sharded(b: &mut Bencher) {
    const SHARDS: usize = 4;
    const PER_SHARD: usize = 4_096; // Adjust toward 250_000 for 1M+ formations.

    let mut formations = Vec::with_capacity(SHARDS);
    let mut pathings = Vec::with_capacity(SHARDS);
    let mut orders = Vec::with_capacity(SHARDS);
    let mut telemetry = Vec::with_capacity(SHARDS);

    for shard in 0..SHARDS {
        let mut shard_forms = Vec::with_capacity(PER_SHARD);
        let mut shard_paths = Vec::with_capacity(PER_SHARD);
        for i in 0..PER_SHARD {
            let base = (shard * PER_SHARD + i) as u32;
            shard_forms.push(FormationCapsule::new(
                base,
                0,
                0,
                40_000,
                10_000,
                50_000,
                120,
                (base % 360) << 16,
                (i as u32) << 8,
                ((i * 2) as u32) << 8,
            ));
            shard_paths.push(PathingCapsule::new(32, 0, 16));
        }
        formations.push(shard_forms);
        pathings.push(shard_paths);
        orders.push(OrderQueueCapsule::new());
        telemetry.push(TelemetryCapsule::new());
    }

    let shard_contexts: Vec<ShardContext<'_, 16>> = (0..SHARDS)
        .map(|idx| ShardContext {
            shard_id: idx,
            orders: &orders[idx],
            formations: &formations[idx],
            pathings: &pathings[idx],
            telemetry: &telemetry[idx],
            formation_breaks: None,
            ballistics: None,
            fire_profile: None,
            terrain: None,
            grenades: None,
            structures: None,
            garrisons: None,
            supply: None,
            courier: None,
            fire_doctrine: None,
            battle_ai: None,
            fog: None,
            generals: None,
            command_hierarchy: None,
            commanders: None,
            strategic: None,
        })
        .collect();

    b.iter(|| {
        for o in &orders {
            let _ = o.push_order(OrderKind::Move, 0, 0, 0);
        }
        black_box(tick_world(0, &shard_contexts));
    });
}
