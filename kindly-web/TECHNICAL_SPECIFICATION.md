# Stripe Payment Integration - Technical Specification

## Overview

This document provides the detailed technical specification for integrating Stripe payment processing into the kindly.software website.

---

## 1. Frontend Specification

### 1.1 Technology Stack

| Component | Version | Purpose |
|-----------|---------|---------|
| Leptos | 0.7 | Frontend framework |
| Rust | 2021 | Language |
| WASM | wasm32-unknown-unknown | Target |
| Stripe.js | v3 | Payment processing |

### 1.2 New Routes

```
Route: /checkout/:tier
├── Component: CheckoutPage
├── Params: {tier: String}
└── Renders: CheckoutForm component

Route: /success
├── Component: SuccessPage
├── Query Params: ?session_id=...
└── Shows: License key, confirmation

Route: /cancel
├── Component: CancelPage
└── Redirects: Back to pricing
```

### 1.3 New Components

#### CheckoutPage (`src/pages/checkout.rs`)
```rust
#[component]
pub fn CheckoutPage() -> impl IntoView {
    // Parse URL params: /checkout/:tier
    // Display checkout form for selected tier
    // Handle form submission
}
```

**Props**:
- `tier`: String (starter, pro, enterprise)

**State**:
- Loading: bool (while processing payment)
- Error: Option<String> (error message)

**Actions**:
- On form submit: Send to Stripe Checkout
- On success: Redirect to /success
- On error: Display error message

#### SuccessPage (`src/pages/success.rs`)
```rust
#[component]
pub fn SuccessPage() -> impl IntoView {
    // Get session_id from query params
    // Fetch order details from backend
    // Display confirmation and license key
}
```

**Props**:
- Query param: `session_id` (from Stripe)

**State**:
- Loading: bool
- Order: Option<OrderDetails>
- Error: Option<String>

**Display**:
- Order confirmation
- License key
- Download link
- Support contact info

#### CheckoutForm (`src/components/sections/checkout_form.rs`)
```rust
#[component]
pub fn CheckoutForm(
    tier: String,
    price: u32,  // in cents: 50000 = $500
) -> impl IntoView {
    // Email input field
    // Stripe Elements card form
    // Submit button
    // Error handling
}
```

**Props**:
- `tier`: String
- `price`: u32 (in cents)

**Form Fields**:
- Email: text input
- Card: Stripe Element
- Name: text input

**Actions**:
- On mount: Initialize Stripe
- On submit: Create payment intent, process payment

### 1.4 Updated Components

#### PricingCard (`src/components/molecular/pricing_card.rs`)

**Add Props**:
```rust
#[prop(optional)] 
cta_text: Option<&'static str>,

#[prop(optional)] 
cta_link: Option<&'static str>,

#[prop(optional)] 
cta_onclick: Option<Box<dyn Fn() + 'static>>,
```

**Render CTA Button**:
```rust
{cta_text.map(|text| {
    view! {
        <a
            href={cta_link.unwrap_or("/checkout")}
            class="cta-button"
            style="..."
        >
            {text}
        </a>
    }
})}
```

#### Pricing Section (`src/components/sections/pricing.rs`)

**Update Card Definitions**:
```rust
// Old: Free tier
// New: Starter tier ($500)
<PricingCard
    tier="Starter"
    price="$500"
    period="one-time license"
    features=vec![...]
    cta_text=Some("Purchase License")
    cta_link=Some("/checkout/starter")
/>

// Old: Pay As You Go
// New: Pro tier ($1,500)
<PricingCard
    tier="Pro"
    price="$1,500"
    period="one-time license"
    featured=true
    features=vec![...]
    cta_text=Some("Purchase License")
    cta_link=Some("/checkout/pro")
/>

// Keep: Enterprise tier
<PricingCard
    tier="Enterprise"
    price="Custom"
    period="contact sales"
    features=vec![...]
    cta_text=Some("Request Quote")
    cta_link=Some("mailto:sales@kindly.software")
/>
```

### 1.5 JavaScript Interop

#### Stripe Initialization (in CheckoutForm)

```javascript
// Load Stripe library
const stripe = Stripe(stripePubKey);

// Create Elements instance
const elements = stripe.elements();

// Create card element
const cardElement = elements.create('card');
cardElement.mount('#card-element');

// Handle card errors
cardElement.addEventListener('change', (event) => {
    if (event.error) {
        showError(event.error.message);
    } else {
        clearError();
    }
});
```

#### Payment Submission

```javascript
// On form submit
form.addEventListener('submit', async (e) => {
    e.preventDefault();
    
    // Validate email
    if (!email.value) {
        showError('Email required');
        return;
    }
    
    // Create token
    const {token, error} = await stripe.createToken(cardElement, {
        name: name.value,
        address_zip: zip.value,
    });
    
    if (error) {
        showError(error.message);
        return;
    }
    
    // Send to backend API
    const response = await fetch('/api/create-checkout-session', {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify({
            token: token.id,
            tier: tier,
            email: email.value,
            name: name.value,
        })
    });
    
    const data = await response.json();
    
    if (data.error) {
        showError(data.error);
        return;
    }
    
    // Redirect to Stripe Checkout
    window.location.href = data.checkout_url;
});
```

### 1.6 HTML Template Updates (`index.html`)

**Add Stripe SDK**:
```html
<head>
    <!-- Existing styles -->
    
    <!-- Add Stripe.js -->
    <script src="https://js.stripe.com/v3/"></script>
</head>
```

### 1.7 Routing Update (`src/lib.rs`)

```rust
use pages::checkout::CheckoutPage;
use pages::success::SuccessPage;
use pages::cancel::CancelPage;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <Navbar />
            <Routes fallback=|| "Page not found.">
                <Route path=path!("/") view=HomePage />
                <Route path=path!("/checkout/:tier") view=CheckoutPage />
                <Route path=path!("/success") view=SuccessPage />
                <Route path=path!("/cancel") view=CancelPage />
            </Routes>
        </Router>
    }
}
```

---

## 2. Backend Specification

### 2.1 Technology Stack

| Component | Purpose |
|-----------|---------|
| Node.js | Runtime |
| Express.js | Web framework |
| Stripe SDK | Payment processing |
| dotenv | Environment variables |

### 2.2 Project Structure

```
kindly-dedup-webhook/
├── server.js              # Main app
├── package.json           # Dependencies
├── .env.example           # Environment template
├── .env                   # (gitignored) Actual secrets
├── Dockerfile             # Container config
├── fly.toml              # Fly.io config
└── src/
    ├── routes/
    │   ├── checkout.js   # POST /api/checkout
    │   └── webhook.js    # POST /webhooks/stripe
    └── utils/
        ├── stripe.js     # Stripe helpers
        ├── license.js    # License generation
        └── email.js      # Email sending
```

### 2.3 API Endpoints

#### POST /api/checkout

**Purpose**: Create Stripe Checkout session

**Request**:
```json
{
    "token": "tok_...",
    "tier": "starter",
    "email": "customer@example.com",
    "name": "Customer Name"
}
```

**Response Success**:
```json
{
    "checkout_url": "https://checkout.stripe.com/pay/..."
}
```

**Response Error**:
```json
{
    "error": "Email is required"
}
```

**Implementation**:
```javascript
app.post('/api/checkout', async (req, res) => {
    const {token, tier, email, name} = req.body;
    
    // Validate inputs
    if (!email || !tier || !token) {
        return res.status(400).json({error: 'Missing fields'});
    }
    
    try {
        // Create Stripe Checkout session
        const session = await stripe.checkout.sessions.create({
            payment_method_types: ['card'],
            line_items: [
                {
                    price_data: {
                        currency: 'usd',
                        product_data: {
                            name: `kindly_dedup ${tier} License`,
                            description: getPricingInfo(tier),
                        },
                        unit_amount: getPriceInCents(tier),
                    },
                    quantity: 1,
                }
            ],
            mode: 'payment',
            success_url: 'https://kindly.software/success?session_id={CHECKOUT_SESSION_ID}',
            cancel_url: 'https://kindly.software/cancel',
            customer_email: email,
            metadata: {
                tier,
                email,
                name,
                customer_ip: req.ip,
            },
        });
        
        res.json({checkout_url: session.url});
    } catch (error) {
        console.error('Checkout error:', error);
        res.status(500).json({error: error.message});
    }
});
```

#### POST /webhooks/stripe

**Purpose**: Handle Stripe webhook events

**Expected Events**:
- `payment_intent.succeeded`
- `payment_intent.payment_failed`
- `charge.refunded`

**Implementation**:
```javascript
app.post('/webhooks/stripe', 
    express.raw({type: 'application/json'}),
    async (req, res) => {
        const sig = req.headers['stripe-signature'];
        
        let event;
        try {
            event = stripe.webhooks.constructEvent(
                req.body,
                sig,
                process.env.STRIPE_WEBHOOK_SECRET
            );
        } catch (err) {
            console.error('Webhook signature verification failed:', err);
            return res.status(400).send(`Webhook Error: ${err.message}`);
        }
        
        try {
            switch (event.type) {
                case 'payment_intent.succeeded':
                    await handlePaymentSucceeded(event.data.object);
                    break;
                    
                case 'payment_intent.payment_failed':
                    await handlePaymentFailed(event.data.object);
                    break;
                    
                case 'charge.refunded':
                    await handleRefund(event.data.object);
                    break;
                    
                default:
                    console.log(`Unhandled event type: ${event.type}`);
            }
        } catch (error) {
            console.error('Webhook handler error:', error);
            // Still return 200 to prevent retry
        }
        
        res.json({received: true});
    }
);
```

### 2.4 Webhook Handlers

#### handlePaymentSucceeded

```javascript
async function handlePaymentSucceeded(paymentIntent) {
    const {id, metadata, amount, currency} = paymentIntent;
    
    console.log(`Payment succeeded: ${id}`);
    
    // Generate license key
    const license = generateLicenseKey(metadata.tier);
    
    // Store transaction (optional)
    // await db.orders.create({
    //     stripe_id: id,
    //     tier: metadata.tier,
    //     email: metadata.email,
    //     amount: amount,
    //     currency: currency,
    //     license: license,
    //     created_at: new Date(),
    // });
    
    // Send confirmation email
    await sendConfirmationEmail(
        metadata.email,
        metadata.name,
        metadata.tier,
        license
    );
    
    // Log to console/monitoring
    console.log({
        event: 'payment_succeeded',
        stripe_id: id,
        tier: metadata.tier,
        license: license,
        email: metadata.email,
    });
}
```

#### handlePaymentFailed

```javascript
async function handlePaymentFailed(paymentIntent) {
    const {id, metadata, last_payment_error} = paymentIntent;
    
    console.error(`Payment failed: ${id}`);
    
    // Send failure email
    await sendFailureEmail(
        metadata.email,
        metadata.name,
        last_payment_error?.message
    );
    
    // Log failure for monitoring
    console.log({
        event: 'payment_failed',
        stripe_id: id,
        email: metadata.email,
        error: last_payment_error?.message,
    });
}
```

### 2.5 Helper Functions

#### generateLicenseKey

```javascript
function generateLicenseKey(tier) {
    const timestamp = Date.now();
    const random = Math.random().toString(36).substr(2, 9).toUpperCase();
    const checksum = (timestamp % 10000).toString().padStart(4, '0');
    
    return `KD-${tier.toUpperCase()}-${checksum}-${random}`;
    // Example: KD-STARTER-5847-ABC123DEF
}
```

**Format**: `KD-{TIER}-{CHECKSUM}-{RANDOM}`
- KD = kindly_dedup
- TIER = starter, pro, enterprise
- CHECKSUM = Last 4 digits of timestamp (for validation)
- RANDOM = Random 9-char alphanumeric

**Validation Function**:
```javascript
function validateLicense(license) {
    const parts = license.split('-');
    if (parts.length !== 4) return false;
    if (parts[0] !== 'KD') return false;
    if (!['STARTER', 'PRO', 'ENTERPRISE'].includes(parts[1])) return false;
    if (!/^\d{4}$/.test(parts[2])) return false;
    if (!/^[A-Z0-9]{9}$/.test(parts[3])) return false;
    return true;
}
```

#### sendConfirmationEmail

```javascript
async function sendConfirmationEmail(email, name, tier, license) {
    // Implement with SendGrid, AWS SES, or similar
    
    const subject = 'Your kindly_dedup License Key';
    const htmlContent = `
        <h2>Thank you for your purchase!</h2>
        <p>Hi ${name},</p>
        <p>Your kindly_dedup ${tier} license key:</p>
        <p style="background: #f0f0f0; padding: 10px; font-family: monospace;">
            ${license}
        </p>
        <p>
            To activate:
            <ol>
                <li>Download kindly_dedup from GitHub</li>
                <li>Add your license key to the config</li>
                <li>Start deduplicating!
            </ol>
        </p>
        <p>Support: support@kindly.software</p>
    `;
    
    // Send email (implementation depends on provider)
    // Example with SendGrid:
    // await sendgrid.send({to: email, subject, htmlContent});
    
    console.log(`Email sent to ${email} with license ${license}`);
}
```

### 2.6 Environment Variables

```env
# Stripe API keys
STRIPE_SECRET_KEY=sk_live_...
STRIPE_WEBHOOK_SECRET=whsec_...
STRIPE_PUBLIC_KEY=pk_live_...  # For reference only

# Email configuration (optional)
SENDGRID_API_KEY=SG....
SENDGRID_FROM_EMAIL=noreply@kindly.software

# Server configuration
NODE_ENV=production
PORT=3000
LOG_LEVEL=info
```

### 2.7 Error Handling

**Rate Limiting** (optional):
```javascript
const rateLimit = require('express-rate-limit');

const limiter = rateLimit({
    windowMs: 15 * 60 * 1000, // 15 minutes
    max: 100 // limit each IP to 100 requests per windowMs
});

app.use('/api/', limiter);
```

**Error Logging**:
```javascript
app.use((error, req, res, next) => {
    console.error({
        error: error.message,
        stack: error.stack,
        path: req.path,
        method: req.method,
        timestamp: new Date(),
    });
    
    res.status(500).json({
        error: 'Internal server error'
    });
});
```

**Health Check**:
```javascript
app.get('/health', (req, res) => {
    res.json({status: 'ok'});
});
```

### 2.8 Database Schema (Optional)

If using PostgreSQL to store orders:

```sql
CREATE TABLE orders (
    id SERIAL PRIMARY KEY,
    stripe_id VARCHAR(100) UNIQUE NOT NULL,
    tier VARCHAR(50) NOT NULL,
    email VARCHAR(255) NOT NULL,
    customer_name VARCHAR(255),
    amount_cents INTEGER NOT NULL,
    currency VARCHAR(3) DEFAULT 'USD',
    license_key VARCHAR(50) UNIQUE NOT NULL,
    status VARCHAR(50) DEFAULT 'completed',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    metadata JSON
);

CREATE INDEX idx_stripe_id ON orders(stripe_id);
CREATE INDEX idx_license_key ON orders(license_key);
CREATE INDEX idx_email ON orders(email);
```

---

## 3. Deployment Specification

### 3.1 Frontend (Fly.io)

**File**: `fly.toml`

```toml
app = "kindly-software-website"
primary_region = "ord"

[build]
  dockerfile = "Dockerfile"

[env]
  STRIPE_PUBLIC_KEY = "pk_live_..."

[http_service]
  internal_port = 8080
  force_https = true
```

**Build & Deploy**:
```bash
cd kindly-web

# Build
trunk build --release

# Deploy
fly deploy
```

### 3.2 Backend (Fly.io)

**File**: `kindly-dedup-webhook/fly.toml`

```toml
app = "kindly-dedup-webhook"
primary_region = "ord"

[build]
  dockerfile = "Dockerfile"

[[services]]
  protocol = "tcp"
  internal_port = 3000

[env]
  NODE_ENV = "production"
  PORT = "3000"

[env.secrets]
  STRIPE_SECRET_KEY = "sk_live_..."
  STRIPE_WEBHOOK_SECRET = "whsec_..."
  SENDGRID_API_KEY = "SG...."
```

**Dockerfile**:
```dockerfile
FROM node:18-alpine

WORKDIR /app

COPY package*.json ./

RUN npm ci --only=production

COPY . .

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD node -e "require('http').get('http://localhost:3000/health', (r) => {if (r.statusCode !== 200) throw new Error(r.statusCode)})"

CMD ["node", "server.js"]
```

**Secrets Configuration**:
```bash
fly secrets set STRIPE_SECRET_KEY=sk_live_... -a kindly-dedup-webhook
fly secrets set STRIPE_WEBHOOK_SECRET=whsec_... -a kindly-dedup-webhook
fly secrets set SENDGRID_API_KEY=SG.... -a kindly-dedup-webhook
```

### 3.3 Deployment Process

1. **Frontend**:
   ```bash
   cd kindly-web
   trunk build --release
   fly deploy
   ```

2. **Backend**:
   ```bash
   cd kindly-dedup-webhook
   fly deploy
   ```

3. **Stripe Configuration**:
   - Create webhook endpoint: `https://kindly-dedup-webhook.fly.dev/webhooks/stripe`
   - Copy webhook signing secret
   - Add to Fly.io secrets

---

## 4. Testing Specification

### 4.1 Unit Tests

**Frontend**:
```bash
cd kindly-web
cargo test
```

**Backend**:
```bash
cd kindly-dedup-webhook
npm test
```

### 4.2 Integration Tests

**Stripe CLI Testing**:
```bash
# Install Stripe CLI
brew install stripe/stripe-cli/stripe

# Forward webhook events
stripe listen --forward-to localhost:3000/webhooks/stripe

# Trigger payment event
stripe trigger payment_intent.succeeded

# Check logs
stripe logs tail
```

### 4.3 Manual Testing

**Payment Flow**:
1. Visit kindly.software
2. Navigate to Pricing section
3. Click "Purchase License" on Starter tier
4. Fill checkout form with test email
5. Use test card: 4242 4242 4242 4242
6. Submit payment
7. Verify webhook triggers
8. Check license key in database

**Test Cards**:
- Success: `4242 4242 4242 4242`
- Decline: `4000 0000 0000 0002`
- CVC error: `4000 0000 0000 0127`

### 4.4 Monitoring

**Logs**:
```bash
# Frontend
fly logs -a kindly-software-website

# Backend
fly logs -a kindly-dedup-webhook
```

**Metrics** (in Fly.io Dashboard):
- Request rate
- Response time
- Error rate
- Memory usage
- CPU usage

---

## 5. Security Specification

### 5.1 API Security

- ✅ HTTPS only (Fly.io enforces)
- ✅ No sensitive data in logs
- ✅ Webhook signature verification (Stripe)
- ✅ CORS headers configured
- ✅ Rate limiting (optional)

### 5.2 Secrets Management

- ✅ Stripe keys in Fly.io secrets (never in git)
- ✅ Environment-specific keys (test vs live)
- ✅ No keys in frontend (JavaScript)
- ✅ Webhook secret verified

### 5.3 Data Security

- ✅ Email addresses encrypted in transit (HTTPS)
- ✅ No password storage (one-click purchase)
- ✅ License keys generated with random component
- ✅ PCI DSS compliance (Stripe handles)

---

## 6. Performance Specification

### 6.1 Frontend Performance

- Load time: <500ms
- Checkout page: <1s
- Stripe form render: <2s
- Bundle size: +0KB (Stripe.js is separate)

### 6.2 Backend Performance

- Checkout API: <500ms response
- Webhook processing: <1s per event
- License generation: <100ms
- Email sending: <5s (async)

### 6.3 Database Performance (if used)

- Insert order: <100ms
- Query by email: <50ms
- Query by license: <50ms

---

## 7. Rollback Specification

### 7.1 Frontend Rollback

```bash
# Revert to previous deployment
fly apps list

# Check deployment history
fly releases -a kindly-software-website

# Revert to specific version
fly releases rollback <VERSION> -a kindly-software-website
```

### 7.2 Backend Rollback

```bash
# Similar process
fly releases rollback <VERSION> -a kindly-dedup-webhook
```

### 7.3 Stripe Revert

- Disable webhook endpoint (don't delete)
- All payments still processed
- Webhook events queue for retry

---

## 8. Monitoring & Alerting

### 8.1 Metrics to Monitor

- Payment success rate (target: >99%)
- Webhook delivery rate (target: 100%)
- Average webhook latency (target: <1s)
- License generation success (target: 100%)
- Email send success rate (target: >95%)

### 8.2 Alert Thresholds

- Payment failure rate >1%
- Webhook processing >5s
- API error rate >1%
- License generation failure >0%

---

## 9. Documentation Links

- [Stripe API Docs](https://stripe.com/docs/api)
- [Stripe Webhooks](https://stripe.com/docs/webhooks)
- [Leptos Documentation](https://leptos.dev)
- [Fly.io Deployment](https://fly.io/docs)
- [Express.js Guide](https://expressjs.com)

---

**Document Version**: 1.0  
**Last Updated**: 2025-11-10  
**Status**: Ready for Implementation
