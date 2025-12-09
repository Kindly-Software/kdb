use crate::aggregator::{AggregationInput, AggregationResult, aggregate};
use crate::inputs::PortfolioInputs;
use crate::layout::ApmHeader;
use crate::slot::ApmSlot;
use core::sync::atomic::Ordering;

/// Manages versioning and publication of portfolio map snapshots.
#[derive(Debug)]
pub struct PortfolioMapWriter {
    slot: ApmSlot,
    version: u8,
    seq: u16,
}

impl PortfolioMapWriter {
    /// Create a new writer with default counters.
    pub fn new(slot: ApmSlot) -> Self {
        Self {
            slot,
            version: 0,
            seq: 0,
        }
    }

    /// Access the underlying slot for read-side wiring or inspection.
    pub fn slot(&self) -> &ApmSlot {
        &self.slot
    }

    /// Consume the writer, returning the owned slot.
    pub fn into_slot(self) -> ApmSlot {
        self.slot
    }

    /// Publish a fresh snapshot assembled from the provided inputs.
    pub fn publish(&mut self, inputs: &PortfolioInputs<'_>) -> AggregationResult {
        let symbol_states = inputs.symbol_states();
        let portfolio_flags = inputs.derive_portfolio_flags(inputs.portfolio_flags, &symbol_states);

        self.seq = self.seq.wrapping_add(1);
        self.version = self.version.wrapping_add(2);

        let aggregation = AggregationInput {
            commit: true,
            stale: false,
            version: self.version,
            seq: self.seq,
            account_id: inputs.account_id,
            forbid_after_min_ct: inputs.forbid_after_min_ct,
            eod_flat_min_ct: inputs.eod_flat_min_ct,
            created_ms_coarse: inputs.created_ms_coarse,
            rem_daily_loss_total_cents: inputs.rem_daily_loss_total_cents,
            trailing_draw_cents: inputs.trailing_draw_cents,
            base_realized_cents: inputs.base_realized_cents,
            portfolio_flags,
            symbol_states: &symbol_states,
        };

        let result = aggregate(&aggregation);
        self.slot.publish(&result.packed);
        result
    }

    /// Mark the current slot contents as stale, preventing readers from consuming it.
    pub fn mark_stale(&self) {
        let raw = self.slot.raw_words();
        if raw[0] == 0 {
            return;
        }
        let mut header = ApmHeader::decode(raw[0]);
        header.stale = true;
        header.commit = false;
        header.version |= 1;
        self.slot.write_head(header.encode(), Ordering::Release);
    }

    /// Reset the internal counters to a known state.
    pub fn reset_counters(&mut self, version: u8, seq: u16) {
        self.version = if version % 2 == 0 {
            version
        } else {
            version.wrapping_add(1)
        };
        self.seq = seq;
    }

    /// Return the latest even version counter that has been published.
    pub fn current_version(&self) -> u8 {
        self.version
    }

    /// Return the most recent sequence identifier used when publishing.
    pub fn current_seq(&self) -> u16 {
        self.seq
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inputs::SymbolInputs;
    use crate::layout::{BreakerLevel, PortfolioFlags};

    fn sample_symbol() -> SymbolInputs {
        SymbolInputs {
            sym_id: 1,
            position: 2,
            unreal_cents: 12_000,
            realized_cents: 8_000,
            rem_daily_loss_cents: 200_000,
            breaker_level: BreakerLevel::L0,
            spread_ticks: 3,
            vol_band: 1,
            edge_surplus_bp: 8,
            priority_offset: 0,
            max_abs_position: 10,
            forbid_after_min_ct: Some(900),
            eod_flat_min_ct: Some(910),
            news_lockout: false,
            eco_lockout: false,
            manual_lockout: false,
            force_reduce_only: false,
        }
    }

    fn sample_inputs<'a>(symbols: &'a [SymbolInputs]) -> PortfolioInputs<'a> {
        PortfolioInputs {
            account_id: 77,
            forbid_after_min_ct: 930,
            eod_flat_min_ct: 940,
            rem_daily_loss_total_cents: 1_000_000,
            trailing_draw_cents: 50_000,
            base_realized_cents: 250_000,
            created_ms_coarse: 48_000,
            portfolio_flags: PortfolioFlags::empty(),
            now_minute_count: 925,
            symbols,
        }
    }

    #[test]
    fn publish_increments_version_and_seq() {
        let slot = ApmSlot::new();
        let mut writer = PortfolioMapWriter::new(slot);
        let symbols = vec![sample_symbol()];
        let inputs = sample_inputs(&symbols);

        writer.publish(&inputs);
        assert_eq!(writer.current_version(), 2);
        assert_eq!(writer.current_seq(), 1);

        let snapshot = writer
            .slot()
            .load_snapshot_relaxed()
            .expect("snapshot should be readable");
        assert_eq!(snapshot.header.version % 2, 0);
        assert_eq!(snapshot.header.seq, 1);
        assert!(snapshot.header.commit);
        assert!(!snapshot.header.stale);
        assert_eq!(snapshot.header.account_id, inputs.account_id);
        assert_eq!(snapshot.slices[0].sym_id, symbols[0].sym_id);

        // Second publish bumps counters again.
        let result_second = writer.publish(&inputs);
        assert_eq!(writer.current_version(), 4);
        assert_eq!(writer.current_seq(), 2);
        assert_eq!(result_second.snapshot.header.seq, 2);
        assert_eq!(result_second.snapshot.header.version % 2, 0);
    }

    #[test]
    fn mark_stale_invalidates_snapshot() {
        let slot = ApmSlot::new();
        let mut writer = PortfolioMapWriter::new(slot);
        let symbols = vec![sample_symbol()];
        let inputs = sample_inputs(&symbols);

        writer.publish(&inputs);
        assert!(writer.slot().load_relaxed().is_some());

        writer.mark_stale();
        assert!(writer.slot().load_relaxed().is_none());

        // Publishing again clears the stale condition.
        writer.publish(&inputs);
        assert!(writer.slot().load_relaxed().is_some());
    }

    #[test]
    fn reset_counters_aligns_to_even_version() {
        let slot = ApmSlot::new();
        let mut writer = PortfolioMapWriter::new(slot);
        writer.reset_counters(5, 100);
        assert_eq!(writer.current_version(), 6);
        assert_eq!(writer.current_seq(), 100);
    }
}
