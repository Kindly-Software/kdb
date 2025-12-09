use atomic_venue_snapshot::{
    layout::{
        clamp_obi, decode_vol_bp_q8_8, encode_vol_bp_q8_8, obi_from_depths, obi_to_ratio,
        OBI_Q1_10_MAX, OBI_Q1_10_MIN, OBI_Q1_10_SCALE, SEQUENCE_MAX, SPREAD_TICKS_MAX,
        SUM_ASK_L1_3_MAX, SUM_BID_L1_3_MAX, TREND_200MS_TICKS_MAX, TREND_200MS_TICKS_MIN,
        TS_COARSE_MS_MAX, VOL_BP_Q8_8_MAX,
    },
    Avs128Snapshot,
};

fn assert_close(lhs: f32, rhs: f32, eps: f32) {
    assert!((lhs - rhs).abs() <= eps, "lhs={lhs}, rhs={rhs}, eps={eps}");
}

#[test]
fn pack_unpack_roundtrip_extremes() {
    let snapshot = Avs128Snapshot {
        spread_ticks: SPREAD_TICKS_MAX,
        obi_q1_10: OBI_Q1_10_MIN,
        micro_off_ticks: 0,
        sum_bid_l1_3: SUM_BID_L1_3_MAX,
        sum_ask_l1_3: SUM_ASK_L1_3_MAX,
        vol_bp_q8_8: VOL_BP_Q8_8_MAX,
        sweep_flag: true,
        trend_200ms_ticks: TREND_200MS_TICKS_MAX,
        ts_coarse_ms: TS_COARSE_MS_MAX,
        version: 0xAB,
        sequence: SEQUENCE_MAX,
    };

    let packed = snapshot.pack();
    let unpacked = packed.unpack();

    assert_eq!(unpacked, snapshot);
}

#[test]
fn packing_clamps_out_of_range_inputs() {
    let snapshot = Avs128Snapshot {
        spread_ticks: u8::MAX,
        obi_q1_10: (i32::from(OBI_Q1_10_MAX) + 512) as i16,
        micro_off_ticks: 4_096_i16,
        sum_bid_l1_3: SUM_BID_L1_3_MAX,
        sum_ask_l1_3: SUM_ASK_L1_3_MAX,
        vol_bp_q8_8: VOL_BP_Q8_8_MAX,
        sweep_flag: true,
        trend_200ms_ticks: (i32::from(TREND_200MS_TICKS_MIN) - 256) as i16,
        ts_coarse_ms: u32::MAX,
        version: 0xFE,
        sequence: 0x6F,
    };

    let unpacked = snapshot.pack().unpack();

    assert_eq!(unpacked.spread_ticks, SPREAD_TICKS_MAX);
    assert_eq!(unpacked.obi_q1_10, OBI_Q1_10_MAX);
    assert_eq!(unpacked.micro_off_ticks, 2_047);
    assert_eq!(unpacked.trend_200ms_ticks, TREND_200MS_TICKS_MIN);
    assert_eq!(unpacked.ts_coarse_ms, TS_COARSE_MS_MAX);
    assert_eq!(unpacked.sequence, SEQUENCE_MAX);
    assert_eq!(unpacked.version, 0xFE);
}

#[test]
fn fixed_point_helpers_have_expected_precision() {
    let obi = obi_from_depths(9_000, 3_000);
    assert_eq!(obi, clamp_obi(((9_000 - 3_000) << 10) / (9_000 + 3_000)));
    assert_close(obi_to_ratio(obi), 0.5, 1.0 / (OBI_Q1_10_SCALE as f32));

    let vol = encode_vol_bp_q8_8(1.75);
    let vol_bp = decode_vol_bp_q8_8(vol);
    assert_close(vol_bp, 1.75, 1.0 / ((1u32 << 8) as f32));
}
