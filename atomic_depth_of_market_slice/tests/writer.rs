use atomic_depth_of_market_slice::writer::{
    DosWriter, InstrumentInput, LevelInput, WriterConfig, WriterInput,
};
use atomic_depth_of_market_slice::{Dos1024, DosInstrumentHeader};

fn make_levels(base_bid: i32, base_ask: i32, qty: u32) -> ([LevelInput; 5], [LevelInput; 5]) {
    let bids = [
        LevelInput {
            px_ticks: base_bid,
            qty,
        },
        LevelInput {
            px_ticks: base_bid - 1,
            qty,
        },
        LevelInput {
            px_ticks: base_bid - 2,
            qty,
        },
        LevelInput {
            px_ticks: base_bid - 3,
            qty,
        },
        LevelInput {
            px_ticks: base_bid - 4,
            qty,
        },
    ];
    let asks = [
        LevelInput {
            px_ticks: base_ask,
            qty,
        },
        LevelInput {
            px_ticks: base_ask + 1,
            qty,
        },
        LevelInput {
            px_ticks: base_ask + 2,
            qty,
        },
        LevelInput {
            px_ticks: base_ask + 3,
            qty,
        },
        LevelInput {
            px_ticks: base_ask + 4,
            qty,
        },
    ];
    (bids, asks)
}

fn make_input(
    now_ms: u64,
    created_ms: u64,
    bids: [LevelInput; 5],
    asks: [LevelInput; 5],
    recent: Option<u32>,
) -> WriterInput {
    WriterInput {
        now_ms,
        created_ms,
        sym_a_id: 11,
        sym_b_id: 22,
        forbid_after_min_ct: 15,
        eod_flat_min_ct: 30,
        flags: 0x155,
        spare: 0,
        force_stale: false,
        instrument_a: InstrumentInput {
            header: DosInstrumentHeader {
                tick_value_cents_q4: 80,
                px_ref_ticks: 100,
                local_ver: 2,
                local_seq: 7,
            },
            bids,
            asks,
            recent_marketable_volume: recent,
        },
        instrument_b: InstrumentInput {
            header: DosInstrumentHeader {
                tick_value_cents_q4: 90,
                px_ref_ticks: 200,
                local_ver: 3,
                local_seq: 5,
            },
            bids,
            asks,
            recent_marketable_volume: None,
        },
    }
}

#[test]
fn writer_publishes_consistent_snapshot() {
    let capsule = Dos1024::default();
    let mut writer = DosWriter::new(&capsule, WriterConfig::default());
    let (bids, asks) = make_levels(100, 101, 50);
    let input = make_input(1_000, 1_000, bids, asks, None);

    writer.publish(input);

    let snapshot = capsule.load_consistent(16).expect("snapshot");
    assert!(snapshot.header.commit);
    assert!(snapshot.head_tail_match());
    assert!(!snapshot.header.stale);
    assert_eq!(snapshot.header.sym_a_id, 11);
    assert_eq!(snapshot.summary.instrument_a.spread_ticks, 1);
    assert_eq!(snapshot.summary.instrument_a.obi_q1_10, 0);
    assert_eq!(snapshot.summary.instrument_a.micro_off_ticks, 0);
    assert!(!snapshot.summary.instrument_a.sweep_flag);
    assert_eq!(snapshot.summary.instrument_a.trend_200ms_ticks, 0);
}

#[test]
fn sweep_flag_triggers_on_mid_jump_and_volume() {
    let capsule = Dos1024::default();
    let mut writer = DosWriter::new(&capsule, WriterConfig::default());
    let (bids, asks) = make_levels(100, 101, 80);
    let input1 = make_input(1_000, 1_000, bids, asks, None);
    writer.publish(input1);

    let (mut bids2, mut asks2) = make_levels(98, 99, 10);
    bids2[0].qty = 10;
    asks2[0].qty = 8;
    let recent_volume = Some(60);
    let input2 = make_input(1_100, 1_100, bids2, asks2, recent_volume);
    writer.publish(input2);

    let snapshot = capsule.load_consistent(16).expect("snapshot");
    assert!(snapshot.summary.instrument_a.sweep_flag);
    assert!(snapshot.summary.instrument_a.trend_200ms_ticks <= 0);
}
