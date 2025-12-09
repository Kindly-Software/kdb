use crate::coordinator::{OrderRequest, StrategyRouter};
use crate::gate::GateOutcome;

/// Simple router adapter that counts outcomes for later inspection.
pub struct CountingRouter {
    allowed: usize,
    denied: usize,
}

impl CountingRouter {
    pub fn new() -> Self {
        Self {
            allowed: 0,
            denied: 0,
        }
    }

    pub fn allowed(&self) -> usize {
        self.allowed
    }

    pub fn denied(&self) -> usize {
        self.denied
    }
}

impl Default for CountingRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl StrategyRouter for CountingRouter {
    fn handle_gate(&mut self, _request: &OrderRequest, outcome: GateOutcome) {
        match outcome {
            GateOutcome::Allow(_) => self.allowed += 1,
            GateOutcome::Deny(_) => self.denied += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinator::OrderRequest;
    use crate::estimator::{OrderIntent, Route, Side};
    use crate::gate::GateDecision;
    use crate::layout::ActSnapshot;

    #[test]
    fn counts_outcomes() {
        let mut router = CountingRouter::new();
        let request = OrderRequest::new(
            "MES",
            Route::Maker,
            0,
            OrderIntent {
                side: Side::Buy,
                route: Route::Maker,
                size: 1.0,
                size_normalizer: 1.0,
                price: 4_000.0,
                tick_size: 0.25,
                gross_edge_signal_bp: Some(3.0),
            },
        );

        router.handle_gate(&request, GateOutcome::Allow(ActSnapshot::default()));
        router.handle_gate(&request, GateOutcome::Deny(GateDecision::NotOkFlag));

        assert_eq!(router.allowed(), 1);
        assert_eq!(router.denied(), 1);
    }
}
