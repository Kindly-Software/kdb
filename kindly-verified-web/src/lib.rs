use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};
use wasm_bindgen::prelude::*;

pub mod capsules;  // 5 computational capsules for cutting-edge effects (71+ tests)
pub mod adaptive_rate_limiter;  // T10+T1 Adaptive Rate Limiter (28 tests, 2-5× mitigation improvement)
mod components;
mod pages;
pub mod utils;  // Byzantine design system

use pages::{home::HomePage, test::TestPage};

/// Main application component
#[component]
pub fn App() -> impl IntoView {
    // Provide meta context for SEO
    provide_meta_context();

    // Hide loading spinner once WASM loads
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(loading) = document.get_element_by_id("loading") {
                    let _ = loading.set_attribute("style", "display: none;");
                }
            }
        }
    }

    view! {
        <Router>
            // Global metadata
            <Title text="Kindly Verified - AI Image Detector" />

            // Application routes
            <Routes fallback=|| "Page not found.">
                <Route path=StaticSegment("") view=HomePage />
                <Route path=StaticSegment("test") view=TestPage />
            </Routes>
        </Router>
    }
}

/// WASM entry point - called from JavaScript
#[wasm_bindgen(start)]
pub fn main() {
    // Setup panic hook for better error messages
    console_error_panic_hook::set_once();

    // Initialize logging
    console_log::init_with_level(log::Level::Debug).expect("error initializing logger");

    log::info!("Starting Kindly Verified Web");

    // Mount the app
    leptos::mount::mount_to_body(App);
}
