use atomic_capsule::verify_alignment_only;
use core::hash::{Hash, Hasher};
use core::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

/// Diplomatic relation state between two factions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiplomaticState {
    Peace = 0,
    War = 1,
    Truce = 2,
    Alliance = 3,
}

impl DiplomaticState {
    pub fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Peace),
            1 => Some(Self::War),
            2 => Some(Self::Truce),
            3 => Some(Self::Alliance),
            _ => None,
        }
    }
}

/// Immutable snapshot of a single relation.
#[derive(Debug, Clone)]
pub struct DiplomaticRelationSnapshot {
    pub a: u16,
    pub b: u16,
    pub state: DiplomaticState,
    pub truce_until_tick: u64,
    pub casus_belli_until_tick: u64,
    pub war_exhaustion_q16: u32,
    pub generation: u64,
}

/// Immutable snapshot of the full diplomatic graph.
#[derive(Debug, Clone)]
pub struct DiplomaticSnapshot {
    pub tick: u64,
    pub generation: u64,
    pub hash_chain: u64,
    pub prev_hash_chain: u64,
    pub relations: Vec<DiplomaticRelationSnapshot>,
}

/// Relation capsule: aligned and generation-tracked to avoid false sharing.
#[repr(C, align(64))]
pub struct DiplomaticRelationCapsule {
    state: AtomicU8,
    truce_until_tick: AtomicU64,
    casus_belli_until_tick: AtomicU64,
    war_exhaustion_q16: AtomicU32,
    generation: AtomicU64,
    _padding: [u8; 32],
}

impl DiplomaticRelationCapsule {
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(DiplomaticState::Peace as u8),
            truce_until_tick: AtomicU64::new(0),
            casus_belli_until_tick: AtomicU64::new(0),
            war_exhaustion_q16: AtomicU32::new(0),
            generation: AtomicU64::new(1),
            _padding: [0; 32],
        }
    }

    #[inline]
    fn bump_generation(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    #[inline]
    fn set_state(&self, state: DiplomaticState) {
        self.state.store(state as u8, Ordering::Release);
        self.bump_generation();
    }

    #[inline]
    fn set_truce_until(&self, tick: u64) {
        self.truce_until_tick.store(tick, Ordering::Release);
        self.bump_generation();
    }

    #[inline]
    fn set_casus_belli_until(&self, tick: u64) {
        self.casus_belli_until_tick.store(tick, Ordering::Release);
        self.bump_generation();
    }

    #[inline]
    fn add_war_exhaustion(&self, delta_q16: u32) {
        let cur = self.war_exhaustion_q16.load(Ordering::Relaxed);
        let next = cur.saturating_add(delta_q16).min(65_536);
        self.war_exhaustion_q16.store(next, Ordering::Release);
        self.bump_generation();
    }

    #[inline]
    fn decay_war_exhaustion(&self, decay_q16: u32) {
        let cur = self.war_exhaustion_q16.load(Ordering::Relaxed);
        let next = cur.saturating_sub(decay_q16);
        self.war_exhaustion_q16.store(next, Ordering::Release);
        self.bump_generation();
    }

    fn snapshot(&self, a: u16, b: u16) -> DiplomaticRelationSnapshot {
        DiplomaticRelationSnapshot {
            a,
            b,
            state: DiplomaticState::from_u8(self.state.load(Ordering::Acquire))
                .unwrap_or(DiplomaticState::Peace),
            truce_until_tick: self.truce_until_tick.load(Ordering::Acquire),
            casus_belli_until_tick: self.casus_belli_until_tick.load(Ordering::Acquire),
            war_exhaustion_q16: self.war_exhaustion_q16.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
        }
    }
}

verify_alignment_only!(DiplomaticRelationCapsule, 64);

/// Diplomatic state graph capsule (faction-faction relations).
#[repr(C, align(128))]
pub struct DiplomaticStateCapsule {
    faction_count: usize,
    relations: Vec<DiplomaticRelationCapsule>,
    generation: AtomicU64,
    hash_chain: AtomicU64,
    last_tick: AtomicU64,
    _padding: [u8; 88],
}

verify_alignment_only!(DiplomaticStateCapsule, 128);

impl DiplomaticStateCapsule {
    pub fn new(faction_count: usize) -> Self {
        let mut relations = Vec::with_capacity(faction_count * faction_count);
        for _ in 0..faction_count * faction_count {
            relations.push(DiplomaticRelationCapsule::new());
        }
        Self {
            faction_count,
            relations,
            generation: AtomicU64::new(1),
            hash_chain: AtomicU64::new(0),
            last_tick: AtomicU64::new(0),
            _padding: [0; 88],
        }
    }

    #[inline]
    fn idx(&self, a: u16, b: u16) -> Option<usize> {
        let fa = a as usize;
        let fb = b as usize;
        if fa >= self.faction_count || fb >= self.faction_count {
            return None;
        }
        Some(fa * self.faction_count + fb)
    }

    fn apply_pair<F: Fn(&DiplomaticRelationCapsule)>(&self, a: u16, b: u16, f: F) {
        if let (Some(iab), Some(iba)) = (self.idx(a, b), self.idx(b, a)) {
            f(&self.relations[iab]);
            if iab != iba {
                f(&self.relations[iba]);
            }
            self.generation.fetch_add(1, Ordering::AcqRel);
        }
    }

    pub fn set_war(&self, a: u16, b: u16) {
        self.apply_pair(a, b, |rel| rel.set_state(DiplomaticState::War));
    }

    pub fn set_peace(&self, a: u16, b: u16) {
        self.apply_pair(a, b, |rel| {
            rel.set_state(DiplomaticState::Peace);
            rel.decay_war_exhaustion(rel.war_exhaustion_q16.load(Ordering::Relaxed));
        });
    }

    pub fn set_truce(&self, a: u16, b: u16, until_tick: u64) {
        self.apply_pair(a, b, |rel| {
            rel.set_state(DiplomaticState::Truce);
            rel.set_truce_until(until_tick);
        });
    }

    pub fn set_alliance(&self, a: u16, b: u16) {
        self.apply_pair(a, b, |rel| rel.set_state(DiplomaticState::Alliance));
    }

    pub fn set_casus_belli(&self, a: u16, b: u16, until_tick: u64) {
        self.apply_pair(a, b, |rel| rel.set_casus_belli_until(until_tick));
    }

    pub fn add_war_exhaustion(&self, a: u16, b: u16, delta_q16: u32) {
        self.apply_pair(a, b, |rel| rel.add_war_exhaustion(delta_q16));
    }

    /// Apply uniform decay to all relations (e.g., per strategic tick).
    pub fn decay_all_war_exhaustion(&self, decay_q16: u32) {
        for rel in &self.relations {
            rel.decay_war_exhaustion(decay_q16);
        }
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    pub fn snapshot(&self, tick: u64) -> DiplomaticSnapshot {
        self.last_tick.store(tick, Ordering::Release);
        let mut rels = Vec::with_capacity(self.faction_count.saturating_mul(self.faction_count) / 2);
        for a in 0..self.faction_count {
            for b in (a + 1)..self.faction_count {
                if let Some(idx) = self.idx(a as u16, b as u16) {
                    rels.push(self.relations[idx].snapshot(a as u16, b as u16));
                }
            }
        }
        let prev_hash = self.hash_chain.load(Ordering::Acquire);
        let hash = self.update_hash_chain(tick, &rels);
        DiplomaticSnapshot {
            tick,
            generation: self.generation.load(Ordering::Acquire),
            hash_chain: hash,
            prev_hash_chain: prev_hash,
            relations: rels,
        }
    }

    fn update_hash_chain(
        &self,
        tick: u64,
        relations: &[DiplomaticRelationSnapshot],
    ) -> u64 {
        let mut hasher = fnv::FnvHasher::with_key(self.hash_chain.load(Ordering::Acquire));
        tick.hash(&mut hasher);
        relations.len().hash(&mut hasher);
        for rel in relations.iter().take(8) {
            rel.a.hash(&mut hasher);
            rel.b.hash(&mut hasher);
            (rel.state as u8).hash(&mut hasher);
            rel.truce_until_tick.hash(&mut hasher);
            rel.casus_belli_until_tick.hash(&mut hasher);
            rel.war_exhaustion_q16.hash(&mut hasher);
            rel.generation.hash(&mut hasher);
        }
        let out = hasher.finish();
        self.hash_chain.store(out, Ordering::Release);
        out
    }
}

// Minimal FNV hasher (u64 seed) for hash-chaining.
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
    fn set_war_applies_bidirectionally() {
        let dip = DiplomaticStateCapsule::new(3);
        dip.set_war(0, 1);
        let snap = dip.snapshot(10);
        let ab = snap
            .relations
            .iter()
            .find(|r| r.a == 0 && r.b == 1)
            .unwrap();
        assert_eq!(ab.state, DiplomaticState::War);
        assert_eq!(ab.war_exhaustion_q16, 0);
    }

    #[test]
    fn truce_and_cb_apply_ticks() {
        let dip = DiplomaticStateCapsule::new(2);
        dip.set_truce(0, 1, 50);
        dip.set_casus_belli(0, 1, 100);
        let snap = dip.snapshot(20);
        let rel = snap.relations.first().unwrap();
        assert_eq!(rel.state, DiplomaticState::Truce);
        assert_eq!(rel.truce_until_tick, 50);
        assert_eq!(rel.casus_belli_until_tick, 100);
    }

    #[test]
    fn war_exhaustion_accumulates_and_hash_updates() {
        let dip = DiplomaticStateCapsule::new(2);
        let snap1 = dip.snapshot(1);
        dip.add_war_exhaustion(0, 1, 10_000);
        dip.set_war(0, 1);
        let snap2 = dip.snapshot(2);
        let rel = snap2.relations.first().unwrap();
        assert!(rel.war_exhaustion_q16 >= 10_000);
        assert_ne!(snap1.hash_chain, snap2.hash_chain);
        assert_eq!(snap2.prev_hash_chain, snap1.hash_chain);
    }
}
