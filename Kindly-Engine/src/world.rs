use atomic_capsule::{verify_alignment_only, verify_capsule_properties};
use core::sync::atomic::{AtomicU64, Ordering};

use crate::math::{q16_from_meters, Q16_16};

pub const WORLD_PAGE_SIZE: usize = 10_000;

/// Compact soldier/unit capsule (≤24B target payload, packed on 64B).
///
/// - Position: Q16.16 meters (x/z)
/// - Heading/state: u8/u8
/// - Regiment: u16
/// - Vitality: bloodiness u8 (0-255)
#[repr(C, align(64))]
pub struct UnitCapsule {
    pos_x_q16: AtomicU64,
    pos_z_q16: AtomicU64,
    heading_deg_q16: AtomicU64,
    state: AtomicU64, // bits: regiment_id(16) | state(8) | reserved(8) | bloodiness(8) | gen(24)
    _padding: [u8; 32],
}

impl UnitCapsule {
    pub fn new(
        pos_x: Q16_16,
        pos_z: Q16_16,
        heading_deg_q16: u32,
        state: u8,
        regiment_id: u16,
        bloodiness: u8,
    ) -> Self {
        let packed_state = pack_state(regiment_id, state, bloodiness, 0);
        Self {
            pos_x_q16: AtomicU64::new(pos_x.to_raw() as u64),
            pos_z_q16: AtomicU64::new(pos_z.to_raw() as u64),
            heading_deg_q16: AtomicU64::new(heading_deg_q16 as u64),
            state: AtomicU64::new(packed_state),
            _padding: [0; 32],
        }
    }

    pub fn snapshot(&self) -> UnitSnapshot {
        let state = self.state.load(Ordering::Relaxed);
        UnitSnapshot {
            pos_x_q16: self.pos_x_q16.load(Ordering::Relaxed) as u32,
            pos_z_q16: self.pos_z_q16.load(Ordering::Relaxed) as u32,
            heading_deg_q16: self.heading_deg_q16.load(Ordering::Relaxed) as u32,
            regiment_id: ((state >> 48) & 0xFFFF) as u16,
            state: ((state >> 40) & 0xFF) as u8,
            bloodiness: ((state >> 32) & 0xFF) as u8,
            generation: (state & 0xFFFF_FF) as u32,
        }
    }

    pub fn set_position(&self, x_q16: u32, z_q16: u32) {
        self.pos_x_q16.store(x_q16 as u64, Ordering::Release);
        self.pos_z_q16.store(z_q16 as u64, Ordering::Release);
    }

    pub fn set_heading(&self, heading_deg_q16: u32) {
        self.heading_deg_q16
            .store(heading_deg_q16 as u64, Ordering::Release);
    }

    pub fn set_state(&self, state: u8, bloodiness: u8) {
        let mut current = self.state.load(Ordering::Relaxed);
        loop {
            let regiment_id = ((current >> 48) & 0xFFFF) as u16;
            let gen = (current & 0xFFFF_FF) as u32;
            let next = pack_state(regiment_id, state, bloodiness, gen + 1);
            match self
                .state
                .compare_exchange(current, next, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
    }

    pub fn from_snapshot(snap: UnitSnapshot) -> Self {
        Self {
            pos_x_q16: AtomicU64::new(snap.pos_x_q16 as u64),
            pos_z_q16: AtomicU64::new(snap.pos_z_q16 as u64),
            heading_deg_q16: AtomicU64::new(snap.heading_deg_q16 as u64),
            state: AtomicU64::new(pack_state(
                snap.regiment_id,
                snap.state,
                snap.bloodiness,
                snap.generation,
            )),
            _padding: [0; 32],
        }
    }
}

verify_capsule_properties!(UnitCapsule, 64, 64);

impl Default for UnitCapsule {
    fn default() -> Self {
        UnitCapsule::new(q16_from_meters(0.0), q16_from_meters(0.0), 0, 0, 0, 0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UnitSnapshot {
    pub pos_x_q16: u32,
    pub pos_z_q16: u32,
    pub heading_deg_q16: u32,
    pub regiment_id: u16,
    pub state: u8,
    pub bloodiness: u8,
    pub generation: u32,
}

struct UnitPage {
    units: Vec<UnitCapsule>,
    len: usize,
}

impl UnitPage {
    fn new() -> Self {
        let mut units = Vec::with_capacity(WORLD_PAGE_SIZE);
        units.resize_with(WORLD_PAGE_SIZE, UnitCapsule::default);
        Self { units, len: 0 }
    }
}

/// Paged world slab: paged array of units, no moves/reallocations.
#[repr(C, align(64))]
pub struct WorldSlabCapsule {
    pages: Vec<UnitPage>,
    len: usize,
}

verify_alignment_only!(WorldSlabCapsule, 64);

impl WorldSlabCapsule {
    pub fn new(initial_pages: usize) -> Self {
        let mut pages = Vec::with_capacity(initial_pages.max(1));
        for _ in 0..initial_pages.max(1) {
            pages.push(UnitPage::new());
        }
        Self { pages, len: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push_unit(&mut self, unit: UnitCapsule) -> usize {
        let idx = self.len;
        let page_idx = idx / WORLD_PAGE_SIZE;
        let offset = idx % WORLD_PAGE_SIZE;
        self.ensure_page(page_idx);
        self.pages[page_idx].units[offset] = unit;
        self.pages[page_idx].len = offset + 1;
        self.len += 1;
        idx
    }

    pub fn snapshot_unit(&self, idx: usize) -> Option<UnitSnapshot> {
        let page_idx = idx / WORLD_PAGE_SIZE;
        let offset = idx % WORLD_PAGE_SIZE;
        self.pages
            .get(page_idx)
            .and_then(|p| p.units.get(offset))
            .map(|u| u.snapshot())
    }

    pub fn iter(&self) -> WorldIter<'_> {
        WorldIter { slab: self, idx: 0 }
    }

    fn ensure_page(&mut self, idx: usize) {
        while self.pages.len() <= idx {
            self.pages.push(UnitPage::new());
        }
    }
}

pub struct WorldIter<'a> {
    slab: &'a WorldSlabCapsule,
    idx: usize,
}

impl<'a> Iterator for WorldIter<'a> {
    type Item = UnitSnapshot;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.slab.len {
            return None;
        }
        let snap = self.slab.snapshot_unit(self.idx);
        self.idx += 1;
        snap
    }
}

const fn pack_state(regiment_id: u16, state: u8, bloodiness: u8, generation: u32) -> u64 {
    ((regiment_id as u64) << 48)
        | ((state as u64) << 40)
        | ((bloodiness as u64) << 32)
        | (generation as u64 & 0xFFFF_FF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_slab_grows_in_pages() {
        let mut slab = WorldSlabCapsule::new(1);
        for i in 0..(WORLD_PAGE_SIZE + 5) {
            let u = UnitCapsule::new(q16_from_meters(i as f64), q16_from_meters(0.0), 0, 1, 2, 3);
            let idx = slab.push_unit(u);
            assert_eq!(idx, i);
        }
        assert_eq!(slab.len(), WORLD_PAGE_SIZE + 5);
    }

    #[test]
    fn world_slab_iterates() {
        let mut slab = WorldSlabCapsule::new(1);
        let u = UnitCapsule::new(q16_from_meters(10.0), q16_from_meters(5.0), 0, 1, 2, 3);
        slab.push_unit(u);
        let snaps: Vec<_> = slab.iter().collect();
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].regiment_id, 2);
    }

    #[test]
    fn world_slab_rehydrates_across_shards() {
        let mut slab = WorldSlabCapsule::new(1);
        for i in 0..32 {
            let snap = UnitSnapshot {
                pos_x_q16: q16_from_meters(i as f64).to_raw() as u32,
                pos_z_q16: q16_from_meters((i * 2) as f64).to_raw() as u32,
                heading_deg_q16: (i as u32) << 8,
                regiment_id: (i % 7) as u16,
                state: (i % 3) as u8,
                bloodiness: (i % 5) as u8,
                generation: i as u32,
            };
            slab.push_unit(UnitCapsule::from_snapshot(snap));
        }

        let snapshots: Vec<_> = slab.iter().collect();
        let mut shard_even = WorldSlabCapsule::new(1);
        let mut shard_odd = WorldSlabCapsule::new(1);
        for (idx, snap) in snapshots.iter().copied().enumerate() {
            if idx % 2 == 0 {
                shard_even.push_unit(UnitCapsule::from_snapshot(snap));
            } else {
                shard_odd.push_unit(UnitCapsule::from_snapshot(snap));
            }
        }

        let mut rehydrated: Vec<_> = shard_even.iter().chain(shard_odd.iter()).collect();
        let mut original = snapshots;
        rehydrated.sort_by_key(|s| s.generation);
        original.sort_by_key(|s| s.generation);

        assert_eq!(original.len(), rehydrated.len());
        for (a, b) in original.iter().zip(rehydrated.iter()) {
            assert_eq!(a.pos_x_q16, b.pos_x_q16);
            assert_eq!(a.pos_z_q16, b.pos_z_q16);
            assert_eq!(a.heading_deg_q16, b.heading_deg_q16);
            assert_eq!(a.regiment_id, b.regiment_id);
            assert_eq!(a.state, b.state);
            assert_eq!(a.bloodiness, b.bloodiness);
            assert_eq!(a.generation, b.generation);
        }
    }
}
