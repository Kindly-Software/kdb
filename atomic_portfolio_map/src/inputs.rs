use crate::aggregator::SymbolState;
use crate::layout::{BreakerLevel, PortfolioFlags, SymbolFlags};

/// Inputs drawn from per-symbol risk capsules and telemetry feeds.
#[derive(Clone, Debug)]
pub struct SymbolInputs {
    pub sym_id: u16,
    pub position: i32,
    pub unreal_cents: i32,
    pub realized_cents: i32,
    pub rem_daily_loss_cents: u32,
    pub breaker_level: BreakerLevel,
    pub spread_ticks: u8,
    pub vol_band: u8,
    pub edge_surplus_bp: i16,
    pub priority_offset: i16,
    pub max_abs_position: i32,
    pub forbid_after_min_ct: Option<u16>,
    pub eod_flat_min_ct: Option<u16>,
    pub news_lockout: bool,
    pub eco_lockout: bool,
    pub manual_lockout: bool,
    pub force_reduce_only: bool,
}

impl SymbolInputs {
    /// Convert the richer per-symbol inputs into the compact `SymbolState` used by the aggregator.
    pub fn to_symbol_state(&self, now_minute_count: u16) -> SymbolState {
        let after_forbid = self
            .forbid_after_min_ct
            .map(|cut| now_minute_count >= cut)
            .unwrap_or(false);
        let after_eod = self
            .eod_flat_min_ct
            .map(|cut| now_minute_count >= cut)
            .unwrap_or(false);

        let lockout = self.manual_lockout || self.eco_lockout;
        let mut reduce_only = self.force_reduce_only;
        if self.rem_daily_loss_cents == 0 {
            reduce_only = true;
        }
        if after_forbid || after_eod {
            reduce_only = true;
        }

        let breaker_guard = self.breaker_level.as_u8() <= BreakerLevel::L1.as_u8();
        let below_limit = if self.max_abs_position <= 0 {
            true
        } else {
            self.position.abs() < self.max_abs_position
        };

        let can_scale_up = !lockout
            && !reduce_only
            && self.rem_daily_loss_cents > 0
            && breaker_guard
            && below_limit;
        let has_risk = self.position != 0;

        SymbolState {
            sym_id: self.sym_id,
            position: self.position,
            unreal_cents: self.unreal_cents,
            realized_cents: self.realized_cents,
            rem_daily_loss_cents: self.rem_daily_loss_cents,
            breaker_level: self.breaker_level,
            spread_ticks: self.spread_ticks,
            vol_band: self.vol_band,
            can_scale_up,
            reduce_only,
            lockout,
            news: self.news_lockout,
            after_forbid,
            has_risk,
            edge_surplus_bp: self.edge_surplus_bp,
            priority_offset: self.priority_offset,
        }
    }
}

/// Snapshot-wide inputs required to build the portfolio header.
#[derive(Clone, Debug)]
pub struct PortfolioInputs<'a> {
    pub account_id: u16,
    pub forbid_after_min_ct: u16,
    pub eod_flat_min_ct: u16,
    pub rem_daily_loss_total_cents: u32,
    pub trailing_draw_cents: u16,
    pub base_realized_cents: i32,
    pub created_ms_coarse: u16,
    pub portfolio_flags: PortfolioFlags,
    pub now_minute_count: u16,
    pub symbols: &'a [SymbolInputs],
}

impl<'a> PortfolioInputs<'a> {
    /// Compute the derived symbol states that the aggregator expects.
    pub fn symbol_states(&self) -> Vec<SymbolState> {
        self.symbols
            .iter()
            .map(|symbol| symbol.to_symbol_state(self.now_minute_count))
            .collect()
    }

    /// Derive portfolio-level flags based on the current clock and symbol states.
    pub fn derive_portfolio_flags(
        &self,
        base: PortfolioFlags,
        symbols: &[SymbolState],
    ) -> PortfolioFlags {
        let mut flags = base;

        if self.now_minute_count >= self.forbid_after_min_ct {
            flags.insert(PortfolioFlags::AFTER_FORBID);
        }
        if self.now_minute_count >= self.eod_flat_min_ct {
            flags.insert(PortfolioFlags::AT_EOD);
        }

        let any_news = symbols
            .iter()
            .any(|state| state.flags().contains(SymbolFlags::NEWS));
        if any_news {
            flags.insert(PortfolioFlags::NEWS_LOCKOUT);
        }

        let any_after_forbid = symbols
            .iter()
            .any(|state| state.flags().contains(SymbolFlags::AFTER_FORBID));
        if any_after_forbid {
            flags.insert(PortfolioFlags::AFTER_FORBID);
        }

        flags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_inputs_translate_flags_correctly() {
        let inputs = SymbolInputs {
            sym_id: 42,
            position: 3,
            unreal_cents: 12_500,
            realized_cents: 8_000,
            rem_daily_loss_cents: 100_000,
            breaker_level: BreakerLevel::L0,
            spread_ticks: 2,
            vol_band: 1,
            edge_surplus_bp: 6,
            priority_offset: 4,
            max_abs_position: 10,
            forbid_after_min_ct: Some(900),
            eod_flat_min_ct: Some(910),
            news_lockout: false,
            eco_lockout: false,
            manual_lockout: false,
            force_reduce_only: false,
        };

        let state = inputs.to_symbol_state(850);
        assert!(state.can_scale_up);
        assert!(!state.reduce_only);
        assert!(!state.lockout);
        assert!(!state.after_forbid);
        assert_eq!(
            state.flags().bits() & SymbolFlags::CAN_SCALE_UP.bits(),
            SymbolFlags::CAN_SCALE_UP.bits()
        );

        let after_forbid_state = inputs.to_symbol_state(905);
        assert!(after_forbid_state.after_forbid);
        assert!(after_forbid_state.reduce_only);
        assert_eq!(
            after_forbid_state.flags().bits() & SymbolFlags::AFTER_FORBID.bits(),
            SymbolFlags::AFTER_FORBID.bits()
        );
    }

    #[test]
    fn portfolio_flags_reflect_symbol_conditions() {
        let symbols = vec![SymbolInputs {
            sym_id: 7,
            position: 0,
            unreal_cents: 0,
            realized_cents: 0,
            rem_daily_loss_cents: 0,
            breaker_level: BreakerLevel::L2,
            spread_ticks: 4,
            vol_band: 2,
            edge_surplus_bp: -10,
            priority_offset: 0,
            max_abs_position: 5,
            forbid_after_min_ct: Some(800),
            eod_flat_min_ct: Some(900),
            news_lockout: true,
            eco_lockout: false,
            manual_lockout: false,
            force_reduce_only: false,
        }];

        let portfolio = PortfolioInputs {
            account_id: 1,
            forbid_after_min_ct: 780,
            eod_flat_min_ct: 920,
            rem_daily_loss_total_cents: 500_000,
            trailing_draw_cents: 10_000,
            base_realized_cents: 25_000,
            created_ms_coarse: 42_000,
            portfolio_flags: PortfolioFlags::PAUSED,
            now_minute_count: 930,
            symbols: &symbols,
        };

        let states = portfolio.symbol_states();
        let flags = portfolio.derive_portfolio_flags(portfolio.portfolio_flags, &states);
        assert!(flags.contains(PortfolioFlags::AT_EOD));
        assert!(flags.contains(PortfolioFlags::NEWS_LOCKOUT));
        assert!(flags.contains(PortfolioFlags::AFTER_FORBID));
        assert!(flags.contains(PortfolioFlags::PAUSED));
    }
}
