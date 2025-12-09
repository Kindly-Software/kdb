use atomic_capsule::{verify_alignment_only, verify_capsule_properties};
use core::sync::atomic::{AtomicU32, Ordering};

pub const STRUCTURE_PAGE_SIZE: usize = 1024;

/// Fortification/structure capsule: cover strips + breach/occupancy state.
#[repr(C, align(64))]
pub struct StructureCapsule {
    structure_id: AtomicU32,
    pos_x_q16: AtomicU32,
    pos_z_q16: AtomicU32,
    half_extent_x_q16: AtomicU32,
    half_extent_z_q16: AtomicU32,
    cover_q16: [AtomicU32; 4], // front, right, back, left
    breach_mask: AtomicU32,
    slots_total: u16,
    apertures: u16,
    slots_used: AtomicU32,
    generation: AtomicU32,
    _padding: [u8; 12],
}

verify_capsule_properties!(StructureCapsule, 64, 64);

impl StructureCapsule {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        structure_id: u32,
        pos_x_q16: u32,
        pos_z_q16: u32,
        half_extent_x_q16: u32,
        half_extent_z_q16: u32,
        cover_front_q16: u32,
        cover_right_q16: u32,
        cover_back_q16: u32,
        cover_left_q16: u32,
        slots_total: u16,
        apertures: u16,
    ) -> Self {
        Self {
            structure_id: AtomicU32::new(structure_id),
            pos_x_q16: AtomicU32::new(pos_x_q16),
            pos_z_q16: AtomicU32::new(pos_z_q16),
            half_extent_x_q16: AtomicU32::new(half_extent_x_q16),
            half_extent_z_q16: AtomicU32::new(half_extent_z_q16),
            cover_q16: [
                AtomicU32::new(cover_front_q16),
                AtomicU32::new(cover_right_q16),
                AtomicU32::new(cover_back_q16),
                AtomicU32::new(cover_left_q16),
            ],
            breach_mask: AtomicU32::new(0),
            slots_total,
            apertures,
            slots_used: AtomicU32::new(0),
            generation: AtomicU32::new(0),
            _padding: [0; 12],
        }
    }

    pub fn snapshot(&self) -> StructureSnapshot {
        StructureSnapshot {
            structure_id: self.structure_id.load(Ordering::Relaxed),
            position_x_q16: self.pos_x_q16.load(Ordering::Relaxed),
            position_z_q16: self.pos_z_q16.load(Ordering::Relaxed),
            half_extent_x_q16: self.half_extent_x_q16.load(Ordering::Relaxed),
            half_extent_z_q16: self.half_extent_z_q16.load(Ordering::Relaxed),
            cover_q16: [
                self.cover_q16[0].load(Ordering::Relaxed),
                self.cover_q16[1].load(Ordering::Relaxed),
                self.cover_q16[2].load(Ordering::Relaxed),
                self.cover_q16[3].load(Ordering::Relaxed),
            ],
            breach_mask: self.breach_mask.load(Ordering::Relaxed),
            slots_total: self.slots_total,
            slots_used: self.slots_used.load(Ordering::Relaxed) as u16,
            apertures: self.apertures,
            generation: self.generation.load(Ordering::Relaxed),
        }
    }

    /// True if the given Q16.16 world position lies within the structure bounds.
    pub fn contains_q16(&self, x_q16: u32, z_q16: u32) -> bool {
        self.snapshot().contains_q16(x_q16, z_q16)
    }

    /// Face-aware cover toward an incoming point (Q16.16). Returns (cover_q16, face_idx).
    pub fn cover_toward(&self, incoming_from_x_q16: u32, incoming_from_z_q16: u32) -> (u32, usize) {
        self.snapshot()
            .cover_toward(incoming_from_x_q16, incoming_from_z_q16)
    }

    pub fn from_snapshot(snap: StructureSnapshot) -> Self {
        Self {
            structure_id: AtomicU32::new(snap.structure_id),
            pos_x_q16: AtomicU32::new(snap.position_x_q16),
            pos_z_q16: AtomicU32::new(snap.position_z_q16),
            half_extent_x_q16: AtomicU32::new(snap.half_extent_x_q16),
            half_extent_z_q16: AtomicU32::new(snap.half_extent_z_q16),
            cover_q16: [
                AtomicU32::new(snap.cover_q16[0]),
                AtomicU32::new(snap.cover_q16[1]),
                AtomicU32::new(snap.cover_q16[2]),
                AtomicU32::new(snap.cover_q16[3]),
            ],
            breach_mask: AtomicU32::new(snap.breach_mask),
            slots_total: snap.slots_total,
            apertures: snap.apertures,
            slots_used: AtomicU32::new(snap.slots_used as u32),
            generation: AtomicU32::new(snap.generation),
            _padding: [0; 12],
        }
    }

    /// Apply a breach mask and decay cover by a Q16.16 scale (≤1.0).
    pub fn apply_breach(&self, breach_bits: u32, cover_scale_q16: u32) {
        self.breach_mask.fetch_or(breach_bits, Ordering::AcqRel);
        for cover in &self.cover_q16 {
            let _ = cover.fetch_update(Ordering::AcqRel, Ordering::Relaxed, |cur| {
                let scaled = ((cur as u64 * cover_scale_q16 as u64) / 65_536) as u32;
                Some(scaled)
            });
        }
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Seal a breach mask (clear bits) and gently restore cover by a Q16.16 scale (≥1.0).
    pub fn seal_breach(&self, breach_bits: u32, cover_scale_q16: u32) {
        self.breach_mask.fetch_and(!breach_bits, Ordering::AcqRel);
        for cover in &self.cover_q16 {
            let _ = cover.fetch_update(Ordering::AcqRel, Ordering::Relaxed, |cur| {
                let scaled = ((cur as u64 * cover_scale_q16 as u64) / 65_536)
                    .min(u32::MAX as u64) as u32;
                Some(scaled)
            });
        }
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Reserve a garrison slot if available.
    pub fn reserve_slot(&self) -> bool {
        self.slots_used
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |cur| {
                if cur < self.slots_total as u32 {
                    Some(cur + 1)
                } else {
                    None
                }
            })
            .is_ok()
    }

    pub fn release_slot(&self) {
        let _ = self
            .slots_used
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |cur| {
                if cur == 0 {
                    None
                } else {
                    Some(cur - 1)
                }
            });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructureSnapshot {
    pub structure_id: u32,
    pub position_x_q16: u32,
    pub position_z_q16: u32,
    pub half_extent_x_q16: u32,
    pub half_extent_z_q16: u32,
    pub cover_q16: [u32; 4],
    pub breach_mask: u32,
    pub slots_total: u16,
    pub slots_used: u16,
    pub apertures: u16,
    pub generation: u32,
}

impl StructureSnapshot {
    /// True if the given Q16.16 world position lies inside the structure AABB.
    #[inline(always)]
    pub fn contains_q16(&self, x_q16: u32, z_q16: u32) -> bool {
        let min_x = self.position_x_q16.saturating_sub(self.half_extent_x_q16);
        let max_x = self.position_x_q16.saturating_add(self.half_extent_x_q16);
        let min_z = self.position_z_q16.saturating_sub(self.half_extent_z_q16);
        let max_z = self.position_z_q16.saturating_add(self.half_extent_z_q16);
        x_q16 >= min_x && x_q16 <= max_x && z_q16 >= min_z && z_q16 <= max_z
    }

    /// Face-aware cover toward an incoming point (Q16.16). Returns (cover_q16, face_idx).
    ///
    /// Face order: 0=front (+Z), 1=right (+X), 2=back (-Z), 3=left (-X).
    #[inline(always)]
    pub fn cover_toward(&self, incoming_from_x_q16: u32, incoming_from_z_q16: u32) -> (u32, usize) {
        let dx = incoming_from_x_q16 as i64 - self.position_x_q16 as i64;
        let dz = incoming_from_z_q16 as i64 - self.position_z_q16 as i64;
        let abs_dx = dx.abs();
        let abs_dz = dz.abs();
        // Primary face is the dominant axis; secondary softens diagonal hits.
        let (primary, secondary) = if abs_dx >= abs_dz {
            let primary = if dx >= 0 { 1 } else { 3 };
            let secondary = if dz >= 0 { 0 } else { 2 };
            (primary, secondary)
        } else {
            let primary = if dz >= 0 { 0 } else { 2 };
            let secondary = if dx >= 0 { 1 } else { 3 };
            (primary, secondary)
        };
        let primary_cover = self.cover_q16[primary];
        let secondary_cover = self.cover_q16[secondary];
        let mut cover =
            ((primary_cover as u64 * 3 + secondary_cover as u64) / 4).min(u32::MAX as u64) as u32;
        // Breach mask: face bit halves cover; any breach shaves ~25% to mimic collapsing walls.
        let face_bit = 1u32 << primary;
        if self.breach_mask & face_bit != 0 {
            cover = ((cover as u64 * 32_768) / 65_536) as u32;
        } else if self.breach_mask != 0 {
            cover = ((cover as u64 * 48_000) / 65_536) as u32;
        }
        (cover, primary)
    }
}

pub fn find_structure_hit<'a>(
    structures: &'a [StructureCapsule],
    target_x_q16: u32,
    target_z_q16: u32,
    incoming_from_x_q16: u32,
    incoming_from_z_q16: u32,
) -> Option<(&'a StructureCapsule, u32, usize)> {
    structures.iter().find_map(|s| {
        let snap = s.snapshot();
        if snap.contains_q16(target_x_q16, target_z_q16) {
            let (cover, face) = snap.cover_toward(incoming_from_x_q16, incoming_from_z_q16);
            Some((s, cover, face))
        } else {
            None
        }
    })
}

pub fn breached_structure_count(structures: &[StructureCapsule]) -> usize {
    structures
        .iter()
        .filter(|s| s.snapshot().breach_mask != 0)
        .count()
}

struct StructurePage {
    structures: Vec<StructureCapsule>,
    len: usize,
}

impl StructurePage {
    fn new() -> Self {
        let mut structures = Vec::with_capacity(STRUCTURE_PAGE_SIZE);
        structures.resize_with(STRUCTURE_PAGE_SIZE, || {
            StructureCapsule::new(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)
        });
        Self { structures, len: 0 }
    }
}

/// Paged structure slab for deterministic placement (no moves/reallocations of elements).
#[repr(C, align(64))]
pub struct StructureSlabCapsule {
    pages: Vec<StructurePage>,
    len: usize,
}

verify_alignment_only!(StructureSlabCapsule, 64);

impl StructureSlabCapsule {
    pub fn new(initial_pages: usize) -> Self {
        let mut pages = Vec::with_capacity(initial_pages.max(1));
        for _ in 0..initial_pages.max(1) {
            pages.push(StructurePage::new());
        }
        Self { pages, len: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push(&mut self, structure: StructureCapsule) -> usize {
        let idx = self.len;
        let page_idx = idx / STRUCTURE_PAGE_SIZE;
        let offset = idx % STRUCTURE_PAGE_SIZE;
        self.ensure_page(page_idx);
        self.pages[page_idx].structures[offset] = structure;
        self.pages[page_idx].len = offset + 1;
        self.len += 1;
        idx
    }

    pub fn snapshot(&self, idx: usize) -> Option<StructureSnapshot> {
        let page_idx = idx / STRUCTURE_PAGE_SIZE;
        let offset = idx % STRUCTURE_PAGE_SIZE;
        self.pages
            .get(page_idx)
            .and_then(|p| p.structures.get(offset))
            .map(|s| s.snapshot())
    }

    pub fn iter(&self) -> StructureIter<'_> {
        StructureIter { slab: self, idx: 0 }
    }

    fn ensure_page(&mut self, idx: usize) {
        while self.pages.len() <= idx {
            self.pages.push(StructurePage::new());
        }
    }
}

pub struct StructureIter<'a> {
    slab: &'a StructureSlabCapsule,
    idx: usize,
}

impl<'a> Iterator for StructureIter<'a> {
    type Item = StructureSnapshot;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.slab.len {
            return None;
        }
        let snap = self.slab.snapshot(self.idx);
        self.idx += 1;
        snap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_slab_grows_and_snapshots() {
        let mut slab = StructureSlabCapsule::new(1);
        for i in 0..(STRUCTURE_PAGE_SIZE + 4) {
            let idx = slab.push(StructureCapsule::new(
                i as u32, 10, 20, 5, 6, 40_000, 40_000, 40_000, 40_000, 12, 6,
            ));
            assert_eq!(idx, i);
        }
        assert_eq!(slab.len(), STRUCTURE_PAGE_SIZE + 4);
        let snap = slab.snapshot(STRUCTURE_PAGE_SIZE + 1).unwrap();
        assert_eq!(snap.structure_id, (STRUCTURE_PAGE_SIZE + 1) as u32);
    }
}
