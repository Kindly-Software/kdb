use atomic_depth_of_market_slice::writer::{
    DosWriter, InstrumentInput, LevelInput, WriterConfig, WriterInput,
};
use atomic_depth_of_market_slice::{Dos1024, DosInstrumentHeader};
use criterion::{criterion_group, criterion_main, Criterion};

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

fn make_input(now_ms: u64) -> WriterInput {
    let (bids_a, asks_a) = make_levels(10_000, 10_001, 200);
    let (bids_b, asks_b) = make_levels(20_000, 20_001, 150);
    WriterInput {
        now_ms,
        created_ms: now_ms,
        sym_a_id: 100,
        sym_b_id: 200,
        forbid_after_min_ct: 45,
        eod_flat_min_ct: 15,
        flags: 0,
        spare: 0,
        force_stale: false,
        instrument_a: InstrumentInput {
            header: DosInstrumentHeader {
                tick_value_cents_q4: 64,
                px_ref_ticks: 10_000,
                local_ver: 1,
                local_seq: 1,
            },
            bids: bids_a,
            asks: asks_a,
            recent_marketable_volume: Some(120),
        },
        instrument_b: InstrumentInput {
            header: DosInstrumentHeader {
                tick_value_cents_q4: 64,
                px_ref_ticks: 20_000,
                local_ver: 2,
                local_seq: 3,
            },
            bids: bids_b,
            asks: asks_b,
            recent_marketable_volume: None,
        },
    }
}

fn bench_publish(c: &mut Criterion) {
    let capsule = Dos1024::default();
    let config = WriterConfig::default();
    let mut writer = DosWriter::new(&capsule, config);
    let mut now = 1_000_000;
    c.bench_function("dos_publish", |b| {
        b.iter(|| {
            let input = make_input(now);
            writer.publish(input);
            now += 1;
        });
    });
}

criterion_group!(benches, bench_publish);
criterion_main!(benches);
