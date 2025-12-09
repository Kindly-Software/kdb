use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionPattern {
    Linear,
    Vibration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionTempo {
    Lent = 0,
    Moyen = 1,
    Rapide = 2,
}

impl MotionTempo {
    pub fn base_bpm(self) -> u32 {
        match self {
            MotionTempo::Lent => 33,
            MotionTempo::Moyen => 66,
            MotionTempo::Rapide => 200,
        }
    }
}

#[derive(Debug, ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
pub struct MotionBlockCapsule {
    id: AtomicU64,
    generation: AtomicU64,
    pattern: AtomicU8,
    range_start_pct: AtomicU8,
    range_end_pct: AtomicU8,
    base_tempo: AtomicU8,
    nominal_duration_ms: AtomicU32,
    _padding: [u8; 40],
}

impl MotionBlockCapsule {
    pub fn new(
        id: u64,
        pattern: MotionPattern,
        range_start_pct: u8,
        range_end_pct: u8,
        base_tempo: MotionTempo,
        nominal_duration_ms: u32,
    ) -> Self {
        Self {
            id: AtomicU64::new(id),
            generation: AtomicU64::new(1),
            pattern: AtomicU8::new(Self::encode_pattern(pattern)),
            range_start_pct: AtomicU8::new(range_start_pct.min(100)),
            range_end_pct: AtomicU8::new(range_end_pct.min(100)),
            base_tempo: AtomicU8::new(Self::encode_tempo(base_tempo)),
            nominal_duration_ms: AtomicU32::new(nominal_duration_ms),
            _padding: [0; 40],
        }
    }

    pub fn id(&self) -> u64 {
        self.id.load(Ordering::Relaxed)
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    pub fn bump_generation(&self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    pub fn pattern(&self) -> MotionPattern {
        Self::decode_pattern(self.pattern.load(Ordering::Relaxed))
    }

    pub fn tempo(&self) -> MotionTempo {
        Self::decode_tempo(self.base_tempo.load(Ordering::Relaxed))
    }

    pub fn range(&self) -> (u8, u8) {
        let start = self.range_start_pct.load(Ordering::Relaxed);
        let end = self.range_end_pct.load(Ordering::Relaxed);
        (start, end)
    }

    pub fn nominal_duration_ms(&self) -> u32 {
        self.nominal_duration_ms.load(Ordering::Relaxed)
    }

    pub fn inverted_range(&self) -> MotionBlockCapsule {
        let (start, end) = self.range();
        let inverted = MotionBlockCapsule::new(
            self.id(),
            self.pattern(),
            end,
            start,
            self.tempo(),
            self.nominal_duration_ms(),
        );
        inverted
    }

    fn encode_pattern(pattern: MotionPattern) -> u8 {
        match pattern {
            MotionPattern::Linear => 0,
            MotionPattern::Vibration => 1,
        }
    }

    fn decode_pattern(value: u8) -> MotionPattern {
        match value {
            0 => MotionPattern::Linear,
            _ => MotionPattern::Vibration,
        }
    }

    fn encode_tempo(tempo: MotionTempo) -> u8 {
        tempo as u8
    }

    fn decode_tempo(value: u8) -> MotionTempo {
        match value {
            0 => MotionTempo::Lent,
            1 => MotionTempo::Moyen,
            _ => MotionTempo::Rapide,
        }
    }
}

impl Clone for MotionBlockCapsule {
    fn clone(&self) -> Self {
        let (start, end) = self.range();
        Self::new(
            self.id(),
            self.pattern(),
            start,
            end,
            self.tempo(),
            self.nominal_duration_ms(),
        )
    }
}

impl Default for MotionBlockCapsule {
    fn default() -> Self {
        Self::new(0, MotionPattern::Linear, 0, 100, MotionTempo::Lent, 1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_block_round_trip() {
        let block = MotionBlockCapsule::new(7, MotionPattern::Vibration, 75, 100, MotionTempo::Rapide, 1200);
        assert_eq!(block.id(), 7);
        assert_eq!(block.pattern(), MotionPattern::Vibration);
        assert_eq!(block.tempo(), MotionTempo::Rapide);
        assert_eq!(block.range(), (75, 100));
        assert_eq!(block.nominal_duration_ms(), 1200);
        assert_eq!(core::mem::size_of::<MotionBlockCapsule>(), 64);
    }
}
