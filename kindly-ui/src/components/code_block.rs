//! Code Block Component
//!
//! Syntax-highlighted code display with copy-to-clipboard functionality.

use leptos::prelude::*;
use crate::theme::colors::*;

#[component]
pub fn CodeBlock(
    #[prop(into)] code: String,
    #[prop(optional, into)] language: String,
) -> impl IntoView {
    let (copied, set_copied) = signal(false);

    let code_block_style = format!(
        "background: rgba(0, 0, 0, 0.4);
         border: 1px solid {};
         border-radius: 12px;
         padding: 1rem 1.25rem;
         font-family: {};
         font-size: 0.875rem;
         color: {};
         overflow-x: auto;
         margin-bottom: 1rem;
         white-space: pre-wrap;
         position: relative;",
        GLASS_BORDER,
        FONT_CODE,
        TEXT_CODE,
    );

    let copy_button_style = "
        position: absolute;
        top: 0.75rem;
        right: 0.75rem;
        background: rgba(255, 255, 255, 0.1);
        border: 1px solid rgba(255, 255, 255, 0.2);
        border-radius: 8px;
        padding: 0.5rem 0.75rem;
        color: #fff;
        font-size: 0.75rem;
        cursor: pointer;
        transition: all 0.2s ease;
    ";

    let code_content = code.clone();
    let copy_code = code.clone();

    let on_copy = move |_| {
        let code_to_copy = copy_code.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(window) = web_sys::window() {
                let navigator = window.navigator();
                let clipboard = navigator.clipboard();
                let _ = wasm_bindgen_futures::JsFuture::from(
                    clipboard.write_text(&code_to_copy)
                ).await;
            }
        });
        set_copied.set(true);

        // Reset after 2 seconds
        gloo_timers::callback::Timeout::new(2000, move || {
            set_copied.set(false);
        }).forget();
    };

    view! {
        <style>
            ".copy-btn:hover {
                background: rgba(255, 215, 0, 0.2) !important;
                border-color: rgba(255, 215, 0, 0.4) !important;
            }"
        </style>
        <div style=code_block_style>
            <button
                style=copy_button_style
                class="copy-btn"
                on:click=on_copy
            >
                {move || if copied.get() { "✓ Copied" } else { "Copy" }}
            </button>
            <pre style="margin: 0;">{code_content}</pre>
        </div>
    }
}
