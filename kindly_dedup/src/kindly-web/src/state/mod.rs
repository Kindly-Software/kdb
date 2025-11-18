// State management capsules (AppState, Theme, etc.)
mod app_state;
mod navigation;
mod scroll;
mod theme;
mod ui;

// Atomic capsules (Tier 1)
mod app_state_capsule;
mod budget_view;
pub mod error;
mod metrics;
mod theme_capsule;
mod websocket;

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
pub use app_state_capsule::AppStateCapsule;
pub use budget_view::BudgetViewCapsule;
pub use error::{CapsuleError, CapsuleResult};
pub use metrics::MetricsCapsule;
pub use theme_capsule::ThemeCapsule;
pub use websocket::{WebSocketState, WebSocketStateCapsule};
