use crate::layout::{
    ApmHeader, ApmSnapshot, ApmSymbolSlice, ApmTail, ApmWords, BreakerLevel, MAX_SYMBOL_SLICES,
    PortfolioFlags, SymbolFlags,
};

/// Input describing one symbol that will be aggregated into the snapshot.
#[derive(Clone, Debug)]
pub struct SymbolState {
    pub sym_id: u16,
    pub position: i32,
    pub unreal_cents: i32,
    pub realized_cents: i32,
    pub rem_daily_loss_cents: u32,
    pub breaker_level: BreakerLevel,
    pub spread_ticks: u8,
    pub vol_band: u8,
    pub can_scale_up: bool,
    pub reduce_only: bool,
    pub lockout: bool,
    pub news: bool,
    pub after_forbid: bool,
    pub has_risk: bool,
    pub edge_surplus_bp: i16,
    pub priority_offset: i16,
}

impl SymbolState {
    pub fn flags(&self) -> SymbolFlags {
        let mut flags = SymbolFlags::empty();
        if self.can_scale_up {
            flags.insert(SymbolFlags::CAN_SCALE_UP);
        }
        if self.reduce_only {
            flags.insert(SymbolFlags::REDUCE_ONLY);
        }
        if self.lockout {
            flags.insert(SymbolFlags::LOCKOUT);
        }
        if self.news {
            flags.insert(SymbolFlags::NEWS);
        }
        if self.after_forbid {
            flags.insert(SymbolFlags::AFTER_FORBID);
        }
        if self.has_risk {
            flags.insert(SymbolFlags::HAS_RISK);
        }
        flags
    }
}

/// Parameters used for a single aggregation publish.
#[derive(Clone, Debug)]
pub struct AggregationInput<'a> {
    pub commit: bool,
    pub stale: bool,
    pub version: u8,
    pub seq: u16,
    pub account_id: u16,
    pub forbid_after_min_ct: u16,
    pub eod_flat_min_ct: u16,
    pub created_ms_coarse: u16,
    pub rem_daily_loss_total_cents: u32,
    pub trailing_draw_cents: u16,
    pub base_realized_cents: i32,
    pub portfolio_flags: PortfolioFlags,
    pub symbol_states: &'a [SymbolState],
}

/// Result of running the aggregator for a single publish cycle.
#[derive(Clone, Debug)]
pub struct AggregationResult {
    pub snapshot: ApmSnapshot,
    pub packed: ApmWords,
}

/// Build an `ApmSnapshot` along with the packed words ready for publication.
pub fn aggregate(input: &AggregationInput<'_>) -> AggregationResult {
    let mut snapshot = ApmSnapshot::empty();
    let capped_len = input.symbol_states.len().min(MAX_SYMBOL_SLICES);

    snapshot.header = ApmHeader {
        commit: input.commit,
        stale: input.stale,
        version: force_even_version(input.version),
        seq: input.seq,
        account_id: input.account_id,
        forbid_after_min_ct: input.forbid_after_min_ct,
        eod_flat_min_ct: input.eod_flat_min_ct,
        rem_daily_loss_total_cents: input.rem_daily_loss_total_cents,
        portfolio_breaker: BreakerLevel::L0,
        symbol_count: capped_len as u8,
        portfolio_flags: input.portfolio_flags,
        created_ms_coarse: input.created_ms_coarse,
    };

    let mut sum_abs: u64 = 0;
    let mut net_unreal: i64 = 0;
    let mut net_realized: i64 = input.base_realized_cents as i64;
    let mut portfolio_breaker = BreakerLevel::L0;

    for (slot, state) in snapshot
        .slices
        .iter_mut()
        .take(capped_len)
        .zip(input.symbol_states.iter())
    {
        let flags = state.flags();
        let priority = compute_priority(state, flags);

        *slot = ApmSymbolSlice {
            sym_id: state.sym_id,
            breaker_level: state.breaker_level,
            flags,
            pos_qty: state.position,
            unreal_cents: state.unreal_cents,
            rem_daily_loss_cents: state.rem_daily_loss_cents,
            spread_ticks: state.spread_ticks,
            vol_band: state.vol_band,
            priority,
        };

        sum_abs = sum_abs.saturating_add(state.position.unsigned_abs() as u64);
        net_unreal += state.unreal_cents as i64;
        net_realized += state.realized_cents as i64;
        if state.breaker_level.as_u8() > portfolio_breaker.as_u8() {
            portfolio_breaker = state.breaker_level;
        }
    }

    snapshot.header.portfolio_breaker = portfolio_breaker;

    snapshot.tail = ApmTail {
        sum_pos_abs_contracts: sum_abs.min(u16::MAX as u64) as u16,
        net_unreal_cents: clamp_i64_to_i32(net_unreal),
        net_realized_cents: clamp_i64_to_i32(net_realized),
        trailing_draw_cents: input.trailing_draw_cents,
        version: snapshot.header.version,
        seq: snapshot.header.seq,
        spare: 0,
    };

    let packed = snapshot.pack();
    AggregationResult { snapshot, packed }
}

fn compute_priority(state: &SymbolState, flags: SymbolFlags) -> u8 {
    let mut score = 128i16;
    if flags.contains(SymbolFlags::CAN_SCALE_UP) {
        score += 32;
    }
    if flags.contains(SymbolFlags::REDUCE_ONLY) {
        score -= 24;
    }
    if flags.contains(SymbolFlags::LOCKOUT) {
        score -= 16;
    }
    score -= 8 * state.breaker_level.as_u8() as i16;

    let edge_adjust = (state.edge_surplus_bp as i16 * 2).clamp(-32, 32);
    score += edge_adjust;
    score += state.priority_offset;

    score.clamp(0, 255) as u8
}

fn clamp_i64_to_i32(value: i64) -> i32 {
    value.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

fn force_even_version(version: u8) -> u8 {
    if version & 1 == 0 {
        version
    } else {
        version.wrapping_add(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{ApmSnapshot, MAX_SYMBOL_SLICES, SymbolFlags};

    #[test]
    fn aggregating_two_symbols_sets_expected_fields() {
        let symbols = vec![
            SymbolState {
                sym_id: 100,
                position: 4,
                unreal_cents: 25_000,
                realized_cents: 10_000,
                rem_daily_loss_cents: 250_000,
                breaker_level: BreakerLevel::L0,
                spread_ticks: 3,
                vol_band: 1,
                can_scale_up: true,
                reduce_only: false,
                lockout: false,
                news: false,
                after_forbid: false,
                has_risk: true,
                edge_surplus_bp: 12,
                priority_offset: 4,
            },
            SymbolState {
                sym_id: 200,
                position: -2,
                unreal_cents: -12_500,
                realized_cents: 5_000,
                rem_daily_loss_cents: 0,
                breaker_level: BreakerLevel::L2,
                spread_ticks: 6,
                vol_band: 3,
                can_scale_up: false,
                reduce_only: true,
                lockout: true,
                news: true,
                after_forbid: true,
                has_risk: false,
                edge_surplus_bp: -8,
                priority_offset: 0,
            },
        ];

        let input = AggregationInput {
            commit: true,
            stale: false,
            version: 7,
            seq: 44,
            account_id: 900,
            forbid_after_min_ct: 915,
            eod_flat_min_ct: 920,
            created_ms_coarse: 32_000,
            rem_daily_loss_total_cents: 750_000,
            trailing_draw_cents: 62_500,
            base_realized_cents: 50_000,
            portfolio_flags: PortfolioFlags::PAUSED,
            symbol_states: &symbols,
        };

        let result = aggregate(&input);
        let snapshot = result.snapshot;

        assert!(snapshot.header.commit);
        assert_eq!(snapshot.header.version % 2, 0);
        assert_eq!(snapshot.header.seq, 44);
        assert_eq!(snapshot.header.symbol_count, 2);
        assert_eq!(snapshot.header.portfolio_breaker, BreakerLevel::L2);
        assert_eq!(snapshot.slices[0].sym_id, 100);
        assert!(
            snapshot.slices[0]
                .flags
                .contains(SymbolFlags::CAN_SCALE_UP | SymbolFlags::HAS_RISK)
        );
        assert_eq!(snapshot.slices[0].priority, 128 + 32 + 24 + 4); // base + scale + edge + offset
        assert!(snapshot.slices[1].flags.contains(SymbolFlags::REDUCE_ONLY));
        assert!(snapshot.slices[1].flags.contains(SymbolFlags::LOCKOUT));
        assert_eq!(snapshot.tail.sum_pos_abs_contracts, 6);
        assert_eq!(snapshot.tail.net_unreal_cents, 12_500);
        assert_eq!(snapshot.tail.net_realized_cents, 65_000);
        assert_eq!(snapshot.tail.version, snapshot.header.version);
        assert_eq!(snapshot.tail.seq, snapshot.header.seq);

        // Packed form round-trips back to the same logical view.
        let unpacked = ApmSnapshot::unpack(&result.packed);
        assert_eq!(snapshot, unpacked);
    }

    #[test]
    fn capping_to_max_symbol_slices_zeroes_remaining() {
        let symbols: Vec<SymbolState> = (0..(MAX_SYMBOL_SLICES + 2))
            .map(|idx| SymbolState {
                sym_id: idx as u16,
                position: idx as i32,
                unreal_cents: idx as i32 * 100,
                realized_cents: 0,
                rem_daily_loss_cents: 1_000,
                breaker_level: BreakerLevel::L1,
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
            })
            .collect();

        let input = AggregationInput {
            commit: true,
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
            symbol_states: &symbols,
        };

        let result = aggregate(&input);
        assert_eq!(
            result.snapshot.header.symbol_count as usize,
            MAX_SYMBOL_SLICES
        );
        assert_eq!(
            result.snapshot.slices[MAX_SYMBOL_SLICES - 1].sym_id as usize,
            MAX_SYMBOL_SLICES - 1
        );
        assert_eq!(
            result.snapshot.slices[MAX_SYMBOL_SLICES - 1].priority,
            128 + 32 - 8 + 10
        );
        assert!(
            result.snapshot.slices[MAX_SYMBOL_SLICES - 1]
                .flags
                .contains(SymbolFlags::CAN_SCALE_UP)
        );
        assert!(
            result
                .snapshot
                .slices
                .iter()
                .skip(result.snapshot.header.symbol_count as usize)
                .all(|slice| slice.sym_id == 0 && slice.priority == 0)
        );
    }
}
