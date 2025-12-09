use atomic_risk_envelope::{
    flag, AtomicRiskEnvelope, Fields, GateOutcome, OrderCheck, RiskEnvelope,
};
use clap::Parser;
use core::sync::atomic::Ordering;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[cfg(feature = "serde")]
#[cfg(feature = "serde")]
use std::fs;

#[derive(Debug, Parser, Clone)]
#[command(
    name = "offline_gateway",
    about = "Offline ARE simulator for multi-account routing"
)]
struct Args {
    #[arg(long, default_value_t = 64)]
    accounts: usize,
    #[arg(long, default_value_t = 1_000_000)]
    cycles: u64,
    #[arg(long, default_value_t = 0xabad1dea)]
    seed: u64,
    #[arg(long, default_value_t = 0)]
    threads: usize,
    #[arg(long, default_value_t = 4)]
    fill_divisor: u32,
    #[arg(long, default_value_t = 0)]
    reset_interval: u64,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long, default_value_t = 50_000)]
    min_daily_loss: u32,
    #[arg(long, default_value_t = 120_000)]
    max_daily_loss: u32,
    #[arg(long, default_value_t = 10_000)]
    min_per_trade: u32,
    #[arg(long, default_value_t = 30_000)]
    max_per_trade: u32,
    #[arg(long, default_value_t = 1_000)]
    order_cost_min: u32,
    #[arg(long, default_value_t = 25_000)]
    order_cost_max: u32,
    #[arg(long, default_value_t = 1)]
    order_contracts_min: u16,
    #[arg(long, default_value_t = 8)]
    order_contracts_max: u16,
    #[arg(long, default_value_t = 500)]
    order_minute_min: u16,
    #[arg(long, default_value_t = 960)]
    order_minute_max: u16,
    #[arg(long, default_value_t = 5_000)]
    order_open_ms_min: u32,
    #[arg(long, default_value_t = 80_000)]
    order_open_ms_max: u32,
}

#[derive(Debug)]
struct AccountRecord {
    env: AtomicRiskEnvelope,
    baseline_daily_loss: u32,
}

#[derive(Default)]
struct Stats {
    loads: u64,
    allowed: u64,
    denied: u64,
    fills: u64,
    fill_rejects: u64,
    resets: u64,
    deny_counts: HashMap<&'static str, u64>,
    elapsed_ms: u64,
}

impl Stats {
    fn merge(&mut self, other: Stats) {
        self.loads += other.loads;
        self.allowed += other.allowed;
        self.denied += other.denied;
        self.fills += other.fills;
        self.fill_rejects += other.fill_rejects;
        self.resets += other.resets;
        for (code, count) in other.deny_counts {
            *self.deny_counts.entry(code).or_insert(0) += count;
        }
    }

    fn report(&self) {
        println!(
            "loads={} allowed={} denied={} fills={} fill_rejects={} resets={} elapsed_ms={}",
            self.loads,
            self.allowed,
            self.denied,
            self.fills,
            self.fill_rejects,
            self.resets,
            self.elapsed_ms
        );
        if !self.deny_counts.is_empty() {
            println!("deny breakdown:");
            for (code, count) in &self.deny_counts {
                println!("  {code}: {count}");
            }
        }
    }
}

fn main() {
    let args = Args::parse();
    let mut rng = StdRng::seed_from_u64(args.seed);

    let accounts_vec = if let Some(path) = &args.config {
        load_accounts_from_config(path)
    } else {
        generate_accounts(&args, &mut rng)
    };
    if let Some(path) = &args.config {
        println!(
            "loaded {} accounts from {}",
            accounts_vec.len(),
            path.display()
        );
    }

    let accounts = Arc::new(accounts_vec);
    println!("gateway bootstrap: {} accounts", accounts.len());

    let start = Instant::now();
    let thread_count = if args.threads == 0 { 1 } else { args.threads };
    let stats = if thread_count == 1 {
        simulate_worker(&args, Arc::clone(&accounts), args.cycles, args.seed)
    } else {
        simulate_parallel(&args, Arc::clone(&accounts), thread_count)
    };

    let mut stats = stats;
    stats.elapsed_ms = start.elapsed().as_millis() as u64;
    stats.report();
}

fn simulate_parallel(args: &Args, accounts: Arc<Vec<AccountRecord>>, threads: usize) -> Stats {
    let base_cycles = args.cycles / threads as u64;
    let remainder = args.cycles % threads as u64;
    let mut handles = Vec::with_capacity(threads);

    for thread_id in 0..threads {
        let cycles = base_cycles + if thread_id < remainder as usize { 1 } else { 0 };
        let seed = args.seed ^ (thread_id as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let thread_args = args.clone();
        let accounts = Arc::clone(&accounts);
        handles.push(std::thread::spawn(move || {
            simulate_worker(&thread_args, accounts, cycles, seed)
        }));
    }

    let mut agg = Stats::default();
    for handle in handles {
        let stats = handle.join().expect("worker thread failed");
        agg.merge(stats);
    }
    agg
}

fn simulate_worker(
    args: &Args,
    accounts: Arc<Vec<AccountRecord>>,
    cycles: u64,
    seed: u64,
) -> Stats {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut stats = Stats::default();

    for iter in 0..cycles {
        if args.reset_interval != 0 && iter % args.reset_interval == 0 {
            let reset_idx = rng.random_range(0..accounts.len());
            let record = &accounts[reset_idx];
            let baseline = record.baseline_daily_loss;
            let _ = record
                .env
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    current.with_rem_daily_loss_cents(baseline).ok()
                });
            stats.resets += 1;
        }

        let (idx, order) = random_order(args, accounts.len(), &mut rng);
        let record = &accounts[idx];
        let snapshot = record.env.load(Ordering::Acquire);
        stats.loads += 1;

        match snapshot.evaluate_order(order) {
            GateOutcome::Allow => {
                stats.allowed += 1;
                let fill = (order.cost_cents / args.fill_divisor).max(1);
                if record
                    .env
                    .debit_daily_loss(fill, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    stats.fills += 1;
                } else {
                    stats.fill_rejects += 1;
                }
            }
            GateOutcome::Deny(reason) => {
                stats.denied += 1;
                *stats.deny_counts.entry(reason.code()).or_insert(0) += 1;
            }
        }
    }

    stats
}

fn generate_accounts(args: &Args, rng: &mut StdRng) -> Vec<AccountRecord> {
    (0..args.accounts)
        .map(|_| {
            let eod_flat = rng.random_range(900u16..=960u16);
            let forbid_lower = args.order_minute_min.min(eod_flat);
            let forbid_after = rng.random_range(forbid_lower..=eod_flat);
            let fields = Fields {
                rem_daily_loss_cents: rng.random_range(args.min_daily_loss..=args.max_daily_loss),
                max_per_trade_cents: rng.random_range(args.min_per_trade..=args.max_per_trade),
                max_contracts: rng.random_range(1..16),
                max_open_ms: rng.random_range(10_000..90_000),
                forbid_after_min_ct: forbid_after,
                eod_flat_min_ct: eod_flat,
                flags: flag::Flags::EMPTY,
                version: 1,
                sequence: rng.random_range(0..64),
            };
            let envelope = RiskEnvelope::try_from_fields(fields).unwrap();
            AccountRecord {
                baseline_daily_loss: envelope.rem_daily_loss_cents(),
                env: AtomicRiskEnvelope::new(envelope),
            }
        })
        .collect()
}

#[cfg(feature = "serde")]
fn load_accounts_from_config(path: &PathBuf) -> Vec<AccountRecord> {
    let data = fs::read_to_string(path).expect("config read");
    let configs: Vec<Fields> = if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
        serde_json::from_str(&data).expect("invalid JSON config")
    } else {
        serde_json::from_str(&data).expect("unsupported format; use JSON")
    };
    if configs.is_empty() {
        panic!("config must contain at least one envelope");
    }
    configs
        .into_iter()
        .map(|fields| {
            let envelope =
                RiskEnvelope::try_from_fields(fields).expect("invalid envelope in config");
            AccountRecord {
                baseline_daily_loss: envelope.rem_daily_loss_cents(),
                env: AtomicRiskEnvelope::new(envelope),
            }
        })
        .collect()
}

#[cfg(not(feature = "serde"))]
fn load_accounts_from_config(_path: &PathBuf) -> Vec<AccountRecord> {
    panic!("config loading requires the `serde` feature");
}

fn random_order(args: &Args, account_len: usize, rng: &mut StdRng) -> (usize, OrderCheck) {
    let idx = rng.random_range(0..account_len);
    let order = OrderCheck::new(
        rng.random_range(args.order_cost_min..=args.order_cost_max),
        rng.random_range(args.order_contracts_min..=args.order_contracts_max),
        rng.random_range(args.order_minute_min..=args.order_minute_max),
        rng.random_range(args.order_open_ms_min..=args.order_open_ms_max),
    );
    (idx, order)
}
