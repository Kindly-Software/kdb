use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU32, Ordering};

/// Snapshot of current weather state.
#[derive(Debug, Clone, Copy)]
pub struct WeatherSnapshot {
    pub wind_dir_deg_q16: u32,
    pub wind_speed_q16: u32,
    pub precipitation_q16: u32,
    pub smoke_density_q16: u32,
    pub visibility_q16: u32,
    pub mud_q16: u32,
}

/// Weather capsule: deterministic wind/rain/smoke fields feeding mud/traction/LOS/supply penalties.
///
/// Alignment: 128B to avoid sharing cache lines with neighbors.
#[repr(C, align(128))]
pub struct WeatherCapsule {
    wind_dir_deg_q16: AtomicU32,
    wind_speed_q16: AtomicU32,
    precipitation_q16: AtomicU32,
    smoke_density_q16: AtomicU32,
    visibility_q16: AtomicU32,
    mud_q16: AtomicU32,
    smoke_decay_q16: AtomicU32,
    precip_to_mud_q16: AtomicU32,
    _padding: [u8; 88],
}

impl WeatherCapsule {
    pub const fn new() -> Self {
        Self {
            wind_dir_deg_q16: AtomicU32::new(0),
            wind_speed_q16: AtomicU32::new(0),
            precipitation_q16: AtomicU32::new(0),
            smoke_density_q16: AtomicU32::new(0),
            visibility_q16: AtomicU32::new(65_536),
            mud_q16: AtomicU32::new(0),
            smoke_decay_q16: AtomicU32::new(4_096), // ~6% decay per tick
            precip_to_mud_q16: AtomicU32::new(1_024), // rain → mud accumulation factor
            _padding: [0; 88],
        }
    }

    pub fn set_wind(&self, dir_deg_q16: u32, speed_q16: u32) {
        self.wind_dir_deg_q16.store(dir_deg_q16, Ordering::Release);
        self.wind_speed_q16.store(speed_q16, Ordering::Release);
    }

    pub fn set_precipitation(&self, precipitation_q16: u32) {
        self.precipitation_q16
            .store(precipitation_q16.min(65_536), Ordering::Release);
    }

    pub fn inject_smoke(&self, density_q16: u32) {
        self.smoke_density_q16
            .store(density_q16.min(65_536), Ordering::Release);
    }

    pub fn set_visibility(&self, visibility_q16: u32) {
        self.visibility_q16
            .store(visibility_q16.min(65_536), Ordering::Release);
    }

    pub fn set_precip_to_mud(&self, factor_q16: u32) {
        self.precip_to_mud_q16
            .store(factor_q16.min(65_536), Ordering::Release);
    }

    pub fn set_smoke_decay(&self, decay_q16: u32) {
        self.smoke_decay_q16
            .store(decay_q16.min(65_536), Ordering::Release);
    }

    /// Advance one tick: decay smoke, accumulate mud from precipitation, adjust visibility.
    pub fn step(&self) -> WeatherSnapshot {
        let wind_dir = self.wind_dir_deg_q16.load(Ordering::Relaxed);
        let wind_speed = self.wind_speed_q16.load(Ordering::Relaxed).min(65_536);
        let precip = self.precipitation_q16.load(Ordering::Relaxed).min(65_536);
        let mut smoke = self.smoke_density_q16.load(Ordering::Acquire).min(65_536);
        let visibility = self.visibility_q16.load(Ordering::Relaxed).min(65_536);
        let mut mud = self.mud_q16.load(Ordering::Acquire).min(65_536);

        // Smoke decays deterministically.
        let decay = self.smoke_decay_q16.load(Ordering::Relaxed).min(65_536);
        smoke = ((smoke as u64 * (65_536 - decay) as u64) / 65_536).min(u32::MAX as u64) as u32;
        self.smoke_density_q16.store(smoke, Ordering::Release);

        // Rain/snow increases mud; scale by factor.
        let precip_to_mud = self.precip_to_mud_q16.load(Ordering::Relaxed).min(65_536);
        let mud_gain = ((precip as u64 * precip_to_mud as u64) / 65_536).min(10_000) as u32;
        mud = mud.saturating_add(mud_gain).min(65_536);
        self.mud_q16.store(mud, Ordering::Release);

        WeatherSnapshot {
            wind_dir_deg_q16: wind_dir,
            wind_speed_q16: wind_speed,
            precipitation_q16: precip,
            smoke_density_q16: smoke,
            visibility_q16: visibility,
            mud_q16: mud,
        }
    }

    /// Penalty applied to traction based on accumulated mud (Q16.16).
    pub fn traction_penalty_q16(&self) -> u32 {
        let mud = self.mud_q16.load(Ordering::Acquire).min(65_536);
        (mud / 4).min(30_000)
    }

    /// LOS/dispersion penalty from smoke/fog (Q16.16).
    pub fn los_penalty_q16(&self) -> u32 {
        let smoke = self.smoke_density_q16.load(Ordering::Acquire).min(65_536);
        (smoke / 3).min(40_000)
    }

    /// Supply decay scaling: higher mud/wind increases decay (Q16.16 multiplier).
    pub fn supply_decay_scale_q16(&self) -> u32 {
        let mud_pen = self.traction_penalty_q16();
        let wind = self.wind_speed_q16.load(Ordering::Acquire).min(65_536);
        let penalty = ((mud_pen as u64 / 2) + (wind as u64 / 4)).min(40_000) as u32;
        65_536u32.saturating_sub(penalty)
    }

    /// Visibility scale (Q16.16) combining base visibility and smoke.
    pub fn visibility_scale_q16(&self) -> u32 {
        let vis = self.visibility_q16.load(Ordering::Acquire).min(65_536);
        let smoke_pen = self.los_penalty_q16();
        vis.saturating_sub(smoke_pen).max(16_384) // clamp to avoid zeroing LOS
    }

    pub fn snapshot(&self) -> WeatherSnapshot {
        WeatherSnapshot {
            wind_dir_deg_q16: self.wind_dir_deg_q16.load(Ordering::Acquire),
            wind_speed_q16: self.wind_speed_q16.load(Ordering::Acquire),
            precipitation_q16: self.precipitation_q16.load(Ordering::Acquire),
            smoke_density_q16: self.smoke_density_q16.load(Ordering::Acquire),
            visibility_q16: self.visibility_q16.load(Ordering::Acquire),
            mud_q16: self.mud_q16.load(Ordering::Acquire),
        }
    }
}

verify_capsule_properties!(WeatherCapsule, 128, 128);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rain_accumulates_mud_and_decay_applies_to_smoke() {
        let weather = WeatherCapsule::new();
        weather.set_precipitation(20_000);
        weather.inject_smoke(40_000);
        let snap = weather.step();
        assert!(snap.mud_q16 > 0);
        assert!(snap.smoke_density_q16 < 40_000);
    }

    #[test]
    fn traction_penalty_scales_with_mud() {
        let weather = WeatherCapsule::new();
        weather.set_precipitation(60_000);
        let _ = weather.step();
        assert!(weather.traction_penalty_q16() > 0);
    }

    #[test]
    fn supply_decay_scale_drops_under_bad_weather() {
        let weather = WeatherCapsule::new();
        weather.set_precipitation(60_000);
        weather.set_wind(45 << 16, 50_000);
        let _ = weather.step();
        assert!(weather.supply_decay_scale_q16() < 65_536);
    }
}
