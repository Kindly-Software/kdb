use core::sync::atomic::{AtomicU64, Ordering};

/// Maximum tracked formations for fire doctrine planning.
const MAX_FORMATIONS: usize = 4096;

/// Fire doctrine variants for musket/rank fire control.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FireDoctrineMode {
    Disabled = 0,
    Volley = 1,
    ByRank = 2,
    Rolling = 3,
    AdvanceAndFire = 4,
}

impl FireDoctrineMode {
    #[inline]
    pub fn from_u8(raw: u8) -> Self {
        match raw {
            1 => Self::Volley,
            2 => Self::ByRank,
            3 => Self::Rolling,
            4 => Self::AdvanceAndFire,
            _ => Self::Disabled,
        }
    }
}

/// Outcome of doctrine planning for a formation on a given tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FireDecision {
    pub fire_now: bool,
    /// Bitmask of ranks to fire this tick (bit 0 = front rank).
    pub rank_mask: u8,
    /// Whether the doctrine expects a step-forward alongside firing.
    pub advance_step: bool,
    pub mode: FireDoctrineMode,
    pub cadence_ticks: u16,
}

/// Capsule managing per-formation firing cadence and rank rotation.
///
/// Encoded state per formation (u64):
/// bits 0..32: next_tick (u32)
/// bits 32..40: next_rank (u8)
/// bits 40..48: mode (u8)
/// bits 48..64: cadence_ticks (u16)
#[repr(C, align(64))]
pub struct FireDoctrineCapsule {
    states: [AtomicU64; MAX_FORMATIONS],
}

impl FireDoctrineCapsule {
    pub const fn new() -> Self {
        const ZERO: AtomicU64 = AtomicU64::new(0);
        Self {
            states: [ZERO; MAX_FORMATIONS],
        }
    }

    #[inline(always)]
    fn encode(mode: FireDoctrineMode, cadence_ticks: u16, next_tick: u32, next_rank: u8) -> u64 {
        (next_tick as u64)
            | ((next_rank as u64) << 32)
            | ((mode as u64) << 40)
            | ((cadence_ticks as u64) << 48)
    }

    #[inline(always)]
    fn decode(bits: u64) -> (FireDoctrineMode, u16, u32, u8) {
        let next_tick = bits as u32;
        let next_rank = ((bits >> 32) & 0xFF) as u8;
        let mode = FireDoctrineMode::from_u8(((bits >> 40) & 0xFF) as u8);
        let cadence = ((bits >> 48) & 0xFFFF) as u16;
        (mode, cadence, next_tick, next_rank)
    }

    fn slot(&self, formation_id: u32) -> Option<&AtomicU64> {
        let idx = formation_id as usize;
        if idx < MAX_FORMATIONS {
            Some(&self.states[idx])
        } else {
            None
        }
    }

    /// Configure doctrine for a formation.
    pub fn set_doctrine(&self, formation_id: u32, mode: FireDoctrineMode, cadence_ticks: u16) {
        if let Some(slot) = self.slot(formation_id) {
            let cadence = cadence_ticks.max(1);
            let bits = Self::encode(mode, cadence, 0, 0);
            slot.store(bits, Ordering::Release);
        }
    }

    /// Read-only helper to expose the current doctrine mode for analytics/AI.
    pub fn mode_for(&self, formation_id: u32) -> FireDoctrineMode {
        self.slot(formation_id)
            .map(|slot| {
                let bits = slot.load(Ordering::Relaxed);
                Self::decode(bits).0
            })
            .unwrap_or(FireDoctrineMode::Disabled)
    }

    /// Plan firing for this tick. Returns which ranks should fire (if any).
    pub fn plan_fire(&self, formation_id: u32, sim_tick: u32, rank_count: u8) -> FireDecision {
        let Some(slot) = self.slot(formation_id) else {
            return FireDecision {
                fire_now: true,
                rank_mask: 0xFF, // fallback: allow all
                advance_step: false,
                mode: FireDoctrineMode::Disabled,
                cadence_ticks: 0,
            };
        };
        let mut current = slot.load(Ordering::Acquire);
        loop {
            let (mode, cadence, next_tick, next_rank) = Self::decode(current);
            if mode == FireDoctrineMode::Disabled || cadence == 0 || rank_count == 0 {
                return FireDecision {
                    fire_now: true,
                    rank_mask: 0xFF,
                    advance_step: false,
                    mode,
                    cadence_ticks: cadence,
                };
            }
            if sim_tick < next_tick {
                return FireDecision {
                    fire_now: false,
                    rank_mask: 0,
                    advance_step: false,
                    mode,
                    cadence_ticks: cadence,
                };
            }

            let mut fire_mask: u8 = 0;
            let mut advance = false;
            let mut next_rank_out = next_rank;
            let mut step = cadence.max(1) as u32;
            let count_mask = if rank_count >= 8 {
                0xFF
            } else {
                (1u16 << rank_count as u16) as u8 - 1
            };

            match mode {
                FireDoctrineMode::Volley => {
                    fire_mask = count_mask;
                    next_rank_out = 0;
                }
                FireDoctrineMode::ByRank => {
                    let rank = (next_rank as u32 % rank_count as u32) as u8;
                    fire_mask = 1 << rank;
                    next_rank_out = rank.wrapping_add(1) % rank_count.max(1);
                }
                FireDoctrineMode::Rolling => {
                    let rank = (next_rank as u32 % rank_count as u32) as u8;
                    fire_mask = 1 << rank;
                    next_rank_out = rank.wrapping_add(1) % rank_count.max(1);
                    step = (cadence as u32 / 2).max(1);
                }
                FireDoctrineMode::AdvanceAndFire => {
                    let rank = (next_rank as u32 % rank_count as u32) as u8;
                    fire_mask = 1 << rank;
                    next_rank_out = rank.wrapping_add(1) % rank_count.max(1);
                    step = (cadence as u32 / 2).max(1);
                    advance = true;
                }
                FireDoctrineMode::Disabled => {}
            }

            let updated = Self::encode(mode, cadence, sim_tick.saturating_add(step), next_rank_out);
            match slot.compare_exchange(current, updated, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => {
                    return FireDecision {
                        fire_now: true,
                        rank_mask: fire_mask,
                        advance_step: advance,
                        mode,
                        cadence_ticks: cadence,
                    };
                }
                Err(new_bits) => current = new_bits,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volley_fires_all() {
        let doctrine = FireDoctrineCapsule::new();
        doctrine.set_doctrine(0, FireDoctrineMode::Volley, 3);
        let plan = doctrine.plan_fire(0, 0, 3);
        assert!(plan.fire_now);
        assert_eq!(plan.rank_mask & 0b111, 0b111);
        assert_eq!(plan.mode, FireDoctrineMode::Volley);
        assert_eq!(plan.cadence_ticks, 3);
        let plan2 = doctrine.plan_fire(0, 1, 3);
        assert!(!plan2.fire_now);
        let plan3 = doctrine.plan_fire(0, 3, 3);
        assert!(plan3.fire_now);
    }

    #[test]
    fn by_rank_rotates() {
        let doctrine = FireDoctrineCapsule::new();
        doctrine.set_doctrine(1, FireDoctrineMode::ByRank, 2);
        let p0 = doctrine.plan_fire(1, 0, 3);
        assert_eq!(p0.rank_mask, 0b001);
        assert_eq!(p0.mode, FireDoctrineMode::ByRank);
        let p1 = doctrine.plan_fire(1, 1, 3);
        assert!(!p1.fire_now);
        let p2 = doctrine.plan_fire(1, 2, 3);
        assert_eq!(p2.rank_mask, 0b010);
        let p3 = doctrine.plan_fire(1, 4, 3);
        assert_eq!(p3.rank_mask, 0b100);
    }

    #[test]
    fn advance_and_fire_sets_flag() {
        let doctrine = FireDoctrineCapsule::new();
        doctrine.set_doctrine(2, FireDoctrineMode::AdvanceAndFire, 4);
        let plan = doctrine.plan_fire(2, 0, 2);
        assert!(plan.fire_now);
        assert!(plan.advance_step);
        assert_eq!(plan.mode, FireDoctrineMode::AdvanceAndFire);
        assert_eq!(plan.cadence_ticks, 4);
    }
}
