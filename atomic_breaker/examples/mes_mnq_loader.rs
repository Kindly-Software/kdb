//! Load MES/MNQ telemetry datasets and run auto-tune + level-feedback analytics.
//!
//! Usage:
//!
//! ```bash
//! cargo run --example mes_mnq_loader --features "std auto_tune" \
//!     -- --dataset data/mes_mnq/liquidity_vacuum.csv --summary vacuum.json
//! ```
//!
//! When no dataset is supplied the tool falls back to the bundled synthetic scenarios.

use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use atomic_breaker::breaker::State;
use atomic_breaker::policy::{adjust_dwell, Policy};
use atomic_breaker::{
    generate_all, tune_policy, ActionOutcome, CalibrationMode, CalibrationTargets, HistoryBuffer,
    HistoryEntry, LevelFeedback, LevelFeedbackConfig, MetricsSnapshot, PolicyDraft, ScenarioData,
    ScenarioKind, TelemetrySample,
};
use serde_json::json;

fn main() {
    let mut datasets: Vec<PathBuf> = Vec::new();
    let mut summary_path: Option<PathBuf> = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dataset" => {
                if let Some(path) = args.next() {
                    datasets.push(PathBuf::from(path));
                }
            }
            "--summary" => {
                if let Some(path) = args.next() {
                    summary_path = Some(PathBuf::from(path));
                }
            }
            token if token.starts_with("--dataset=") => {
                let (_, value) = token.split_at("--dataset=".len());
                datasets.push(PathBuf::from(value));
            }
            token if token.starts_with("--summary=") => {
                let (_, value) = token.split_at("--summary=".len());
                summary_path = Some(PathBuf::from(value));
            }
            other => datasets.push(PathBuf::from(other)),
        }
    }

    let mut summaries = Vec::new();

    if datasets.is_empty() {
        println!("no dataset supplied; replaying bundled synthetic scenarios");
        for ScenarioData { kind, history } in generate_all(360) {
            let summary = run_analytics(kind_name(kind), &history, None);
            summaries.push(summary);
        }
    } else {
        for path in datasets {
            let dataset = load_history(&path);
            println!(
                "loaded {} rows from {}",
                dataset.history.len(),
                path.display()
            );
            let summary = run_analytics(
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("custom"),
                &dataset.history,
                dataset.extras.as_ref(),
            );
            summaries.push(summary);
        }
    }

    if let Some(path) = summary_path {
        let json = serde_json::to_string_pretty(&summaries).expect("serialize summary");
        fs::write(&path, json).expect("write summary");
        println!("wrote summary to {}", path.display());
    }
}

struct ExtraStats {
    primary_latency_avg: f32,
    secondary_latency_avg: f32,
    primary_queue_avg: f32,
    secondary_queue_avg: f32,
    active_primary_ratio: f32,
}

struct LoadedDataset {
    history: HistoryBuffer,
    extras: Option<ExtraStats>,
}

fn run_analytics(
    label: &str,
    history: &HistoryBuffer,
    extras: Option<&ExtraStats>,
) -> serde_json::Value {
    let baseline = Policy::arb_venue();
    let targets = CalibrationTargets::default();
    let draft = tune_policy(history, &baseline, &targets, CalibrationMode::Offline);
    let feedback = LevelFeedback::new(LevelFeedbackConfig::default()).analyze(history);

    println!("=== scenario: {label} ===");

    let auto_tune_json = if let Some(PolicyDraft {
        policy,
        notes,
        metrics,
    }) = draft
    {
        println!(
            "  auto-tune Δ mu_trip={} sg_trip={} err_trip={}",
            policy.mu_trip as i32 - baseline.mu_trip as i32,
            policy.sg_trip as i32 - baseline.sg_trip as i32,
            policy.err_trip as i32 - baseline.err_trip as i32
        );
        println!(
            "  window: success_rate={:.2} transitions/min={:.1} mu_p95={:.2} sg_p95={:.2}",
            metrics.success_rate, metrics.transitions_per_min, metrics.mu_p95, metrics.sg_p95
        );
        for note in &notes {
            println!("    note: {note}");
        }
        Some(json!({
            "delta_mu_trip": policy.mu_trip as i32 - baseline.mu_trip as i32,
            "delta_sg_trip": policy.sg_trip as i32 - baseline.sg_trip as i32,
            "delta_err_trip": policy.err_trip as i32 - baseline.err_trip as i32,
            "success_rate": metrics.success_rate,
            "transitions_per_min": metrics.transitions_per_min,
            "mu_p95": metrics.mu_p95,
            "sg_p95": metrics.sg_p95,
            "notes": notes,
        }))
    } else {
        println!("  auto-tune: no adjustments suggested");
        None
    };

    let dwell_json = if let Some(result) = feedback {
        println!(
            "  level feedback Δcool={}ms Δok={}ms backoff_hint={:?}",
            result.cool_down_delta_ms, result.ok_window_delta_ms, result.backoff_hint
        );
        for note in &result.notes {
            println!("    lvl: {note}");
        }
        let mut policy = baseline;
        let hint = adjust_dwell(&mut policy, &result);
        println!(
            "  adjusted policy cool_down_ms={} ok_window_ms={} backoff_hint={:?}",
            policy.cool_down_ms, policy.ok_window_ms, hint
        );
        Some(json!({
            "delta_cool_down_ms": result.cool_down_delta_ms,
            "delta_ok_window_ms": result.ok_window_delta_ms,
            "backoff_hint": result.backoff_hint,
            "notes": result.notes,
        }))
    } else {
        println!("  level feedback: no dwell adjustments recommended");
        None
    };

    if let Some(extra) = extras {
        println!(
            "  gateways: primary_latency={:.2}ms secondary_latency={:.2}ms active_primary={:.2}",
            extra.primary_latency_avg, extra.secondary_latency_avg, extra.active_primary_ratio
        );
    }

    json!({
        "scenario": label,
        "auto_tune": auto_tune_json,
        "dwell": dwell_json,
        "extras": extras.map(|extra| json!({
            "primary_latency_avg": extra.primary_latency_avg,
            "secondary_latency_avg": extra.secondary_latency_avg,
            "primary_queue_avg": extra.primary_queue_avg,
            "secondary_queue_avg": extra.secondary_queue_avg,
            "active_primary_ratio": extra.active_primary_ratio,
        })),
    })
}

fn load_history(path: &PathBuf) -> LoadedDataset {
    let file = File::open(path).expect("unable to open dataset");
    let mut rdr = BufReader::new(file);
    let mut header = String::new();
    rdr.read_line(&mut header).expect("read header");

    let mut entries = Vec::new();
    let mut err_total = 0u16;

    let mut sum_primary_latency = 0.0;
    let mut sum_secondary_latency = 0.0;
    let mut sum_primary_queue = 0.0;
    let mut sum_secondary_queue = 0.0;
    let mut sum_primary_active = 0.0;
    let mut extras_count = 0usize;

    for line in rdr.lines() {
        let line = line.expect("line");
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 14 {
            continue;
        }
        let timestamp_ms: u32 = cols[0].parse().unwrap_or(0);
        let spread_ticks: f32 = cols[2].parse().unwrap_or(1.0);
        let latency_ms: f32 = cols[3].parse().unwrap_or(5.0);
        let fill_ratio: f32 = cols[4].parse().unwrap_or(0.0);
        let micro_vol: f32 = cols[6].parse().unwrap_or(0.0);
        let pnl_ticks: f32 = cols[7].parse().unwrap_or(0.0);
        let err_inc: u16 = cols[8].parse().unwrap_or(0);
        err_total = err_total.saturating_add(err_inc).min(0x3fff);
        let cause: u8 = cols[9].parse().unwrap_or(0) as u8;
        let mu_norm: f32 = cols[10].parse().unwrap_or(0.0);
        let sg_norm: f32 = cols[11].parse().unwrap_or(0.0);
        let success_window: u32 = cols[12].parse().unwrap_or(0);
        let recovered: bool = cols[13].trim() == "1";

        if cols.len() >= 19 {
            let primary_latency: f32 = cols[14].parse().unwrap_or(latency_ms);
            let secondary_latency: f32 = cols[15].parse().unwrap_or(latency_ms);
            let primary_queue: f32 = cols[16].parse().unwrap_or(10.0);
            let secondary_queue: f32 = cols[17].parse().unwrap_or(10.0);
            let active_primary: f32 = cols[18].parse().unwrap_or(1.0) as f32;
            sum_primary_latency += primary_latency;
            sum_secondary_latency += secondary_latency;
            sum_primary_queue += primary_queue;
            sum_secondary_queue += secondary_queue;
            sum_primary_active += active_primary;
            extras_count += 1;
        }

        let next_state = if mu_norm > 1.6 || err_inc > 1 {
            State::Open
        } else if mu_norm > 1.2 || sg_norm > 1.2 {
            State::HalfOpen
        } else {
            State::Closed
        };
        let level = if mu_norm > 2.5 || micro_vol > 3.0 {
            3
        } else if mu_norm > 2.0 || spread_ticks >= 3.0 {
            2
        } else if mu_norm > 1.2 || fill_ratio < 0.6 {
            1
        } else {
            0
        };
        let success = recovered || pnl_ticks >= 0.0;

        let snapshot = MetricsSnapshot {
            state: next_state,
            level,
            err: err_total,
            mu_norm,
            sg_norm,
            cause,
            backoff: 0,
        };
        let sample = TelemetrySample {
            mu_norm,
            sg_norm,
            err_inc,
            cause,
            backoff_hint: None,
        };
        let action_outcome = Some(ActionOutcome {
            recovered_within_target: recovered,
            observed_recovery_ms: if success_window > 0 {
                Some(success_window)
            } else {
                None
            },
        });

        entries.push(HistoryEntry {
            timestamp_ms,
            prev_state: State::Closed,
            next_state,
            prev_level: 0,
            next_level: level,
            dwell_ms: 100,
            success,
            before: snapshot,
            after: snapshot,
            sample,
            action_outcome,
        });
    }

    let mut history = HistoryBuffer::new(entries.len());
    for entry in entries {
        history.record(entry);
    }

    let extras = if extras_count > 0 {
        Some(ExtraStats {
            primary_latency_avg: (sum_primary_latency / extras_count as f32),
            secondary_latency_avg: (sum_secondary_latency / extras_count as f32),
            primary_queue_avg: (sum_primary_queue / extras_count as f32),
            secondary_queue_avg: (sum_secondary_queue / extras_count as f32),
            active_primary_ratio: (sum_primary_active / extras_count as f32),
        })
    } else {
        None
    };

    LoadedDataset { history, extras }
}

fn kind_name(kind: ScenarioKind) -> &'static str {
    match kind {
        ScenarioKind::ChronicOverload => "liquidity_vacuum",
        ScenarioKind::UnderUtilised => "normal_liquidity",
        ScenarioKind::Flicker => "queue_loss",
        ScenarioKind::MixedRecovery => "over_trading",
        ScenarioKind::ErrorHeavy => "infrastructure_impairment",
    }
}
