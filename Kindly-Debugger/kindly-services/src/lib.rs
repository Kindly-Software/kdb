//! kindly.video Landing Page
//!
//! Premium WASM landing page with WebGL effects and glassmorphism UI.
//!
//! Built with Leptos 0.7 (CSR) + WebGL2 + Byzantine Royal Purple design.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

pub mod components;
pub mod effects;
pub mod utils;

use components::{Cta, Docs, Features, Footer, Hero, Navbar, Pricing, PrivacyPage, TermsPage, LicensePage};
use effects::MeshGradient;

/// Get current page from URL hash
fn get_current_page() -> String {
    let window = web_sys::window().expect("no window");
    let hash = window.location().hash().unwrap_or_default();
    hash.trim_start_matches('#').to_string()
}

/// Main application component
#[component]
pub fn App() -> impl IntoView {
    // Track current page via hash
    let (current_page, set_current_page) = signal(get_current_page());

    // Listen for hash changes and scroll to anchor
    Effect::new(move |_| {
        let window = web_sys::window().expect("no window");
        let set_page = set_current_page.clone();

        let hashchange_callback = Closure::wrap(Box::new(move || {
            let new_page = get_current_page();
            set_page.set(new_page.clone());

            // Scroll to anchor element after route change
            if !new_page.is_empty() {
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        // Use setTimeout to wait for DOM update after route change
                        let hash = new_page.clone();
                        let scroll_callback = Closure::wrap(Box::new(move || {
                            if let Some(element) = document.get_element_by_id(&hash) {
                                element.scroll_into_view();
                            }
                        }) as Box<dyn FnMut()>);

                        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                            scroll_callback.as_ref().unchecked_ref(),
                            50
                        );
                        std::mem::forget(scroll_callback);
                    }
                }
            }
        }) as Box<dyn FnMut()>);

        window
            .add_event_listener_with_callback("hashchange", hashchange_callback.as_ref().unchecked_ref())
            .expect("failed to add hashchange listener");

        std::mem::forget(hashchange_callback);
    });

    // Initialize WebGL mesh gradient on mount
    // Firefox compatibility: gracefully handle WebGL failures
    Effect::new(move |_| {
        // Initialize mesh gradient using the canvas ID
        match MeshGradient::new("gradient-canvas") {
            Ok(gradient) => {
                // Use Rc<RefCell> to share gradient in animation loop
                let gradient = std::rc::Rc::new(std::cell::RefCell::new(gradient));
                let g = gradient.clone();

                // Create animation loop using requestAnimationFrame
                let f: std::rc::Rc<std::cell::RefCell<Option<Closure<dyn FnMut()>>>> =
                    std::rc::Rc::new(std::cell::RefCell::new(None));
                let f_clone = f.clone();

                *f_clone.borrow_mut() = Some(Closure::wrap(Box::new(move || {
                    // Get current time from performance.now()
                    let window = web_sys::window().expect("no window");
                    let time = window.performance().expect("no performance").now();

                    // Render frame
                    g.borrow().render(time);

                    // Request next frame
                    if let Some(ref closure) = *f.borrow() {
                        window
                            .request_animation_frame(closure.as_ref().unchecked_ref())
                            .expect("failed to request animation frame");
                    }
                }) as Box<dyn FnMut()>));

                // Start the animation loop
                let window = web_sys::window().expect("no window");
                if let Some(ref closure) = *f_clone.borrow() {
                    window
                        .request_animation_frame(closure.as_ref().unchecked_ref())
                        .expect("failed to start animation");
                }

                // Leak the closure to keep it alive (animation runs forever)
                std::mem::forget(f_clone);
                std::mem::forget(gradient);

                web_sys::console::log_1(&"WebGL mesh gradient initialized successfully".into());
            }
            Err(e) => {
                // Firefox compatibility: WebGL may fail but page should still work
                // Common in Firefox due to driver blacklisting or disabled WebGL2
                web_sys::console::warn_1(&format!(
                    "WebGL init failed (this is OK - page will work without animated background): {:?}",
                    e
                ).into());

                // Hide the canvas so the CSS gradient fallback shows through
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        if let Some(canvas) = document.get_element_by_id("gradient-canvas") {
                            if let Some(canvas) = canvas.dyn_ref::<web_sys::HtmlElement>() {
                                let _ = canvas.style().set_property("display", "none");
                            }
                        }
                    }
                }
            }
        }
    });

    // Handle window resize for canvas
    Effect::new(move |_| {
        let window = web_sys::window().expect("no window");
        let document = window.document().expect("no document");

        let resize_callback = Closure::wrap(Box::new(move || {
            if let Some(canvas) = document.get_element_by_id("gradient-canvas") {
                let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into().unwrap();
                let window = web_sys::window().expect("no window");
                let width = window.inner_width().unwrap().as_f64().unwrap() as u32;
                let height = window.inner_height().unwrap().as_f64().unwrap() as u32;
                canvas.set_width(width);
                canvas.set_height(height);
            }
        }) as Box<dyn FnMut()>);

        window
            .add_event_listener_with_callback("resize", resize_callback.as_ref().unchecked_ref())
            .expect("failed to add resize listener");

        // Keep closure alive
        std::mem::forget(resize_callback);
    });

    let main_style = "
        min-height: 100vh;
        position: relative;
        z-index: 1;
    ";

    let canvas_style = "
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        z-index: 0;
        pointer-events: none;
    ";

    view! {
        // WebGL background canvas
        <canvas
            id="gradient-canvas"
            style=canvas_style
        />

        // Main content with hash-based routing
        <main style=main_style>
            <Navbar />
            {move || {
                let page = current_page.get();
                match page.as_str() {
                    "docs" => view! { <Docs /> }.into_any(),
                    "privacy" => view! { <PrivacyPage /> }.into_any(),
                    "terms" => view! { <TermsPage /> }.into_any(),
                    "license" => view! { <LicensePage /> }.into_any(),
                    _ => view! {
                        <Hero />
                        <Features />
                        <Pricing />
                        <Cta />
                    }.into_any(),
                }
            }}
            <Footer />
        </main>
    }
}

/// WASM entry point - mount the Leptos app
#[wasm_bindgen(start)]
pub fn main() {
    // Initialize panic hook for better error messages
    console_error_panic_hook::set_once();

    // Mount the app to the #app div, replacing the loading content
    let window = web_sys::window().expect("no window");
    let document = window.document().expect("no document");
    let app_element = document.get_element_by_id("app").expect("no #app element");

    // Cast to HtmlElement and clear the loading content
    let app_html: web_sys::HtmlElement = app_element.dyn_into().expect("not an HtmlElement");
    app_html.set_inner_html("");

    // Mount Leptos app (forget handle to keep it mounted forever)
    leptos::mount::mount_to(app_html, App).forget();
}
