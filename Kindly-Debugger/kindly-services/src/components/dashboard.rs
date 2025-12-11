//! Dashboard Page Component
//!
//! User dashboard shown after OAuth for existing users.
//! Displays license information, subscription status, and quick actions.
//! Handles #dashboard?token=XXX&callback=YYY URL hash format.
//! Matches Byzantine Royal theme with glassmorphism styling.
//!
//! Features:
//! - Fetches license info from /api/v1/my-license with Bearer token
//! - License key display with copy button
//! - Subscription tier and session usage
//! - Promo status (7-day trial active/expired)
//! - Quick actions (download license file, download setup script)
//! - MCP client configuration tabs (Claude Code, Cursor, VS Code)
//! - "Continue to Claude" button (if callback present)

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::script_generator::{
    generate_enhanced_setup_script, generate_linux_desktop_file, Platform, ScriptOptions,
};

/// Dashboard state machine
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DashboardState {
    /// Initial state - fetching license data
    Loading,
    /// License data successfully loaded
    Loaded,
    /// API request failed
    Error,
    /// No token provided in URL
    TokenMissing,
    /// Token is invalid or expired (401 from API)
    TokenExpired,
}

/// License information from API response
#[derive(Clone, Debug, Default)]
pub struct LicenseInfo {
    pub license_key: String,
    pub tier: String,
    pub email: String,
    pub org_name: Option<String>,
    pub is_promo: bool,
    pub promo_expires_at: Option<u64>,
    pub sessions_used: u32,
    pub sessions_limit: u32,
    pub feature_flags: u16,
}

/// Parse query parameters from a URL hash string
/// Handles format: #dashboard?token=XXX&callback=YYY
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

/// Trigger browser download of the setup script
///
/// For Linux: Downloads BOTH .sh script AND .desktop launcher file.
/// The .desktop file enables double-click installation (no terminal commands needed).
fn trigger_script_download(license: &str, platform: Platform) {
    let options = ScriptOptions::default();
    let script = generate_enhanced_setup_script(license, platform, options);

    match platform {
        Platform::Linux | Platform::Unknown => {
            // For Linux: Download BOTH .sh script AND .desktop launcher
            // 1. First download the .sh script (contains the license and setup logic)
            trigger_download(&script, "kdb-setup.sh", "application/x-sh");

            // 2. Download the .desktop launcher after a 500ms delay
            if let Some(window) = web_sys::window() {
                let desktop_content = generate_linux_desktop_file();
                let callback = Closure::once(Box::new(move || {
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
                    500,
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

/// Format tier display name
fn format_tier_name(tier: &str) -> String {
    match tier.to_uppercase().as_str() {
        "HOBBY" => "Hobby".to_string(),
        "PRO" => "Pro".to_string(),
        "ENGINEER" => "Engineer".to_string(),
        "TEAMS" => "Teams".to_string(),
        "ENTERPRISE" => "Enterprise".to_string(),
        _ => tier.to_string(),
    }
}

/// Get tier color
fn get_tier_color(tier: &str) -> &str {
    match tier.to_uppercase().as_str() {
        "HOBBY" => "#4CAF50",      // Green
        "PRO" => "#2196F3",        // Blue
        "ENGINEER" => "#9C27B0",   // Purple
        "TEAMS" => "#FF9800",      // Orange
        "ENTERPRISE" => "#FFD700", // Gold
        _ => "#6e3fff",            // Default purple
    }
}

/// Dashboard page - displays license info and quick actions for existing users
#[component]
pub fn Dashboard() -> impl IntoView {
    // State signals
    let (state, set_state) = signal(DashboardState::Loading);
    let (license, set_license) = signal(LicenseInfo::default());
    let (callback_url, set_callback_url) = signal(None::<String>);
    let (_token, set_token) = signal(String::new());
    let (error_msg, set_error_msg) = signal(String::new());
    let (copied, set_copied) = signal(false);
    let (platform, set_platform) = signal(Platform::Unknown);
    let (active_tab, set_active_tab) = signal(0u8); // 0=Claude Code, 1=Cursor, 2=VS Code

    // Parse URL and fetch license on mount
    Effect::new(move |_| {
        // Detect platform first
        let detected_platform = detect_platform();
        set_platform.set(detected_platform);

        if let Some(window) = web_sys::window() {
            if let Ok(hash) = window.location().hash() {
                let params = parse_hash_params(&hash);

                // Extract callback URL if present
                if let Some(cb) = params.get("callback") {
                    if !cb.is_empty() {
                        set_callback_url.set(Some(cb.clone()));
                    }
                }

                // Extract token
                if let Some(tok) = params.get("token") {
                    if tok.is_empty() {
                        set_state.set(DashboardState::TokenMissing);
                        return;
                    }
                    set_token.set(tok.clone());

                    // Fetch license from API
                    let token_clone = tok.clone();
                    let set_state_clone = set_state;
                    let set_license_clone = set_license;
                    let set_error_msg_clone = set_error_msg;

                    wasm_bindgen_futures::spawn_local(async move {
                        match fetch_license_info(&token_clone).await {
                            Ok(info) => {
                                set_license_clone.set(info);
                                set_state_clone.set(DashboardState::Loaded);
                            }
                            Err(e) => {
                                set_error_msg_clone.set(e.clone());
                                if e.contains("401") || e.contains("unauthorized") || e.contains("expired") {
                                    set_state_clone.set(DashboardState::TokenExpired);
                                } else {
                                    set_state_clone.set(DashboardState::Error);
                                }
                            }
                        }
                    });
                } else {
                    set_state.set(DashboardState::TokenMissing);
                }
            } else {
                set_state.set(DashboardState::TokenMissing);
            }
        }
    });

    // Copy button handler with reset
    let on_copy = move |_| {
        let license_val = license.get();
        copy_to_clipboard(&license_val.license_key);
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

    // Download license handler
    let on_download_license = move |_| {
        let license_val = license.get();
        trigger_license_download(&license_val.license_key);
    };

    // Download script handler
    let on_download_script = move |_| {
        let license_val = license.get();
        trigger_script_download(&license_val.license_key, platform.get());
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

    let section_style = "
        background: rgba(255, 255, 255, 0.03);
        border-radius: 16px;
        padding: 1.5rem;
        margin-bottom: 1.5rem;
    ";

    let section_title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 1.125rem;
        font-weight: 600;
        color: #fff;
        margin-bottom: 1rem;
        display: flex;
        align-items: center;
        gap: 0.5rem;
    ";

    let license_box_style = "
        background: rgba(26, 0, 51, 0.6);
        border: 2px solid rgba(110, 63, 255, 0.5);
        border-radius: 12px;
        padding: 1.25rem;
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
        font-size: 0.875rem;
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
        padding: 0.5rem 1rem;
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
        padding: 0.5rem 1rem;
        color: #a88bff;
        font-size: 0.875rem;
        cursor: pointer;
        transition: background 0.2s ease;
        font-weight: 500;
    ";

    let stat_grid_style = "
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
        gap: 1rem;
    ";

    let stat_card_style = "
        background: rgba(26, 0, 51, 0.4);
        border-radius: 12px;
        padding: 1rem;
        text-align: center;
    ";

    let stat_value_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 1.5rem;
        font-weight: 700;
        color: #fff;
        margin-bottom: 0.25rem;
    ";

    let stat_label_style = "
        color: rgba(255, 255, 255, 0.6);
        font-size: 0.75rem;
        text-transform: uppercase;
        letter-spacing: 0.05em;
    ";

    let promo_badge_style = "
        background: linear-gradient(135deg, #4CAF50, #45a049);
        color: #fff;
        padding: 0.375rem 0.75rem;
        border-radius: 20px;
        font-size: 0.75rem;
        font-weight: 600;
        display: inline-flex;
        align-items: center;
        gap: 0.375rem;
    ";

    let tab_container_style = "
        display: flex;
        gap: 0.5rem;
        margin-bottom: 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        padding-bottom: 0.5rem;
    ";

    let tab_style_active = "
        background: rgba(110, 63, 255, 0.3);
        border: none;
        border-radius: 8px;
        padding: 0.5rem 1rem;
        color: #fff;
        font-size: 0.875rem;
        cursor: pointer;
        font-weight: 500;
    ";

    let tab_style_inactive = "
        background: transparent;
        border: none;
        border-radius: 8px;
        padding: 0.5rem 1rem;
        color: rgba(255, 255, 255, 0.6);
        font-size: 0.875rem;
        cursor: pointer;
        font-weight: 500;
    ";

    let config_code_style = "
        background: rgba(10, 0, 21, 0.8);
        color: #00ff00;
        padding: 1rem;
        border-radius: 8px;
        font-family: 'JetBrains Mono', monospace;
        font-size: 0.75rem;
        overflow-x: auto;
        white-space: pre;
        line-height: 1.5;
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

    let continue_section_style = "
        text-align: center;
        margin-top: 2rem;
        padding-top: 1.5rem;
        border-top: 1px solid rgba(255, 255, 255, 0.1);
    ";

    let error_box_style = "
        background: rgba(244, 67, 54, 0.15);
        border: 1px solid rgba(244, 67, 54, 0.4);
        border-radius: 12px;
        padding: 1.5rem;
        text-align: center;
        color: #f44336;
    ";

    let loading_style = "
        text-align: center;
        padding: 3rem;
        color: rgba(255, 255, 255, 0.7);
    ";

    let link_style = "
        color: #FFD700;
        text-decoration: none;
        font-weight: 600;
    ";

    view! {
        <style>
            ".dashboard-copy-btn:hover {
                background: rgba(255, 215, 0, 0.3) !important;
            }
            .dashboard-download-btn:hover {
                background: rgba(110, 63, 255, 0.3) !important;
            }
            .dashboard-primary-btn:hover {
                transform: translateY(-2px);
                box-shadow: 0 12px 40px rgba(110, 63, 255, 0.5);
            }
            .dashboard-primary-btn:active {
                transform: translateY(0);
            }
            .dashboard-tab:hover {
                background: rgba(110, 63, 255, 0.2) !important;
            }
            .dashboard-link:hover {
                text-decoration: underline;
            }
            @keyframes spin {
                to { transform: rotate(360deg); }
            }
            .spinner {
                animation: spin 1s linear infinite;
                display: inline-block;
            }
            @media (max-width: 600px) {
                .dashboard-card {
                    padding: 1.5rem !important;
                }
                .dashboard-title {
                    font-size: 1.5rem !important;
                }
                .dashboard-license-key {
                    font-size: 0.75rem !important;
                }
                .dashboard-config-code {
                    font-size: 0.625rem !important;
                }
            }"
        </style>

        <section id="dashboard" style=page_style>
            <div style=card_style class="dashboard-card">
                {move || {
                    match state.get() {
                        DashboardState::Loading => view! {
                            <div style=loading_style>
                                <div class="spinner" style="font-size: 3rem; margin-bottom: 1rem;">"&#8635;"</div>
                                <p>"Loading your account..."</p>
                            </div>
                        }.into_any(),

                        DashboardState::TokenMissing => view! {
                            <div style="text-align: center;">
                                <div style="font-size: 4rem; margin-bottom: 1rem;">"&#128274;"</div>
                                <h1 style=title_style class="dashboard-title">"Authentication Required"</h1>
                                <p style=subtitle_style>"No authentication token found. Please sign in again."</p>
                                <a href="/" style=link_style class="dashboard-link">"Return to Home"</a>
                            </div>
                        }.into_any(),

                        DashboardState::TokenExpired => view! {
                            <div style="text-align: center;">
                                <div style="font-size: 4rem; margin-bottom: 1rem;">"&#9203;"</div>
                                <h1 style=title_style class="dashboard-title">"Session Expired"</h1>
                                <p style=subtitle_style>"Your session has expired. Please sign in again."</p>
                                <a href="/#signup" style=link_style class="dashboard-link">"Sign In Again"</a>
                            </div>
                        }.into_any(),

                        DashboardState::Error => view! {
                            <div style=error_box_style>
                                <div style="font-size: 3rem; margin-bottom: 1rem;">"&#9888;"</div>
                                <h2 style="font-size: 1.25rem; font-weight: 600; margin-bottom: 0.5rem;">"Something went wrong"</h2>
                                <p style="font-size: 0.875rem; margin-bottom: 1rem;">{move || error_msg.get()}</p>
                                <button
                                    style="background: rgba(244, 67, 54, 0.2); border: 1px solid rgba(244, 67, 54, 0.4); border-radius: 8px; padding: 0.5rem 1rem; color: #f44336; cursor: pointer;"
                                    on:click=move |_| {
                                        if let Some(window) = web_sys::window() {
                                            let _ = window.location().reload();
                                        }
                                    }
                                >
                                    "Try Again"
                                </button>
                            </div>
                        }.into_any(),

                        DashboardState::Loaded => {
                            let license_info = license.get();
                            // Clone values to avoid borrow issues in view
                            let tier = license_info.tier.clone();
                            let tier_for_status = tier.clone();
                            let tier_display = format_tier_name(&tier);
                            let tier_color = get_tier_color(&tier).to_string();
                            let email = license_info.email.clone();
                            let license_key = license_info.license_key.clone();
                            let sessions_used = license_info.sessions_used;
                            let sessions_limit = license_info.sessions_limit;
                            let is_promo = license_info.is_promo;

                            view! {
                                // Header
                                <div style="text-align: center; margin-bottom: 2rem;">
                                    <div style="font-size: 3rem; margin-bottom: 0.5rem;">"&#128100;"</div>
                                    <h1 style=title_style class="dashboard-title">"Welcome Back!"</h1>
                                    <p style=subtitle_style>{email}</p>
                                </div>

                                // License Key Section
                                <div style=section_style>
                                    <h3 style=section_title_style>"&#128273; License Key"</h3>
                                    <div style=license_box_style>
                                        <div style=license_label_style>"Your License Key"</div>
                                        <div style=license_key_style class="dashboard-license-key">
                                            {license_key}
                                        </div>
                                        <div style=button_row_style>
                                            <button
                                                style=copy_button_style
                                                class="dashboard-copy-btn"
                                                on:click=on_copy
                                            >
                                                {move || if copied.get() { "&#9989; Copied!" } else { "&#128203; Copy Key" }}
                                            </button>
                                            <button
                                                style=download_button_style
                                                class="dashboard-download-btn"
                                                on:click=on_download_license
                                            >
                                                "&#128190; Download License"
                                            </button>
                                            <button
                                                style=download_button_style
                                                class="dashboard-download-btn"
                                                on:click=on_download_script
                                            >
                                                "&#128220; Setup Script"
                                            </button>
                                        </div>
                                    </div>
                                </div>

                                // Subscription Section
                                <div style=section_style>
                                    <h3 style=section_title_style>"&#128176; Subscription"</h3>
                                    <div style=stat_grid_style>
                                        <div style=stat_card_style>
                                            <div style=format!("{}; color: {};", stat_value_style, tier_color)>
                                                {tier_display}
                                            </div>
                                            <div style=stat_label_style>"Current Tier"</div>
                                        </div>
                                        <div style=stat_card_style>
                                            <div style=stat_value_style>
                                                {format!("{}/{}", sessions_used, sessions_limit)}
                                            </div>
                                            <div style=stat_label_style>"Sessions This Month"</div>
                                        </div>
                                        <div style=stat_card_style>
                                            {if is_promo {
                                                view! {
                                                    <div style=promo_badge_style>"&#10024; Trial Active"</div>
                                                    <div style="margin-top: 0.5rem;">
                                                        <div style=stat_label_style>"Full Features Unlocked"</div>
                                                    </div>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <div style=stat_value_style>
                                                        {if tier_for_status.to_uppercase() == "HOBBY" { "Free" } else { "Active" }}
                                                    </div>
                                                    <div style=stat_label_style>"Status"</div>
                                                }.into_any()
                                            }}
                                        </div>
                                    </div>
                                </div>

                                // MCP Client Configuration Section
                                <div style=section_style>
                                    <h3 style=section_title_style>"&#9881; MCP Client Configuration"</h3>
                                    <div style=tab_container_style>
                                        <button
                                            style=move || if active_tab.get() == 0 { tab_style_active } else { tab_style_inactive }
                                            class="dashboard-tab"
                                            on:click=move |_| set_active_tab.set(0)
                                        >
                                            "Claude Code"
                                        </button>
                                        <button
                                            style=move || if active_tab.get() == 1 { tab_style_active } else { tab_style_inactive }
                                            class="dashboard-tab"
                                            on:click=move |_| set_active_tab.set(1)
                                        >
                                            "Cursor"
                                        </button>
                                        <button
                                            style=move || if active_tab.get() == 2 { tab_style_active } else { tab_style_inactive }
                                            class="dashboard-tab"
                                            on:click=move |_| set_active_tab.set(2)
                                        >
                                            "VS Code"
                                        </button>
                                    </div>

                                    {move || {
                                        let license_key = license.get().license_key.clone();
                                        let tab = active_tab.get();

                                        match tab {
                                            0 => view! {
                                                <div>
                                                    <p style="color: rgba(255, 255, 255, 0.7); font-size: 0.875rem; margin-bottom: 0.75rem;">
                                                        "Add to " <code style="background: rgba(10, 0, 21, 0.6); padding: 0.125rem 0.375rem; border-radius: 4px; color: #FFD700;">"~/.claude/claude_desktop_config.json"</code> ":"
                                                    </p>
                                                    <pre style=config_code_style class="dashboard-config-code">
{format!(r#"{{
  "mcpServers": {{
    "kdb": {{
      "command": "npx",
      "args": ["@kindly-software-inc/kdb"],
      "env": {{
        "KDB_LICENSE_KEY": "{}"
      }}
    }}
  }}
}}"#, license_key)}
                                                    </pre>
                                                </div>
                                            }.into_any(),
                                            1 => view! {
                                                <div>
                                                    <p style="color: rgba(255, 255, 255, 0.7); font-size: 0.875rem; margin-bottom: 0.75rem;">
                                                        "Add to Cursor settings (Settings " <code style="background: rgba(10, 0, 21, 0.6); padding: 0.125rem 0.375rem; border-radius: 4px; color: #FFD700;">">" </code> " MCP):"
                                                    </p>
                                                    <pre style=config_code_style class="dashboard-config-code">
{format!(r#"{{
  "mcpServers": {{
    "kdb": {{
      "command": "npx",
      "args": ["@kindly-software-inc/kdb"],
      "env": {{
        "KDB_LICENSE_KEY": "{}"
      }}
    }}
  }}
}}"#, license_key)}
                                                    </pre>
                                                </div>
                                            }.into_any(),
                                            _ => view! {
                                                <div>
                                                    <p style="color: rgba(255, 255, 255, 0.7); font-size: 0.875rem; margin-bottom: 0.75rem;">
                                                        "Add to VS Code settings.json (requires MCP extension):"
                                                    </p>
                                                    <pre style=config_code_style class="dashboard-config-code">
{format!(r#"{{
  "mcp.servers": {{
    "kdb": {{
      "command": "npx",
      "args": ["@kindly-software-inc/kdb"],
      "env": {{
        "KDB_LICENSE_KEY": "{}"
      }}
    }}
  }}
}}"#, license_key)}
                                                    </pre>
                                                </div>
                                            }.into_any(),
                                        }
                                    }}

                                    <p style="color: rgba(255, 255, 255, 0.5); font-size: 0.75rem; margin-top: 1rem;">
                                        "Or run: " <code style="background: rgba(10, 0, 21, 0.6); padding: 0.125rem 0.375rem; border-radius: 4px; color: #00ff00;">"npx kdb-configure --auto"</code> " to auto-configure all clients."
                                    </p>
                                </div>

                                // Continue to Claude Section
                                <div style=continue_section_style>
                                    {move || {
                                        if callback_url.get().is_some() {
                                            view! {
                                                <button
                                                    style=primary_button_style
                                                    class="dashboard-primary-btn"
                                                    on:click=on_continue
                                                >
                                                    "Continue to Claude &#10230;"
                                                </button>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <p style="color: rgba(255, 255, 255, 0.6); font-size: 0.9375rem;">
                                                    "You're all set! "
                                                    <a href="#docs" style=link_style class="dashboard-link">
                                                        "Read the documentation"
                                                    </a>
                                                    " or start debugging with Claude."
                                                </p>
                                            }.into_any()
                                        }
                                    }}
                                </div>
                            }.into_any()
                        }
                    }
                }}
            </div>
        </section>
    }
}

/// Fetch license info from API
async fn fetch_license_info(token: &str) -> Result<LicenseInfo, String> {
    let window = web_sys::window().ok_or("No window")?;

    // Create fetch request with Bearer token
    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(web_sys::RequestMode::Cors);

    // Create headers with Authorization
    let headers = web_sys::Headers::new().map_err(|_| "Failed to create headers")?;
    headers
        .set("Authorization", &format!("Bearer {}", token))
        .map_err(|_| "Failed to set Authorization header")?;
    opts.set_headers(&headers);

    let request = web_sys::Request::new_with_str_and_init(
        "https://api.kindly.software/api/v1/my-license",
        &opts,
    )
    .map_err(|_| "Failed to create request")?;

    // Execute fetch
    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch failed: {:?}", e))?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "Invalid response type")?;

    let status = resp.status();

    if status == 401 {
        return Err("401 unauthorized - token expired".to_string());
    }

    if status != 200 {
        return Err(format!("API returned status {}", status));
    }

    // Parse JSON response
    let json = wasm_bindgen_futures::JsFuture::from(
        resp.json().map_err(|_| "Failed to get JSON promise")?,
    )
    .await
    .map_err(|e| format!("Failed to parse JSON: {:?}", e))?;

    // Extract fields from JSON
    let license_key = js_sys::Reflect::get(&json, &JsValue::from_str("license_key"))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();

    let tier = js_sys::Reflect::get(&json, &JsValue::from_str("tier"))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| "HOBBY".to_string());

    let email = js_sys::Reflect::get(&json, &JsValue::from_str("email"))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();

    let org_name = js_sys::Reflect::get(&json, &JsValue::from_str("org_name"))
        .ok()
        .and_then(|v| v.as_string());

    let is_promo = js_sys::Reflect::get(&json, &JsValue::from_str("is_promo"))
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let promo_expires_at = js_sys::Reflect::get(&json, &JsValue::from_str("promo_expires_at"))
        .ok()
        .and_then(|v| v.as_f64())
        .map(|v| v as u64);

    let sessions_used = js_sys::Reflect::get(&json, &JsValue::from_str("sessions_used"))
        .ok()
        .and_then(|v| v.as_f64())
        .map(|v| v as u32)
        .unwrap_or(0);

    let sessions_limit = js_sys::Reflect::get(&json, &JsValue::from_str("sessions_limit"))
        .ok()
        .and_then(|v| v.as_f64())
        .map(|v| v as u32)
        .unwrap_or(5);

    let feature_flags = js_sys::Reflect::get(&json, &JsValue::from_str("feature_flags"))
        .ok()
        .and_then(|v| v.as_f64())
        .map(|v| v as u16)
        .unwrap_or(0x0F);

    Ok(LicenseInfo {
        license_key,
        tier,
        email,
        org_name,
        is_promo,
        promo_expires_at,
        sessions_used,
        sessions_limit,
        feature_flags,
    })
}
