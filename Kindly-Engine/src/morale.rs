use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU32, Ordering};

/// Morale adjacency snapshot for overlays/telemetry.
#[derive(Debug, Clone)]
pub struct MoraleSnapshot {
    pub morale_q16: Vec<u32>,
    pub cohesion_q16: Vec<u32>,
    pub steady_ticks: Vec<u32>,
}

/// Morale network capsule: propagates shock/steady effects across adjacency links.
#[repr(C, align(128))]
pub struct MoraleNetworkCapsule {
    adjacency: Vec<(u32, u32)>,
    morale_q16: Vec<AtomicU32>,
    cohesion_q16: Vec<AtomicU32>,
    steady_ticks: Vec<AtomicU32>,
    shock_decay_q16: AtomicU32,
    steady_bonus_q16: AtomicU32,
    routed_threshold_q16: AtomicU32,
    reform_ticks: AtomicU32,
    _padding: [u8; 60],
}

impl MoraleNetworkCapsule {
    /// `nodes` equals formation count; adjacency is undirected edges (u32 indices).
    pub fn new(nodes: usize, adjacency: Vec<(u32, u32)>) -> Self {
        let mut morale = Vec::with_capacity(nodes);
        let mut cohesion = Vec::with_capacity(nodes);
        let mut steady = Vec::with_capacity(nodes);
        for _ in 0..nodes {
            morale.push(AtomicU32::new(50_000));
            cohesion.push(AtomicU32::new(50_000));
            steady.push(AtomicU32::new(0));
        }
        Self {
            adjacency,
            morale_q16: morale,
            cohesion_q16: cohesion,
            steady_ticks: steady,
            shock_decay_q16: AtomicU32::new(2_000), // ~3% per tick
            steady_bonus_q16: AtomicU32::new(512),  // +0.5 per tick when steady-under-fire
            routed_threshold_q16: AtomicU32::new(8_000),
            reform_ticks: AtomicU32::new(10),
            _padding: [0; 60],
        }
    }

    pub fn set_decay(&self, decay_q16: u32) {
        self.shock_decay_q16
            .store(decay_q16.min(65_536), Ordering::Release);
    }

    pub fn set_thresholds(&self, routed_threshold_q16: u32, reform_ticks: u32) {
        self.routed_threshold_q16
            .store(routed_threshold_q16.min(65_536), Ordering::Release);
        self.reform_ticks
            .store(reform_ticks.max(1), Ordering::Release);
    }

    /// Inject a shock into a node and let it propagate to neighbors (simple averaging).
    pub fn apply_shock(&self, node: usize, shock_q16: u32) {
        if let Some(m) = self.morale_q16.get(node) {
            let current = m.load(Ordering::Acquire);
            let drop = shock_q16.min(current);
            m.store(current.saturating_sub(drop), Ordering::Release);
        }
        for &(a, b) in &self.adjacency {
            if a as usize == node {
                self.propagate(b as usize, shock_q16 / 4);
            } else if b as usize == node {
                self.propagate(a as usize, shock_q16 / 4);
            }
        }
    }

    fn propagate(&self, node: usize, shock_q16: u32) {
        if let Some(m) = self.morale_q16.get(node) {
            let current = m.load(Ordering::Acquire);
            m.store(
                current.saturating_sub(shock_q16.min(current)),
                Ordering::Release,
            );
        }
    }

    /// Tick decay and steady-under-fire accumulation; returns routed/reformed indices.
    pub fn step(&self) -> (Vec<usize>, Vec<usize>) {
        let mut routed = Vec::new();
        let mut reformed = Vec::new();
        let decay = self.shock_decay_q16.load(Ordering::Relaxed).min(65_536);
        let steady_bonus = self.steady_bonus_q16.load(Ordering::Relaxed).min(65_536);
        let routed_threshold = self
            .routed_threshold_q16
            .load(Ordering::Relaxed)
            .min(65_536);
        let reform_needed = self.reform_ticks.load(Ordering::Relaxed).max(1);
        for idx in 0..self.morale_q16.len() {
            let m = self.morale_q16[idx].load(Ordering::Acquire);
            let c = self.cohesion_q16[idx].load(Ordering::Acquire);
            let decayed =
                ((m as u64 * (65_536 - decay) as u64) / 65_536).min(u32::MAX as u64) as u32;
            self.morale_q16[idx].store(decayed, Ordering::Release);
            if decayed < routed_threshold {
                routed.push(idx);
                let ticks = self.steady_ticks[idx].fetch_add(1, Ordering::AcqRel) + 1;
                if ticks >= reform_needed {
                    let recovered = (decayed + steady_bonus).min(65_536);
                    self.morale_q16[idx].store(recovered, Ordering::Release);
                    self.cohesion_q16[idx].store(
                        c.saturating_add(steady_bonus / 2).min(65_536),
                        Ordering::Release,
                    );
                    reformed.push(idx);
                    self.steady_ticks[idx].store(0, Ordering::Release);
                }
            } else {
                let ticks = self.steady_ticks[idx].fetch_add(1, Ordering::AcqRel) + 1;
                if ticks >= reform_needed && decayed + steady_bonus < 65_536 {
                    self.morale_q16[idx]
                        .store((decayed + steady_bonus).min(65_536), Ordering::Release);
                    self.cohesion_q16[idx].store(
                        c.saturating_add(steady_bonus / 2).min(65_536),
                        Ordering::Release,
                    );
                    if ticks == reform_needed {
                        reformed.push(idx);
                    }
                }
            }
        }
        (routed, reformed)
    }

    pub fn snapshot(&self) -> MoraleSnapshot {
        let mut morale = Vec::with_capacity(self.morale_q16.len());
        let mut cohesion = Vec::with_capacity(self.cohesion_q16.len());
        let mut steady = Vec::with_capacity(self.steady_ticks.len());
        for idx in 0..self.morale_q16.len() {
            morale.push(self.morale_q16[idx].load(Ordering::Acquire));
            cohesion.push(self.cohesion_q16[idx].load(Ordering::Acquire));
            steady.push(self.steady_ticks[idx].load(Ordering::Acquire));
        }
        MoraleSnapshot {
            morale_q16: morale,
            cohesion_q16: cohesion,
            steady_ticks: steady,
        }
    }
}

verify_capsule_properties!(MoraleNetworkCapsule, 128, 256);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shock_propagates_and_decay_runs() {
        let morale = MoraleNetworkCapsule::new(3, vec![(0, 1), (1, 2)]);
        morale.apply_shock(1, 10_000);
        let snap = morale.snapshot();
        assert!(snap.morale_q16[0] < 50_000);
        assert!(snap.morale_q16[2] < 50_000);
        let (_r, _f) = morale.step();
        let snap2 = morale.snapshot();
        assert!(snap2.morale_q16[1] <= snap.morale_q16[1]);
    }

    #[test]
    fn routed_units_can_reform() {
        let morale = MoraleNetworkCapsule::new(1, vec![]);
        morale.set_thresholds(40_000, 2);
        morale.apply_shock(0, 20_000);
        let (routed, _reformed) = morale.step();
        assert_eq!(routed.len(), 1);
        let (_routed2, reformed2) = morale.step();
        assert!(reformed2.contains(&0));
    }
}
