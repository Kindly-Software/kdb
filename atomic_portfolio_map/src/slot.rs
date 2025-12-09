use crate::layout::{ApmHeader, ApmSnapshot, ApmTail, ApmWords, BreakerLevel, PortfolioFlags};
use atomic_breaker::{AtomicBreakerSWeMR, breaker::State as BreakerState};
use core::array;
use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
struct AtomicWord {
    lo: AtomicU64,
    hi: AtomicU64,
}

impl AtomicWord {
    const fn new(value: u128) -> Self {
        Self {
            lo: AtomicU64::new(value as u64),
            hi: AtomicU64::new((value >> 64) as u64),
        }
    }

    fn store(&self, value: u128, order: Ordering) {
        let lo = value as u64;
        let hi = (value >> 64) as u64;
        self.lo.store(lo, Ordering::Relaxed);
        self.hi.store(hi, order);
    }

    fn load(&self, order: Ordering) -> u128 {
        loop {
            let hi_first = self.hi.load(order);
            let lo = self.lo.load(Ordering::Relaxed);
            let hi_second = self.hi.load(Ordering::Relaxed);
            if hi_first == hi_second {
                return ((hi_first as u128) << 64) | lo as u128;
            }
        }
    }
}

impl Default for AtomicWord {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Shared slot that stores the packed snapshot words for atomic publication.
pub struct ApmSlot {
    words: [AtomicWord; 8],
    /// Unified circuit breaker for portfolio-level risk management.
    ///
    /// #ASSUME_LOCKFREE_ONLY: AtomicBreakerSWeMR provides lockfree circuit breaking
    /// #VERIFY_NO_BLOCKING: atomic_breaker crate guarantees no mutex/rwlock usage
    breaker: AtomicBreakerSWeMR,
}

impl ApmSlot {
    /// Create a new slot with all words set to zero.
    pub fn new() -> Self {
        Self {
            words: array::from_fn(|_| AtomicWord::new(0)),
            breaker: AtomicBreakerSWeMR::new(BreakerState::Closed),
        }
    }

    /// Create a new slot with a specific initial breaker level.
    pub fn new_with_breaker_level(level: BreakerLevel) -> Self {
        Self {
            words: array::from_fn(|_| AtomicWord::new(0)),
            breaker: level.create_atomic_breaker(),
        }
    }

    /// Publish the provided words to the slot using the SWeMR protocol.
    ///
    /// Also synchronizes the atomic breaker level with the portfolio breaker level
    /// from the header to ensure unified risk management.
    pub fn publish(&self, words: &ApmWords) {
        let raw = words.as_words();
        let final_head = raw[0];

        // Stage an in-progress header (odd version, stale) so readers drop until commit.
        let mut staging_header = ApmHeader::decode(final_head);
        staging_header.commit = false;
        staging_header.stale = true;
        staging_header.version = staging_header.version | 1;
        let staging_word = staging_header.encode();

        self.write_head(staging_word, Ordering::Relaxed);

        // Write the body first (tail included), then flip the header with a release store.
        for idx in 1..raw.len() {
            self.words[idx].store(raw[idx], Ordering::Relaxed);
        }

        // Synchronize breaker level before committing the header
        let final_header = ApmHeader::decode(final_head);
        self.breaker.set_level(final_header.portfolio_breaker.to_atomic_breaker_level());

        self.write_head(final_head, Ordering::Release);
    }

    /// Attempt to load the latest committed snapshot using relaxed loads.
    ///
    /// Returns `None` if the slot is empty, stale, or a writer publish is in progress.
    pub fn load_relaxed(&self) -> Option<ApmWords> {
        let head_raw = self.words[0].load(Ordering::Relaxed);
        if head_raw == 0 {
            return None;
        }
        let header = ApmHeader::decode(head_raw);
        if !header.commit || header.stale || (header.version & 1) != 0 {
            return None;
        }

        let mut raw = [0u128; 8];
        raw[0] = head_raw;
        for idx in 1..raw.len() {
            raw[idx] = self.words[idx].load(Ordering::Relaxed);
        }
        let tail = ApmTail::decode(raw[7]);
        if tail.version != header.version || tail.seq != header.seq {
            return None;
        }

        Some(ApmWords::from_words(raw))
    }

    /// Convenience helper that decodes the loaded words into a structured snapshot.
    pub fn load_snapshot_relaxed(&self) -> Option<ApmSnapshot> {
        self.load_relaxed().map(|words| ApmSnapshot::unpack(&words))
    }

    /// Access the underlying words for testing.
    pub fn raw_words(&self) -> [u128; 8] {
        array::from_fn(|idx| self.words[idx].load(Ordering::Relaxed))
    }

    /// Access the integrated atomic breaker for risk management.
    ///
    /// #ASSUME_LOCKFREE_ONLY: All breaker operations are lockfree
    /// #VERIFY_NO_BLOCKING: AtomicBreakerSWeMR provides lockfree guarantees
    pub fn breaker(&self) -> &AtomicBreakerSWeMR {
        &self.breaker
    }

    /// Check if the current breaker state allows operations.
    ///
    /// Returns true if the breaker is in Closed or HalfOpen state,
    /// false if Open or ForcedOpen.
    pub fn is_breaker_allowing(&self) -> bool {
        match self.breaker.state() {
            BreakerState::Closed | BreakerState::HalfOpen => true,
            BreakerState::Open | BreakerState::ForcedOpen => false,
        }
    }

    /// Get the current breaker level as a BreakerLevel enum.
    pub fn current_breaker_level(&self) -> BreakerLevel {
        BreakerLevel::from_atomic_breaker_level(self.breaker.level())
    }

    /// Update the breaker level directly.
    ///
    /// This provides a way to change the breaker level independent of
    /// portfolio snapshot publication.
    pub fn set_breaker_level(&self, level: BreakerLevel) {
        self.breaker.set_level(level.to_atomic_breaker_level());
    }

    /// Force the breaker to open state, typically used in emergency scenarios.
    pub fn force_breaker_open(&self) {
        self.breaker.force_open();
    }

    /// Reset the breaker to closed state.
    pub fn close_breaker(&self) {
        self.breaker.close();
    }

    /// Get effective portfolio flags considering both stored flags and breaker state.
    ///
    /// This combines the portfolio flags from the latest snapshot with
    /// flags derived from the current breaker state, providing a unified view.
    pub fn effective_portfolio_flags(&self) -> Option<PortfolioFlags> {
        let snapshot = self.load_snapshot_relaxed()?;
        let breaker_state = self.breaker.state();
        Some(snapshot.header.portfolio_flags.with_breaker_state(breaker_state))
    }

    /// Check if the portfolio is effectively paused considering both flags and breaker.
    pub fn is_effectively_paused(&self) -> bool {
        // Check breaker state first (fastest check)
        if PortfolioFlags::is_breaker_paused(self.breaker.state()) {
            return true;
        }

        // Check stored portfolio flags
        if let Some(snapshot) = self.load_snapshot_relaxed() {
            snapshot.header.portfolio_flags.contains(PortfolioFlags::PAUSED)
        } else {
            false // No snapshot available, assume not paused
        }
    }

    pub(crate) fn write_head(&self, word: u128, order: Ordering) {
        self.words[0].store(word, order);
    }
}

impl Default for ApmSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ApmSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApmSlot")
            .field("words", &"[8 atomic words]")
            .field("breaker_state", &self.breaker.state())
            .field("breaker_level", &self.breaker.level())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregator::{AggregationInput, SymbolState, aggregate};
    use crate::layout::{BreakerLevel, PortfolioFlags};

    #[test]
    fn publish_and_load_round_trip() {
        let slot = ApmSlot::new();

        let symbols = vec![SymbolState {
            sym_id: 1,
            position: 2,
            unreal_cents: 10_000,
            realized_cents: 4_000,
            rem_daily_loss_cents: 150_000,
            breaker_level: BreakerLevel::L0,
            spread_ticks: 2,
            vol_band: 1,
            can_scale_up: true,
            reduce_only: false,
            lockout: false,
            news: false,
            after_forbid: false,
            has_risk: true,
            edge_surplus_bp: 6,
            priority_offset: 0,
        }];

        let input = AggregationInput {
            commit: true,
            stale: false,
            version: 4,
            seq: 12,
            account_id: 101,
            forbid_after_min_ct: 900,
            eod_flat_min_ct: 905,
            created_ms_coarse: 40_000,
            rem_daily_loss_total_cents: 600_000,
            trailing_draw_cents: 10_000,
            base_realized_cents: 20_000,
            portfolio_flags: PortfolioFlags::empty(),
            symbol_states: &symbols,
        };

        let result = aggregate(&input);
        slot.publish(&result.packed);

        let loaded = slot.load_relaxed().expect("snapshot should be available");
        assert_eq!(loaded.as_words(), result.packed.as_words());

        let snapshot = slot
            .load_snapshot_relaxed()
            .expect("decoded snapshot should succeed");
        assert_eq!(snapshot, result.snapshot);
    }

    #[test]
    fn load_returns_none_when_commit_not_set() {
        let slot = ApmSlot::new();

        let input = AggregationInput {
            commit: false,
            stale: false,
            version: 2,
            seq: 1,
            account_id: 1,
            forbid_after_min_ct: 0,
            eod_flat_min_ct: 0,
            created_ms_coarse: 0,
            rem_daily_loss_total_cents: 0,
            trailing_draw_cents: 0,
            base_realized_cents: 0,
            portfolio_flags: PortfolioFlags::empty(),
            symbol_states: &[],
        };

        let result = aggregate(&input);
        slot.publish(&result.packed);

        assert!(slot.load_relaxed().is_none());
    }

    #[test]
    fn breaker_level_conversion_round_trip() {
        // Test all level conversions
        for level_u8 in 0..=3 {
            let original = BreakerLevel::from_u8(level_u8);
            let atomic_level = original.to_atomic_breaker_level();
            let converted_back = BreakerLevel::from_atomic_breaker_level(atomic_level);
            assert_eq!(original, converted_back);
            assert_eq!(original.as_u8(), level_u8.min(3));
        }
    }

    #[test]
    fn breaker_integration_synchronizes_levels() {
        let slot = ApmSlot::new();

        // Create a snapshot with L2 breaker level
        let symbols = vec![SymbolState {
            sym_id: 1,
            position: 10,
            unreal_cents: 5_000,
            realized_cents: 2_000,
            rem_daily_loss_cents: 100_000,
            breaker_level: BreakerLevel::L2,
            spread_ticks: 1,
            vol_band: 1,
            can_scale_up: true,
            reduce_only: false,
            lockout: false,
            news: false,
            after_forbid: false,
            has_risk: true,
            edge_surplus_bp: 5,
            priority_offset: 0,
        }];

        let input = AggregationInput {
            commit: true,
            stale: false,
            version: 1,
            seq: 1,
            account_id: 1,
            forbid_after_min_ct: 0,
            eod_flat_min_ct: 0,
            created_ms_coarse: 0,
            rem_daily_loss_total_cents: 0,
            trailing_draw_cents: 0,
            base_realized_cents: 0,
            portfolio_flags: PortfolioFlags::empty(),
            symbol_states: &symbols,
        };

        // Set portfolio breaker to L2 and publish
        let mut result = aggregate(&input);
        result.snapshot.header.portfolio_breaker = BreakerLevel::L2;
        let words = result.snapshot.pack();

        slot.publish(&words);

        // Verify breaker level is synchronized
        assert_eq!(slot.current_breaker_level(), BreakerLevel::L2);
        assert_eq!(slot.breaker().level(), 2);
    }

    #[test]
    fn breaker_state_affects_portfolio_flags() {
        let slot = ApmSlot::new();

        // Initially closed, not paused
        assert_eq!(slot.breaker().state(), BreakerState::Closed);
        assert!(!slot.is_effectively_paused());

        // Open the breaker
        slot.force_breaker_open();
        assert_eq!(slot.breaker().state(), BreakerState::ForcedOpen);
        assert!(slot.is_effectively_paused());

        // Close it again
        slot.close_breaker();
        assert_eq!(slot.breaker().state(), BreakerState::Closed);
        assert!(!slot.is_effectively_paused());
    }

    #[test]
    fn effective_flags_combine_stored_and_breaker_state() {
        let slot = ApmSlot::new();

        // Publish a snapshot with TRAIL_WARN flag
        let input = AggregationInput {
            commit: true,
            stale: false,
            version: 1,
            seq: 1,
            account_id: 1,
            forbid_after_min_ct: 0,
            eod_flat_min_ct: 0,
            created_ms_coarse: 0,
            rem_daily_loss_total_cents: 0,
            trailing_draw_cents: 0,
            base_realized_cents: 0,
            portfolio_flags: PortfolioFlags::TRAIL_WARN,
            symbol_states: &[],
        };

        let result = aggregate(&input);
        slot.publish(&result.packed);

        // Initially, only TRAIL_WARN should be set
        let flags = slot.effective_portfolio_flags().unwrap();
        assert!(flags.contains(PortfolioFlags::TRAIL_WARN));
        assert!(!flags.contains(PortfolioFlags::PAUSED));

        // Open the breaker - should add PAUSED flag
        slot.force_breaker_open();
        let flags = slot.effective_portfolio_flags().unwrap();
        assert!(flags.contains(PortfolioFlags::TRAIL_WARN));
        assert!(flags.contains(PortfolioFlags::PAUSED));
    }

    #[test]
    fn breaker_level_initialization() {
        // Test creating slot with specific breaker level
        let slot = ApmSlot::new_with_breaker_level(BreakerLevel::L3);
        assert_eq!(slot.current_breaker_level(), BreakerLevel::L3);
        assert_eq!(slot.breaker().level(), 3);

        // Test that breaker starts in closed state
        assert_eq!(slot.breaker().state(), BreakerState::Closed);
        assert!(slot.is_breaker_allowing());
    }

    #[test]
    fn backward_compatibility_preserved() {
        // Ensure that existing functionality still works
        let slot = ApmSlot::new();

        let symbols = vec![SymbolState {
            sym_id: 1,
            position: 5,
            unreal_cents: 1_000,
            realized_cents: 500,
            rem_daily_loss_cents: 50_000,
            breaker_level: BreakerLevel::L1,
            spread_ticks: 2,
            vol_band: 1,
            can_scale_up: false,
            reduce_only: true,
            lockout: false,
            news: false,
            after_forbid: false,
            has_risk: true,
            edge_surplus_bp: 3,
            priority_offset: 0,
        }];

        let input = AggregationInput {
            commit: true,
            stale: false,
            version: 1,
            seq: 1,
            account_id: 1,
            forbid_after_min_ct: 0,
            eod_flat_min_ct: 0,
            created_ms_coarse: 0,
            rem_daily_loss_total_cents: 0,
            trailing_draw_cents: 0,
            base_realized_cents: 0,
            portfolio_flags: PortfolioFlags::empty(),
            symbol_states: &symbols,
        };

        let result = aggregate(&input);
        slot.publish(&result.packed);

        // Verify old functionality still works
        let loaded = slot.load_relaxed().expect("should load snapshot");
        let snapshot = slot.load_snapshot_relaxed().expect("should decode snapshot");

        assert_eq!(loaded.as_words(), result.packed.as_words());
        assert_eq!(snapshot.header.version, 2); // force_even_version(1) = 2
        assert_eq!(snapshot.slices[0].sym_id, 1);
        assert_eq!(snapshot.slices[0].breaker_level, BreakerLevel::L1);
    }
}
