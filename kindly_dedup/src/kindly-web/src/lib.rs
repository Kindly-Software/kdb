use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

// Module declarations
mod components;
mod error;
mod pages;
mod state;
mod utils;

// Public exports
pub use error::{AppError, AppResult};

// Export atomic capsules (some may be unused internally but are part of public API)
pub use state::{
    AppStateCapsule, BudgetViewCapsule, CapsuleError, CapsuleResult, MetricsCapsule, ThemeCapsule, WebSocketState,
    WebSocketStateCapsule,
};

// Internal imports
use components::Navbar;
use pages::home::HomePage;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <Navbar />
            <Routes fallback=|| "Page not found.">
                <Route path=path!("/") view=HomePage />
            </Routes>
        </Router>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_renders() {
        // Basic test to ensure app compiles
    }
}
