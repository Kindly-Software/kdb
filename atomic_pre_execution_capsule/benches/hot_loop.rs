use atomic_pre_execution_capsule::{
    BracketTemplate, Header, PexCapsule, PexDraft, Play, RouteTemplate, TailDefaults,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn sample_draft() -> PexDraft {
    let plays = [
        Play {
            enable: true,
            dir_sell: false,
            anchor: 2,
            order_type: 0,
            tif: 1,
            sym_id: 17,
            qty: 120,
            px_ticks: -1,
            route_template_id: 0,
            bracket_template_id: 0,
            slip_cap_bp: 6,
            lat_budget_us: 900,
            ttl_ms: 900,
            priority: 10,
            trig_mask: 0b0011_1111,
            trig_param: 3,
            spare: 0,
        },
        Play {
            enable: true,
            dir_sell: true,
            anchor: 3,
            order_type: 0,
            tif: 1,
            sym_id: 18,
            qty: 120,
            px_ticks: 1,
            route_template_id: 0,
            bracket_template_id: 0,
            slip_cap_bp: 6,
            lat_budget_us: 900,
            ttl_ms: 900,
            priority: 9,
            trig_mask: 0b0011_1111,
            trig_param: 3,
            spare: 0,
        },
        Play {
            enable: true,
            dir_sell: false,
            anchor: 3,
            order_type: 1,
            tif: 0,
            sym_id: 42,
            qty: 60,
            px_ticks: 0,
            route_template_id: 2,
            bracket_template_id: 1,
            slip_cap_bp: 10,
            lat_budget_us: 700,
            ttl_ms: 300,
            priority: 6,
            trig_mask: 0b0001_1111,
            trig_param: 12,
            spare: 0,
        },
        Play {
            enable: true,
            dir_sell: true,
            anchor: 1,
            order_type: 1,
            tif: 0,
            sym_id: 43,
            qty: 60,
            px_ticks: 0,
            route_template_id: 3,
            bracket_template_id: 1,
            slip_cap_bp: 10,
            lat_budget_us: 700,
            ttl_ms: 300,
            priority: 5,
            trig_mask: 0b0001_1111,
            trig_param: 12,
            spare: 0,
        },
    ];

    let bracket_templates = [
        BracketTemplate {
            tp_ticks: 1,
            sl_ticks: -2,
            trail_ticks: 0,
            tstop_ms: 1_500,
            exit_tif: 1,
            scale_out_pct: 0,
            flags: 0b001,
        },
        BracketTemplate {
            tp_ticks: 2,
            sl_ticks: -2,
            trail_ticks: 0,
            tstop_ms: 1_000,
            exit_tif: 0,
            scale_out_pct: 0,
            flags: 0b010,
        },
    ];

    let route_templates = [
        RouteTemplate {
            route_id: 512,
            maker_taker: false,
            ioc_fok: 0,
            post_only: true,
            allow_partial: true,
            slip_cap_bp: 6,
        },
        RouteTemplate {
            route_id: 520,
            maker_taker: false,
            ioc_fok: 1,
            post_only: true,
            allow_partial: true,
            slip_cap_bp: 6,
        },
        RouteTemplate {
            route_id: 32,
            maker_taker: true,
            ioc_fok: 2,
            post_only: false,
            allow_partial: false,
            slip_cap_bp: 10,
        },
        RouteTemplate {
            route_id: 40,
            maker_taker: true,
            ioc_fok: 2,
            post_only: false,
            allow_partial: false,
            slip_cap_bp: 10,
        },
    ];

    PexDraft {
        commit: true,
        stale: false,
        odd_version: 1,
        seq: 0,
        header: Header {
            account_id: 77,
            created_ms_coarse: 1_234_567,
            default_ttl_ms: 1_500,
            forbid_after_min_ct: 120,
            eod_flat_min_ct: 90,
            global_flags: 0b0001_0001,
            portfolio_breaker_level: 1,
            symbol_count: 2,
            play_mask_override: None,
        },
        plays,
        bracket_templates,
        route_templates,
        defaults: TailDefaults {
            slip_cap_default_bp: 6,
            lat_budget_default_us: 2_500,
            router_hints: 0b0000_0010,
        },
    }
}

fn bench_publish(c: &mut Criterion) {
    let capsule = PexCapsule::new();
    let mut draft = sample_draft();
    let mut group = c.benchmark_group("pex_publish");
    group.bench_function(BenchmarkId::new("publish", "draft_reuse"), |b| {
        let mut odd_version = draft.odd_version;
        let mut seq = draft.seq;
        b.iter(|| {
            odd_version = odd_version.wrapping_add(2);
            seq = seq.wrapping_add(1);
            draft.odd_version = odd_version;
            draft.seq = seq;
            let words = capsule.publish(&draft);
            black_box(words.header_word());
        });
    });
    group.finish();
}

fn bench_snapshot(c: &mut Criterion) {
    let capsule = PexCapsule::new();
    let mut draft = sample_draft();
    draft.seq = 10;
    draft.odd_version = 5;
    capsule.publish(&draft);

    c.bench_function("pex_snapshot", |b| {
        b.iter(|| {
            let snapshot = capsule.load_snapshot().expect("snapshot");
            black_box(snapshot.header().ver_even);
        });
    });
}

criterion_group!(benches, bench_publish, bench_snapshot);
criterion_main!(benches);
