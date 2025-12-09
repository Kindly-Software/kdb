#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(feature = "simd-los", feature(portable_simd))]
//! Capsule-first foundation for the Kindly Engine (Napoleonic warfare simulation).
//! Uses atomic_capsule primitives for deterministic, lockfree coordination.

pub mod ballistics;
pub mod battle_ai;
pub mod command;
pub mod counter_battery;
pub mod courier;
#[cfg(feature = "io-uring")]
pub mod driver;
pub mod campaign;
pub mod diplomacy;
pub mod engineering;
pub mod fire_doctrine;
pub mod fog;
pub mod formation;
pub mod frame_stream;
pub mod garrison;
pub mod general;
pub mod grenade;
#[cfg(feature = "io-uring")]
pub mod io_bridge;
pub mod kgpu_bridge;
pub mod kgpu_ingest;
pub mod math;
pub mod morale;
pub mod order;
pub mod pathing;
pub mod physics;
pub mod province_economy;
pub mod replay;
pub mod shock;
pub mod snapshot;
pub mod strategic_map;
pub mod structure;
pub mod siege;
pub mod supply;
pub mod telemetry;
pub mod terrain;
pub mod tick;
pub mod weather;
pub mod world;

use atomic_capsule::{verify_capsule_properties, BackoffStrategy, RetryPolicy};
use core::sync::atomic::{AtomicU64, Ordering};

/// Lockfree world clock capsule: single-writer, many-readers.
///
/// - Alignment: 128B (separate cache lines from neighboring capsules)
/// - Size: 128B (padding prevents false sharing)
/// - Publication: fetch_add for tick advancement; relaxed loads for reads.
#[repr(C, align(128))]
pub struct WorldClockCapsule {
    tick: AtomicU64,
    tick_duration_ns: AtomicU64,
    _padding: [u8; 112],
}

impl WorldClockCapsule {
    /// Create a new clock starting at `start_tick` with fixed tick duration in nanoseconds.
    pub const fn new(start_tick: u64, tick_duration_ns: u64) -> Self {
        Self {
            tick: AtomicU64::new(start_tick),
            tick_duration_ns: AtomicU64::new(tick_duration_ns),
            _padding: [0; 112],
        }
    }

    /// Current tick snapshot (relaxed read: caller tolerates eventual consistency).
    #[inline(always)]
    pub fn now(&self) -> u64 {
        self.tick.load(Ordering::Relaxed)
    }

    /// Tick duration in nanoseconds (relaxed read).
    #[inline(always)]
    pub fn tick_duration_ns(&self) -> u64 {
        self.tick_duration_ns.load(Ordering::Relaxed)
    }

    /// Advance a single tick; returns the committed tick value.
    #[inline(always)]
    pub fn advance(&self) -> u64 {
        // Fetch-add publishes the increment with release semantics for writers.
        self.tick.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Advance by `steps` with retry/backoff; falls back to fetch_add after exhaustion.
    #[inline(always)]
    pub fn advance_by(&self, steps: u64, mut policy: RetryPolicy) -> u64 {
        loop {
            let current = self.tick.load(Ordering::Relaxed);
            let next = current.saturating_add(steps);

            match self
                .tick
                .compare_exchange(current, next, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => return next,
                Err(_) if policy.is_exhausted() => {
                    // Fallback: accept contention cost but make forward progress.
                    return self.tick.fetch_add(steps, Ordering::AcqRel) + steps;
                }
                Err(_) => policy.backoff(),
            }
        }
    }

    /// Reset to a specific tick (e.g., scenario reload).
    #[inline(always)]
    pub fn reset(&self, tick: u64) {
        self.tick.store(tick, Ordering::Release);
    }
}

verify_capsule_properties!(WorldClockCapsule, 128, 128);

/// Deterministic RNG capsule (xorshift64*) with per-stream isolation.
///
/// - Alignment: 64B to avoid sharing cache lines with adjacent state.
/// - Size: 64B to keep snapshot/restore simple and auditable.
#[repr(C, align(64))]
pub struct DeterministicRngCapsule {
    state: AtomicU64,
    stream: AtomicU64,
    _padding: [u8; 48],
}

impl DeterministicRngCapsule {
    /// Create a seeded RNG; `stream` separates independent generators.
    pub const fn new(seed: u64, stream: u64) -> Self {
        Self {
            state: AtomicU64::new(seed),
            stream: AtomicU64::new(stream),
            _padding: [0; 48],
        }
    }

    /// Reseed and switch stream atomically.
    #[inline(always)]
    pub fn reseed(&self, seed: u64, stream: u64) {
        self.state.store(seed, Ordering::Release);
        self.stream.store(stream, Ordering::Release);
    }

    /// Next u64 in the current stream (xorshift64*). Returns (value, stream_id).
    #[inline(always)]
    pub fn next_u64(&self) -> (u64, u64) {
        let mut current = self.state.load(Ordering::Relaxed);
        loop {
            let next = xorshift64star(current);
            match self
                .state
                .compare_exchange(current, next, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => {
                    let stream = self.stream.load(Ordering::Relaxed);
                    return (next, stream);
                }
                Err(observed) => {
                    current = observed;
                    // Lightweight hint; RetryPolicy is overkill for single CAS here.
                    core::hint::spin_loop();
                }
            }
        }
    }
}

verify_capsule_properties!(DeterministicRngCapsule, 64, 64);

#[inline(always)]
fn xorshift64star(mut x: u64) -> u64 {
    // Marsaglia xorshift64*: cheap, deterministic, and adequate for simulation RNG.
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

impl Default for WorldClockCapsule {
    fn default() -> Self {
        Self::new(0, 16_666_667) // ~60Hz tick
    }
}

impl Default for DeterministicRngCapsule {
    fn default() -> Self {
        Self::new(0x0123_4567_89AB_CDEF, 0)
    }
}

/// Convenience preset for standard backoff when advancing many ticks.
#[inline(always)]
pub fn standard_retry() -> RetryPolicy {
    RetryPolicy::new(BackoffStrategy::STANDARD)
}

// Re-exports for downstream modules
pub use ballistics::{BallisticsCapsule, FireControlPhysicsContext};
pub use battle_ai::{
    BattleAiCapsule, BattleAiDecision, BattleAiInputs, BattleAiIntent, BattleAiPlan,
};
pub use command::{
    commanders_to_generals, CommandHierarchyCapsule, CommanderCapsule, CommanderSnapshot,
};
pub use counter_battery::CounterBatteryCapsule;
pub use diplomacy::{
    DiplomaticRelationSnapshot, DiplomaticSnapshot, DiplomaticState, DiplomaticStateCapsule,
};
pub use courier::{CourierCapsule, CourierSnapshot, Doctrine};
#[cfg(feature = "io-uring")]
pub use driver::{run_driver_tick, DriverCapsule};
pub use engineering::EngineeringCapsule;
pub use fog::{FogOfWarCapsule, FogOfWarView};
pub use formation::FormationCapsule;
pub use frame_stream::{FrameStreamCapsule, FrameStreamSnapshot};
pub use garrison::{GarrisonCapsule, GarrisonSlabCapsule, GarrisonSnapshot, GARRISON_CAPACITY};
pub use general::{GeneralCapsule, GeneralSnapshot};
pub use grenade::{grenade_retry_policy, GrenadeCapsule, GrenadeOutcome, GrenadeSnapshot};
#[cfg(feature = "io-uring")]
pub use io_bridge::{
    RenderUringSinkCapsule, RuntimeStreamCapsule, RuntimeStreamError, StreamingFrame,
};
pub use kgpu_bridge::{
    KgpuRenderSlice, KgpuTerminalCapsule, RenderOverlayCapsule, RenderOverlaySnapshot,
    RenderSnapshot,
};
pub use kgpu_ingest::KgpuIngestCapsule;
pub use math::{
    q16_from_meters, q16_to_meters, q8_add, q8_from_f64, q8_mul, q8_to_f64, Q16_16, Q8_8,
};
pub use morale::{MoraleNetworkCapsule, MoraleSnapshot};
pub use order::{
    CommandDelayBufferCapsule, OrderCapsule, OrderData, OrderKind, OrderQueueCapsule, OrderState,
};
pub use pathing::PathingCapsule;
pub use physics::{
    FormationPhysicsCapsule, FormationPhysicsSnapshot, PhysicsPreset, PhysicsProfile,
};
pub use province_economy::{
    BuildOrderKind, BuildOrderSnapshot, EconomySnapshot, ProvinceEconomyCapsule,
};
#[cfg(feature = "io-uring")]
pub use replay::ReplayIoUringWriterCapsule;
pub use replay::{
    ReplayEvent, ReplayFlushCapsule, ReplayIndexCapsule, ReplayIndexSnapshot, ReplayLogCapsule,
    ReplayMmapCapsule, ReplayPersistCapsule, ReplayPersistSnapshot, ReplayStats,
};
pub use snapshot::CampaignSnapshotCapsule;
pub use strategic_map::{
    ProvinceCapsule, ProvinceSnapshot, StrategicEventKind, StrategicEventSnapshot,
    StrategicMapCapsule, StrategicSnapshot, WeatherKeyframe,
};
pub use structure::{
    StructureCapsule, StructureSlabCapsule, StructureSnapshot, STRUCTURE_PAGE_SIZE,
};
pub use supply::{SupplyCapsule, SupplySnapshot};
pub use weather::{WeatherCapsule, WeatherSnapshot};

pub use telemetry::{FormationBreakTelemetryCapsule, TelemetryCapsule};
pub use terrain::{TerrainGridCapsule, TerrainTileCapsule};
pub use tick::{
    collect_world_render_slab, tick_shard, tick_world, RenderEntry, RenderIter, RenderOverflow,
    RenderSoaSlabCapsule, RenderSoaView, SchedulerCapsule, ShardContext, ShardTickStats,
    WorldFrame, WorldLoopCapsule, WorldPersistError, WorldPersistenceCapsule, WorldRuntimeCapsule,
};
pub use world::{UnitCapsule, UnitSnapshot, WorldIter, WorldSlabCapsule};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_advances() {
        let clock = WorldClockCapsule::new(0, 1_000_000);
        assert_eq!(clock.now(), 0);
        assert_eq!(clock.advance(), 1);
        assert_eq!(clock.advance_by(5, standard_retry()), 6);
        assert_eq!(clock.now(), 6);
    }

    #[test]
    fn rng_is_deterministic() {
        let rng_a = DeterministicRngCapsule::new(42, 1);
        let rng_b = DeterministicRngCapsule::new(42, 1);

        let sequence_a: Vec<u64> = (0..4).map(|_| rng_a.next_u64().0).collect();
        let sequence_b: Vec<u64> = (0..4).map(|_| rng_b.next_u64().0).collect();

        assert_eq!(sequence_a, sequence_b);
    }
}
