//! OAuth Success Page Component
//!
//! Shown after OAuth completes with license key display and download.
//! Handles #oauth-success?license=XXX&callback=YYY URL hash format.
//! Matches Byzantine Royal theme with glassmorphism styling.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Parse query parameters from a URL hash string
/// Handles format: #oauth-success?license=XXX&callback=YYY
fn parse_hash_params(hash: &str) -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();

    // Remove leading # if present
    let hash = hash.trim_start_matches('#');

    // Find the query string part (after ?)
    let query = if let Some(idx) = hash.find('?') {
        &hash[idx + 1..]
    } else {
        return params;
    };

    // Parse key=value pairs
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            // URL decode the key and value
            let decoded_key = urlencoding_decode(key);
            let decoded_value = urlencoding_decode(value);
            params.insert(decoded_key, decoded_value);
        }
    }

    params
}

/// Simple URL decoding (handles %XX sequences)
fn urlencoding_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            result.push('%');
            result.push_str(&hex);
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}

/// Trigger browser download of the license file
fn trigger_license_download(license: &str) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };

    let document = match window.document() {
        Some(d) => d,
        None => return,
    };

    // Create Blob from license text using JavaScript interop
    // web-sys Blob API can be complex, use js_sys for simplicity
    let array = js_sys::Array::new();
    array.push(&JsValue::from_str(license));

    let blob_opts = web_sys::BlobPropertyBag::new();
    blob_opts.set_type("text/plain");

    let blob = match web_sys::Blob::new_with_str_sequence_and_options(&array, &blob_opts) {
        Ok(b) => b,
        Err(_) => return,
    };

    // Create object URL
    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(_) => return,
    };

    // Create temporary anchor element and trigger download
    if let Ok(elem) = document.create_element("a") {
        if let Some(anchor) = elem.dyn_ref::<web_sys::HtmlAnchorElement>() {
            anchor.set_href(&url);
            anchor.set_download(".kdb-license");

            // Append to body, click, then remove
            if let Some(body) = document.body() {
                let _ = body.append_child(anchor);
                anchor.click();
                let _ = body.remove_child(anchor);
            }

            // Revoke the object URL to free memory
            let _ = web_sys::Url::revoke_object_url(&url);
        }
    }
}

/// Copy text to clipboard
fn copy_to_clipboard(text: &str) {
    if let Some(window) = web_sys::window() {
        let clipboard = window.navigator().clipboard();
        let _ = clipboard.write_text(text);
    }
}

/// OAuth Success page - displays license key after OAuth callback
#[component]
pub fn OAuthSuccess() -> impl IntoView {
    // Parse license and callback from URL hash
    let (license, set_license) = signal(String::new());
    let (callback_url, set_callback_url) = signal(None::<String>);
    let (download_triggered, set_download_triggered) = signal(false);
    let (copied, set_copied) = signal(false);

    // Parse URL on mount
    Effect::new(move |_| {
        if let Some(window) = web_sys::window() {
            if let Ok(hash) = window.location().hash() {
                let params = parse_hash_params(&hash);

                if let Some(lic) = params.get("license") {
                    if !lic.is_empty() {
                        set_license.set(lic.clone());

                        // Auto-trigger download (only once)
                        if !download_triggered.get() {
                            set_download_triggered.set(true);
                            trigger_license_download(lic);
                        }
                    }
                }

                if let Some(cb) = params.get("callback") {
                    if !cb.is_empty() {
                        set_callback_url.set(Some(cb.clone()));
                    }
                }
            }
        }
    });

    // Copy button handler with reset
    let on_copy = move |_| {
        let license_val = license.get();
        copy_to_clipboard(&license_val);
        set_copied.set(true);

        // Reset after 2 seconds
        if let Some(window) = web_sys::window() {
            let set_copied_clone = set_copied;
            let reset_callback = Closure::wrap(Box::new(move || {
                set_copied_clone.set(false);
            }) as Box<dyn FnMut()>);

            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                reset_callback.as_ref().unchecked_ref(),
                2000,
            );
            std::mem::forget(reset_callback);
        }
    };

    // Continue to callback URL
    let on_continue = move |_| {
        if let Some(url) = callback_url.get() {
            if let Some(window) = web_sys::window() {
                let _ = window.location().set_href(&url);
            }
        }
    };

    // Re-download handler
    let on_redownload = move |_| {
        let license_val = license.get();
        trigger_license_download(&license_val);
    };

    // Styles - Byzantine Royal purple theme with glassmorphism
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
        max-width: 800px;
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
        margin-bottom: 0.5rem;
    ";

    let subtitle_style = "
        color: rgba(255, 255, 255, 0.7);
        text-align: center;
        margin-bottom: 2rem;
    ";

    let license_box_style = "
        background: rgba(26, 0, 51, 0.6);
        border: 2px solid rgba(110, 63, 255, 0.5);
        border-radius: 12px;
        padding: 1.5rem;
        margin: 2rem 0;
        position: relative;
    ";

    let license_label_style = "
        color: rgba(255, 255, 255, 0.6);
        font-size: 0.75rem;
        text-transform: uppercase;
        letter-spacing: 0.05em;
        margin-bottom: 0.5rem;
    ";

    let license_key_style = "
        font-family: 'JetBrains Mono', monospace;
        font-size: 1rem;
        color: #FFD700;
        word-break: break-all;
        line-height: 1.5;
    ";

    let button_row_style = "
        display: flex;
        gap: 0.75rem;
        margin-top: 1rem;
        flex-wrap: wrap;
    ";

    let copy_button_style = "
        background: rgba(255, 215, 0, 0.2);
        border: 1px solid rgba(255, 215, 0, 0.3);
        border-radius: 8px;
        padding: 0.625rem 1.25rem;
        color: #FFD700;
        font-size: 0.875rem;
        cursor: pointer;
        transition: background 0.2s ease;
        font-weight: 500;
    ";

    let download_button_style = "
        background: rgba(110, 63, 255, 0.2);
        border: 1px solid rgba(110, 63, 255, 0.4);
        border-radius: 8px;
        padding: 0.625rem 1.25rem;
        color: #a88bff;
        font-size: 0.875rem;
        cursor: pointer;
        transition: background 0.2s ease;
        font-weight: 500;
    ";

    let setup_section_style = "
        background: rgba(255, 255, 255, 0.03);
        border-radius: 16px;
        padding: 1.5rem;
        margin-top: 2rem;
    ";

    let setup_title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 1.25rem;
        font-weight: 600;
        color: #fff;
        margin-bottom: 1.5rem;
    ";

    let option_style = "
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 12px;
        padding: 1.25rem;
        margin-bottom: 1rem;
    ";

    let option_title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 1rem;
        font-weight: 600;
        color: #FFD700;
        margin-bottom: 0.75rem;
        display: flex;
        align-items: center;
        gap: 0.5rem;
    ";

    let recommended_badge_style = "
        background: linear-gradient(135deg, #4CAF50, #45a049);
        color: #fff;
        padding: 0.25rem 0.5rem;
        border-radius: 4px;
        font-size: 0.625rem;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.05em;
    ";

    let command_style = "
        background: rgba(10, 0, 21, 0.8);
        color: #00ff00;
        padding: 1rem;
        border-radius: 8px;
        font-family: 'JetBrains Mono', monospace;
        font-size: 0.8125rem;
        overflow-x: auto;
        white-space: pre-wrap;
        line-height: 1.6;
    ";

    let env_command_style = "
        background: rgba(10, 0, 21, 0.8);
        color: #00ff00;
        padding: 1rem;
        border-radius: 8px;
        font-family: 'JetBrains Mono', monospace;
        font-size: 0.8125rem;
        overflow-x: auto;
        white-space: pre-wrap;
        line-height: 1.6;
    ";

    let option_text_style = "
        color: rgba(255, 255, 255, 0.7);
        font-size: 0.9375rem;
        line-height: 1.5;
    ";

    let link_style = "
        color: #FFD700;
        text-decoration: none;
        font-weight: 600;
    ";

    let continue_section_style = "
        text-align: center;
        margin-top: 2rem;
        padding-top: 1.5rem;
        border-top: 1px solid rgba(255, 255, 255, 0.1);
    ";

    let primary_button_style = "
        background: linear-gradient(135deg, #6e3fff, #8e5fff);
        color: white;
        padding: 1rem 2rem;
        border: none;
        border-radius: 8px;
        font-size: 1rem;
        font-weight: 600;
        cursor: pointer;
        transition: transform 0.2s ease, box-shadow 0.2s ease;
        box-shadow: 0 8px 30px rgba(110, 63, 255, 0.4);
    ";

    let fallback_text_style = "
        color: rgba(255, 255, 255, 0.6);
        font-size: 0.9375rem;
    ";

    let download_notice_style = "
        background: rgba(76, 175, 80, 0.1);
        border: 1px solid rgba(76, 175, 80, 0.3);
        border-radius: 8px;
        padding: 0.75rem 1rem;
        margin-bottom: 1.5rem;
        display: flex;
        align-items: center;
        gap: 0.5rem;
        color: #4CAF50;
        font-size: 0.875rem;
    ";

    view! {
        <style>
            ".oauth-copy-btn:hover {
                background: rgba(255, 215, 0, 0.3) !important;
            }
            .oauth-download-btn:hover {
                background: rgba(110, 63, 255, 0.3) !important;
            }
            .oauth-primary-btn:hover {
                transform: translateY(-2px);
                box-shadow: 0 12px 40px rgba(110, 63, 255, 0.5);
            }
            .oauth-primary-btn:active {
                transform: translateY(0);
            }
            .oauth-link:hover {
                text-decoration: underline;
            }
            @media (max-width: 600px) {
                .oauth-card {
                    padding: 1.5rem !important;
                }
                .oauth-title {
                    font-size: 1.5rem !important;
                }
                .oauth-license-key {
                    font-size: 0.75rem !important;
                }
                .oauth-command {
                    font-size: 0.6875rem !important;
                    padding: 0.75rem !important;
                }
            }"
        </style>

        <section id="oauth-success" style=page_style>
            <div style=card_style class="oauth-card">
                // Header
                <div style="text-align: center;">
                    <div style="font-size: 4rem; margin-bottom: 1rem;">"🎉"</div>
                    <span style=success_badge_style>"Account Connected"</span>
                    <h1 style=title_style class="oauth-title">"Welcome to KDB!"</h1>
                    <h2 style=subtitle_style>"Your account is ready to use"</h2>
                </div>

                // Download notice
                <div style=download_notice_style>
                    <span>"✓"</span>
                    <span>"Your license file (.kdb-license) is downloading automatically"</span>
                </div>

                // License key box
                <div style=license_box_style>
                    <div style=license_label_style>"Your License Key"</div>
                    <div style=license_key_style class="oauth-license-key">
                        {move || license.get()}
                    </div>
                    <div style=button_row_style>
                        <button
                            style=copy_button_style
                            class="oauth-copy-btn"
                            on:click=on_copy
                        >
                            {move || if copied.get() { "Copied!" } else { "Copy Key" }}
                        </button>
                        <button
                            style=download_button_style
                            class="oauth-download-btn"
                            on:click=on_redownload
                        >
                            "Download Again"
                        </button>
                    </div>
                </div>

                // Setup instructions
                <div style=setup_section_style>
                    <h3 style=setup_title_style>"Quick Setup (Choose One)"</h3>

                    // Option 1: Auto-Configure
                    <div style=option_style>
                        <div style=option_title_style>
                            <span>"Option 1: Auto-Configure"</span>
                            <span style=recommended_badge_style>"Recommended"</span>
                        </div>
                        <pre style=command_style class="oauth-command">
                            {"# Move downloaded file\nmkdir -p ~/.kdb\nmv ~/Downloads/.kdb-license ~/.kdb/license\n\n# Auto-configure all MCP clients\nnpx kdb-configure --auto"}
                        </pre>
                    </div>

                    // Option 2: Environment Variable
                    <div style=option_style>
                        <div style=option_title_style>
                            "Option 2: Environment Variable"
                        </div>
                        <pre style=env_command_style class="oauth-command">
                            {move || format!("export KDB_LICENSE_KEY=\"{}\"\nnpx kdb-configure --auto", license.get())}
                        </pre>
                    </div>

                    // Option 3: Manual Configuration
                    <div style=option_style>
                        <div style=option_title_style>
                            "Option 3: Manual Configuration"
                        </div>
                        <p style=option_text_style>
                            "See full instructions at: "
                            <a href="#docs" style=link_style class="oauth-link">
                                "kindly.software/docs"
                            </a>
                        </p>
                    </div>
                </div>

                // Continue section
                <div style=continue_section_style>
                    {move || {
                        if callback_url.get().is_some() {
                            view! {
                                <button
                                    style=primary_button_style
                                    class="oauth-primary-btn"
                                    on:click=on_continue
                                >
                                    "Continue to Claude Desktop"
                                </button>
                            }.into_any()
                        } else {
                            view! {
                                <p style=fallback_text_style>
                                    "You can close this window or "
                                    <a href="#docs" style=link_style class="oauth-link">
                                        "read the documentation"
                                    </a>
                                </p>
                            }.into_any()
                        }
                    }}
                </div>
            </div>
        </section>
    }
}
