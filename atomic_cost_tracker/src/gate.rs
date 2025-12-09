use crate::layout::{ActFlags, ActSnapshot, ActWord};

/// Configuration for the hot-path gate.
#[derive(Debug, Clone, Copy, Default)]
pub struct GateConfig {
    /// Optional sigma multiplier to subtract from the net edge before checking the floor.
    pub sigma_k: Option<f64>,
    /// Reject trades outright when the snapshot is flagged as high jitter.
    pub reject_high_jitter: bool,
    /// Reject trades outright when the spread is tagged as wide.
    pub reject_wide_spread: bool,
    /// Reject trades that required an emergency buffer.
    pub reject_emergency_buffer: bool,
}

/// Result of evaluating the gate for a snapshot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GateOutcome {
    /// Order may proceed. Carries the raw snapshot for downstream use.
    Allow(ActSnapshot),
    /// Order must be rejected. Provides the failure classification.
    Deny(GateDecision),
}

/// Reason the gate denied a trade proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateDecision {
    NotOkFlag,
    NetBelowFloor,
    HighJitter,
    WideSpread,
    EmergencyBuffer,
}

/// Evaluate a packed ACT word against the gate configuration.
pub fn evaluate_gate(word: ActWord, config: &GateConfig) -> GateOutcome {
    let snapshot = word.unpack();

    if !snapshot.flags.contains(ActFlags::OK) {
        return GateOutcome::Deny(GateDecision::NotOkFlag);
    }

    if let Some(decision) = check_cost_floor(&snapshot, config) {
        return decision;
    }

    if config.reject_high_jitter && snapshot.flags.contains(ActFlags::HIGH_JITTER) {
        return GateOutcome::Deny(GateDecision::HighJitter);
    }

    if config.reject_wide_spread && snapshot.flags.contains(ActFlags::WIDE_SPREAD) {
        return GateOutcome::Deny(GateDecision::WideSpread);
    }

    if config.reject_emergency_buffer && snapshot.flags.contains(ActFlags::EMERG_BUF) {
        return GateOutcome::Deny(GateDecision::EmergencyBuffer);
    }

    GateOutcome::Allow(snapshot)
}

fn check_cost_floor(snapshot: &ActSnapshot, config: &GateConfig) -> Option<GateOutcome> {
    let mut effective_net = snapshot.net.to_bp();
    if let Some(k) = config.sigma_k {
        let sigma = snapshot.sigma.to_bp().abs();
        effective_net -= k * sigma;
    }

    if effective_net < snapshot.min_required.to_bp() {
        return Some(GateOutcome::Deny(GateDecision::NetBelowFloor));
    }

    None
}

/// Convenience for callers that want the net edge delta in basis points.
pub fn net_minus_floor(snapshot: &ActSnapshot, sigma_k: Option<f64>) -> f64 {
    let mut net = snapshot.net.to_bp();
    if let Some(k) = sigma_k {
        net -= k * snapshot.sigma.to_bp().abs();
    }
    net - snapshot.min_required.to_bp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{ActFlags, FixedQ8_8};

    fn snapshot_with(net: f64, floor: f64, sigma: f64, flags: ActFlags) -> ActSnapshot {
        ActSnapshot {
            net: FixedQ8_8::saturating_from_bp(net),
            min_required: FixedQ8_8::saturating_from_bp(floor),
            sigma: FixedQ8_8::saturating_from_bp(sigma),
            flags,
            ..ActSnapshot::empty()
        }
    }

    #[test]
    fn rejects_when_ok_flag_cleared() {
        let snapshot = snapshot_with(5.0, 1.0, 0.5, ActFlags::empty());
        let word = ActWord::pack(&snapshot);
        let outcome = evaluate_gate(word, &GateConfig::default());
        assert_eq!(outcome, GateOutcome::Deny(GateDecision::NotOkFlag));
    }

    #[test]
    fn allows_when_net_above_floor() {
        let snapshot = snapshot_with(5.0, 1.0, 0.4, ActFlags::OK);
        let word = ActWord::pack(&snapshot);
        let outcome = evaluate_gate(word, &GateConfig::default());
        assert!(matches!(outcome, GateOutcome::Allow(_)));
    }

    #[test]
    fn rejects_when_sigma_adjusted_net_below_floor() {
        let snapshot = snapshot_with(2.0, 1.5, 2.0, ActFlags::OK);
        let word = ActWord::pack(&snapshot);
        let config = GateConfig {
            sigma_k: Some(0.5),
            ..GateConfig::default()
        };
        let outcome = evaluate_gate(word, &config);
        assert_eq!(outcome, GateOutcome::Deny(GateDecision::NetBelowFloor));
    }

    #[test]
    fn jitter_and_spread_rejections_are_configurable() {
        let snapshot = snapshot_with(3.0, 1.0, 0.5, ActFlags::OK | ActFlags::HIGH_JITTER);
        let word = ActWord::pack(&snapshot);

        // Without rejecting high jitter the trade should pass.
        let outcome = evaluate_gate(word, &GateConfig::default());
        assert!(matches!(outcome, GateOutcome::Allow(_)));

        // Enabling the guard flips the outcome.
        let config = GateConfig {
            reject_high_jitter: true,
            ..GateConfig::default()
        };
        let outcome = evaluate_gate(word, &config);
        assert_eq!(outcome, GateOutcome::Deny(GateDecision::HighJitter));
    }
}
