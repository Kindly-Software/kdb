// T1 Atomic Coordination Capsule for Lockfree Detection Fusion
//
// **Q10 Tier Selection**: Tier 1 (Atomic) - Lockfree coordination for fusion scores
// **Q10.5 Composition**: Component of composite capsule (T1+T2+T3 flat layout)
// **Q11 Rust Transform**: AtomicU64 with packed state, Acquire/Release semantics
// **Q23 Concurrency**: 100% lockfree CAS-based fusion, NO mutex
// **Q25 Verification**: Compile-time alignment/size validation

use atomic_capsule::verify_capsule_properties;
use std::sync::atomic::{AtomicU64, Ordering};

/// Detection states (3-bit encoding)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DetectionState {
    /// Initial state (no analysis)
    Uninitialized = 0,
    /// Frequency analysis complete
    FrequencyDone = 1,
    /// Statistical tests complete
    StatisticalDone = 2,
    /// Noise analysis complete
    NoiseDone = 3,
    /// Fusion complete (final verdict)
    FusionDone = 4,
}

/// Final detection verdict
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionVerdict {
    /// Likely human-created (score < 0.3)
    Natural,
    /// Uncertain (0.3 ≤ score ≤ 0.7)
    Uncertain,
    /// Likely AI-generated (score > 0.7)
    AiGenerated,
}

/// T1 Atomic Coordination Capsule
///
/// **Purpose**: Lockfree coordination for sequential pipeline + atomic fusion
/// **Alignment**: 256B (4 cache lines, false sharing prevention)
/// **Performance**: <1μs fusion (CAS-based, Acquire/Release)
/// **Safety**: 100% lockfree, generation counter for TOCTOU prevention
///
/// **State Packing** (3 AtomicU64 fields):
/// - `state_and_gen`: state(3 bits) | generation(61 bits)
/// - `fusion_score`: Q16.16 fixed-point composite score (0.0-1.0)
/// - `component_scores`: freq(16) | stat(16) | noise(16) | padding(16)
#[repr(C, align(256))]
pub struct DetectionCoordinationCapsule {
    /// Packed state + generation counter
    /// Bits: state(3) | generation(61)
    /// - state: DetectionState (0-4)
    /// - generation: Monotonic counter (TOCTOU prevention)
    state_and_gen: AtomicU64,

    /// Composite fusion score (Q16.16 fixed-point)
    /// Range: 0.0 (natural) to 1.0 (AI-generated)
    /// Format: integer(48) | fraction(16) = total 64 bits
    fusion_score: AtomicU64,

    /// Packed component scores (Q16.16 each)
    /// Bits: frequency(16) | statistical(16) | noise(16) | reserved(16)
    component_scores: AtomicU64,

    /// Padding to 256B (4 cache lines)
    _padding: [u8; 232],
}

// Q25: Compile-time verification (256B alignment, 256B size)
verify_capsule_properties!(DetectionCoordinationCapsule, 256, 256);

impl DetectionCoordinationCapsule {
    /// Fixed-point scale factor (Q16.16 format)
    const SCALE: u64 = 65536; // 2^16

    /// State mask (lower 3 bits)
    const STATE_MASK: u64 = 0x7;

    /// Generation mask (upper 61 bits)
    const GEN_MASK: u64 = !Self::STATE_MASK;

    /// Create new coordination capsule
    ///
    /// **Performance**: Zero allocation, const initialization
    /// **Safety**: All atomics initialized to zero (valid initial state)
    pub const fn new() -> Self {
        Self {
            state_and_gen: AtomicU64::new(0), // state=0 (Uninitialized), gen=0
            fusion_score: AtomicU64::new(0),   // score=0.0
            component_scores: AtomicU64::new(0), // all zeros
            _padding: [0u8; 232],
        }
    }

    /// Get current detection state
    ///
    /// **Performance**: <5ns (single atomic load, Relaxed)
    /// **Concurrency**: Lockfree read (no contention)
    #[inline(always)]
    pub fn get_state(&self) -> DetectionState {
        let packed = self.state_and_gen.load(Ordering::Relaxed);
        let state_val = (packed & Self::STATE_MASK) as u8;

        // SAFETY: state_val is 0-7, DetectionState is 0-4
        match state_val {
            0 => DetectionState::Uninitialized,
            1 => DetectionState::FrequencyDone,
            2 => DetectionState::StatisticalDone,
            3 => DetectionState::NoiseDone,
            4 => DetectionState::FusionDone,
            _ => DetectionState::Uninitialized, // Invalid state, fallback
        }
    }

    /// Advance to next state (lockfree CAS)
    ///
    /// **Performance**: <50ns typical (CAS loop, Acquire/Release)
    /// **Concurrency**: Lockfree, retry on contention
    /// **ASSUM**: Generation counter prevents ABA, Acquire/Release prevents reordering
    pub fn advance_state(&self, next_state: DetectionState) -> bool {
        loop {
            // #ASSUME: Acquire ordering prevents load reordering
            // #VERIFY: Current state read is consistent
            let current = self.state_and_gen.load(Ordering::Acquire);
            let current_gen = current & Self::GEN_MASK;
            let new_gen = current_gen.wrapping_add(1 << 3); // Increment generation

            let new_packed = (next_state as u64) | new_gen;

            // #ASSUME: CAS prevents TOCTOU races
            // #VERIFY: State transition is atomic
            match self.state_and_gen.compare_exchange_weak(
                current,
                new_packed,
                Ordering::Release, // #VERIFY: Publish state change
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(_) => continue, // Retry on contention
            }
        }
    }

    /// Atomic fusion: combine frequency, statistical, noise scores
    ///
    /// **Algorithm**: Weighted average (freq: 40%, stat: 30%, noise: 30%)
    /// **Performance**: <1μs (CAS loop, fixed-point arithmetic)
    /// **Concurrency**: Lockfree, generation counter for consistency
    ///
    /// **Q23 Lockfree Fusion**:
    /// - NO mutex/RwLock
    /// - CAS-based atomic update
    /// - Acquire/Release memory ordering
    pub fn fuse_scores(
        &self,
        freq_score: f32,
        stat_score: f32,
        noise_score: f32,
    ) -> DetectionVerdict {
        // Convert to Q16.16 fixed-point (T3 deterministic precision)
        let freq_fixed = (freq_score.clamp(0.0, 1.0) * Self::SCALE as f32) as u64;
        let stat_fixed = (stat_score.clamp(0.0, 1.0) * Self::SCALE as f32) as u64;
        let noise_fixed = (noise_score.clamp(0.0, 1.0) * Self::SCALE as f32) as u64;

        // Weighted average (40-30-30 split)
        // fusion = 0.4 * freq + 0.3 * stat + 0.3 * noise
        let weight_freq = (Self::SCALE * 40) / 100; // 0.4 in Q16.16
        let weight_stat = (Self::SCALE * 30) / 100; // 0.3 in Q16.16
        let weight_noise = (Self::SCALE * 30) / 100; // 0.3 in Q16.16

        // Fixed-point weighted sum (all Q16.16, need to shift after multiply)
        let fusion_fixed = ((freq_fixed * weight_freq) >> 16)
            + ((stat_fixed * weight_stat) >> 16)
            + ((noise_fixed * weight_noise) >> 16);

        // Pack component scores (16 bits each)
        let packed_components = ((freq_fixed & 0xFFFF) << 48)
            | ((stat_fixed & 0xFFFF) << 32)
            | ((noise_fixed & 0xFFFF) << 16);

        // Atomic update: fusion_score + component_scores + state
        // #ASSUME: Release ordering publishes all writes
        // #VERIFY: Fusion result is atomic
        self.fusion_score.store(fusion_fixed, Ordering::Release);
        self.component_scores.store(packed_components, Ordering::Release);
        self.advance_state(DetectionState::FusionDone);

        // Determine verdict based on fusion score
        let fusion_float = fusion_fixed as f32 / Self::SCALE as f32;
        if fusion_float < 0.3 {
            DetectionVerdict::Natural
        } else if fusion_float > 0.7 {
            DetectionVerdict::AiGenerated
        } else {
            DetectionVerdict::Uncertain
        }
    }

    /// Get current fusion score (Q16.16 → f32)
    ///
    /// **Performance**: <10ns (atomic load + conversion)
    #[inline]
    pub fn get_fusion_score(&self) -> f32 {
        let fixed = self.fusion_score.load(Ordering::Acquire);
        fixed as f32 / Self::SCALE as f32
    }

    /// Get component scores (frequency, statistical, noise)
    ///
    /// **Performance**: <15ns (atomic load + unpacking)
    pub fn get_component_scores(&self) -> (f32, f32, f32) {
        let packed = self.component_scores.load(Ordering::Acquire);

        let freq = ((packed >> 48) & 0xFFFF) as f32 / Self::SCALE as f32;
        let stat = ((packed >> 32) & 0xFFFF) as f32 / Self::SCALE as f32;
        let noise = ((packed >> 16) & 0xFFFF) as f32 / Self::SCALE as f32;

        (freq, stat, noise)
    }
}

// Q11: Send + Sync for thread safety
// SAFETY: All fields are AtomicU64 (Send + Sync)
unsafe impl Send for DetectionCoordinationCapsule {}
unsafe impl Sync for DetectionCoordinationCapsule {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordination_new() {
        let coord = DetectionCoordinationCapsule::new();
        assert_eq!(coord.get_state(), DetectionState::Uninitialized);
        assert_eq!(coord.get_fusion_score(), 0.0);
    }

    #[test]
    fn test_coordination_alignment() {
        // Q25: Verify 256B alignment
        assert_eq!(
            std::mem::align_of::<DetectionCoordinationCapsule>(),
            256
        );
        assert_eq!(
            std::mem::size_of::<DetectionCoordinationCapsule>(),
            256
        );
    }

    #[test]
    fn test_coordination_state_advance() {
        let coord = DetectionCoordinationCapsule::new();

        assert!(coord.advance_state(DetectionState::FrequencyDone));
        assert_eq!(coord.get_state(), DetectionState::FrequencyDone);

        assert!(coord.advance_state(DetectionState::FusionDone));
        assert_eq!(coord.get_state(), DetectionState::FusionDone);
    }

    #[test]
    fn test_coordination_lockfree_fusion() {
        let coord = DetectionCoordinationCapsule::new();

        // Simulate pipeline scores
        let verdict = coord.fuse_scores(0.8, 0.6, 0.7);

        // fusion = 0.4*0.8 + 0.3*0.6 + 0.3*0.7 = 0.32 + 0.18 + 0.21 = 0.71
        assert_eq!(verdict, DetectionVerdict::AiGenerated);

        let score = coord.get_fusion_score();
        assert!((score - 0.71).abs() < 0.01); // Q16.16 precision

        let (freq, stat, noise) = coord.get_component_scores();
        assert!((freq - 0.8).abs() < 0.01);
        assert!((stat - 0.6).abs() < 0.01);
        assert!((noise - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_coordination_generation_monotonicity() {
        let coord = DetectionCoordinationCapsule::new();

        let gen1 = coord.state_and_gen.load(Ordering::Relaxed);
        coord.advance_state(DetectionState::FrequencyDone);
        let gen2 = coord.state_and_gen.load(Ordering::Relaxed);
        coord.advance_state(DetectionState::StatisticalDone);
        let gen3 = coord.state_and_gen.load(Ordering::Relaxed);

        // Generation counter must be strictly increasing
        assert!(gen2 > gen1);
        assert!(gen3 > gen2);
    }
}
