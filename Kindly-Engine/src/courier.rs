use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Courier snapshot for telemetry/replay.
#[derive(Debug, Clone, Copy)]
pub struct CourierSnapshot {
    pub id: u64,
    pub origin: u32,
    pub dest: u32,
    pub eta_ticks: u32,
    pub intercepted: bool,
    pub spoofed: bool,
}

/// Lightweight debug snapshot for overlays/analytics.
#[derive(Debug, Clone, Copy)]
pub struct CourierDebugSnapshot {
    pub base_eta_ticks: u32,
    pub cadence_ticks: u32,
    pub deliveries: u64,
    pub losses: u64,
    pub spoofed: u64,
}

/// Doctrine presets influencing courier cadence and retreat posture.
#[derive(Debug, Clone, Copy)]
pub struct Doctrine {
    pub cadence_ticks: u32,
    pub retreat_posture: u8,
    pub spoofable: bool,
}

impl Doctrine {
    pub const fn aggressive() -> Self {
        Self {
            cadence_ticks: 8,
            retreat_posture: 0,
            spoofable: false,
        }
    }
    pub const fn defensive() -> Self {
        Self {
            cadence_ticks: 16,
            retreat_posture: 1,
            spoofable: true,
        }
    }
}

/// Courier capsule: deterministic physical order delivery with interception/spoof hooks.
#[repr(C, align(128))]
pub struct CourierCapsule {
    next_id: AtomicU64,
    deliveries: AtomicU64,
    losses: AtomicU64,
    spoofed: AtomicU64,
    doctrine: Doctrine,
    base_eta_ticks: AtomicU32,
    _padding: [u8; 84],
}

impl CourierCapsule {
    pub const fn new(doctrine: Doctrine, base_eta_ticks: u32) -> Self {
        Self {
            next_id: AtomicU64::new(1),
            deliveries: AtomicU64::new(0),
            losses: AtomicU64::new(0),
            spoofed: AtomicU64::new(0),
            doctrine,
            base_eta_ticks: AtomicU32::new(base_eta_ticks),
            _padding: [0; 84],
        }
    }

    /// Spawn a courier for an order; returns its snapshot (caller stores it).
    pub fn dispatch(&self, origin: u32, dest: u32, distance_tiles: u32) -> CourierSnapshot {
        let id = self.next_id.fetch_add(1, Ordering::AcqRel);
        let eta = self.eta_ticks(distance_tiles);
        CourierSnapshot {
            id,
            origin,
            dest,
            eta_ticks: eta,
            intercepted: false,
            spoofed: false,
        }
    }

    /// Compute ETA in ticks deterministically from distance and doctrine cadence.
    pub fn eta_ticks(&self, distance_tiles: u32) -> u32 {
        let base = self.base_eta_ticks.load(Ordering::Relaxed).max(1);
        let cadence = self.doctrine.cadence_ticks.max(1);
        base + distance_tiles / 4 + cadence / 2
    }

    /// Mark a courier as delivered.
    pub fn delivered(&self) {
        self.deliveries.fetch_add(1, Ordering::Release);
    }

    /// Mark a courier as intercepted; optionally spoof it.
    pub fn intercepted(&self, allow_spoof: bool) -> bool {
        self.losses.fetch_add(1, Ordering::Release);
        if allow_spoof && self.doctrine.spoofable {
            self.spoofed.fetch_add(1, Ordering::Release);
            true
        } else {
            false
        }
    }

    pub fn snapshot(&self) -> (u64, u64, u64) {
        (
            self.deliveries.load(Ordering::Acquire),
            self.losses.load(Ordering::Acquire),
            self.spoofed.load(Ordering::Acquire),
        )
    }

    /// Gather debug stats for overlays/telemetry without mutating state.
    pub fn debug_snapshot(&self) -> CourierDebugSnapshot {
        CourierDebugSnapshot {
            base_eta_ticks: self.base_eta_ticks.load(Ordering::Relaxed).max(1),
            cadence_ticks: self.doctrine.cadence_ticks.max(1),
            deliveries: self.deliveries.load(Ordering::Relaxed),
            losses: self.losses.load(Ordering::Relaxed),
            spoofed: self.spoofed.load(Ordering::Relaxed),
        }
    }
}

verify_capsule_properties!(CourierCapsule, 128, 128);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_sets_eta_and_ids() {
        let courier = CourierCapsule::new(Doctrine::aggressive(), 6);
        let a = courier.dispatch(1, 2, 20);
        let b = courier.dispatch(1, 3, 4);
        assert!(a.id != b.id);
        assert!(a.eta_ticks > 0);
        assert!(b.eta_ticks >= a.eta_ticks.saturating_sub(10));
    }

    #[test]
    fn interception_can_spoof_when_allowed() {
        let courier = CourierCapsule::new(Doctrine::defensive(), 4);
        let spoofed = courier.intercepted(true);
        assert!(spoofed);
        let (_, losses, sp) = courier.snapshot();
        assert_eq!(losses, 1);
        assert_eq!(sp, 1);
    }
}
