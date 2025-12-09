//! Generate synthetic histories for each auto-tune scenario and print summary statistics.
//!
//! Run with `cargo run --example auto_tune_scenarios --features "std auto_tune"`.

use atomic_breaker::policy::{self, Policy};
use atomic_breaker::{generate_all, CalibrationTargets, ScenarioKind};

fn main() {
    let baseline = Policy::ui_holographic();
    let targets = CalibrationTargets::default();
    for scenario in generate_all(24) {
        let name = match scenario.kind {
            ScenarioKind::ChronicOverload => "chronic_overload",
            ScenarioKind::UnderUtilised => "under_utilised",
            ScenarioKind::Flicker => "flicker",
            ScenarioKind::MixedRecovery => "mixed_recovery",
            ScenarioKind::ErrorHeavy => "error_heavy",
        };
        let stats = scenario
            .history
            .iter()
            .fold((0usize, 0usize, 0f32, 0f32), |mut acc, entry| {
                acc.0 += 1;
                if entry.success {
                    acc.1 += 1;
                }
                acc.2 += entry.after.mu_norm;
                acc.3 += entry.after.sg_norm;
                acc
            });
        let observations = stats.0.max(1) as f32;
        println!(
            "scenario={name} observations={} success_rate={:.2} mu_avg={:.2} sg_avg={:.2}",
            stats.0,
            stats.1 as f32 / observations,
            stats.2 / observations,
            stats.3 / observations
        );

        if let Some(draft) = policy::tune(&scenario.history, &baseline, &targets) {
            println!(
                "  -> suggested mu_trip={} sg_trip={} err_trip={}",
                draft.policy.mu_trip, draft.policy.sg_trip, draft.policy.err_trip
            );
            for note in draft.notes {
                println!("     note: {note}");
            }
        } else {
            println!("  -> no adjustments recommended");
        }
    }
}
