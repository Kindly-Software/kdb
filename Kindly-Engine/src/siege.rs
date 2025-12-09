use crate::replay::encode_siege_event_detail;
use crate::structure::StructureCapsule;
use atomic_capsule::{verify_alignment_only, verify_capsule_properties};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

const BREACH_THRESHOLD_Q16: u32 = 24_000;
const REPAIR_THRESHOLD_Q16: u32 = 52_000;

/// Per-wall-section siege capsule: tracks integrity, breach progress, and sapper attrition.
#[repr(C, align(64))]
pub struct SiegeSectionCapsule {
    structure_id: AtomicU32,
    face_idx: AtomicU32,
    base_integrity_q16: AtomicU32,
    integrity_q16: AtomicU32,
    breach_progress_q16: AtomicU32,
    sapper_attrition_q16: AtomicU32,
    breached: AtomicBool,
    generation: AtomicU32,
    _padding: [u8; 20],
}

impl SiegeSectionCapsule {
    pub const fn new(structure_id: u32, face_idx: u32, base_integrity_q16: u32) -> Self {
        Self {
            structure_id: AtomicU32::new(structure_id),
            face_idx: AtomicU32::new(face_idx),
            base_integrity_q16: AtomicU32::new(base_integrity_q16),
            integrity_q16: AtomicU32::new(base_integrity_q16),
            breach_progress_q16: AtomicU32::new(0),
            sapper_attrition_q16: AtomicU32::new(0),
            breached: AtomicBool::new(false),
            generation: AtomicU32::new(0),
            _padding: [0; 20],
        }
    }

    fn snapshot(&self) -> SiegeSectionSnapshot {
        SiegeSectionSnapshot {
            structure_id: self.structure_id.load(Ordering::Relaxed),
            face_idx: self.face_idx.load(Ordering::Relaxed) as u8,
            base_integrity_q16: self.base_integrity_q16.load(Ordering::Relaxed),
            integrity_q16: self.integrity_q16.load(Ordering::Relaxed),
            breach_progress_q16: self.breach_progress_q16.load(Ordering::Relaxed),
            sapper_attrition_q16: self.sapper_attrition_q16.load(Ordering::Relaxed),
            breached: self.breached.load(Ordering::Relaxed),
            generation: self.generation.load(Ordering::Relaxed),
        }
    }

    fn from_snapshot(snap: SiegeSectionSnapshot) -> Self {
        Self {
            structure_id: AtomicU32::new(snap.structure_id),
            face_idx: AtomicU32::new(snap.face_idx as u32),
            base_integrity_q16: AtomicU32::new(snap.base_integrity_q16),
            integrity_q16: AtomicU32::new(snap.integrity_q16),
            breach_progress_q16: AtomicU32::new(snap.breach_progress_q16),
            sapper_attrition_q16: AtomicU32::new(snap.sapper_attrition_q16),
            breached: AtomicBool::new(snap.breached),
            generation: AtomicU32::new(snap.generation),
            _padding: [0; 20],
        }
    }
}

verify_capsule_properties!(SiegeSectionCapsule, 64, 64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiegeSectionSnapshot {
    pub structure_id: u32,
    pub face_idx: u8,
    pub base_integrity_q16: u32,
    pub integrity_q16: u32,
    pub breach_progress_q16: u32,
    pub sapper_attrition_q16: u32,
    pub breached: bool,
    pub generation: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiegeHitEvent {
    pub structure_id: u32,
    pub face_idx: u8,
    pub breached: bool,
    pub integrity_q16: u32,
    pub breach_progress_q16: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SiegeTickSnapshot {
    pub integrity_avg_q16: u32,
    pub breach_progress_avg_q16: u32,
    pub breach_events: u32,
    pub repair_events: u32,
    pub sections_sampled: u32,
}

/// Global siege capsule coordinating all wall sections for a battle.
#[repr(C, align(64))]
pub struct SiegeCapsule {
    sections: Vec<SiegeSectionCapsule>,
    integrity_accum_q64: AtomicU64,
    breach_progress_accum_q64: AtomicU64,
    integrity_samples: AtomicU32,
    breach_events: AtomicU32,
    repair_events: AtomicU32,
    last_tick_recorded: AtomicU64,
    last_event_payload: AtomicU64,
}

verify_alignment_only!(SiegeCapsule, 64);

impl SiegeCapsule {
    pub fn new_from_structures(structures: &[StructureCapsule]) -> Self {
        let mut sections = Vec::with_capacity(structures.len().saturating_mul(4));
        for s in structures {
            let snap = s.snapshot();
            for (face_idx, &cover) in snap.cover_q16.iter().enumerate() {
                let base = cover.max(32_768);
                sections.push(SiegeSectionCapsule::new(
                    snap.structure_id,
                    face_idx as u32,
                    base,
                ));
            }
        }
        Self {
            sections,
            integrity_accum_q64: AtomicU64::new(0),
            breach_progress_accum_q64: AtomicU64::new(0),
            integrity_samples: AtomicU32::new(0),
            breach_events: AtomicU32::new(0),
            repair_events: AtomicU32::new(0),
            last_tick_recorded: AtomicU64::new(0),
            last_event_payload: AtomicU64::new(0),
        }
    }

    pub fn from_snapshots(snaps: &[SiegeSectionSnapshot]) -> Self {
        let sections = snaps
            .iter()
            .map(|s| SiegeSectionCapsule::from_snapshot(*s))
            .collect();
        Self {
            sections,
            integrity_accum_q64: AtomicU64::new(0),
            breach_progress_accum_q64: AtomicU64::new(0),
            integrity_samples: AtomicU32::new(0),
            breach_events: AtomicU32::new(0),
            repair_events: AtomicU32::new(0),
            last_tick_recorded: AtomicU64::new(0),
            last_event_payload: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> SiegeSnapshot {
        SiegeSnapshot {
            sections: self.sections.iter().map(|s| s.snapshot()).collect(),
        }
    }

    fn record_event(
        &self,
        structure_id: u32,
        face_idx: usize,
        breached: bool,
        integrity_q16: u32,
        breach_progress_q16: u32,
    ) {
        let payload = encode_siege_event_detail(
            structure_id,
            face_idx as u8,
            breached,
            integrity_q16,
            breach_progress_q16,
        );
        self.last_event_payload.store(payload, Ordering::Release);
    }

    /// Take the last recorded siege face event (if any) for replay/overlay logging.
    pub fn take_event_payload(&self) -> Option<u64> {
        let payload = self.last_event_payload.swap(0, Ordering::AcqRel);
        if payload == 0 { None } else { Some(payload) }
    }

    /// Apply artillery damage to a structure face using deterministic Q16 math.
    pub fn apply_artillery_hit(
        &self,
        structure: &StructureCapsule,
        face_idx: usize,
        volley: u16,
        expected_casualties: u32,
    ) -> Option<SiegeHitEvent> {
        let section = self.find_section(structure.snapshot().structure_id, face_idx)?;
        let base_integrity = section
            .base_integrity_q16
            .load(Ordering::Relaxed)
            .max(1);
        let damage_q16 = ((expected_casualties as u64 * 32)
            .saturating_add(volley as u64 * 512))
            .min(98_304) as u32;
        let scaled = ((damage_q16 as u64 * 65_536) / base_integrity as u64)
            .min(u32::MAX as u64) as u32;
        let _ = section.breach_progress_q16.fetch_add(scaled, Ordering::AcqRel);
        let mut breached_now = false;
        let mut sealed_now = false;
        let mut new_integrity = section.integrity_q16.load(Ordering::Relaxed);
        let _ = section
            .integrity_q16
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |cur| {
                let next = cur.saturating_sub(damage_q16);
                new_integrity = next;
                Some(next)
            })
            .unwrap_or_else(|v| {
                new_integrity = v;
                v
            });
        if new_integrity <= BREACH_THRESHOLD_Q16 && !section.breached.load(Ordering::Relaxed) {
            breached_now = true;
            section.breached.store(true, Ordering::Release);
            structure.apply_breach(1u32 << face_idx, 48_000);
        }
        if new_integrity >= REPAIR_THRESHOLD_Q16 && section.breached.load(Ordering::Relaxed) {
            sealed_now = true;
            section.breached.store(false, Ordering::Release);
            structure.seal_breach(1u32 << face_idx, 68_000);
        }
        let _ = section.generation.fetch_add(1, Ordering::AcqRel);
        self.integrity_accum_q64
            .fetch_add(new_integrity as u64, Ordering::AcqRel);
        self.breach_progress_accum_q64
            .fetch_add(section.breach_progress_q16.load(Ordering::Relaxed) as u64, Ordering::AcqRel);
        self.integrity_samples.fetch_add(1, Ordering::AcqRel);
        if breached_now {
            self.breach_events.fetch_add(1, Ordering::AcqRel);
        } else if sealed_now {
            self.repair_events.fetch_add(1, Ordering::AcqRel);
        }
        if breached_now || sealed_now {
            let sid = section.structure_id.load(Ordering::Relaxed);
            let progress = section.breach_progress_q16.load(Ordering::Relaxed);
            self.record_event(sid, face_idx, breached_now, new_integrity, progress);
        }
        Some(SiegeHitEvent {
            structure_id: section.structure_id.load(Ordering::Relaxed),
            face_idx: face_idx as u8,
            breached: breached_now,
            integrity_q16: new_integrity,
            breach_progress_q16: section.breach_progress_q16.load(Ordering::Relaxed),
        })
    }

    /// Apply sapper attrition (fixed-point) to simulate undermining walls.
    pub fn apply_sapper_attrition(
        &self,
        structure_id: u32,
        face_idx: usize,
        attrition_q16: u32,
    ) -> bool {
        let section = match self.find_section(structure_id, face_idx) {
            Some(s) => s,
            None => return false,
        };
        let _ = section
            .sapper_attrition_q16
            .fetch_add(attrition_q16, Ordering::AcqRel);
        let _ = section.generation.fetch_add(1, Ordering::AcqRel);
        self.integrity_accum_q64
            .fetch_add(section.integrity_q16.load(Ordering::Relaxed) as u64, Ordering::AcqRel);
        self.integrity_samples.fetch_add(1, Ordering::AcqRel);
        true
    }

    /// Repair a wall face using engineering resources; may seal a breach if integrity recovers.
    pub fn apply_repair(
        &self,
        structure: &StructureCapsule,
        face_idx: usize,
        repair_q16: u32,
    ) -> Option<SiegeHitEvent> {
        let section = self.find_section(structure.snapshot().structure_id, face_idx)?;
        let mut new_integrity = section.integrity_q16.load(Ordering::Relaxed);
        let _ = section
            .integrity_q16
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |cur| {
                let next = cur.saturating_add(repair_q16).min(98_304);
                new_integrity = next;
                Some(next)
            })
            .unwrap_or_else(|v| {
                new_integrity = v;
                v
            });
        let breached_before = section.breached.load(Ordering::Relaxed);
        if new_integrity >= REPAIR_THRESHOLD_Q16 && breached_before {
            section.breached.store(false, Ordering::Release);
            structure.seal_breach(1u32 << face_idx, 70_000);
            self.repair_events.fetch_add(1, Ordering::AcqRel);
            let progress = section.breach_progress_q16.load(Ordering::Relaxed);
            let sid = section.structure_id.load(Ordering::Relaxed);
            self.record_event(sid, face_idx, false, new_integrity, progress);
        }
        self.integrity_accum_q64
            .fetch_add(new_integrity as u64, Ordering::AcqRel);
        self.breach_progress_accum_q64
            .fetch_add(section.breach_progress_q16.load(Ordering::Relaxed) as u64, Ordering::AcqRel);
        self.integrity_samples.fetch_add(1, Ordering::AcqRel);
        let _ = section.generation.fetch_add(1, Ordering::AcqRel);
        Some(SiegeHitEvent {
            structure_id: section.structure_id.load(Ordering::Relaxed),
            face_idx: face_idx as u8,
            breached: breached_before && !section.breached.load(Ordering::Relaxed),
            integrity_q16: new_integrity,
            breach_progress_q16: section.breach_progress_q16.load(Ordering::Relaxed),
        })
    }

    /// Capture per-tick siege metrics; returns Some only once per tick.
    pub fn capture_tick_snapshot(&self, tick: u64) -> Option<SiegeTickSnapshot> {
        let prev = self.last_tick_recorded.load(Ordering::Relaxed);
        if prev == tick {
            return None;
        }
        if self
            .last_tick_recorded
            .compare_exchange(prev, tick, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
            && self.last_tick_recorded.load(Ordering::Acquire) == tick
        {
            return None;
        }
        let samples = self.integrity_samples.swap(0, Ordering::AcqRel);
        let integrity_accum = self.integrity_accum_q64.swap(0, Ordering::AcqRel);
        let progress_accum = self.breach_progress_accum_q64.swap(0, Ordering::AcqRel);
        let breach_events = self.breach_events.swap(0, Ordering::AcqRel);
        let repair_events = self.repair_events.swap(0, Ordering::AcqRel);
        if samples == 0 {
            return None;
        }
        let integrity_avg_q16 =
            (integrity_accum / samples as u64).min(u32::MAX as u64) as u32;
        let breach_progress_avg_q16 =
            (progress_accum / samples as u64).min(u32::MAX as u64) as u32;
        Some(SiegeTickSnapshot {
            integrity_avg_q16,
            breach_progress_avg_q16,
            breach_events,
            repair_events,
            sections_sampled: samples,
        })
    }

    fn find_section(&self, structure_id: u32, face_idx: usize) -> Option<&SiegeSectionCapsule> {
        self.sections
            .iter()
            .find(|s| s.structure_id.load(Ordering::Relaxed) == structure_id
                && s.face_idx.load(Ordering::Relaxed) == face_idx as u32)
    }
}

#[derive(Debug, Clone)]
pub struct SiegeSnapshot {
    pub sections: Vec<SiegeSectionSnapshot>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structure::StructureCapsule;

    #[test]
    fn artillery_hit_tracks_breach_progress() {
        let structures = vec![StructureCapsule::new(
            1, 10, 10, 5, 5, 60_000, 60_000, 60_000, 60_000, 4, 2,
        )];
        let siege = SiegeCapsule::new_from_structures(&structures);
        let event = siege
            .apply_artillery_hit(&structures[0], 0, 6, 1_200)
            .expect("hit applied");
        assert!(event.breach_progress_q16 > 0);
        assert!(event.integrity_q16 < 60_000);
        let snap = siege.capture_tick_snapshot(1).expect("tick snapshot");
        assert!(snap.integrity_avg_q16 > 0);
    }
}
