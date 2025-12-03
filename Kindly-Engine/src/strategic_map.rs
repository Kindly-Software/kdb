use crate::supply::{SupplyCapsule, SupplyRoad, SupplySnapshot};
use crate::weather::{WeatherCapsule, WeatherSnapshot};
use atomic_capsule::verify_capsule_properties;
use core::hash::{Hash, Hasher};
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Weather keyframe for campaign scripts (tick-aligned).
#[derive(Debug, Clone)]
pub struct WeatherKeyframe {
    pub tick: u64,
    pub precipitation_q16: u32,
    pub wind_speed_q16: u32,
    pub gust_q16: u32,
    pub wind_dir_deg_q16: u32,
}

/// Strategic event kinds (ownership/repair).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategicEventKind {
    OwnershipChange = 0,
    InfrastructureRepair = 1,
}

impl StrategicEventKind {
    pub fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::OwnershipChange),
            1 => Some(Self::InfrastructureRepair),
            _ => None,
        }
    }
}

/// Immutable snapshot of a strategic event (for snapshot/replay persistence).
#[derive(Debug, Clone)]
pub struct StrategicEventSnapshot {
    pub kind: StrategicEventKind,
    pub province_id: u32,
    pub from_owner_id: u32,
    pub to_owner_id: u32,
    pub from_infra_q16: u32,
    pub to_infra_q16: u32,
    pub resistance_q16: u32,
    pub generation: u64,
}

/// Immutable snapshot of a province for campaign analytics/UI.
#[derive(Debug, Clone)]
pub struct ProvinceSnapshot {
    pub id: u32,
    pub owner_id: u32,
    pub population: u32,
    pub infrastructure_q16: u32,
    pub depot_pressure_q16: u32,
    pub supply_output_q16: u32,
    pub resistance_q16: u32,
    pub generation: u64,
}

/// Combined strategic tick snapshot (campaign → tactical feed).
#[derive(Debug, Clone)]
pub struct StrategicSnapshot {
    pub tick: u64,
    pub generation: u64,
    pub provinces: Vec<ProvinceSnapshot>,
    pub supply: SupplySnapshot,
    pub weather: WeatherSnapshot,
    pub events: Vec<StrategicEventSnapshot>,
    pub prev_hash_chain: u64,
    pub hash_chain: u64,
}

/// Province capsule (ownership + population + infra + depot pressure).
///
/// Alignment: 128B to avoid false sharing with neighbors.
#[repr(C, align(128))]
pub struct ProvinceCapsule {
    owner_id: AtomicU32,
    population: AtomicU32,
    infrastructure_q16: AtomicU32,
    depot_pressure_q16: AtomicU32,
    supply_output_q16: AtomicU32,
    resistance_q16: AtomicU32,
    generation: AtomicU64,
    _padding: [u8; 96],
}

impl ProvinceCapsule {
    pub const fn new(owner_id: u32, population: u32, infrastructure_q16: u32) -> Self {
        Self {
            owner_id: AtomicU32::new(owner_id),
            population: AtomicU32::new(population),
            infrastructure_q16: AtomicU32::new(infrastructure_q16),
            depot_pressure_q16: AtomicU32::new(0),
            supply_output_q16: AtomicU32::new(infrastructure_q16), // default: infra drives supply output
            resistance_q16: AtomicU32::new(0),
            generation: AtomicU64::new(1),
            _padding: [0; 96],
        }
    }

    pub fn snapshot(&self, id: u32) -> ProvinceSnapshot {
        ProvinceSnapshot {
            id,
            owner_id: self.owner_id.load(Ordering::Relaxed),
            population: self.population.load(Ordering::Relaxed),
            infrastructure_q16: self.infrastructure_q16.load(Ordering::Relaxed),
            depot_pressure_q16: self.depot_pressure_q16.load(Ordering::Relaxed),
            supply_output_q16: self.supply_output_q16.load(Ordering::Relaxed),
            resistance_q16: self.resistance_q16.load(Ordering::Relaxed),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    pub fn set_owner(&self, owner_id: u32) {
        self.owner_id.store(owner_id, Ordering::Release);
        self.bump_generation();
    }

    pub fn set_population(&self, population: u32) {
        self.population.store(population, Ordering::Release);
        self.bump_generation();
    }

    pub fn set_infrastructure(&self, infrastructure_q16: u32) {
        self.infrastructure_q16
            .store(infrastructure_q16.min(65_536), Ordering::Release);
        self.bump_generation();
    }

    pub fn set_depot_pressure(&self, pressure_q16: u32) {
        self.depot_pressure_q16
            .store(pressure_q16.min(65_536), Ordering::Release);
    }

    pub fn set_supply_output(&self, supply_q16: u32) {
        self.supply_output_q16
            .store(supply_q16.min(65_536), Ordering::Release);
        self.bump_generation();
    }

    pub fn infrastructure_q16(&self) -> u32 {
        self.infrastructure_q16.load(Ordering::Acquire)
    }

    pub fn resistance_q16(&self) -> u32 {
        self.resistance_q16.load(Ordering::Acquire)
    }

    pub fn owner_id(&self) -> u32 {
        self.owner_id.load(Ordering::Acquire)
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub fn set_resistance(&self, resistance_q16: u32) {
        self.resistance_q16
            .store(resistance_q16.min(65_536), Ordering::Release);
        self.bump_generation();
    }

    /// Capture/flip ownership with optional infrastructure damage and resistance surge.
    pub fn capture(
        &self,
        new_owner_id: u32,
        resistance_q16: u32,
        infra_damage_q16: u32,
    ) {
        let current_infra = self.infrastructure_q16.load(Ordering::Acquire);
        let damaged = current_infra.saturating_sub(infra_damage_q16.min(65_536));
        self.owner_id.store(new_owner_id, Ordering::Release);
        self.infrastructure_q16
            .store(damaged.min(65_536), Ordering::Release);
        self.resistance_q16
            .store(resistance_q16.min(65_536), Ordering::Release);
        self.bump_generation();
    }

    fn bump_generation(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

verify_capsule_properties!(ProvinceCapsule, 128, 128);

/// Strategic map capsule: provinces + supply graph + weather script with hash-chained snapshots.
#[repr(C, align(128))]
pub struct StrategicMapCapsule {
    provinces: Vec<ProvinceCapsule>,
    supply: SupplyCapsule,
    weather: WeatherCapsule,
    weather_script: Option<Vec<WeatherKeyframe>>,
    weather_cursor: usize,
    prev_provinces: Vec<ProvinceSnapshot>,
    generation: AtomicU64,
    hash_chain: AtomicU64,
    last_tick: AtomicU64,
    _padding: [u8; 48],
}

impl StrategicMapCapsule {
    pub fn new(
        provinces: Vec<ProvinceCapsule>,
        roads: Option<&[SupplyRoad]>,
        weather_script: Option<Vec<WeatherKeyframe>>,
    ) -> Self {
        let mut supply = SupplyCapsule::new(provinces.len());
        if let Some(edges) = roads {
            for edge in edges {
                supply.add_road(
                    edge.from,
                    edge.to,
                    edge.capacity_q16,
                    edge.loss_q16,
                    edge.distance_tiles,
                );
            }
        }
        let prev_provinces = provinces
            .iter()
            .enumerate()
            .map(|(i, p)| p.snapshot(i as u32))
            .collect();
        Self {
            provinces,
            supply,
            weather: WeatherCapsule::new(),
            weather_script,
            weather_cursor: 0,
            prev_provinces,
            generation: AtomicU64::new(1),
            hash_chain: AtomicU64::new(0),
            last_tick: AtomicU64::new(0),
            _padding: [0; 48],
        }
    }

    /// Apply a campaign weather script (tick-indexed); falls back to seasonal cycle if none.
    fn advance_weather(&mut self, tick: u64) -> WeatherSnapshot {
        if let Some(script) = self.weather_script.as_ref() {
            while self.weather_cursor + 1 < script.len()
                && script[self.weather_cursor + 1].tick <= tick
            {
                self.weather_cursor += 1;
            }
            let key = &script[self.weather_cursor];
            self.weather
                .set_precipitation(key.precipitation_q16.min(65_536));
            self.weather.set_wind(
                key.wind_dir_deg_q16.min(65_536),
                key.wind_speed_q16.min(65_536),
            );
        } else {
            let phase = (tick % 16) as u32;
            let precip = match phase {
                0..=3 => 12_000,
                4..=7 => 28_000,
                8..=11 => 45_000,
                _ => 20_000,
            };
            let wind = (14_000 + phase.saturating_mul(1_500)).min(50_000);
            self.weather.set_precipitation(precip);
            self.weather.set_wind(0, wind);
        }
        self.weather.step()
    }

    /// Publish a strategic snapshot and feed weather into the supply capsule.
    pub fn step(&mut self, tick: u64) -> StrategicSnapshot {
        self.last_tick.store(tick, Ordering::Release);
        let map_generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;

        // Weather → supply decay/penalties.
        let wx = self.advance_weather(tick);
        self.supply.set_weather(wx.mud_q16, wx.wind_speed_q16);
        let weather_decay = 65_536u32
            .saturating_sub(self.weather.supply_decay_scale_q16().min(65_536));

        // Inject depot/base pressure from provinces before stepping supply.
        for (idx, province) in self.provinces.iter().enumerate() {
            let pressure = province.depot_pressure_q16.load(Ordering::Acquire);
            let supply_output = province.supply_output_q16.load(Ordering::Acquire);
            let infra_q16 = province.infrastructure_q16.load(Ordering::Acquire).min(65_536);
            let resistance_q16 = province.resistance_q16.load(Ordering::Acquire).min(65_536);
            let loyalty_scale = 65_536u32.saturating_sub(resistance_q16 / 2);
            let infra_scale = infra_q16;
            let combined = pressure.saturating_add(supply_output);
            let effective = (((combined as u64 * infra_scale as u64) / 65_536)
                * loyalty_scale as u64
                / 65_536)
            .min(u32::MAX as u64) as u32;
            if effective > 0 {
                self.supply.inject_pressure(idx as u32, effective);
            }
        }
        // Decay scales with weather + infra deficit + resistance.
        let infra_deficit = self
            .provinces
            .iter()
            .map(|p| 65_536u32.saturating_sub(
                p.infrastructure_q16.load(Ordering::Relaxed).min(65_536),
            ))
            .max()
            .unwrap_or(0)
            / 4; // soften impact
        let resistance_penalty = self
            .provinces
            .iter()
            .map(|p| p.resistance_q16.load(Ordering::Relaxed).min(65_536) / 3)
            .max()
            .unwrap_or(0);
        let decay_q16 = weather_decay
            .saturating_add(infra_deficit)
            .saturating_add(resistance_penalty)
            .min(65_536);
        self.supply.set_decay_q16(decay_q16);
        let supply_snap = self.supply.step();

        let provinces = self
            .provinces
            .iter()
            .enumerate()
            .map(|(i, p)| p.snapshot(i as u32))
            .collect::<Vec<_>>();

        let events = self.diff_events(&provinces);
        let prev_hash = self.hash_chain.load(Ordering::Acquire);
        let hash_chain = self.update_hash_chain(tick, &provinces, &supply_snap, &wx, &events);
        self.prev_provinces = provinces.clone();

        StrategicSnapshot {
            tick,
            generation: map_generation,
            provinces,
            supply: supply_snap,
            weather: wx,
            events,
            prev_hash_chain: prev_hash,
            hash_chain,
        }
    }

    fn update_hash_chain(
        &self,
        tick: u64,
        provinces: &[ProvinceSnapshot],
        supply: &SupplySnapshot,
        weather: &WeatherSnapshot,
        events: &[StrategicEventSnapshot],
    ) -> u64 {
        let mut hasher = fnv::FnvHasher::with_key(self.hash_chain.load(Ordering::Acquire));
        tick.hash(&mut hasher);
        weather.mud_q16.hash(&mut hasher);
        weather.wind_speed_q16.hash(&mut hasher);
        weather.wind_dir_deg_q16.hash(&mut hasher);
        supply.avg_pressure_q16.hash(&mut hasher);
        supply.baggage_captured.hash(&mut hasher);
        for p in provinces.iter().take(8) {
            p.owner_id.hash(&mut hasher);
            p.depot_pressure_q16.hash(&mut hasher);
            p.infrastructure_q16.hash(&mut hasher);
            p.supply_output_q16.hash(&mut hasher);
            p.resistance_q16.hash(&mut hasher);
            p.generation.hash(&mut hasher);
        }
        events.len().hash(&mut hasher);
        for ev in events.iter().take(4) {
            (ev.kind as u8).hash(&mut hasher);
            ev.province_id.hash(&mut hasher);
            ev.to_owner_id.hash(&mut hasher);
            ev.to_infra_q16.hash(&mut hasher);
            ev.generation.hash(&mut hasher);
        }
        let out = hasher.finish();
        self.hash_chain.store(out, Ordering::Release);
        out
    }

    fn diff_events(&self, provinces: &[ProvinceSnapshot]) -> Vec<StrategicEventSnapshot> {
        let mut events = Vec::new();
        for (idx, province) in provinces.iter().enumerate() {
            if let Some(prev) = self.prev_provinces.get(idx) {
                if province.owner_id != prev.owner_id {
                    events.push(StrategicEventSnapshot {
                        kind: StrategicEventKind::OwnershipChange,
                        province_id: province.id,
                        from_owner_id: prev.owner_id,
                        to_owner_id: province.owner_id,
                        from_infra_q16: prev.infrastructure_q16,
                        to_infra_q16: province.infrastructure_q16,
                        resistance_q16: province.resistance_q16,
                        generation: province.generation,
                    });
                } else if province.infrastructure_q16 > prev.infrastructure_q16 {
                    events.push(StrategicEventSnapshot {
                        kind: StrategicEventKind::InfrastructureRepair,
                        province_id: province.id,
                        from_owner_id: province.owner_id,
                        to_owner_id: province.owner_id,
                        from_infra_q16: prev.infrastructure_q16,
                        to_infra_q16: province.infrastructure_q16,
                        resistance_q16: province.resistance_q16,
                        generation: province.generation,
                    });
                }
            }
        }
        events
    }

    pub fn provinces(&self) -> &[ProvinceCapsule] {
        &self.provinces
    }

    pub fn provinces_mut(&mut self) -> &mut [ProvinceCapsule] {
        &mut self.provinces
    }

    /// Seed ammo stock for a node (mirrors SupplyCapsule API).
    pub fn set_ammo(&self, node: u32, ammo_units: u32) {
        self.supply.set_ammo(node, ammo_units);
    }
}

// Minimal FNV hasher (u64 seed) for hash-chaining snapshots.
mod fnv {
    use core::hash::Hasher;

    pub struct FnvHasher {
        hash: u64,
    }

    impl FnvHasher {
        pub const fn with_key(key: u64) -> Self {
            Self {
                hash: key ^ 0xCBF2_9CE4_8422_2325,
            }
        }
    }

    impl Hasher for FnvHasher {
        fn write(&mut self, bytes: &[u8]) {
            let mut hash = self.hash;
            for &b in bytes {
                hash ^= b as u64;
                hash = hash.wrapping_mul(0x100_0000_01B3);
            }
            self.hash = hash;
        }

        fn finish(&self) -> u64 {
            self.hash
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategic_step_updates_supply_and_hash_chain() {
        let provinces = vec![ProvinceCapsule::new(0, 100_000, 40_000)];
        let mut map = StrategicMapCapsule::new(provinces, None, None);
        map.provinces()[0].set_depot_pressure(50_000);
        let snap1 = map.step(1);
        assert_eq!(snap1.provinces.len(), 1);
        assert_eq!(snap1.supply.pressure.len(), 1);
        assert!(snap1.supply.pressure[0] > 0);
        let snap2 = map.step(2);
        assert_ne!(snap1.hash_chain, snap2.hash_chain);
        assert_eq!(snap2.prev_hash_chain, snap1.hash_chain);
        assert_eq!(snap2.tick, 2);
    }

    #[test]
    fn capture_and_resistance_reduce_supply_and_bump_generation() {
        let mut map = StrategicMapCapsule::new(vec![ProvinceCapsule::new(0, 50_000, 40_000)], None, None);
        map.provinces()[0].set_depot_pressure(50_000);
        let baseline = map.step(1);
        // Capture with resistance/infrastructure damage.
        map.provinces()[0].capture(1, 40_000, 20_000);
        let after = map.step(2);
        assert!(after.provinces[0].generation > baseline.provinces[0].generation);
        assert!(after.supply.pressure[0] < baseline.supply.pressure[0]);
        assert_ne!(baseline.hash_chain, after.hash_chain);
    }

    #[test]
    fn low_infrastructure_and_resistance_raise_decay() {
        let mut map = StrategicMapCapsule::new(vec![ProvinceCapsule::new(0, 80_000, 20_000)], None, None);
        map.provinces()[0].set_depot_pressure(40_000);
        map.provinces()[0].set_resistance(50_000);
        let snap = map.step(1);
        // With heavy resistance/low infra, baggage should be at risk due to weak pressure.
        assert!(snap.supply.baggage_captured || snap.supply.pressure[0] < 20_000);
    }

    #[test]
    fn ownership_change_emits_event() {
        let mut map =
            StrategicMapCapsule::new(vec![ProvinceCapsule::new(0, 50_000, 40_000)], None, None);
        // Baseline step to seed prev_provinces.
        let _ = map.step(1);
        map.provinces()[0].capture(2, 30_000, 10_000);
        let snap = map.step(2);
        assert_eq!(snap.events.len(), 1);
        let ev = &snap.events[0];
        assert_eq!(ev.kind, StrategicEventKind::OwnershipChange);
        assert_eq!(ev.province_id, 0);
        assert_eq!(ev.from_owner_id, 0);
        assert_eq!(ev.to_owner_id, 2);
        assert!(ev.to_infra_q16 < ev.from_infra_q16);
        assert_eq!(ev.generation, snap.provinces[0].generation);
    }

    #[test]
    fn infrastructure_repair_emits_event() {
        let mut map =
            StrategicMapCapsule::new(vec![ProvinceCapsule::new(0, 80_000, 20_000)], None, None);
        let _ = map.step(1);
        map.provinces()[0].set_infrastructure(25_000);
        let snap = map.step(2);
        assert_eq!(snap.events.len(), 1);
        let ev = &snap.events[0];
        assert_eq!(ev.kind, StrategicEventKind::InfrastructureRepair);
        assert_eq!(ev.from_infra_q16, 20_000);
        assert_eq!(ev.to_infra_q16, 25_000);
        assert_eq!(ev.province_id, 0);
    }
}
