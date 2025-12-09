# Kindly-Engine

Game engine initiative built on computational capsules to simulate large-scale Napoleonic warfare. All components must prefer in-tree capsules from `atomic_capsule` before adding dependencies.

## Goals
- Real-scale battles (10K–100K entities) with sub-10ms ticks on modern desktop CPUs.
- Deterministic, replayable simulation (T28 determinism + T9 persistence ready).
- B32-honest performance claims; T28 test coverage across unit/property/integration/production/determinism.

## Capsule Architecture Overview
- **T1 Atomic**: authority handoff, formation state, morale/condition, command queues, time-step clock, RNG seeds (DualAtomicU64).
- **T2 SIMD**: vectorized movement integration, collision broad-phase, line-of-sight, wind/ballistics sampling, terrain sampling.
- **T3 Fixed-Point**: deterministic physics budgets, fatigue/morale decay, weather modifiers, damage resolution.
- **T4 Batch**: per-brigade/column updates (e.g., 128–512 soldiers per batch), terrain tile heatmaps, supply/resupply ticks.
- **T5 Streaming**: event log, telemetry, deterministic replays, rolling metrics (p50/p99 casualties, ammo).
- **T6 Mixed**: orchestrators combining T1/T2/T3/T4 pipelines for each tick; optional network sync (future T8) and persistence (T9) for campaign saves.

## Core Systems (capsule-first)
- **World Clock & Scheduler (T1/T4/T6)**: lockfree tick counter, priority queues for orders/events, batch-oriented stepping.
- **Terrain & Weather (T2/T3/T4)**: tiled heightmap capsules with SIMD sampling; fixed-point weather fields (wind/visibility/mud) influencing movement and ballistics.
- **Unit Model (T1/T3)**: formation capsule (line/column/square), posture, ammo, fatigue, morale; packed one-read snapshot for AI/physics.
- **Command & Control (T1/T4)**: hierarchical orders (army → corps → division → brigade → battalion); lockfree queues; delay/latency modeled via generation counters.
- **Movement & Pathing (T2/T4)**: SIMD integration for step updates; batch marching columns; avoidance using capsule-based grid (atomic occupancy).
- **Ballistics & Effects (T2/T3/T4)**: musket/arty trajectories, dispersion, penetration with SIMD; fixed-point energy/penetration; batch impact resolution.
- **Morale & Cohesion (T3/T4)**: deterministic decay and shock; formation-specific effects; batch morale recompute per company/battalion.
- **Logistics (T4/T5)**: ammo wagons, resupply ticks, baggage trains; streaming counters; fixed-point consumption.
- **Sensing & LOS (T2/T4)**: SIMD ray/segment tests against terrain; batch LOS updates per unit cluster.
- **AI/Doctrine (T4/T6)**: scripted doctrine capsules; stance/state machines enforced via packed snapshots; batch evaluation per formation group.
- **Replay/Determinism (T5/T9-ready)**: streaming event log (ring buffer) + optional persistent snapshots for campaign saves and audit.
- **Kernel (Waterloo)**: `math` (Q8.8/Q16.16 helpers), `WorldSlabCapsule` paged slab (10K pages, no realloc), `FrameStreamCapsule` for frame dumps, `KgpuTerminalCapsule` zero-copy ingest of `RenderSoaView`.

## Existing Capsules to Reuse (examples)
- Ring buffers, histograms, concurrent maps, lockfree caches from `atomic_capsule`.
- SIMD math helpers (portable_simd) for geometry and LOS.
- Fixed-point patterns for deterministic damage/morale.
- Generation counters + DualAtomicU64 for authoritative state and C2 latency modeling.

## New Capsules to Add (proposed)
- **FormationCapsule**: packed formation geometry, frontage/depth, spacing, facing, cohesion, fatigue, ammo, morale.
- **OrderQueueCapsule**: lockfree hierarchical orders with send/ack generation.
- **BallisticsCapsule**: SIMD-friendly projectile batches with wind/drag tables (fixed-point).
- **TerrainTileCapsule**: height + material + mud index; SIMD sampling view; cached slope/cover.
- **LogisticsCapsule**: supply/ammo per node with batch resupply rules.
- **TelemetryCapsule**: streaming combat log with compact encoding for replay and p99/p999 latencies.

## Feature Flags
- `simd-los`: nightly portable_simd LOS averaging.
- `io-uring`: enable `FrameStreamCapsule::submit_render_frame_uring` for pinned NVMe writes.
- `kgpu-driver*`: forward kgpu driver stacks (`kgpu-driver`, `kgpu-driver-linux`, `kgpu-driver-intel`, `kgpu-driver-amd`, `kgpu-driver-nvidia`, `kgpu-driver-all`) for renderer bring-up.

## IO + GPU Bring-up Notes
- **io_uring NVMe path**: enable `--features io-uring`, pre-register a pinned buffer, then call `FrameStreamCapsule::submit_render_frame_uring` with your `IoUringBatchCapsule`, target fd, and offset; or use `RenderUringSinkCapsule` to own the ring/batch/buffer and submit frames in one call. Keeps IO → disk zero-copy for replays/frame dumps.
- **kgpu renderer path**: enable the relevant `kgpu-driver*` feature for your platform before integrating the renderer. `KgpuTerminalCapsule` hands zero-copy `RenderSoaView` slices to the driver without realloc.

## Testing & Validation
- **T28**: unit (capsule invariants), property (stability under varied terrain/weather), integration (C2 → movement → fire → morale), production (long-run soak), determinism (replay identical).
- **B32**: measure tick latency, p99/p999 under 10K/50K/100K entities; remote kindly-hub for benches.
- **ASSUM**: document memory ordering, fixed-point scale, RNG determinism; every #ASSUME has #VERIFY.

## Workstream Outline
1) Skeleton: workspace + capsule wiring, minimal world clock, deterministic RNG capsule.  
2) Terrain/LOS: tile capsules + SIMD sampling; verify alignment and B32 baseline.  
3) Unit/Formation capsule + movement pathing batches.  
4) Ballistics + effects (musket/arty) with fixed-point energy.  
5) Morale/cohesion + logistics loops.  
6) Replay/telemetry pipeline (streaming ring buffer, optional persistence).  
7) Doctrine/AI layer + scenario scripts.  
8) Validation: T28+B32 on kindly-hub, publish honest claims.

## Performance Pipeline (1M-entity target)
- Tick staging: ingest orders → batch by shard → apply to formations (posture/facing/morale/fatigue) → terrain/LOS sampling → fire-control/ballistics → morale/cohesion decay → telemetry emit.
- Sharding: spatial shards over terrain grid; T4 batches per shard; read-only snapshots for cross-shard reads; deferred writes for shared resources (ammo/supply).
- SIMD: LOS/cover averaging, range bin hit-probability tables, slope sampling; fixed-point math for morale/fatigue/damage to stay deterministic.
- Movement: pathing capsule uses backstep-aware fallback (keep facing enemy, slower pace) to avoid “turn backs” on retreat; batch step formations by shard with SoA snapshots.
- Render bridge: expose immutable SoA snapshots (positions/facing/posture/morale/ammo) per shard for renderer; `RenderSoaView::{iter_strided,iter_shard}` for LOD bucket sampling; paged slab avoids realloc; zero-copy into renderer.
- Budget: aim <10ms tick on desktop; measure p99/p999 on kindly-hub with 100K/500K/1M entities; prefer T2+T4 hot paths, avoid heap churn in tick loop.
- Multi-shard scheduler: `tick_world` runs per-shard `tick_shard` with retreat-aware movement; pair with paged `RenderSoaSlabCapsule` (`collect_world_render_slab`) to hand immutable SoA buffers (shard offsets, no realloc/copy) to renderer; grows by pages, never stutters. For top-level orchestration use `WorldLoopCapsule` (per-tick deterministic reseed) wrapping `SchedulerCapsule`.

## Persistence / Replay / Telemetry
- Replay ring (T5) + mmap persistence (T9-ready) already available; drain tick events (orders applied, LOS samples, fire outcomes, morale breaks, retreats) into `ReplayLogCapsule` → `ReplayMmapCapsule::append_from_log` or `append_from_log_with_index` (hash-chain via `ReplayIndexCapsule`).
- Telemetry capsule tracks events/casualties/ammo/retreats/morale shocks and last flush tick; extend with per-weapon/per-formation counters for campaign stats.
- Determinism: log RNG seeds + order stream + terrain/weather seeds; property-test replay equivalence (log → replay → identical snapshots).
- Bench/validate on kindly-hub: long-run soak with replays, B32 honesty (p50/p99/p999 tick latency, bytes written to mmap, retreat counts).

## Nightly SIMD LOS
- Build/check locally: `cargo +nightly check -p kindly-engine --features simd-los`
- Bench SIMD path: `cargo +nightly bench -p kindly-engine --features simd-los --bench los`
