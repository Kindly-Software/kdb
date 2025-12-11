//! OAuth Success Page Component
//!
//! Shown after OAuth completes with license key display and setup script download.
//! Handles #oauth-success?license=XXX&callback=YYY URL hash format.
//! Matches Byzantine Royal theme with glassmorphism styling.
//!
//! Features:
//! - Auto-downloads platform-specific setup script (.command/.sh/.bat)
//! - Auto-downloads .kdb-license file
//! - Platform detection from User-Agent
//! - Clear, platform-specific instructions

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::script_generator::{
    generate_enhanced_setup_script, generate_linux_desktop_file, Platform, ScriptOptions,
};

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

/// Detect platform from browser User-Agent
fn detect_platform() -> Platform {
    if let Some(window) = web_sys::window() {
        if let Ok(user_agent) = window.navigator().user_agent() {
            return Platform::detect_from_user_agent(&user_agent);
        }
    }
    Platform::Unknown
}

/// Trigger browser download of a file with given content and filename
fn trigger_download(content: &str, filename: &str, mime_type: &str) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };

    let document = match window.document() {
        Some(d) => d,
        None => return,
    };

    // Create Blob from content
    let array = js_sys::Array::new();
    array.push(&JsValue::from_str(content));

    let blob_opts = web_sys::BlobPropertyBag::new();
    blob_opts.set_type(mime_type);

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
            anchor.set_download(filename);

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

/// Trigger browser download of the license file
fn trigger_license_download(license: &str) {
    trigger_download(license, ".kdb-license", "text/plain");
}

/// Trigger browser download of the setup script (enhanced version)
///
/// For Linux: Downloads BOTH .sh script AND .desktop launcher file.
/// The .desktop file enables double-click installation (no terminal commands needed).
fn trigger_script_download(license: &str, platform: Platform) {
    // Use enhanced script with full UX improvements
    let options = ScriptOptions::default();
    let script = generate_enhanced_setup_script(license, platform, options.clone());

    match platform {
        Platform::Linux | Platform::Unknown => {
            // For Linux: Download BOTH .sh script AND .desktop launcher
            // 1. First download the .sh script (contains the license and setup logic)
            trigger_download(&script, "kdb-setup.sh", "application/x-sh");

            // 2. Download the .desktop launcher after a 500ms delay
            // This ensures both files land in Downloads folder
            if let Some(window) = web_sys::window() {
                let desktop_content = generate_linux_desktop_file();
                let callback = Closure::once(Box::new(move || {
                    // Create and trigger the .desktop file download
                    if let Some(window) = web_sys::window() {
                        if let Some(document) = window.document() {
                            let array = js_sys::Array::new();
                            array.push(&JsValue::from_str(&desktop_content));

                            let blob_opts = web_sys::BlobPropertyBag::new();
                            blob_opts.set_type("application/x-desktop");

                            if let Ok(blob) =
                                web_sys::Blob::new_with_str_sequence_and_options(&array, &blob_opts)
                            {
                                if let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) {
                                    if let Ok(elem) = document.create_element("a") {
                                        if let Some(anchor) =
                                            elem.dyn_ref::<web_sys::HtmlAnchorElement>()
                                        {
                                            anchor.set_href(&url);
                                            anchor.set_download("kdb-setup.desktop");

                                            if let Some(body) = document.body() {
                                                let _ = body.append_child(anchor);
                                                anchor.click();
                                                let _ = body.remove_child(anchor);
                                            }

                                            let _ = web_sys::Url::revoke_object_url(&url);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }) as Box<dyn FnOnce()>);

                let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                    callback.as_ref().unchecked_ref(),
                    500, // 500ms delay between downloads
                );
                callback.forget();
            }
        }
        Platform::MacOS => {
            trigger_download(&script, "kdb-setup.command", "application/x-sh");
        }
        Platform::Windows => {
            trigger_download(&script, "kdb-setup.bat", "application/x-bat");
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

/// Try kdb:// protocol handler with fallback to script download
///
/// Creates a hidden iframe to trigger the kdb:// protocol.
/// If the handler is registered, the terminal will open automatically.
/// After 2 seconds, falls back to downloading scripts (in case handler isn't registered).
///
/// # Arguments
/// * `window` - Web window object
/// * `license` - License key to pass to handler
/// * `fallback` - Closure to execute after timeout (downloads scripts)
fn try_protocol_handler<F>(window: &web_sys::Window, license: &str, fallback: F)
where
    F: FnOnce() + 'static,
{
    let document = match window.document() {
        Some(d) => d,
        None => {
            fallback();
            return;
        }
    };

    // Create hidden iframe to try kdb:// protocol
    let iframe = match document.create_element("iframe") {
        Ok(elem) => elem,
        Err(_) => {
            fallback();
            return;
        }
    };

    // Hide the iframe
    let _ = iframe.set_attribute("style", "display: none; width: 0; height: 0; border: 0;");

    // Build kdb:// URL
    let protocol_url = format!("kdb://setup?license={}", license);
    let _ = iframe.set_attribute("src", &protocol_url);

    // Append iframe to body (this triggers the protocol handler if registered)
    if let Some(body) = document.body() {
        let _ = body.append_child(&iframe);

        // Set 2 second timeout for fallback
        // If protocol handler opens, user gets terminal; if not, we download scripts
        let body_clone = body.clone();
        let iframe_clone = iframe.clone();

        let fallback_callback = Closure::once(Box::new(move || {
            // Remove the iframe
            let _ = body_clone.remove_child(&iframe_clone);

            // Execute fallback (download scripts)
            fallback();
        }) as Box<dyn FnOnce()>);

        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            fallback_callback.as_ref().unchecked_ref(),
            2000, // 2 second timeout
        );

        // Leak the closure so it stays alive for the timeout
        fallback_callback.forget();
    } else {
        // No body, just run fallback immediately
        fallback();
    }
}

/// OAuth Success page - displays license key after OAuth callback
#[component]
pub fn OAuthSuccess() -> impl IntoView {
    // Parse license and callback from URL hash
    let (license, set_license) = signal(String::new());
    let (callback_url, set_callback_url) = signal(None::<String>);
    let (download_triggered, set_download_triggered) = signal(false);
    let (script_downloaded, set_script_downloaded) = signal(false);
    let (copied, set_copied) = signal(false);
    let (platform, set_platform) = signal(Platform::Unknown);

    // Parse URL on mount
    Effect::new(move |_| {
        // Detect platform first
        let detected_platform = detect_platform();
        set_platform.set(detected_platform);

        if let Some(window) = web_sys::window() {
            if let Ok(hash) = window.location().hash() {
                let params = parse_hash_params(&hash);

                if let Some(lic) = params.get("license") {
                    if !lic.is_empty() {
                        set_license.set(lic.clone());

                        // Auto-trigger setup (only once)
                        if !download_triggered.get() {
                            set_download_triggered.set(true);

                            // Try protocol handler first (kdb://setup?license=XXX)
                            // If handler is registered, terminal opens automatically
                            // After 2s timeout, fall back to script download
                            let lic_for_protocol = lic.clone();
                            let lic_for_fallback = lic.clone();
                            let set_script_downloaded_clone = set_script_downloaded;

                            try_protocol_handler(&window, &lic_for_protocol, move || {
                                // Fallback: download license file and setup script
                                trigger_license_download(&lic_for_fallback);
                                trigger_script_download(&lic_for_fallback, detected_platform);
                                set_script_downloaded_clone.set(true);
                            });
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

    // Re-download license handler
    let on_redownload_license = move |_| {
        let license_val = license.get();
        trigger_license_download(&license_val);
    };

    // Re-download script handler
    let on_redownload_script = move |_| {
        let license_val = license.get();
        trigger_script_download(&license_val, platform.get());
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
        max-width: 900px;
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

    let download_notice_style = "
        background: rgba(76, 175, 80, 0.15);
        border: 1px solid rgba(76, 175, 80, 0.4);
        border-radius: 12px;
        padding: 1rem 1.25rem;
        margin-bottom: 1.5rem;
        display: flex;
        align-items: flex-start;
        gap: 0.75rem;
        color: #4CAF50;
        font-size: 0.9375rem;
    ";

    let download_icon_style = "
        font-size: 1.5rem;
        line-height: 1;
    ";

    let download_text_container_style = "
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    ";

    let download_main_text_style = "
        font-weight: 600;
        color: #4CAF50;
    ";

    let download_sub_text_style = "
        color: rgba(76, 175, 80, 0.8);
        font-size: 0.8125rem;
    ";

    let license_box_style = "
        background: rgba(26, 0, 51, 0.6);
        border: 2px solid rgba(110, 63, 255, 0.5);
        border-radius: 12px;
        padding: 1.5rem;
        margin: 1.5rem 0;
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
        font-size: 0.9375rem;
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

    let instructions_section_style = "
        background: rgba(255, 255, 255, 0.03);
        border-radius: 16px;
        padding: 1.5rem;
        margin-top: 2rem;
    ";

    let instructions_title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 1.25rem;
        font-weight: 600;
        color: #fff;
        margin-bottom: 1.25rem;
        display: flex;
        align-items: center;
        gap: 0.5rem;
    ";

    let platform_badge_style = "
        background: linear-gradient(135deg, #6e3fff, #8e5fff);
        color: #fff;
        padding: 0.25rem 0.75rem;
        border-radius: 6px;
        font-size: 0.75rem;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.05em;
    ";

    let step_list_style = "
        list-style: none;
        padding: 0;
        margin: 0;
    ";

    let step_item_style = "
        display: flex;
        align-items: flex-start;
        gap: 1rem;
        padding: 1rem 0;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    ";

    let step_number_style = "
        background: linear-gradient(135deg, #6e3fff, #8e5fff);
        color: #fff;
        width: 28px;
        height: 28px;
        border-radius: 50%;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 0.875rem;
        font-weight: 700;
        flex-shrink: 0;
    ";

    let step_content_style = "
        color: rgba(255, 255, 255, 0.9);
        font-size: 0.9375rem;
        line-height: 1.5;
    ";

    let code_inline_style = "
        background: rgba(10, 0, 21, 0.6);
        color: #FFD700;
        padding: 0.2rem 0.5rem;
        border-radius: 4px;
        font-family: 'JetBrains Mono', monospace;
        font-size: 0.8125rem;
    ";

    let alternative_section_style = "
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 12px;
        padding: 1.25rem;
        margin-top: 1.5rem;
    ";

    let alternative_title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 1rem;
        font-weight: 600;
        color: rgba(255, 255, 255, 0.8);
        margin-bottom: 0.75rem;
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

    let link_style = "
        color: #FFD700;
        text-decoration: none;
        font-weight: 600;
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
                .oauth-step-item {
                    flex-direction: column;
                    gap: 0.5rem !important;
                }
            }"
        </style>

        <section id="oauth-success" style=page_style>
            <div style=card_style class="oauth-card">
                // Header
                <div style="text-align: center;">
                    <div style="font-size: 4rem; margin-bottom: 1rem;">"\u{1F389}"</div>
                    <span style=success_badge_style>"Account Connected"</span>
                    <h1 style=title_style class="oauth-title">"Welcome to KDB!"</h1>
                    <h2 style=subtitle_style>"Your debugger is almost ready"</h2>
                </div>

                // Download status notice
                {move || {
                    let p = platform.get();
                    let files_text = match p {
                        Platform::Linux | Platform::Unknown => "kdb-setup.desktop and kdb-setup.sh".to_string(),
                        _ => platform.get().script_filename(),
                    };
                    if script_downloaded.get() {
                        view! {
                            <div style=download_notice_style>
                                <span style=download_icon_style>"\u{2705}"</span>
                                <div style=download_text_container_style>
                                    <span style=download_main_text_style>
                                        "Setup files downloaded!"
                                    </span>
                                    <span style=download_sub_text_style>
                                        "Check your Downloads folder for "
                                        <code style=code_inline_style>
                                            {files_text}
                                        </code>
                                    </span>
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div style=download_notice_style>
                                <span style=download_icon_style>"\u{2B07}\u{FE0F}"</span>
                                <div style=download_text_container_style>
                                    <span style=download_main_text_style>
                                        "Downloading setup files..."
                                    </span>
                                    <span style=download_sub_text_style>
                                        "License and setup script downloading automatically"
                                    </span>
                                </div>
                            </div>
                        }.into_any()
                    }
                }}

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
                            {move || if copied.get() { "\u{2705} Copied!" } else { "\u{1F4CB} Copy Key" }}
                        </button>
                        <button
                            style=download_button_style
                            class="oauth-download-btn"
                            on:click=on_redownload_license
                        >
                            "\u{1F4BE} Download License"
                        </button>
                        <button
                            style=download_button_style
                            class="oauth-download-btn"
                            on:click=on_redownload_script
                        >
                            "\u{1F4DC} Download Script"
                        </button>
                    </div>
                </div>

                // Platform-specific setup instructions
                <div style=instructions_section_style>
                    <h3 style=instructions_title_style>
                        "\u{1F4E5} Quick Setup"
                        <span style=platform_badge_style>
                            {move || platform.get().display_name()}
                        </span>
                    </h3>

                    // Platform-specific steps
                    {move || {
                        let p = platform.get();
                        match p {
                            Platform::MacOS => view! {
                                <ol style=step_list_style>
                                    <li style=step_item_style class="oauth-step-item">
                                        <span style=step_number_style>"1"</span>
                                        <span style=step_content_style>
                                            "Find "
                                            <code style=code_inline_style>"kdb-setup.command"</code>
                                            " in your Downloads folder"
                                        </span>
                                    </li>
                                    <li style=step_item_style class="oauth-step-item">
                                        <span style=step_number_style>"2"</span>
                                        <span style=step_content_style>
                                            "Double-click to run (Terminal will open automatically)"
                                        </span>
                                    </li>
                                    <li style=step_item_style class="oauth-step-item">
                                        <span style=step_number_style>"3"</span>
                                        <span style=step_content_style>
                                            "If macOS blocks it: Right-click \u{2192} Open \u{2192} Open anyway"
                                        </span>
                                    </li>
                                    <li class="oauth-step-item" style="display: flex; align-items: flex-start; gap: 1rem; padding: 1rem 0;">
                                        <span style=step_number_style>"4"</span>
                                        <span style=step_content_style>
                                            "Restart your terminal or IDE"
                                        </span>
                                    </li>
                                </ol>
                            }.into_any(),

                            Platform::Windows => view! {
                                <ol style=step_list_style>
                                    <li style=step_item_style class="oauth-step-item">
                                        <span style=step_number_style>"1"</span>
                                        <span style=step_content_style>
                                            "Find "
                                            <code style=code_inline_style>"kdb-setup.bat"</code>
                                            " in your Downloads folder"
                                        </span>
                                    </li>
                                    <li style=step_item_style class="oauth-step-item">
                                        <span style=step_number_style>"2"</span>
                                        <span style=step_content_style>
                                            "Double-click to run (Command Prompt will open)"
                                        </span>
                                    </li>
                                    <li style=step_item_style class="oauth-step-item">
                                        <span style=step_number_style>"3"</span>
                                        <span style=step_content_style>
                                            "If Windows blocks it: Click \"More info\" \u{2192} \"Run anyway\""
                                        </span>
                                    </li>
                                    <li class="oauth-step-item" style="display: flex; align-items: flex-start; gap: 1rem; padding: 1rem 0;">
                                        <span style=step_number_style>"4"</span>
                                        <span style=step_content_style>
                                            "Restart your terminal or IDE"
                                        </span>
                                    </li>
                                </ol>
                            }.into_any(),

                            Platform::Linux | Platform::Unknown => view! {
                                <ol style=step_list_style>
                                    <li style=step_item_style class="oauth-step-item">
                                        <span style=step_number_style>"1"</span>
                                        <span style=step_content_style>
                                            "Find "
                                            <code style=code_inline_style>"kdb-setup.desktop"</code>
                                            " and "
                                            <code style=code_inline_style>"kdb-setup.sh"</code>
                                            " in Downloads"
                                        </span>
                                    </li>
                                    <li style=step_item_style class="oauth-step-item">
                                        <span style=step_number_style>"2"</span>
                                        <span style=step_content_style>
                                            "Double-click "
                                            <code style=code_inline_style>"kdb-setup.desktop"</code>
                                            " (Terminal opens automatically)"
                                        </span>
                                    </li>
                                    <li style=step_item_style class="oauth-step-item">
                                        <span style=step_number_style>"3"</span>
                                        <span style=step_content_style>
                                            "Or manually: "
                                            <code style=code_inline_style>"chmod +x ~/Downloads/kdb-setup.sh && ~/Downloads/kdb-setup.sh"</code>
                                        </span>
                                    </li>
                                    <li class="oauth-step-item" style="display: flex; align-items: flex-start; gap: 1rem; padding: 1rem 0;">
                                        <span style=step_number_style>"4"</span>
                                        <span style=step_content_style>
                                            "Restart your terminal or IDE"
                                        </span>
                                    </li>
                                </ol>
                            }.into_any(),
                        }
                    }}

                    // Alternative method
                    <div style=alternative_section_style>
                        <h4 style=alternative_title_style>"\u{26A1} Alternative: One-Line Setup"</h4>
                        <pre style=command_style class="oauth-command">
                            {move || format!("npx kdb-configure --auto --license \"{}\"", license.get())}
                        </pre>
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
                                    "Continue to Claude Desktop \u{2192}"
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
