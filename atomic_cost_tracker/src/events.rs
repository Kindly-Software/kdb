use crate::coordinator::OrderRequest;
use crate::estimator::{FillFeedback, LatencyTicket, Route, SlipFeeSurface, VenueSnapshot};
use crate::gate::GateOutcome;
use atomic_slip_fee_surface::AsfSnapshot;

/// Event flowing through the ACT coordinator pipeline.
#[derive(Debug)]
pub enum ActEvent {
    /// Update the slip/fee surface (ASF-256).
    Surface {
        symbol: String,
        route: Route,
        surface: SlipFeeSurface,
    },
    /// Update the slip/fee surface from an ASF snapshot.
    SurfaceAsf {
        symbol: String,
        route: Route,
        snapshot: AsfSnapshot,
    },
    /// Update venue snapshot (AVS-128).
    Venue {
        symbol: String,
        route: Route,
        venue: VenueSnapshot,
    },
    /// Update latency ticket (ALT-128).
    Latency {
        symbol: String,
        route: Route,
        latency: LatencyTicket,
    },
    /// Evaluate an order intent.
    Order(OrderRequest),
    /// Record a realized fill for EWMA feedback.
    Fill {
        symbol: String,
        route: Route,
        size_bucket: u8,
        feedback: FillFeedback,
    },
    /// Force a telemetry flush.
    Flush,
}

/// Result emitted from processing an ACT event.
#[derive(Debug, PartialEq)]
pub enum ActEventResult {
    /// No output generated (e.g., feed update).
    None,
    /// Gate outcome returned for a processed order.
    Gate(GateOutcome),
}
