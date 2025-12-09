use crate::feed::FeedSnapshot;
use crate::inputs::PortfolioInputs;
use crate::writer::PortfolioMapWriter;
use crate::{AggregationResult, PortfolioFlags, SymbolInputs, build_symbol_inputs};
use core::time::Duration;

/// Account-level dynamic state used when publishing snapshots.
#[derive(Clone, Debug)]
pub struct AccountSnapshot {
    pub account_id: u16,
    pub forbid_after_min_ct: u16,
    pub eod_flat_min_ct: u16,
    pub rem_daily_loss_total_cents: u32,
    pub trailing_draw_cents: u16,
    pub base_realized_cents: i32,
    pub created_ms_coarse: u16,
    pub portfolio_flags: PortfolioFlags,
}

/// Controller that ingests live feeds, publishes snapshots, and marks stale on timeouts.
#[derive(Debug)]
pub struct PortfolioController {
    writer: PortfolioMapWriter,
    stale_after_ms: u64,
    last_publish_ms: Option<u64>,
    symbol_buffer: Vec<SymbolInputs>,
}

impl PortfolioController {
    /// Create a new controller that marks the slot stale if no publish occurs within `stale_after`.
    pub fn new(writer: PortfolioMapWriter, stale_after: Duration) -> Self {
        Self {
            writer,
            stale_after_ms: stale_after.as_millis() as u64,
            last_publish_ms: None,
            symbol_buffer: Vec::new(),
        }
    }

    /// Access the underlying writer for integration with other systems.
    pub fn writer(&self) -> &PortfolioMapWriter {
        &self.writer
    }

    /// Mutably access the writer (e.g., to reset counters).
    pub fn writer_mut(&mut self) -> &mut PortfolioMapWriter {
        &mut self.writer
    }

    /// Publish a snapshot using the provided account state, feeds, and clock information.
    pub fn publish(
        &mut self,
        account: &AccountSnapshot,
        feeds: &[FeedSnapshot],
        now_minute_count: u16,
        now_ms: u64,
    ) -> AggregationResult {
        self.symbol_buffer.clear();
        self.symbol_buffer
            .extend(feeds.iter().map(build_symbol_inputs));

        let inputs = PortfolioInputs {
            account_id: account.account_id,
            forbid_after_min_ct: account.forbid_after_min_ct,
            eod_flat_min_ct: account.eod_flat_min_ct,
            rem_daily_loss_total_cents: account.rem_daily_loss_total_cents,
            trailing_draw_cents: account.trailing_draw_cents,
            base_realized_cents: account.base_realized_cents,
            created_ms_coarse: account.created_ms_coarse,
            portfolio_flags: account.portfolio_flags,
            now_minute_count,
            symbols: &self.symbol_buffer,
        };

        let result = self.writer.publish(&inputs);
        self.last_publish_ms = Some(now_ms);
        result
    }

    /// Advance the controller's timer; marks the slot stale if the timeout elapses.
    pub fn tick(&mut self, now_ms: u64) {
        if self.stale_after_ms == 0 {
            return;
        }

        if let Some(last) = self.last_publish_ms {
            if now_ms.saturating_sub(last) >= self.stale_after_ms {
                self.writer.mark_stale();
                self.last_publish_ms = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApmSlot;
    use crate::feed::{ActEdge, ApcSnapshot, AvsSnapshot, SymbolGates, SymbolPolicy};
    use crate::layout::BreakerLevel;

    fn make_feed(sym_id: u16, position: i32, rem_daily_loss_cents: u32) -> FeedSnapshot {
        FeedSnapshot {
            policy: SymbolPolicy {
                sym_id,
                max_abs_position: 10,
                forbid_after_min_ct: Some(900),
                eod_flat_min_ct: Some(910),
                priority_offset: 0,
            },
            apc: ApcSnapshot {
                position,
                unreal_cents: 10_000,
                realized_cents: 5_000,
                rem_daily_loss_cents,
                breaker_level: BreakerLevel::L0,
            },
            act: Some(ActEdge { edge_surplus_bp: 8 }),
            avs: Some(AvsSnapshot {
                spread_ticks: 2,
                vol_band: 1,
            }),
            gates: SymbolGates::default(),
        }
    }

    fn account_snapshot() -> AccountSnapshot {
        AccountSnapshot {
            account_id: 500,
            forbid_after_min_ct: 905,
            eod_flat_min_ct: 920,
            rem_daily_loss_total_cents: 800_000,
            trailing_draw_cents: 30_000,
            base_realized_cents: 120_000,
            created_ms_coarse: 35_000,
            portfolio_flags: PortfolioFlags::empty(),
        }
    }

    #[test]
    fn publish_updates_last_timestamp_and_slot() {
        let writer = PortfolioMapWriter::new(ApmSlot::new());
        let mut controller = PortfolioController::new(writer, Duration::from_millis(5_000));

        let account = account_snapshot();
        let feeds = vec![make_feed(1, 2, 150_000)];

        let result = controller.publish(&account, &feeds, 890, 1_000);
        assert_eq!(result.snapshot.header.seq, 1);
        assert!(controller.writer().slot().load_snapshot_relaxed().is_some());

        controller.tick(5_999); // within timeout
        assert!(controller.writer().slot().load_snapshot_relaxed().is_some());

        controller.tick(6_500); // stale threshold crossed
        assert!(controller.writer().slot().load_snapshot_relaxed().is_none());
    }

    #[test]
    fn subsequent_publish_resets_timeout() {
        let writer = PortfolioMapWriter::new(ApmSlot::new());
        let mut controller = PortfolioController::new(writer, Duration::from_millis(1_000));
        let account = account_snapshot();
        let feeds = vec![make_feed(1, 1, 200_000)];

        controller.publish(&account, &feeds, 900, 100);
        controller.tick(1_050);
        assert!(controller.writer().slot().load_snapshot_relaxed().is_some());

        controller.publish(&account, &feeds, 901, 1_200);
        controller.tick(2_150);
        assert!(controller.writer().slot().load_snapshot_relaxed().is_some());

        controller.tick(2_400);
        assert!(controller.writer().slot().load_snapshot_relaxed().is_none());
    }
}
