//! Verified Page Component
//!
//! Shown after successful email verification with license key display.
//! Matches Byzantine Royal theme with glassmorphism styling.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Verified page - displays license key and next steps after email verification
#[component]
pub fn Verified() -> impl IntoView {
    // Parse license from URL query params (check both hash and search)
    let license = Memo::new(move |_| {
        let window = web_sys::window().expect("window");

        // Try URL search params first (e.g., ?license=XXX)
        let search = window.location().search().unwrap_or_default();
        if let Some(license) = search
            .strip_prefix('?')
            .and_then(|s| {
                s.split('&')
                    .find(|p| p.starts_with("license="))
                    .map(|p| p.strip_prefix("license=").unwrap_or("").to_string())
            })
        {
            if !license.is_empty() {
                return license;
            }
        }

        // Try hash params (e.g., #verified?license=XXX)
        let hash = window.location().hash().unwrap_or_default();
        if let Some(query) = hash.split_once('?').map(|(_, q)| q) {
            if let Some(license) = query
                .split('&')
                .find(|p| p.starts_with("license="))
                .and_then(|p| p.strip_prefix("license="))
            {
                if !license.is_empty() {
                    return license.to_string();
                }
            }
        }

        "License key not found".to_string()
    });

    let (copied, set_copied) = signal(false);

    let copy_to_clipboard = move |_| {
        let license_val = license.get();
        if let Some(window) = web_sys::window() {
            let clipboard = window.navigator().clipboard();
            let _ = clipboard.write_text(&license_val);
            set_copied.set(true);

            // Reset after 2 seconds using setTimeout
            let set_copied_clone = set_copied;
            let reset_callback = Closure::wrap(Box::new(move || {
                set_copied_clone.set(false);
            }) as Box<dyn FnMut()>);

            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                reset_callback.as_ref().unchecked_ref(),
                2000,
            );
            // Keep closure alive until it fires
            std::mem::forget(reset_callback);
        }
    };

    // Styles
    let page_style = "
        min-height: 100vh;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 2rem;
        position: relative;
        z-index: 1;
    ";

    let card_style = "
        background: rgba(255, 255, 255, 0.05);
        backdrop-filter: blur(20px);
        -webkit-backdrop-filter: blur(20px);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 24px;
        padding: 3rem;
        max-width: 600px;
        width: 100%;
    ";

    let success_badge_style = "
        background: linear-gradient(135deg, #4CAF50, #45a049);
        color: #fff;
        padding: 0.5rem 1rem;
        border-radius: 20px;
        font-size: 0.875rem;
        font-weight: 600;
        display: inline-block;
        margin-bottom: 1.5rem;
    ";

    let title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 2rem;
        font-weight: 700;
        color: #fff;
        text-align: center;
        margin-bottom: 1rem;
    ";

    let license_box_style = "
        background: rgba(0, 0, 0, 0.3);
        border: 2px solid rgba(255, 215, 0, 0.3);
        border-radius: 12px;
        padding: 1.5rem;
        margin: 2rem 0;
        position: relative;
    ";

    let license_label_style = "
        color: rgba(255, 255, 255, 0.6);
        font-size: 0.75rem;
        text-transform: uppercase;
        margin-bottom: 0.5rem;
    ";

    let license_key_style = "
        font-family: 'JetBrains Mono', monospace;
        font-size: 1.125rem;
        color: #FFD700;
        word-break: break-all;
    ";

    let copy_button_style = "
        position: absolute;
        top: 1rem;
        right: 1rem;
        background: rgba(255, 215, 0, 0.2);
        border: 1px solid rgba(255, 215, 0, 0.3);
        border-radius: 8px;
        padding: 0.5rem 1rem;
        color: #FFD700;
        font-size: 0.875rem;
        cursor: pointer;
        transition: background 0.2s ease;
    ";

    let promo_box_style = "
        background: linear-gradient(135deg, rgba(255, 215, 0, 0.1), rgba(255, 165, 0, 0.05));
        border: 1px solid rgba(255, 215, 0, 0.2);
        border-radius: 12px;
        padding: 1rem;
        margin-bottom: 1rem;
    ";

    let promo_header_style = "
        display: flex;
        align-items: center;
        gap: 0.5rem;
        color: #FFD700;
    ";

    let promo_text_style = "
        color: rgba(255, 255, 255, 0.7);
        font-size: 0.875rem;
        margin-top: 0.5rem;
    ";

    let steps_style = "
        background: rgba(255, 255, 255, 0.03);
        border-radius: 16px;
        padding: 1.5rem;
        margin-top: 2rem;
    ";

    let step_title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 1.125rem;
        font-weight: 600;
        color: #fff;
        margin-bottom: 1rem;
    ";

    let step_style = "
        display: flex;
        align-items: flex-start;
        gap: 1rem;
        padding: 0.75rem 0;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    ";

    let step_style_last = "
        display: flex;
        align-items: flex-start;
        gap: 1rem;
        padding: 0.75rem 0;
        border-bottom: none;
    ";

    let step_number_style = "
        background: linear-gradient(135deg, #FFD700, #FFA500);
        color: #000;
        width: 24px;
        height: 24px;
        border-radius: 50%;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 0.75rem;
        font-weight: 700;
        flex-shrink: 0;
    ";

    let step_text_style = "
        color: rgba(255, 255, 255, 0.8);
        font-size: 0.9375rem;
        line-height: 1.5;
    ";

    let code_style = "
        background: rgba(0, 0, 0, 0.3);
        padding: 0.25rem 0.5rem;
        border-radius: 4px;
        font-family: 'JetBrains Mono', monospace;
        font-size: 0.875rem;
        color: #FFD700;
        display: block;
        margin-top: 0.5rem;
        word-break: break-all;
    ";

    let docs_link_style = "
        color: #FFD700;
        text-decoration: none;
        font-weight: 600;
    ";

    view! {
        <style>
            ".copy-btn:hover {
                background: rgba(255, 215, 0, 0.3) !important;
            }
            .docs-link:hover {
                text-decoration: underline;
            }"
        </style>
        <section id="verified" style=page_style>
            <div style=card_style>
                <div style="text-align: center;">
                    <div style="font-size: 4rem; margin-bottom: 1rem;">"🎉"</div>
                    <span style=success_badge_style>"Email Verified"</span>
                    <h1 style=title_style>"Welcome to KDB!"</h1>
                </div>

                <div style=license_box_style>
                    <div style=license_label_style>"Your License Key"</div>
                    <div style=license_key_style>{move || license.get()}</div>
                    <button style=copy_button_style class="copy-btn" on:click=copy_to_clipboard>
                        {move || if copied.get() { "Copied! ✓" } else { "Copy" }}
                    </button>
                </div>

                <div style=promo_box_style>
                    <div style=promo_header_style>
                        <span>"🎁"</span>
                        <span style="font-weight: 600;">"Launch Week Bonus!"</span>
                    </div>
                    <p style=promo_text_style>
                        "Enjoy unlimited debugging sessions this week. After the promo, your Hobby tier includes 5 sessions/month."
                    </p>
                </div>

                <div style=steps_style>
                    <div style=step_title_style>"Quick Start"</div>

                    <div style=step_style>
                        <span style=step_number_style>"1"</span>
                        <div>
                            <span style=step_text_style>"Install the Claude Code extension in VS Code"</span>
                        </div>
                    </div>

                    <div style=step_style>
                        <span style=step_number_style>"2"</span>
                        <div>
                            <span style=step_text_style>"Add your license key to MCP settings:"</span>
                            <code style=code_style>"KDB_LICENSE_KEY="{move || license.get()}</code>
                        </div>
                    </div>

                    <div style=step_style_last>
                        <span style=step_number_style>"3"</span>
                        <div>
                            <span style=step_text_style>"Start debugging! Ask Claude to attach to any process."</span>
                        </div>
                    </div>
                </div>

                // MCP Configuration section
                <div style="
                    margin-top: 2rem;
                    padding: 2rem;
                    background: rgba(255, 255, 255, 0.05);
                    border: 1px solid rgba(255, 215, 0, 0.3);
                    border-radius: 12px;
                ">
                    <h3 style="
                        font-size: 1.25rem;
                        font-weight: 600;
                        color: #FFD700;
                        margin-bottom: 1rem;
                    ">"Claude Code Setup"</h3>
                    <p style="color: rgba(255,255,255,0.8); margin-bottom: 1rem;">
                        "Add this to your Claude Code MCP settings file at "
                        <code style="color: #FFD700;">"~/.claude.json"</code>
                    </p>
                    <pre style="
                        background: rgba(0, 0, 0, 0.3);
                        padding: 1.5rem;
                        border-radius: 8px;
                        font-family: 'JetBrains Mono', 'Courier New', monospace;
                        font-size: 0.875rem;
                        color: #fff;
                        overflow-x: auto;
                        margin-bottom: 1rem;
                        white-space: pre-wrap;
                    ">
                        <code>{move || {
                            let key = license.get();
                            format!(r#"{{
  "mcpServers": {{
    "kdb": {{
      "transport": "sse",
      "url": "https://mcp.kindly.software/sse",
      "headers": {{
        "X-License-Key": "{}"
      }}
    }}
  }}
}}"#, key)
                        }}</code>
                    </pre>
                    {
                        let (config_copied, set_config_copied) = signal(false);
                        let license_for_copy = license.clone();
                        let copy_config = move |_| {
                            let key = license_for_copy.get();
                            let config = format!(r#"{{
  "mcpServers": {{
    "kdb": {{
      "transport": "sse",
      "url": "https://mcp.kindly.software/sse",
      "headers": {{
        "X-License-Key": "{}"
      }}
    }}
  }}
}}"#, key);
                            if let Some(window) = web_sys::window() {
                                let clipboard = window.navigator().clipboard();
                                let _ = clipboard.write_text(&config);
                                set_config_copied.set(true);

                                // Reset after 2 seconds
                                let set_config_copied_clone = set_config_copied;
                                let reset_callback = Closure::wrap(Box::new(move || {
                                    set_config_copied_clone.set(false);
                                }) as Box<dyn FnMut()>);

                                let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                                    reset_callback.as_ref().unchecked_ref(),
                                    2000,
                                );
                                std::mem::forget(reset_callback);
                            }
                        };
                        view! {
                            <button
                                style="background: rgba(255, 215, 0, 0.2); color: #FFD700; border: 1px solid rgba(255, 215, 0, 0.4); padding: 0.75rem 1.5rem; border-radius: 8px; cursor: pointer; font-weight: 600;"
                                class="copy-btn"
                                on:click=copy_config
                            >
                                {move || if config_copied.get() { "Copied! ✓" } else { "Copy Configuration" }}
                            </button>
                        }
                    }

                    <p style="color: rgba(255,255,255,0.6); font-size: 0.875rem; margin-top: 1rem;">
                        "Works with Claude Code, Cursor, and any MCP-compatible client"
                    </p>
                </div>

                <div style="text-align: center; margin-top: 2rem;">
                    <a href="#docs" style=docs_link_style class="docs-link">
                        "Read the Documentation →"
                    </a>
                </div>
            </div>
        </section>
    }
}
