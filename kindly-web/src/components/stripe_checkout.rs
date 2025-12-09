// [TRADE SECRET] Stripe Checkout component (Leptos)
// Client-side checkout redirect handling

use leptos::*;
use crate::utils::stripe_api::create_checkout_session;

/// Stripe checkout button component
#[component]
pub fn CheckoutButton(
    price_id: String,
    tier_name: String,
    #[prop(optional)]
    disabled: Signal<bool>,
) -> impl IntoView {
    let (loading, set_loading) = create_signal(false);
    let (error, set_error) = create_signal::<Option<String>>(None);

    let on_click = move |_| {
        let price_id = price_id.clone();
        spawn_local(async move {
            set_loading(true);
            set_error(None);

            match create_checkout_session(&price_id).await {
                Ok(session_id) => {
                    // Redirect to Stripe Checkout
                    if let Some(stripe) = get_stripe_handle() {
                        match stripe.redirectToCheckout(&session_id).await {
                            Ok(_) => {
                                // Page will redirect
                            }
                            Err(e) => {
                                set_error(Some(format!("Checkout error: {}", e)));
                                set_loading(false);
                            }
                        }
                    } else {
                        set_error(Some("Stripe not initialized".to_string()));
                        set_loading(false);
                    }
                }
                Err(e) => {
                    set_error(Some(e));
                    set_loading(false);
                }
            }
        });
    };

    view! {
        <div class="checkout-wrapper">
            <button
                on:click=on_click
                disabled=move || loading() || disabled()
                class="checkout-button"
            >
                {move || if loading() {
                    view! { <span>"Processing..."</span> }
                } else {
                    view! { <span>"Purchase License"</span> }
                }}
            </button>

            {move || {
                error().map(|e| view! {
                    <div class="error-message">{e}</div>
                })
            }}
        </div>
    }
}

/// Get Stripe.js handle from window object
fn get_stripe_handle() -> Option<StripeHandle> {
    // In real implementation, this would use wasm-bindgen to access window.Stripe
    // For now, this is a type placeholder
    None
}

/// Stripe handle (placeholder - implement via wasm-bindgen)
struct StripeHandle;

impl StripeHandle {
    async fn redirectToCheckout(&self, _session_id: &str) -> Result<(), String> {
        // Actual implementation would use Stripe.js
        Err("Not implemented".to_string())
    }
}

// Styles
const CHECKOUT_STYLES: &str = r#"
.checkout-wrapper {
    width: 100%;
}

.checkout-button {
    width: 100%;
    padding: 1rem;
    background: linear-gradient(135deg, #4B0082 0%, #6d28d9 100%);
    color: white;
    border: none;
    border-radius: 8px;
    font-size: 1rem;
    font-weight: 600;
    cursor: pointer;
    transition: transform 0.2s, box-shadow 0.2s;
}

.checkout-button:hover:not(:disabled) {
    transform: scale(1.02);
    box-shadow: 0 4px 12px rgba(75, 0, 130, 0.3);
}

.checkout-button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
}

.error-message {
    margin-top: 1rem;
    padding: 1rem;
    background-color: #fee2e2;
    color: #b91c1c;
    border-radius: 8px;
    font-size: 0.875rem;
}
"#;
