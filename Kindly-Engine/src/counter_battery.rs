use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU32, Ordering};

const MAX_FLASHES: usize = 8;

/// Counter-battery flash event (ring-buffered).
#[derive(Debug, Clone, Copy)]
struct FlashSample {
    x: u32,
    z: u32,
    intensity_q16: u32,
    tick: u32,
}

/// Capsule for tracking artillery flashes/smoke and selecting counter-battery targets.
///
/// Alignment: 128B to stay isolated; fixed-size ring buffer, lock-free atomics.
#[repr(C, align(128))]
pub struct CounterBatteryCapsule {
    flash_x: [AtomicU32; MAX_FLASHES],
    flash_z: [AtomicU32; MAX_FLASHES],
    flash_intensity_q16: [AtomicU32; MAX_FLASHES],
    flash_tick: [AtomicU32; MAX_FLASHES],
    write_idx: AtomicU32,
    decay_q16: AtomicU32,
    _padding: [u8; 60],
}

impl CounterBatteryCapsule {
    pub const fn new() -> Self {
        const ZERO: AtomicU32 = AtomicU32::new(0);
        Self {
            flash_x: [ZERO; MAX_FLASHES],
            flash_z: [ZERO; MAX_FLASHES],
            flash_intensity_q16: [ZERO; MAX_FLASHES],
            flash_tick: [ZERO; MAX_FLASHES],
            write_idx: AtomicU32::new(0),
            decay_q16: AtomicU32::new(8_192), // ~12% decay per tick
            _padding: [0; 60],
        }
    }

    /// Record an artillery flash/smoke observation (tile coords, Q16.16 intensity, tick).
    pub fn record_flash(&self, x: u32, z: u32, intensity_q16: u32, tick: u32) {
        let idx = (self.write_idx.fetch_add(1, Ordering::AcqRel) % MAX_FLASHES as u32) as usize;
        self.flash_x[idx].store(x, Ordering::Release);
        self.flash_z[idx].store(z, Ordering::Release);
        self.flash_intensity_q16[idx].store(intensity_q16.min(65_536), Ordering::Release);
        self.flash_tick[idx].store(tick, Ordering::Release);
    }

    /// Select the highest-scoring flash after decay; returns target coords if any.
    pub fn select_target(&self, current_tick: u32) -> Option<(u32, u32, u32)> {
        let decay = self.decay_q16.load(Ordering::Relaxed).min(65_536);
        let mut best: Option<FlashSample> = None;
        for i in 0..MAX_FLASHES {
            let intensity = self.flash_intensity_q16[i].load(Ordering::Acquire);
            if intensity == 0 {
                continue;
            }
            let tick = self.flash_tick[i].load(Ordering::Acquire);
            let age = current_tick.saturating_sub(tick).min(255);
            let decay_scale = ((65_536 - decay) as u64)
                .saturating_pow(age as u32)
                .min(u32::MAX as u64) as u32;
            let score =
                ((intensity as u64 * decay_scale as u64) / 65_536).min(u32::MAX as u64) as u32;
            if score == 0 {
                continue;
            }
            if let Some(b) = &best {
                if score > b.intensity_q16 {
                    best = Some(FlashSample {
                        x: self.flash_x[i].load(Ordering::Acquire),
                        z: self.flash_z[i].load(Ordering::Acquire),
                        intensity_q16: score,
                        tick,
                    });
                }
            } else {
                best = Some(FlashSample {
                    x: self.flash_x[i].load(Ordering::Acquire),
                    z: self.flash_z[i].load(Ordering::Acquire),
                    intensity_q16: score,
                    tick,
                });
            }
        }
        best.map(|s| (s.x, s.z, s.intensity_q16))
    }

    pub fn set_decay_q16(&self, decay_q16: u32) {
        self.decay_q16
            .store(decay_q16.min(65_536), Ordering::Release);
    }
}

verify_capsule_properties!(CounterBatteryCapsule, 128, 256);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_strongest_recent_flash() {
        let cb = CounterBatteryCapsule::new();
        cb.record_flash(2, 3, 50_000, 10);
        cb.record_flash(5, 6, 30_000, 12);
        let target = cb.select_target(13).unwrap();
        assert_eq!((target.0, target.1), (2, 3));
    }
}
