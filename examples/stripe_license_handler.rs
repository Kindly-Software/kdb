// Example: Stripe License Payment Handler for kindly_dedup
// This example shows how to integrate Stripe MCP with license management
//
// Use Case: License sales website for kindly_dedup
// Features: Product creation, checkout sessions, webhook handling, license creation

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH, Duration};

// ============================================================================
// Domain Models
// ============================================================================

/// License tier for kindly_dedup
#[derive(Debug, Clone, PartialEq)]
pub enum LicenseTier {
    Basic,      // 100K docs/month
    Pro,        // 1M docs/month
    Enterprise, // Unlimited, custom quote
}

impl LicenseTier {
    pub fn description(&self) -> &str {
        match self {
            Self::Basic => "Basic - 100,000 docs/month",
            Self::Pro => "Pro - 1,000,000 docs/month",
            Self::Enterprise => "Enterprise - Unlimited",
        }
    }

    pub fn price_usd(&self) -> u32 {
        match self {
            Self::Basic => 99,
            Self::Pro => 299,
            Self::Enterprise => 9999, // Custom quote
        }
    }

    pub fn stripe_product_id(&self) -> &str {
        match self {
            Self::Basic => "prod_kindly_dedup_basic",
            Self::Pro => "prod_kindly_dedup_pro",
            Self::Enterprise => "prod_kindly_dedup_enterprise",
        }
    }
}

/// License record in database
#[derive(Debug, Clone)]
pub struct License {
    pub id: String,
    pub customer_id: String,
    pub customer_email: String,
    pub tier: LicenseTier,
    pub payment_intent_id: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub is_active: bool,
}

impl License {
    /// Create new license from payment
    pub fn from_payment(
        customer_id: String,
        customer_email: String,
        tier: LicenseTier,
        payment_intent_id: String,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Licenses valid for 1 year (365 days)
        let expires_at = now + (365 * 24 * 60 * 60);

        Self {
            id: format!("lic_{}", uuid::Uuid::new_v4().to_string()),
            customer_id,
            customer_email,
            tier,
            payment_intent_id,
            issued_at: now,
            expires_at,
            is_active: true,
        }
    }

    /// Check if license is currently valid
    pub fn is_valid(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.is_active && now < self.expires_at
    }

    /// Get remaining time in days
    pub fn days_remaining(&self) -> Option<u64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if now < self.expires_at {
            Some((self.expires_at - now) / (24 * 60 * 60))
        } else {
            None
        }
    }
}

// ============================================================================
// Stripe Integration Layer
// ============================================================================

/// Stripe API client wrapper
pub struct StripeClient {
    api_key: String,
    base_url: String,
}

impl StripeClient {
    /// Create new Stripe client
    pub fn new(api_key: String) -> Self {
        let base_url = if api_key.contains("sk_test") {
            "https://api.stripe.com/v1".to_string()
        } else {
            "https://api.stripe.com/v1".to_string()
        };

        Self { api_key, base_url }
    }

    /// Create a checkout session for license purchase
    ///
    /// Example: kindly_dedup Basic tier license
    pub async fn create_checkout_session(
        &self,
        tier: &LicenseTier,
        customer_email: &str,
    ) -> Result<CheckoutSession, String> {
        // In real implementation, use stripe-rs or HTTP client
        // This is pseudocode showing the flow

        let payload = serde_json::json!({
            "mode": "payment",
            "customer_email": customer_email,
            "line_items": [{
                "price_data": {
                    "currency": "usd",
                    "product_data": {
                        "name": format!("kindly_dedup {}", tier.description()),
                        "description": "LLM dataset deduplication license",
                    },
                    "unit_amount": tier.price_usd() * 100, // Stripe uses cents
                },
                "quantity": 1,
            }],
            "success_url": "https://yoursite.com/success?session_id={CHECKOUT_SESSION_ID}",
            "cancel_url": "https://yoursite.com/cancel",
        });

        // Make HTTP request to Stripe API
        // POST /v1/checkout/sessions
        // Authorization: Bearer sk_test_...

        Ok(CheckoutSession {
            id: "cs_test_abc123".to_string(),
            url: "https://checkout.stripe.com/pay/cs_test_abc123".to_string(),
            customer_email: customer_email.to_string(),
            tier: tier.clone(),
        })
    }

    /// Verify webhook signature (security critical)
    pub fn verify_webhook_signature(
        signature: &str,
        body: &str,
        webhook_secret: &str,
    ) -> Result<(), String> {
        // Use stripe::webhook::Webhook::verify()
        // In production: stripe-rs crate handles this
        //
        // Pseudocode:
        // let event = stripe::webhook::construct_event(body, signature, webhook_secret)?;
        // Ok(())

        Ok(())
    }

    /// Handle payment success webhook
    pub fn handle_payment_success(&self, payment_intent_id: &str) -> Result<PaymentData, String> {
        // In real implementation:
        // 1. Fetch payment intent from Stripe API
        // 2. Validate customer email
        // 3. Determine license tier from metadata
        // 4. Return payment data

        Ok(PaymentData {
            payment_intent_id: payment_intent_id.to_string(),
            customer_email: "user@example.com".to_string(),
            tier: LicenseTier::Basic,
            amount_usd: 99,
            status: "succeeded".to_string(),
        })
    }
}

/// Checkout session response
#[derive(Debug, Clone)]
pub struct CheckoutSession {
    pub id: String,
    pub url: String,
    pub customer_email: String,
    pub tier: LicenseTier,
}

/// Payment data from webhook
#[derive(Debug, Clone)]
pub struct PaymentData {
    pub payment_intent_id: String,
    pub customer_email: String,
    pub tier: LicenseTier,
    pub amount_usd: u32,
    pub status: String,
}

// ============================================================================
// License Database (Mock)
// ============================================================================

/// In-memory license database (use real database in production)
pub struct LicenseDatabase {
    licenses: HashMap<String, License>,
}

impl LicenseDatabase {
    pub fn new() -> Self {
        Self {
            licenses: HashMap::new(),
        }
    }

    /// Store new license
    pub fn create(&mut self, license: License) -> Result<String, String> {
        let id = license.id.clone();
        self.licenses.insert(id.clone(), license);
        Ok(id)
    }

    /// Retrieve license by ID
    pub fn get(&self, license_id: &str) -> Option<License> {
        self.licenses.get(license_id).cloned()
    }

    /// Retrieve license by email
    pub fn get_by_email(&self, email: &str) -> Vec<License> {
        self.licenses
            .values()
            .filter(|l| l.customer_email == email)
            .cloned()
            .collect()
    }

    /// Retrieve license by payment intent
    pub fn get_by_payment_intent(&self, payment_intent_id: &str) -> Option<License> {
        self.licenses
            .values()
            .find(|l| l.payment_intent_id == payment_intent_id)
            .cloned()
    }

    /// Update license
    pub fn update(&mut self, license: License) -> Result<(), String> {
        self.licenses
            .insert(license.id.clone(), license)
            .ok_or_else(|| "License not found".to_string())?;
        Ok(())
    }

    /// List all active licenses
    pub fn list_active(&self) -> Vec<License> {
        self.licenses
            .values()
            .filter(|l| l.is_valid())
            .cloned()
            .collect()
    }
}

// ============================================================================
// Webhook Handler
// ============================================================================

/// Handle Stripe webhook events
pub struct WebhookHandler {
    db: LicenseDatabase,
    stripe: StripeClient,
}

impl WebhookHandler {
    pub fn new(db: LicenseDatabase, stripe: StripeClient) -> Self {
        Self { db, stripe }
    }

    /// Process webhook event
    pub async fn handle_event(&mut self, event_type: &str, data: &str) -> Result<(), String> {
        match event_type {
            "payment_intent.succeeded" => self.handle_payment_succeeded(data).await,
            "payment_intent.payment_failed" => self.handle_payment_failed(data).await,
            "customer.subscription.updated" => self.handle_subscription_updated(data).await,
            "invoice.payment_failed" => self.handle_invoice_payment_failed(data).await,
            _ => {
                eprintln!("Unknown event type: {}", event_type);
                Ok(())
            }
        }
    }

    /// Create license after successful payment
    async fn handle_payment_succeeded(&mut self, data: &str) -> Result<(), String> {
        // Parse webhook data to get payment_intent_id
        let payment_intent_id = "pi_test_abc123"; // Extract from data

        // Get payment details from Stripe
        let payment_data = self.stripe.handle_payment_success(payment_intent_id)?;

        // Create license
        let license = License::from_payment(
            format!("cus_{}", uuid::Uuid::new_v4()),
            payment_data.customer_email.clone(),
            payment_data.tier.clone(),
            payment_data.payment_intent_id.clone(),
        );

        // Store in database
        let license_id = self.db.create(license.clone())?;

        // Log success
        println!(
            "✅ License created: {} for {} ({})",
            license_id, payment_data.customer_email, payment_data.tier.description()
        );

        // TODO: Send activation email to customer
        // TODO: Update customer record with license ID

        Ok(())
    }

    /// Handle failed payment
    async fn handle_payment_failed(&mut self, data: &str) -> Result<(), String> {
        println!("❌ Payment failed: {}", data);
        // TODO: Send failure notification email
        Ok(())
    }

    /// Handle subscription updates (for renewal)
    async fn handle_subscription_updated(&mut self, data: &str) -> Result<(), String> {
        println!("📝 Subscription updated: {}", data);
        // TODO: Update license expiry date
        Ok(())
    }

    /// Handle invoice payment failure
    async fn handle_invoice_payment_failed(&mut self, data: &str) -> Result<(), String> {
        println!("⚠️  Invoice payment failed: {}", data);
        // TODO: Notify customer of payment issue
        Ok(())
    }
}

// ============================================================================
// Example Usage
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Stripe License Handler Example ===\n");

    // 1. Initialize Stripe client
    let stripe = StripeClient::new("sk_test_YOUR_API_KEY".to_string());

    // 2. Initialize license database
    let db = LicenseDatabase::new();

    // 3. Create checkout session for Basic tier
    println!("📋 Creating checkout session for kindly_dedup Basic tier...");
    let session = stripe
        .create_checkout_session(&LicenseTier::Basic, "customer@example.com")
        .await?;

    println!("Checkout Session Created:");
    println!("  ID: {}", session.id);
    println!("  URL: {}", session.url);
    println!("  Customer: {}", session.customer_email);
    println!("  Tier: {}\n", session.tier.description());

    // 4. Simulate webhook: payment_intent.succeeded
    println!("🔔 Simulating webhook: payment_intent.succeeded");
    let mut handler = WebhookHandler::new(db, stripe);
    handler
        .handle_event("payment_intent.succeeded", "payment_intent_data")
        .await?;

    // 5. Verify license
    println!("\n✅ Example complete!");
    println!("In production, this would:");
    println!("  1. Create Stripe products for each tier");
    println!("  2. Generate checkout links for customers");
    println!("  3. Receive and verify webhook signatures");
    println!("  4. Create license records after payment");
    println!("  5. Send activation emails to customers");

    Ok(())
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_license_creation() {
        let license = License::from_payment(
            "cus_test123".to_string(),
            "test@example.com".to_string(),
            LicenseTier::Basic,
            "pi_test123".to_string(),
        );

        assert_eq!(license.customer_email, "test@example.com");
        assert_eq!(license.tier, LicenseTier::Basic);
        assert!(license.is_valid());
        assert!(license.days_remaining().unwrap() > 360);
    }

    #[test]
    fn test_license_tier_pricing() {
        assert_eq!(LicenseTier::Basic.price_usd(), 99);
        assert_eq!(LicenseTier::Pro.price_usd(), 299);
        assert_eq!(LicenseTier::Enterprise.price_usd(), 9999);
    }

    #[test]
    fn test_license_database() {
        let mut db = LicenseDatabase::new();

        let license = License::from_payment(
            "cus_test123".to_string(),
            "test@example.com".to_string(),
            LicenseTier::Pro,
            "pi_test123".to_string(),
        );

        let id = db.create(license.clone()).unwrap();
        assert_eq!(db.get(&id).unwrap().tier, LicenseTier::Pro);
        assert_eq!(db.get_by_email("test@example.com").len(), 1);
    }

    #[test]
    fn test_license_expiry() {
        let license = License::from_payment(
            "cus_test123".to_string(),
            "test@example.com".to_string(),
            LicenseTier::Basic,
            "pi_test123".to_string(),
        );

        // Fresh license should be valid
        assert!(license.is_valid());

        // Simulate expiry by setting expiration to past
        let mut expired_license = license;
        expired_license.expires_at = 0;
        assert!(!expired_license.is_valid());
    }
}

/*
=============================================================================
INTEGRATION CHECKLIST FOR STRIPE MCP
=============================================================================

Phase 1: Setup
✓ Install Stripe MCP (remote or local)
✓ Get API key from Stripe Dashboard
✓ Store key securely (.env or keyring)
✓ Configure ~/.claude/settings.json

Phase 2: Products
- Use Stripe MCP to create products for each tier:
  * kindly_dedup Basic ($99/month)
  * kindly_dedup Pro ($299/month)
  * kindly_dedup Enterprise (custom)
- Set product metadata:
  * max_documents_per_month
  * support_tier
  * renewal_period_days

Phase 3: Checkout
- Create checkout sessions via Stripe MCP
- Generate checkout URLs for frontend
- Track session IDs in database
- Set success/cancel URLs

Phase 4: Webhooks
- Register webhook endpoint:
  POST https://yoursite.com/webhooks/stripe
- Subscribe to events:
  * payment_intent.succeeded
  * payment_intent.payment_failed
  * customer.subscription.updated
  * invoice.payment_failed
- Implement signature verification (security-critical)

Phase 5: License Management
- Store license records in database
- Track customer associations
- Implement license validation
- Handle renewals and cancellations

Phase 6: Go Live
- Switch to live API keys (sk_live_...)
- Enable email notifications
- Set up monitoring and alerts
- Test payment flow end-to-end

=============================================================================
STRIPE MCP TOOLS FOR LICENSE SALES
=============================================================================

1. create_product (create products for each tier)
   Example: "Create product 'kindly_dedup Basic' for $99"

2. create_price (set pricing)
   Example: "Set price to $99/month for kindly_dedup Basic"

3. create_checkout_session (generate checkout links)
   Example: "Create checkout session for user@example.com"

4. create_webhook_endpoint (register webhooks)
   Example: "Register webhook at https://mysite.com/stripe-webhooks"

5. create_customer (track customers)
   Example: "Create customer for john@example.com"

6. create_payment_intent (manual payments)
   Example: "Create $99 payment intent for john@example.com"

=============================================================================
*/
