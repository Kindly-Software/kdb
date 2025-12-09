use atomic_risk_envelope::{flag, AtomicRiskEnvelope, Fields, OrderCheck, RiskEnvelope};
use core::sync::atomic::Ordering;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use std::time::{Duration, Instant};

fn main() {
    let cfg = SimConfig::default();
    let mut rng = StdRng::seed_from_u64(cfg.seed);

    let mut accounts = init_accounts(&cfg, &mut rng);
    println!(
        "bootstrapped {} accounts ({} words validated)",
        accounts.len(),
        accounts.len()
    );

    let mut stats = Stats::default();
    let start = Instant::now();

    for cycle in 0..cfg.cycles {
        let (account_id, order) = random_order(&cfg, &accounts, &mut rng);
        let account = accounts.get_mut(&account_id).unwrap();

        let env_snapshot = account.env.load(Ordering::Acquire);
        stats.total_loads += 1;

        match env_snapshot.evaluate_order(order) {
            atomic_risk_envelope::GateOutcome::Allow => {
                stats.orders_allowed += 1;
                if cfg.apply_fills {
                    let fill_cents = (order.cost_cents / cfg.fill_divisor).max(1);
                    if account
                        .env
                        .debit_daily_loss(fill_cents, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        stats.fills_applied += 1;
                    } else {
                        stats.fill_rejected += 1;
                    }
                }
            }
            atomic_risk_envelope::GateOutcome::Deny(_) => {
                stats.orders_denied += 1;
            }
        }

        if cfg.sequence_bump_every != 0 && cycle % cfg.sequence_bump_every == 0 {
            let current = account.env.load(Ordering::Acquire);
            let next_seq = current.sequence().wrapping_add(1) & 0x3F;
            let bumped = current.with_sequence(next_seq).unwrap();
            account.env.store(bumped, Ordering::Release);
        }
    }

    stats.elapsed = start.elapsed();
    stats.report();
}

struct Account {
    env: AtomicRiskEnvelope,
}

#[derive(Debug, Clone)]
struct SimConfig {
    accounts: usize,
    cycles: u64,
    seed: u64,
    apply_fills: bool,
    fill_divisor: u32,
    sequence_bump_every: u64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            accounts: 64,
            cycles: 500_000,
            seed: 0xfeed_cafe_dead_beef,
            apply_fills: true,
            fill_divisor: 4,
            sequence_bump_every: 10_000,
        }
    }
}

#[derive(Default)]
struct Stats {
    total_loads: u64,
    orders_allowed: u64,
    orders_denied: u64,
    fills_applied: u64,
    fill_rejected: u64,
    elapsed: Duration,
}

impl Stats {
    fn report(&self) {
        println!(
            "sim finished in {:?}\nloads={} allowed={} denied={} fills={} rejects={}",
            self.elapsed,
            self.total_loads,
            self.orders_allowed,
            self.orders_denied,
            self.fills_applied,
            self.fill_rejected
        );
        if self.elapsed > Duration::ZERO {
            let hz = self.total_loads as f64 / self.elapsed.as_secs_f64();
            println!("{} loads/sec", hz as u64);
        }
    }
}

fn init_accounts(cfg: &SimConfig, rng: &mut StdRng) -> HashMap<String, Account> {
    (0..cfg.accounts)
        .map(|idx| {
            let fields = random_fields(rng);
            let env = RiskEnvelope::try_from_fields(fields).unwrap();
            let atomic = AtomicRiskEnvelope::new(env);
            (format!("acct-{idx:03}"), Account { env: atomic })
        })
        .collect()
}

fn random_fields(rng: &mut StdRng) -> Fields {
    let rem = rng.random_range(30_000..120_000);
    let max_trade = rng.random_range(5_000..=rem);
    let eod = rng.random_range(850..930);
    let forbid = rng.random_range(0..=eod);
    Fields {
        rem_daily_loss_cents: rem,
        max_per_trade_cents: max_trade,
        max_contracts: rng.random_range(1..24),
        max_open_ms: rng.random_range(10_000..120_000),
        forbid_after_min_ct: forbid,
        eod_flat_min_ct: eod,
        flags: if rng.random_bool(0.05) {
            flag::NEWS_LOCK
        } else {
            flag::Flags::EMPTY
        },
        version: 1,
        sequence: rng.random_range(0..64),
    }
}

fn random_order(
    _cfg: &SimConfig,
    accounts: &HashMap<String, Account>,
    rng: &mut StdRng,
) -> (String, OrderCheck) {
    let idx = rng.random_range(0..accounts.len());
    let account_id = format!("acct-{idx:03}");
    let cost = rng.random_range(1_000..15_000);
    let contracts = rng.random_range(1..12);
    let minute_ct = rng.random_range(600..950);
    let open_ms = rng.random_range(5_000..120_000);
    (
        account_id,
        OrderCheck::new(cost, contracts, minute_ct, open_ms),
    )
}
