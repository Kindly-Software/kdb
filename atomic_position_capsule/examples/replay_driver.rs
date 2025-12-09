use atomic_position_capsule::{
    AtomicPositionCapsule, CapsuleDraft, EquityWord, GateDecision, GateMetrics, PositionHeadWord,
    SessionWord, TailWord,
};
use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

#[derive(Deserialize)]
struct ReplayHead {
    position_qty: i32,
    avg_px_ticks: i32,
    remaining_daily_loss_cents: u32,
    flags: u8,
}

#[derive(Deserialize)]
struct ReplayEquity {
    realized_cents: i32,
    unrealized_cents: i32,
    peak_equity_cents: i32,
    trailing_draw_cents: u32,
}

#[derive(Deserialize)]
struct ReplaySession {
    now_min_ct: u16,
    forbid_after_min_ct: u16,
    eod_flat_min_ct: u16,
    open_since_ms: u32,
    max_open_ms: u32,
    max_contracts: u16,
    max_per_trade_cents: u32,
    risk_flags: u8,
    reserved_bits: u16,
}

#[derive(Deserialize)]
struct ReplayTail {
    symbol_id: u16,
    account_id: u16,
    last_exec_id: u32,
    breaker_level: u8,
    alt_health: u8,
    violation_bits: u16,
}

#[derive(Deserialize)]
struct ReplayEvent {
    head: ReplayHead,
    equity: ReplayEquity,
    session: ReplaySession,
    tail: ReplayTail,
    delta_qty: i32,
}

fn to_head(src: &ReplayHead) -> PositionHeadWord {
    PositionHeadWord {
        position_qty: src.position_qty,
        avg_px_ticks: src.avg_px_ticks,
        remaining_daily_loss_cents: src.remaining_daily_loss_cents,
        flags: src.flags,
    }
}

fn to_equity(src: &ReplayEquity) -> EquityWord {
    EquityWord {
        realized_cents: src.realized_cents,
        unrealized_cents: src.unrealized_cents,
        peak_equity_cents: src.peak_equity_cents,
        trailing_draw_cents: src.trailing_draw_cents,
    }
}

fn to_session(src: &ReplaySession) -> SessionWord {
    SessionWord {
        now_min_ct: src.now_min_ct,
        forbid_after_min_ct: src.forbid_after_min_ct,
        eod_flat_min_ct: src.eod_flat_min_ct,
        open_since_ms: src.open_since_ms,
        max_open_ms: src.max_open_ms,
        max_contracts: src.max_contracts,
        max_per_trade_cents: src.max_per_trade_cents,
        risk_flags: src.risk_flags,
        reserved_bits: src.reserved_bits,
    }
}

fn to_tail(src: &ReplayTail) -> TailWord {
    TailWord {
        symbol_id: src.symbol_id,
        account_id: src.account_id,
        last_exec_id: src.last_exec_id,
        breaker_level: src.breaker_level,
        alt_health: src.alt_health,
        violation_bits: src.violation_bits,
    }
}

fn read_events<P: AsRef<Path>>(path: P) -> io::Result<Vec<ReplayEvent>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let event: ReplayEvent = serde_json::from_str(trimmed)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        events.push(event);
    }
    Ok(events)
}

fn main() -> io::Result<()> {
    let default_path = "examples/replay_sample.jsonl";
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| default_path.to_string());

    let events = read_events(&path)?;
    if events.is_empty() {
        eprintln!("no replay events found");
        return Ok(());
    }

    let capsule = AtomicPositionCapsule::new();
    let mut draft = CapsuleDraft::new();
    let metrics = GateMetrics::new();
    let mut denies = Vec::new();

    for (idx, event) in events.iter().enumerate() {
        let snapshot = capsule.publish_with_reuse(&mut draft, |draft| {
            draft
                .set_head(to_head(&event.head))
                .set_equity(to_equity(&event.equity))
                .set_session(to_session(&event.session))
                .set_tail(to_tail(&event.tail));
        });

        let decision = snapshot.gate_order_with_metrics(event.delta_qty, &metrics);
        if let GateDecision::Deny(reason) = decision {
            denies.push((idx + 1, reason));
        }
    }

    let snapshot = metrics.snapshot();
    println!("replayed {} events", events.len());
    println!(
        "allow={}, reduce_only={}, deny_daily_loss={}, deny_violation_bits={}, deny_session_forbid={}, deny_session_past_eod={}, deny_halted={}, deny_size_limit={}",
        snapshot.allow,
        snapshot.reduce_only,
        snapshot.deny_daily_loss,
        snapshot.deny_violation_bits,
        snapshot.deny_session_forbid,
        snapshot.deny_session_past_eod,
        snapshot.deny_halted,
        snapshot.deny_size_limit,
    );

    if !denies.is_empty() {
        println!("denies:");
        for (line, reason) in denies {
            println!("  line {} -> {:?}", line, reason);
        }
    }

    Ok(())
}
