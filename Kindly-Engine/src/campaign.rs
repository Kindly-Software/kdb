use crate::diplomacy::{DiplomaticSnapshot, DiplomaticStateCapsule};
use crate::province_economy::{EconomySnapshot, ProvinceEconomyCapsule};
use crate::strategic_map::{StrategicMapCapsule, StrategicSnapshot};
use atomic_capsule::verify_alignment_only;
use core::hash::{Hash, Hasher};
use core::sync::atomic::{AtomicU64, Ordering};

/// Combined campaign frame: strategic + diplomacy + economy with hash chain.
#[derive(Debug, Clone)]
pub struct CampaignFrame {
    pub tick: u64,
    pub generation: u64,
    pub prev_hash_chain: u64,
    pub hash_chain: u64,
    pub strategic: StrategicSnapshot,
    pub diplomacy: DiplomaticSnapshot,
    pub economy: EconomySnapshot,
    pub war_exhaustion_avg_q16: u32,
}

/// Metacapsule orchestrating strategy/diplomacy/economy layers.
///
/// - Align 128B to avoid false sharing (Chaos).
/// - Hash-chains combined state for replay/audit.
#[repr(C, align(128))]
pub struct CampaignMetacapsule {
    strategic: StrategicMapCapsule,
    diplomacy: DiplomaticStateCapsule,
    economy: ProvinceEconomyCapsule,
    generation: AtomicU64,
    hash_chain: AtomicU64,
    _padding: [u8; 72],
}

verify_alignment_only!(CampaignMetacapsule, 128);

impl CampaignMetacapsule {
    pub fn new(
        strategic: StrategicMapCapsule,
        diplomacy: DiplomaticStateCapsule,
        economy: ProvinceEconomyCapsule,
    ) -> Self {
        Self {
            strategic,
            diplomacy,
            economy,
            generation: AtomicU64::new(1),
            hash_chain: AtomicU64::new(0),
            _padding: [0; 72],
        }
    }

    /// Step the campaign: decay war exhaustion, apply unrest to provinces,
    /// advance economy orders, and publish a combined snapshot bundle.
    pub fn step(&mut self, tick: u64) -> CampaignFrame {
        // Soften war exhaustion over time.
        self.diplomacy.decay_all_war_exhaustion(64);
        let war_exhaustion = self.diplomacy.war_exhaustion_by_faction();
        // Apply unrest pressure to provinces based on owner war exhaustion.
        for province in self.strategic.provinces_mut() {
            let owner = province.owner_id() as usize;
            if let Some(exhaustion) = war_exhaustion.get(owner) {
                let unrest = exhaustion.saturating_div(2).min(50_000);
                let current = province.resistance_q16();
                if unrest > current {
                    province.set_resistance(unrest);
                }
            }
        }

        // Advance economy and merge infra events into strategic stream.
        let economy_events = self.economy.tick(tick, &mut self.strategic);
        let mut strategic = self.strategic.step(tick);
        strategic.events.extend(economy_events);
        let diplomacy = self.diplomacy.snapshot(tick);
        let economy = self.economy.snapshot(tick);

        let prev_hash = self.hash_chain.load(Ordering::Acquire);
        let combined_hash = self.update_hash_chain(
            tick,
            &strategic,
            &diplomacy,
            &economy,
            prev_hash,
        );
        let war_exhaustion_avg_q16 = average_q16(&war_exhaustion);
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        CampaignFrame {
            tick,
            generation,
            prev_hash_chain: prev_hash,
            hash_chain: combined_hash,
            strategic,
            diplomacy,
            economy,
            war_exhaustion_avg_q16,
        }
    }

    pub fn strategic(&self) -> &StrategicMapCapsule {
        &self.strategic
    }

    pub fn diplomacy(&self) -> &DiplomaticStateCapsule {
        &self.diplomacy
    }

    pub fn economy(&self) -> &ProvinceEconomyCapsule {
        &self.economy
    }

    fn update_hash_chain(
        &self,
        tick: u64,
        strategic: &StrategicSnapshot,
        diplomacy: &DiplomaticSnapshot,
        economy: &EconomySnapshot,
        prev: u64,
    ) -> u64 {
        let mut hasher = fnv::FnvHasher::with_key(prev);
        tick.hash(&mut hasher);
        strategic.hash_chain.hash(&mut hasher);
        diplomacy.hash_chain.hash(&mut hasher);
        economy.hash_chain.hash(&mut hasher);
        strategic.provinces.len().hash(&mut hasher);
        diplomacy.relations.len().hash(&mut hasher);
        economy.orders.len().hash(&mut hasher);
        let out = hasher.finish();
        self.hash_chain.store(out, Ordering::Release);
        out
    }
}

fn average_q16(values: &[u32]) -> u32 {
    if values.is_empty() {
        return 0;
    }
    let sum: u64 = values.iter().map(|v| *v as u64).sum();
    (sum / values.len() as u64).min(u32::MAX as u64) as u32
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
            for &b in bytes {
                self.hash ^= b as u64;
                self.hash = self.hash.wrapping_mul(0x1000_0000_01B3);
            }
        }

        fn finish(&self) -> u64 {
            self.hash
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategic_map::{ProvinceCapsule, WeatherKeyframe};

    #[test]
    fn war_exhaustion_raises_resistance_and_hash_chains() {
        let provinces = vec![
            ProvinceCapsule::new(0, 100_000, 40_000),
            ProvinceCapsule::new(1, 80_000, 30_000),
        ];
        let weather = vec![WeatherKeyframe {
            tick: 0,
            precipitation_q16: 10_000,
            wind_speed_q16: 10_000,
            gust_q16: 0,
            wind_dir_deg_q16: 0,
        }];
        let strategic = StrategicMapCapsule::new(provinces, None, Some(weather));
        let diplomacy = {
            let d = DiplomaticStateCapsule::new(2);
            d.set_war(0, 1);
            d.add_war_exhaustion(0, 1, 20_000);
            d
        };
        let economy = ProvinceEconomyCapsule::new();
        let mut campaign = CampaignMetacapsule::new(strategic, diplomacy, economy);

        let frame = campaign.step(1);
        let prov0 = frame.strategic.provinces.get(0).unwrap();
        assert!(prov0.resistance_q16 > 0);
        assert!(frame.hash_chain != 0);
        assert!(frame.generation > 0);
        assert!(frame.war_exhaustion_avg_q16 > 0);
    }

    #[test]
    fn economy_events_are_merged_into_strategic() {
        let provinces = vec![ProvinceCapsule::new(0, 100_000, 30_000)];
        let strategic = StrategicMapCapsule::new(provinces, None, None);
        let diplomacy = DiplomaticStateCapsule::new(1);
        let mut economy = ProvinceEconomyCapsule::new();
        economy.enqueue_infrastructure(0, 35_000, 1);
        let mut campaign = CampaignMetacapsule::new(strategic, diplomacy, economy);

        let frame = campaign.step(1);
        // Events should include the infra repair completion after one tick.
        assert!(!frame.strategic.events.is_empty());
        assert_eq!(frame.strategic.events[0].kind as u8, 1); // InfrastructureRepair
    }
}
