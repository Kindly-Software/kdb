//! Signup Page Component
//!
//! Email signup form for KDB Hobby tier with launch week promo.
//! T0 Auditable tier - stateless form with client-side validation.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Signup form state machine
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SignupState {
    #[default]
    Input,
    Loading,
    Success,
    Error,
}

/// Signup page component
///
/// Renders a glassmorphism email signup form with:
/// - Email validation (client-side)
/// - Organization name (optional)
/// - POST to /api/v1/signup
/// - Loading/success/error states
#[component]
pub fn Signup() -> impl IntoView {
    // Form field signals
    let (email, set_email) = signal(String::new());
    let (org_name, set_org_name) = signal(String::new());
    let (state, set_state) = signal(SignupState::Input);
    let (error_msg, set_error_msg) = signal(String::new());

    // Form submission handler
    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();

        let email_val = email.get();
        let org_val = org_name.get();

        // Client-side validation
        if email_val.is_empty() || !email_val.contains('@') || !email_val.contains('.') {
            set_error_msg.set("Please enter a valid email address".to_string());
            set_state.set(SignupState::Error);
            return;
        }

        set_state.set(SignupState::Loading);

        // Spawn async API call
        wasm_bindgen_futures::spawn_local(async move {
            match submit_signup(&email_val, &org_val).await {
                Ok(()) => {
                    set_state.set(SignupState::Success);
                }
                Err(err) => {
                    set_error_msg.set(err);
                    set_state.set(SignupState::Error);
                }
            }
        });
    };

    // Page container style
    let page_style = "
        min-height: 100vh;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 2rem;
        position: relative;
        z-index: 1;
    ";

    // Glassmorphism card style
    let card_style = "
        background: rgba(255, 255, 255, 0.05);
        backdrop-filter: blur(20px);
        -webkit-backdrop-filter: blur(20px);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 24px;
        padding: 3rem;
        max-width: 480px;
        width: 100%;
    ";

    // Title style (Space Grotesk)
    let title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 2rem;
        font-weight: 700;
        color: #fff;
        text-align: center;
        margin-bottom: 0.5rem;
    ";

    // Subtitle style
    let subtitle_style = "
        color: rgba(255, 255, 255, 0.7);
        text-align: center;
        margin-bottom: 2rem;
    ";

    // Launch week promo badge
    let promo_badge_style = "
        background: linear-gradient(135deg, #FFD700, #FFA500);
        color: #000;
        padding: 0.5rem 1rem;
        border-radius: 20px;
        font-size: 0.875rem;
        font-weight: 600;
        display: inline-block;
        margin-bottom: 1.5rem;
        text-align: center;
    ";

    // Input field style
    let input_style = "
        width: 100%;
        padding: 1rem;
        background: rgba(255, 255, 255, 0.1);
        border: 1px solid rgba(255, 255, 255, 0.2);
        border-radius: 12px;
        color: #fff;
        font-size: 1rem;
        margin-bottom: 1rem;
        outline: none;
        transition: border-color 0.2s ease;
        box-sizing: border-box;
    ";

    // Primary button (gold gradient)
    let button_style = "
        width: 100%;
        padding: 1rem;
        background: linear-gradient(135deg, #FFD700, #FFA500);
        color: #000;
        border: none;
        border-radius: 12px;
        font-size: 1rem;
        font-weight: 600;
        cursor: pointer;
        transition: transform 0.2s ease, box-shadow 0.2s ease;
        box-shadow: 0 8px 30px rgba(255, 215, 0, 0.3);
    ";

    // Disabled button style
    let button_disabled_style = "
        width: 100%;
        padding: 1rem;
        background: rgba(255, 215, 0, 0.5);
        color: rgba(0, 0, 0, 0.6);
        border: none;
        border-radius: 12px;
        font-size: 1rem;
        font-weight: 600;
        cursor: not-allowed;
        box-shadow: none;
    ";

    // Error message style
    let error_style = "
        color: #FF5252;
        font-size: 0.875rem;
        margin-bottom: 1rem;
        text-align: center;
        padding: 0.75rem;
        background: rgba(255, 82, 82, 0.1);
        border-radius: 8px;
        border: 1px solid rgba(255, 82, 82, 0.3);
    ";

    // Terms/privacy links style
    let terms_style = "
        color: rgba(255, 255, 255, 0.5);
        font-size: 0.75rem;
        text-align: center;
        margin-top: 1rem;
    ";

    let link_style = "
        color: #FFD700;
        text-decoration: none;
    ";

    // Google OAuth button style
    let google_button_style = "
        width: 100%;
        padding: 1rem;
        background: #fff;
        color: #333;
        border: 2px solid rgba(255, 215, 0, 0.4);
        border-radius: 12px;
        font-size: 1rem;
        font-weight: 600;
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 12px;
        transition: background 0.2s ease, border-color 0.2s ease, transform 0.2s ease;
        box-sizing: border-box;
    ";

    // "or" separator style
    let separator_style = "
        display: flex;
        align-items: center;
        text-align: center;
        margin: 1.5rem 0;
    ";

    let separator_line_style = "
        flex: 1;
        height: 1px;
        background: linear-gradient(90deg, transparent, rgba(255, 215, 0, 0.4), transparent);
    ";

    let separator_text_style = "
        padding: 0 1rem;
        color: rgba(255, 255, 255, 0.5);
        font-size: 0.875rem;
    ";

    // Generate CSRF state for OAuth
    let generate_oauth_state = || -> String {
        use getrandom::getrandom;
        let mut bytes = [0u8; 16];
        let _ = getrandom(&mut bytes);
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    };

    // Google OAuth click handler
    let handle_google_signin = move |_: web_sys::MouseEvent| {
        let state = generate_oauth_state();

        // Build OAuth URL
        let oauth_url = format!(
            "https://mcp.kindly.software/oauth/authorize?response_type=code&client_id=kdb-web-client&redirect_uri={}&scope=openid%20email%20profile&state={}",
            "https%3A%2F%2Fmcp.kindly.software%2Foauth%2Fcallback",
            state
        );

        // Redirect to OAuth flow
        if let Some(window) = web_sys::window() {
            let _ = window.location().set_href(&oauth_url);
        }
    };

    view! {
        <style>
            ".signup-input:focus {
                border-color: rgba(255, 215, 0, 0.5) !important;
                box-shadow: 0 0 0 3px rgba(255, 215, 0, 0.1);
            }
            .signup-input::placeholder {
                color: rgba(255, 255, 255, 0.4);
            }
            .signup-btn:hover:not(:disabled) {
                transform: translateY(-2px);
                box-shadow: 0 12px 40px rgba(255, 215, 0, 0.4);
            }
            .signup-btn:active:not(:disabled) {
                transform: translateY(0);
            }
            @keyframes spin {
                to { transform: rotate(360deg); }
            }
            .loading-spinner {
                display: inline-block;
                width: 16px;
                height: 16px;
                border: 2px solid rgba(0, 0, 0, 0.3);
                border-top-color: #000;
                border-radius: 50%;
                animation: spin 0.8s linear infinite;
                margin-right: 8px;
                vertical-align: middle;
            }
            .google-signin-btn:hover {
                background: #f5f5f5 !important;
                border-color: rgba(255, 215, 0, 0.6) !important;
                transform: translateY(-2px);
            }
            .google-signin-btn:active {
                transform: translateY(0);
            }
            .google-signin-btn img {
                width: 20px;
                height: 20px;
            }"
        </style>

        <section id="signup" style=page_style>
            <div style=card_style>
                {move || match state.get() {
                    SignupState::Input | SignupState::Error | SignupState::Loading => view! {
                        <div style="text-align: center;">
                            <span style=promo_badge_style>"LAUNCH WEEK: Unlimited Sessions!"</span>
                        </div>
                        <h1 style=title_style>"Get Your Free License"</h1>
                        <p style=subtitle_style>"Start debugging with AI assistance in seconds"</p>

                        {move || if state.get() == SignupState::Error {
                            Some(view! { <p style=error_style>{error_msg.get()}</p> })
                        } else {
                            None
                        }}

                        <form on:submit=on_submit>
                            <input
                                type="email"
                                placeholder="Your email address"
                                style=input_style
                                class="signup-input"
                                prop:value=move || email.get()
                                on:input=move |ev| {
                                    let target = ev.target().unwrap();
                                    let input: web_sys::HtmlInputElement = target.dyn_into().unwrap();
                                    set_email.set(input.value());
                                }
                                required=true
                            />
                            <input
                                type="text"
                                placeholder="Organization name (optional)"
                                style=input_style
                                class="signup-input"
                                prop:value=move || org_name.get()
                                on:input=move |ev| {
                                    let target = ev.target().unwrap();
                                    let input: web_sys::HtmlInputElement = target.dyn_into().unwrap();
                                    set_org_name.set(input.value());
                                }
                            />
                            <button
                                type="submit"
                                style=move || if state.get() == SignupState::Loading {
                                    button_disabled_style
                                } else {
                                    button_style
                                }
                                class="signup-btn"
                                disabled=move || state.get() == SignupState::Loading
                            >
                                {move || if state.get() == SignupState::Loading {
                                    view! {
                                        <span class="loading-spinner"></span>
                                        <span>"Creating account..."</span>
                                    }.into_any()
                                } else {
                                    view! {
                                        <span>"Get Free License"</span>
                                    }.into_any()
                                }}
                            </button>
                        </form>

                        <p style=terms_style>
                            "By signing up, you agree to our "
                            <a href="#terms" style=link_style>"Terms"</a>
                            " and "
                            <a href="#privacy" style=link_style>"Privacy Policy"</a>
                        </p>

                        // "or" separator
                        <div style=separator_style>
                            <div style=separator_line_style></div>
                            <span style=separator_text_style>"or"</span>
                            <div style=separator_line_style></div>
                        </div>

                        // Google Sign-In button
                        <button
                            type="button"
                            style=google_button_style
                            class="google-signin-btn"
                            on:click=handle_google_signin
                        >
                            <img src="/google-icon.svg" alt="Google" />
                            <span>"Sign in with Google"</span>
                        </button>
                    }.into_any(),

                    SignupState::Success => view! {
                        <div style="text-align: center;">
                            <div style="font-size: 4rem; margin-bottom: 1rem;">"✉"</div>
                            <h1 style=title_style>"Check Your Inbox!"</h1>
                            <p style=subtitle_style>
                                "We've sent a verification link to your email. Click it to get your license key."
                            </p>
                            <p style="color: rgba(255,255,255,0.5); font-size: 0.875rem;">
                                "Didn't receive it? Check spam or "
                                <a
                                    href="#signup"
                                    style=link_style
                                    on:click=move |_| {
                                        set_state.set(SignupState::Input);
                                        set_email.set(String::new());
                                        set_org_name.set(String::new());
                                    }
                                >
                                    "try again"
                                </a>
                            </p>
                        </div>
                    }.into_any(),
                }}
            </div>
        </section>
    }
}

/// Submit signup form to API
///
/// POST /api/v1/signup with JSON body:
/// ```json
/// { "email": "...", "organization": "..." }
/// ```
async fn submit_signup(email: &str, organization: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("No window object")?;

    // Build JSON payload
    // Backend expects "org_name" not "organization"
    let payload = serde_json::json!({
        "email": email,
        "org_name": organization
    });

    // Create fetch options
    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&JsValue::from_str(&payload.to_string()));

    // Set headers
    let headers = web_sys::Headers::new().map_err(|_| "Failed to create headers")?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|_| "Failed to set Content-Type")?;
    opts.set_headers(&headers);

    // Create request - use absolute URL to api.kindly.software
    let request = web_sys::Request::new_with_str_and_init("https://api.kindly.software/api/v1/signup", &opts)
        .map_err(|_| "Failed to create request")?;

    // Execute fetch
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| "Network error - please check your connection")?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "Invalid response")?;

    // Check status
    if resp.ok() {
        Ok(())
    } else {
        // Try to extract error message from response
        let status = resp.status();
        match status {
            400 => Err("Invalid email address".to_string()),
            409 => Err("This email is already registered".to_string()),
            429 => Err("Too many requests - please try again later".to_string()),
            500..=599 => Err("Server error - please try again later".to_string()),
            _ => Err(format!("Request failed (status {})", status)),
        }
    }
}
