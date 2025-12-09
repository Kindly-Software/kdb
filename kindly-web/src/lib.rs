use leptos::prelude::*;
use leptos_router::components::{Router, Routes, Route};
use leptos_router::path;

// Module declarations
mod components;
mod error;
mod pages;
mod utils;

// Public exports
pub use error::{AppError, AppResult};

// Export utilities (plain Rust, no capsules)
pub use utils::{
    theme, glassmorphism, layout,
};
pub use utils::layout::Breakpoint;

// Internal imports
use components::Navbar;
use components::LicensePage;
use pages::home::HomePage;
use pages::pricing_stripe::PricingPage;
use pages::success::SuccessPage;
use pages::cancel::CancelPage;
use pages::privacy::PrivacyPage;
use pages::terms::TermsPage;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <div style="position: relative; min-height: 100vh;">
            // Imperial Gold Columns - Full Page (Left)
            <div
                class="imperial-column-full-left"
                style="position: fixed; \
                       left: 0; \
                       top: 0; \
                       bottom: 0; \
                       width: 8px; \
                       background: linear-gradient(180deg, \
                           #FFD700 0%, \
                           #D4AF37 25%, \
                           #FFD700 50%, \
                           #D4AF37 75%, \
                           #FFD700 100%); \
                       box-shadow: \
                           0 0 30px rgba(255, 215, 0, 0.8), \
                           inset 0 0 20px rgba(255, 255, 255, 0.5), \
                           inset 0 0 40px rgba(255, 215, 0, 0.3); \
                       animation: holographic-shimmer 3s ease-in-out infinite; \
                       z-index: 1000;"
            />

            // Imperial Gold Columns - Full Page (Right)
            <div
                class="imperial-column-full-right"
                style="position: fixed; \
                       right: 0; \
                       top: 0; \
                       bottom: 0; \
                       width: 8px; \
                       background: linear-gradient(180deg, \
                           #FFD700 0%, \
                           #D4AF37 25%, \
                           #FFD700 50%, \
                           #D4AF37 75%, \
                           #FFD700 100%); \
                       box-shadow: \
                           0 0 30px rgba(255, 215, 0, 0.8), \
                           inset 0 0 20px rgba(255, 255, 255, 0.5), \
                           inset 0 0 40px rgba(255, 215, 0, 0.3); \
                       animation: holographic-shimmer 3s ease-in-out infinite 1.5s; \
                       z-index: 1000;"
            />

            <Router>
                <Navbar />
                <Routes fallback=|| "Page not found.">
                    <Route path=path!("/") view=HomePage />
                    <Route path=path!("/pricing") view=PricingPage />
                    <Route path=path!("/pricing/success") view=SuccessPage />
                    <Route path=path!("/pricing/cancel") view=CancelPage />
                    <Route path=path!("/privacy") view=PrivacyPage />
                    <Route path=path!("/terms") view=TermsPage />
                    <Route path=path!("/license") view=LicensePage />
                </Routes>
            </Router>
        </div>
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
