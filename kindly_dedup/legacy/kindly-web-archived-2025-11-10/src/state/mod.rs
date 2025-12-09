// State management capsules (AppState, Theme, etc.)
mod app_state;
mod navigation;
mod scroll;
mod theme;
mod ui;

// Atomic capsules (Tier 1)
pub mod error;
mod app_state_capsule;
mod budget_view;
mod theme_capsule;
mod websocket;
mod metrics;

// Leptos state exports (existing) - Allow unused as they're part of public API
#[allow(unused_imports)]
pub use app_state::AppState;
#[allow(unused_imports)]
pub use navigation::NavigationState;
#[allow(unused_imports)]
pub use scroll::{ScrollDirection, ScrollState};
#[allow(unused_imports)]
pub use theme::Theme;
#[allow(unused_imports)]
pub use ui::{Toast, ToastVariant, UiState};

// Atomic capsule exports (Tier 1)
pub use error::{CapsuleError, CapsuleResult};
pub use app_state_capsule::AppStateCapsule;
pub use budget_view::BudgetViewCapsule;
pub use theme_capsule::ThemeCapsule;
pub use websocket::{WebSocketStateCapsule, WebSocketState};
pub use metrics::MetricsCapsule;
