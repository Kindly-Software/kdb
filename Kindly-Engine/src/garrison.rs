use atomic_capsule::{verify_alignment_only, verify_capsule_properties};
use core::sync::atomic::{AtomicU32, Ordering};

/// Maximum concurrent garrison records (preallocated).
pub const GARRISON_CAPACITY: usize = 512;

/// Garrison record capsule: which formation occupies which structure/slot with an aperture heading.
#[repr(C, align(64))]
pub struct GarrisonCapsule {
    structure_id: AtomicU32,
    formation_id: AtomicU32,
    slot: AtomicU32,
    aperture_deg_q16: AtomicU32,
    aperture_width_deg_q16: AtomicU32,
    generation: AtomicU32,
    _padding: [u8; 40],
}

verify_capsule_properties!(GarrisonCapsule, 64, 64);

impl GarrisonCapsule {
    pub fn new() -> Self {
        Self {
            structure_id: AtomicU32::new(0),
            formation_id: AtomicU32::new(u32::MAX),
            slot: AtomicU32::new(0),
            aperture_deg_q16: AtomicU32::new(0),
            aperture_width_deg_q16: AtomicU32::new(0),
            generation: AtomicU32::new(0),
            _padding: [0; 40],
        }
    }

    pub fn occupy(
        &self,
        structure_id: u32,
        formation_id: u32,
        slot: u16,
        aperture_deg_q16: u32,
        aperture_width_deg_q16: u32,
    ) -> bool {
        // Only occupy if currently free (formation_id == u32::MAX).
        let mut current = self.formation_id.load(Ordering::Relaxed);
        while current == u32::MAX {
            match self.formation_id.compare_exchange(
                u32::MAX,
                formation_id,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    self.structure_id.store(structure_id, Ordering::Release);
                    self.slot.store(slot as u32, Ordering::Release);
                    self.aperture_deg_q16
                        .store(aperture_deg_q16, Ordering::Release);
                    self.aperture_width_deg_q16
                        .store(aperture_width_deg_q16, Ordering::Release);
                    self.generation.fetch_add(1, Ordering::AcqRel);
                    return true;
                }
                Err(observed) => current = observed,
            }
        }
        false
    }

    pub fn vacate(&self) {
        self.formation_id.store(u32::MAX, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    pub fn snapshot(&self) -> GarrisonSnapshot {
        GarrisonSnapshot {
            structure_id: self.structure_id.load(Ordering::Relaxed),
            formation_id: self.formation_id.load(Ordering::Relaxed),
            slot: self.slot.load(Ordering::Relaxed) as u16,
            aperture_deg_q16: self.aperture_deg_q16.load(Ordering::Relaxed),
            aperture_width_deg_q16: self.aperture_width_deg_q16.load(Ordering::Relaxed),
            generation: self.generation.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GarrisonSnapshot {
    pub structure_id: u32,
    pub formation_id: u32,
    pub slot: u16,
    pub aperture_deg_q16: u32,
    pub aperture_width_deg_q16: u32,
    pub generation: u32,
}

/// Fixed-capacity garrison slab; uses atomics for lock-free occupy/vacate.
#[repr(C, align(64))]
pub struct GarrisonSlabCapsule {
    records: Vec<GarrisonCapsule>,
    len: AtomicU32,
}

verify_alignment_only!(GarrisonSlabCapsule, 64);

impl GarrisonSlabCapsule {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(GARRISON_CAPACITY);
        let mut records = Vec::with_capacity(cap);
        records.resize_with(cap, GarrisonCapsule::new);
        Self {
            records,
            len: AtomicU32::new(0),
        }
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::Relaxed) as usize
    }

    pub fn occupy(
        &self,
        structure_id: u32,
        formation_id: u32,
        slot: u16,
        aperture_deg_q16: u32,
        aperture_width_deg_q16: u32,
    ) -> Option<usize> {
        let idx = self
            .len
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |cur| {
                if cur as usize >= self.records.len() {
                    None
                } else {
                    Some(cur + 1)
                }
            });
        let idx = match idx {
            Ok(v) => v as usize,
            Err(_) => return None,
        };
        if let Some(rec) = self.records.get(idx) {
            rec.occupy(
                structure_id,
                formation_id,
                slot,
                aperture_deg_q16,
                aperture_width_deg_q16,
            );
            Some(idx)
        } else {
            None
        }
    }

    pub fn vacate(&self, idx: usize) {
        if let Some(rec) = self.records.get(idx) {
            rec.vacate();
        }
    }

    /// Release any record held by the given formation.
    pub fn vacate_formation(&self, formation_id: u32) {
        for rec in &self.records {
            if rec.snapshot().formation_id == formation_id {
                rec.vacate();
            }
        }
    }

    pub fn get(&self, idx: usize) -> Option<&GarrisonCapsule> {
        self.records.get(idx)
    }

    pub fn iter(&self) -> impl Iterator<Item = GarrisonSnapshot> + '_ {
        self.records.iter().map(|r| r.snapshot())
    }

    /// Find the garrison record for a formation, if any.
    pub fn find_by_formation(&self, formation_id: u32) -> Option<GarrisonSnapshot> {
        self.records
            .iter()
            .map(|r| r.snapshot())
            .find(|snap| snap.formation_id == formation_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garrison_slab_occupy_and_vacate() {
        let slab = GarrisonSlabCapsule::new(4);
        let idx = slab.occupy(10, 3, 0, 90 << 16, 45 << 16).unwrap();
        let snap = slab.get(idx).unwrap().snapshot();
        assert_eq!(snap.structure_id, 10);
        assert_eq!(snap.formation_id, 3);
        slab.vacate(idx);
        let vacated = slab.get(idx).unwrap().snapshot();
        assert_eq!(vacated.formation_id, u32::MAX);
    }
}
