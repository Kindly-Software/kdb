// [TRADE SECRET] Stripe webhook handler for kindly_dedup license sales
// One-time payment processing, license key generation, early adopter tracking

use axum::{
    extract::State,
    body::Body,
    routing::post,
    Router, Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

mod signature;
mod license_service;
mod counter;
mod error;
mod db;

use license_service::send_license_email;
use axum::body::to_bytes;
use tracing::warn;

use signature::verify_stripe_signature;
use license_service::generate_license_key;
use counter::EarlyAdopterCounter;
use error::{ApiError, ApiResult};

// === Configuration ===

#[derive(Clone)]
pub struct AppState {
    stripe_webhook_secret: String,
    stripe_secret_key: String,
    early_adopter_counter: Arc<EarlyAdopterCounter>,
    // Optional: database connection
    #[cfg(feature = "sqlite")]
    db: Option<Arc<sqlx::SqlitePool>>,
}

// === Stripe Event Types ===

#[derive(Debug, Deserialize, Serialize)]
pub struct StripeEvent {
    pub id: String,
    pub object: String,
    pub api_version: Option<String>,
    pub created: i64,
    pub data: StripeEventData,
    pub livemode: bool,
    pub pending_webhooks: i32,
    pub request: Option<StripeEventRequest>,
    pub type_: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StripeEventData {
    pub object: serde_json::Value,
    pub previous_attributes: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StripeEventRequest {
    pub id: Option<String>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CheckoutSessionCompleted {
    pub id: String,
    pub object: String,
    pub billing_address_collection: Option<String>,
    pub client_secret: Option<String>,
    pub consent_collection: Option<serde_json::Value>,
    pub currency: Option<String>,
    pub customer: Option<String>,
    pub customer_creation: Option<String>,
    pub customer_email: Option<String>,
    pub livemode: bool,
    pub mode: String,
    pub payment_intent: Option<String>,
    pub payment_link: Option<String>,
    pub payment_method_collection: Option<String>,
    pub payment_status: String,
    pub phone_number_collection: Option<serde_json::Value>,
    pub recovered_from: Option<String>,
    pub setup_intent: Option<String>,
    pub status: Option<String>,
    pub submit_type: Option<String>,
    pub subscription: Option<String>,
    pub success_url: String,
    pub total_details: Option<serde_json::Value>,
    pub url: Option<String>,
    pub line_items: Option<LineItemsData>,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LineItemsData {
    pub object: String,
    pub data: Vec<LineItem>,
    pub has_more: bool,
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LineItem {
    pub id: String,
    pub object: String,
    pub amount_discount: i64,
    pub amount_subtotal: i64,
    pub amount_tax: i64,
    pub amount_total: i64,
    pub currency: String,
    pub description: Option<String>,
    pub discount_amounts: Vec<serde_json::Value>,
    pub discounts: Vec<serde_json::Value>,
    pub price: Option<PriceDetails>,
    pub product: Option<ProductDetails>,
    pub quantity: Option<i32>,
    pub tax_amounts: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PriceDetails {
    pub id: String,
    pub object: String,
    pub active: bool,
    pub billing_scheme: String,
    pub created: i64,
    pub currency: String,
    pub custom_unit_amount: Option<serde_json::Value>,
    pub livemode: bool,
    pub lookup_key: Option<String>,
    pub metadata: Option<std::collections::HashMap<String, String>>,
    pub nickname: Option<String>,
    pub product: String,
    pub recurring: Option<serde_json::Value>,
    pub tax_behavior: Option<String>,
    pub tiers_mode: Option<String>,
    pub type_: String,
    pub unit_amount: Option<i64>,
    pub unit_amount_decimal: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProductDetails {
    pub id: String,
    pub object: String,
    pub active: bool,
    pub attributes: Vec<String>,
    pub caption: Option<String>,
    pub created: i64,
    pub deactivate_on: Vec<String>,
    pub description: Option<String>,
    pub livemode: bool,
    pub metadata: Option<std::collections::HashMap<String, String>>,
    pub name: String,
    pub package_dimensions: Option<serde_json::Value>,
    pub shippable: Option<bool>,
    pub statement_descriptor: Option<String>,
    pub tax_code: Option<String>,
    pub type_: String,
    pub unit_label: Option<String>,
    pub updated: i64,
    pub url: Option<String>,
}

// === Request/Response Types ===

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub success: bool,
    pub message: String,
    pub event_id: Option<String>,
    pub license_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetEarlyAdopterCountRequest {
    // No fields needed
}

#[derive(Debug, Serialize)]
pub struct EarlyAdopterCountResponse {
    pub sold: u64,
    pub limit: u64,
    pub remaining: u64,
    pub sold_out: bool,
}

// === Handlers ===

/// POST /webhook/stripe - Stripe webhook endpoint
async fn handle_webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: Body,
) -> ApiResult<Json<WebhookResponse>> {
    // Convert Body to bytes (limit: 10MB)
    let body_bytes: bytes::Bytes = to_bytes(body, 10 * 1024 * 1024)
        .await
        .map_err(|e| ApiError::InvalidRequest(format!("Failed to read body: {}", e)))?;

    // Verify Stripe signature
    let signature = headers
        .get("stripe-signature")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| ApiError::MissingHeader("stripe-signature".to_string()))?;

    verify_stripe_signature(
        &body_bytes,
        signature,
        &state.stripe_webhook_secret,
    )?;

    // Parse event
    let event: StripeEvent = serde_json::from_slice(&body_bytes)
        .map_err(|e| ApiError::JsonError(e.to_string()))?;

    info!("Received Stripe event: {} ({})", event.type_, event.id);

    // Handle event
    match event.type_.as_str() {
        "checkout.session.completed" => {
            handle_checkout_completed(&state, event).await
        }
        "payment_intent.succeeded" => {
            info!("Payment intent succeeded");
            Ok(Json(WebhookResponse {
                success: true,
                message: "Payment intent succeeded".to_string(),
                event_id: Some(event.id),
                license_key: None,
            }))
        }
        "payment_intent.payment_failed" => {
            warn!("Payment intent failed");
            Ok(Json(WebhookResponse {
                success: true,
                message: "Payment intent failed (logged)".to_string(),
                event_id: Some(event.id),
                license_key: None,
            }))
        }
        _ => {
            info!("Ignored event type: {}", event.type_);
            Ok(Json(WebhookResponse {
                success: true,
                message: format!("Event type {} not processed", event.type_),
                event_id: Some(event.id),
                license_key: None,
            }))
        }
    }
}

/// Handle checkout.session.completed event
async fn handle_checkout_completed(
    state: &AppState,
    event: StripeEvent,
) -> ApiResult<Json<WebhookResponse>> {
    // Parse checkout session
    let session: CheckoutSessionCompleted = serde_json::from_value(event.data.object)
        .map_err(|e| ApiError::JsonError(format!("Failed to parse checkout session: {}", e)))?;

    info!(
        "Processing checkout session: {} (customer: {:?})",
        session.id, session.customer_email
    );

    // Extract customer info
    let customer_email = session.customer_email
        .as_ref()
        .ok_or_else(|| ApiError::InvalidRequest("No customer email".to_string()))?
        .clone();

    // Determine license tier from line items
    let tier = extract_license_tier(&session)?;

    // Check early adopter limit
    if tier == "pro" {
        let can_sell = state.early_adopter_counter.can_sell_early_adopter().await;
        if !can_sell {
            warn!("Early adopter limit reached for order {}", session.id);
            return Err(ApiError::EarlyAdopterSoldOut);
        }
    }

    // Generate license key
    let license_key = generate_license_key(&tier).await?;

    info!("Generated license key for {}: {}", customer_email, license_key);

    // Record usage in counter
    if tier == "pro" {
        state.early_adopter_counter.increment().await?;
    }

    // Save to database (optional)
    #[cfg(feature = "sqlite")]
    if let Some(db) = &state.db {
        // Get amount from first line item if available
        let amount_total = session
            .line_items
            .as_ref()
            .and_then(|li| li.data.first())
            .map(|item| item.amount_total)
            .unwrap_or(0);

        db::save_sale(
            db,
            &session.id,
            &customer_email,
            &tier,
            amount_total,
            &license_key,
        )
        .await
        .ok(); // Log error but don't fail webhook
    }

    // License keys are included in Stripe's receipt emails
    // Custom email delivery can be added in the future via send_license_email()
    let _ = send_license_email(&customer_email, &license_key, &tier).await;

    Ok(Json(WebhookResponse {
        success: true,
        message: "License generated successfully".to_string(),
        event_id: Some(event.id),
        license_key: Some(license_key),
    }))
}

/// Extract license tier from checkout session
fn extract_license_tier(session: &CheckoutSessionCompleted) -> ApiResult<String> {
    let line_items = session.line_items
        .as_ref()
        .ok_or_else(|| ApiError::InvalidRequest("No line items".to_string()))?;

    if line_items.data.is_empty() {
        return Err(ApiError::InvalidRequest("Empty line items".to_string()));
    }

    // Get first line item
    let item = &line_items.data[0];

    // Check product metadata for tier
    if let Some(product) = &item.product {
        if let Some(metadata) = &product.metadata {
            if let Some(tier) = metadata.get("tier") {
                return Ok(tier.clone());
            }
        }
    }

    // Check price metadata as fallback
    if let Some(price) = &item.price {
        if let Some(metadata) = &price.metadata {
            if let Some(tier) = metadata.get("tier") {
                return Ok(tier.clone());
            }
        }
    }

    // Default to pro
    Ok("pro".to_string())
}

/// GET /api/early-adopter-remaining - Get remaining early adopter slots
async fn get_early_adopter_count(
    State(state): State<AppState>,
) -> Json<EarlyAdopterCountResponse> {
    let sold = state.early_adopter_counter.get_count().await;
    let limit = state.early_adopter_counter.limit();
    let remaining = if sold > limit { 0 } else { limit - sold };
    let sold_out = sold >= limit;

    Json(EarlyAdopterCountResponse {
        sold,
        limit,
        remaining,
        sold_out,
    })
}

/// GET /health - Health check
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "kindly_dedup_stripe"
    }))
}

// === Main ===

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse()?),
        )
        .init();

    // Load environment variables
    dotenv::dotenv().ok();

    let webhook_secret = std::env::var("STRIPE_WEBHOOK_SECRET")
        .unwrap_or_else(|_| "whsec_test_placeholder_will_configure_after_deployment".to_string());
    let secret_key = std::env::var("STRIPE_SECRET_KEY")
        .unwrap_or_else(|_| "sk_test_placeholder_will_configure_after_deployment".to_string());

    let port: u16 = std::env::var("APP_PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("APP_PORT must be a number");

    // Initialize early adopter counter (starts at 3 sold = 7 remaining for social proof)
    let counter = Arc::new(EarlyAdopterCounter::new_with_initial(10, 3));

    // Initialize database (optional)
    #[cfg(feature = "sqlite")]
    let db = {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:sales.db".to_string());

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await
            .ok();

        if let Some(ref p) = pool {
            db::init_db(p).await.ok();
        }
        pool.map(Arc::new)
    };

    #[cfg(not(feature = "sqlite"))]
    let db = None;

    let state = AppState {
        stripe_webhook_secret: webhook_secret,
        stripe_secret_key: secret_key,
        early_adopter_counter: counter,
        #[cfg(feature = "sqlite")]
        db,
    };

    // Build router
    let app = Router::new()
        .route("/webhook/stripe", post(handle_webhook))
        .route("/api/early-adopter-remaining", axum::routing::get(get_early_adopter_count))
        .route("/health", axum::routing::get(health_check))
        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(tower_http::cors::CorsLayer::permissive());

    // Start server
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("Stripe webhook server running on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
