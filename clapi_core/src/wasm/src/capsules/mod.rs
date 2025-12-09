// Phase 2 Capsules Module
//
// UCE34 Q10: Tier 1 Atomic (DashboardStateCapsule), Tier 5 Streaming (WsMessageCapsule)

pub mod dashboard_state;
pub mod ws_message;

pub use dashboard_state::DashboardStateCapsule;
pub use ws_message::{WsMessageCapsule, WsMessageType, WsPriority, WsMessageError};
