//! KDB API Landing Page
//!
//! Leptos WASM application for api.kindly.software documentation.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

mod components;
mod app;

use app::App;

/// WASM entry point (called automatically by Trunk)
#[wasm_bindgen(start)]
pub fn main() {
    // Better WASM panic messages
    console_error_panic_hook::set_once();

    // Mount Leptos app to #app div (replacing loading content)
    let window = web_sys::window().expect("no window");
    let document = window.document().expect("no document");
    let app_element = document.get_element_by_id("app").expect("no #app element");

    // Cast to HtmlElement and clear the loading content
    use wasm_bindgen::JsCast;
    let app_html: web_sys::HtmlElement = app_element.dyn_into().expect("not an HtmlElement");
    app_html.set_inner_html("");

    // Mount Leptos app
    leptos::mount::mount_to(app_html, App).forget();
}
