//! Main App Component
//!
//! Root component for KDB API landing page.

use leptos::prelude::*;
use kindly_ui::theme::colors::*;
use kindly_ui::MeshGradient;
use crate::components::{Navbar, ApiHero, EndpointList, LiveStats};

#[component]
pub fn App() -> impl IntoView {
    // Initialize WebGL mesh gradient
    Effect::new(move |_| {
        use wasm_bindgen::JsCast;
        use wasm_bindgen::closure::Closure;

        match MeshGradient::new("webgl-bg") {
            Ok(gradient) => {
                let gradient = std::rc::Rc::new(std::cell::RefCell::new(gradient));
                let g = gradient.clone();

                // Animation loop
                let f: std::rc::Rc<std::cell::RefCell<Option<Closure<dyn FnMut()>>>> =
                    std::rc::Rc::new(std::cell::RefCell::new(None));
                let f_clone = f.clone();

                *f_clone.borrow_mut() = Some(Closure::wrap(Box::new(move || {
                    let window = web_sys::window().expect("no window");
                    let time = window.performance().expect("no performance").now();
                    g.borrow().render(time);

                    if let Some(ref closure) = *f.borrow() {
                        window
                            .request_animation_frame(closure.as_ref().unchecked_ref())
                            .expect("failed to request animation frame");
                    }
                }) as Box<dyn FnMut()>));

                let window = web_sys::window().expect("no window");
                if let Some(ref closure) = *f_clone.borrow() {
                    window
                        .request_animation_frame(closure.as_ref().unchecked_ref())
                        .expect("failed to start animation");
                }

                std::mem::forget(f_clone);
                std::mem::forget(gradient);
            }
            Err(e) => {
                web_sys::console::error_1(&format!("WebGL init failed: {:?}", e).into());
            }
        }
    });

    let main_style = "
        min-height: 100vh;
        position: relative;
        z-index: 1;
    ";

    let footer_style = format!(
        "padding: 3rem 2rem;
         text-align: center;
         border-top: 1px solid {};
         color: {};",
        GLASS_BORDER,
        TEXT_MUTED
    );

    view! {
        <style>
            "html {
                scroll-behavior: smooth;
            }"
        </style>

        // WebGL background
        <canvas id="webgl-bg" style="position: fixed; top: 0; left: 0; width: 100%; height: 100%; z-index: 0; pointer-events: none;" />

        <main style=main_style>
            <Navbar />
            <ApiHero />
            <LiveStats />
            <EndpointList />

            <footer style=footer_style>
                <div style="font-size: 0.875rem;">
                    "KDB Debug API v0.1.0 | "
                    <a href="https://kindly.software" style="color: #FFD700; text-decoration: none;">"kindly.software"</a>
                    " | "
                    <a href="mailto:support@kindly.software" style="color: #FFD700; text-decoration: none;">"support@kindly.software"</a>
                </div>
                <div style="margin-top: 0.5rem; font-size: 0.75rem; opacity: 0.6;">
                    "Powered by Rust + Leptos"
                </div>
            </footer>
        </main>
    }
}
