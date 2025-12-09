use atomic_pre_execution_capsule::{
    BracketTemplate, Header, PexCapsule, PexDraft, Play, RouteTemplate, TailDefaults,
};
use core::convert::TryInto;
use proptest::prelude::*;

const PLAY_COUNT: usize = 4;
const BRACKET_COUNT: usize = 2;
const ROUTE_COUNT: usize = 4;

fn take_u8<I: Iterator<Item = u8>>(iter: &mut I) -> u8 {
    iter.next().expect("u8")
}

fn take_u16<I: Iterator<Item = u8>>(iter: &mut I) -> u16 {
    let lo = take_u8(iter);
    let hi = take_u8(iter);
    u16::from_le_bytes([lo, hi])
}

fn take_i16<I: Iterator<Item = u8>>(iter: &mut I) -> i16 {
    i16::from_le_bytes([take_u8(iter), take_u8(iter)])
}

fn take_u32<I: Iterator<Item = u8>>(iter: &mut I) -> u32 {
    let b0 = take_u8(iter);
    let b1 = take_u8(iter);
    let b2 = take_u8(iter);
    let b3 = take_u8(iter);
    u32::from_le_bytes([b0, b1, b2, b3])
}

fn play_strategy() -> impl Strategy<Value = Play> {
    any::<[u8; 26]>().prop_map(|bytes| {
        let mut it = bytes.into_iter();
        Play {
            enable: take_u8(&mut it) & 1 != 0,
            dir_sell: take_u8(&mut it) & 1 != 0,
            anchor: take_u8(&mut it) % 4,
            order_type: take_u8(&mut it) % 8,
            tif: take_u8(&mut it) % 8,
            sym_id: take_u16(&mut it) & ((1 << 12) - 1),
            qty: take_u32(&mut it) & ((1 << 18) - 1),
            px_ticks: take_i16(&mut it),
            route_template_id: take_u8(&mut it) % 4,
            bracket_template_id: take_u8(&mut it) % 2,
            slip_cap_bp: take_u16(&mut it) & ((1 << 10) - 1),
            lat_budget_us: take_u16(&mut it) & ((1 << 10) - 1),
            ttl_ms: take_u16(&mut it) & ((1 << 10) - 1),
            priority: take_u8(&mut it) % 64,
            trig_mask: take_u16(&mut it),
            trig_param: take_u8(&mut it),
            spare: take_u8(&mut it) % 64,
        }
    })
}

fn bracket_strategy() -> impl Strategy<Value = BracketTemplate> {
    any::<[u8; 12]>().prop_map(|bytes| {
        let mut it = bytes.into_iter();
        BracketTemplate {
            tp_ticks: take_i16(&mut it).clamp(-512, 511),
            sl_ticks: take_i16(&mut it).clamp(-512, 511),
            trail_ticks: take_i16(&mut it).clamp(-512, 511),
            tstop_ms: take_u16(&mut it) & ((1 << 12) - 1),
            exit_tif: take_u8(&mut it) % 4,
            scale_out_pct: take_u8(&mut it) % 128,
            flags: take_u8(&mut it) % 8,
        }
    })
}

fn route_strategy() -> impl Strategy<Value = RouteTemplate> {
    any::<[u8; 8]>().prop_map(|bytes| {
        let mut it = bytes.into_iter();
        RouteTemplate {
            route_id: take_u16(&mut it) & ((1 << 10) - 1),
            maker_taker: take_u8(&mut it) & 1 != 0,
            ioc_fok: take_u8(&mut it) % 4,
            post_only: take_u8(&mut it) & 1 != 0,
            allow_partial: take_u8(&mut it) & 1 != 0,
            slip_cap_bp: take_u16(&mut it) & ((1 << 10) - 1),
        }
    })
}

fn header_strategy() -> impl Strategy<Value = Header> {
    any::<[u8; 14]>().prop_map(|bytes| {
        let mut it = bytes.into_iter();
        let mask_seed = take_u8(&mut it);
        Header {
            account_id: take_u16(&mut it),
            created_ms_coarse: take_u32(&mut it) & 0x00ff_ffff,
            default_ttl_ms: take_u16(&mut it) & ((1 << 12) - 1),
            forbid_after_min_ct: take_u16(&mut it) & ((1 << 11) - 1),
            eod_flat_min_ct: take_u16(&mut it) & ((1 << 11) - 1),
            global_flags: take_u8(&mut it),
            portfolio_breaker_level: take_u8(&mut it) % 4,
            symbol_count: take_u8(&mut it) % 16,
            play_mask_override: if mask_seed & 0x80 != 0 {
                Some(mask_seed & 0x0f)
            } else {
                None
            },
        }
    })
}

fn defaults_strategy() -> impl Strategy<Value = TailDefaults> {
    any::<[u8; 5]>().prop_map(|bytes| {
        let mut it = bytes.into_iter();
        TailDefaults {
            slip_cap_default_bp: take_u16(&mut it) & ((1 << 10) - 1),
            lat_budget_default_us: take_u16(&mut it) & ((1 << 12) - 1),
            router_hints: take_u8(&mut it),
        }
    })
}

fn draft_strategy() -> impl Strategy<Value = PexDraft> {
    (
        header_strategy(),
        prop::collection::vec(play_strategy(), PLAY_COUNT),
        prop::collection::vec(bracket_strategy(), BRACKET_COUNT),
        prop::collection::vec(route_strategy(), ROUTE_COUNT),
        defaults_strategy(),
        any::<u8>(),
        any::<u16>(),
    )
        .prop_map(
            |(header, plays_vec, brackets_vec, routes_vec, defaults, odd_seed, seq)| {
                let mut odd_version = odd_seed | 1;
                if odd_version == 0 {
                    odd_version = 1;
                }
                let plays: [Play; PLAY_COUNT] = plays_vec.try_into().expect("play array len");
                let bracket_templates: [BracketTemplate; BRACKET_COUNT] =
                    brackets_vec.try_into().expect("bracket array len");
                let route_templates: [RouteTemplate; ROUTE_COUNT] =
                    routes_vec.try_into().expect("route array len");
                PexDraft {
                    commit: true,
                    stale: false,
                    odd_version,
                    seq,
                    header,
                    plays,
                    bracket_templates,
                    route_templates,
                    defaults,
                }
            },
        )
}

fn computed_mask(plays: &[Play; PLAY_COUNT]) -> u8 {
    let mut mask = 0u8;
    for (idx, play) in plays.iter().enumerate() {
        if play.enable {
            mask |= 1 << idx;
        }
    }
    mask
}

proptest! {
    #[test]
    fn publish_roundtrip_matches_draft(draft in draft_strategy()) {
        let capsule = PexCapsule::new();
        capsule.publish(&draft);
        let snapshot = capsule.load_snapshot().expect("snapshot");
        let header = snapshot.header();
        let expected_mask = draft
            .header
            .play_mask_override
            .unwrap_or_else(|| computed_mask(&draft.plays))
            & 0x0f;

        prop_assert_eq!(header.account_id, draft.header.account_id);
        prop_assert_eq!(header.created_ms_coarse, draft.header.created_ms_coarse);
        prop_assert_eq!(header.default_ttl_ms, draft.header.default_ttl_ms);
        prop_assert_eq!(header.forbid_after_min_ct, draft.header.forbid_after_min_ct);
        prop_assert_eq!(header.eod_flat_min_ct, draft.header.eod_flat_min_ct);
        prop_assert_eq!(header.global_flags, draft.header.global_flags);
        prop_assert_eq!(header.portfolio_breaker_level, draft.header.portfolio_breaker_level);
        prop_assert_eq!(header.symbol_count, draft.header.symbol_count);
        prop_assert_eq!(header.play_mask, expected_mask);
        prop_assert_eq!(header.seq_head, draft.seq);
        prop_assert_eq!(header.ver_even, draft.odd_version.wrapping_add(1));

        for idx in 0..PLAY_COUNT {
            let play = draft.plays[idx];
            let view = snapshot.play(idx);
            prop_assert_eq!(view.enable, play.enable);
            prop_assert_eq!(view.dir_sell, play.dir_sell);
            prop_assert_eq!(view.anchor, play.anchor);
            prop_assert_eq!(view.order_type, play.order_type);
            prop_assert_eq!(view.tif, play.tif);
            prop_assert_eq!(view.sym_id, play.sym_id);
            prop_assert_eq!(view.qty, play.qty);
            prop_assert_eq!(view.px_ticks, play.px_ticks);
            prop_assert_eq!(view.route_template_id, play.route_template_id);
            prop_assert_eq!(view.bracket_template_id, play.bracket_template_id);
            prop_assert_eq!(view.slip_cap_bp, play.slip_cap_bp);
            prop_assert_eq!(view.lat_budget_us, play.lat_budget_us);
            prop_assert_eq!(view.ttl_ms, play.ttl_ms);
            prop_assert_eq!(view.priority, play.priority);
            prop_assert_eq!(view.trig_mask, play.trig_mask);
            prop_assert_eq!(view.trig_param, play.trig_param);
            prop_assert_eq!(view.spare, play.spare);
        }

        let tail = snapshot.tail();
        prop_assert_eq!(tail.ver_tail, draft.odd_version.wrapping_add(1));
        prop_assert_eq!(tail.seq_tail, draft.seq);
        prop_assert_eq!(tail.slip_cap_default_bp, draft.defaults.slip_cap_default_bp);
        prop_assert_eq!(tail.lat_budget_default_us, draft.defaults.lat_budget_default_us);
        prop_assert_eq!(tail.router_hints, draft.defaults.router_hints);
    }
}
