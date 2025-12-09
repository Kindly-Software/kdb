# API Reference

**REST API and WebSocket integration for developers**

---

## Base URLs

- **Mainnet**: `https://api.kindly.coin`
- **Testnet**: `https://testnet-api.kindly.coin`
- **Local**: `http://localhost:9000`

---

## Authentication

All API requests require API key or JWT token:

```bash
# API Key (Header)
curl -H "X-API-Key: your_api_key_here" https://api.kindly.coin/v1/transactions

# JWT Token (Header)
curl -H "Authorization: Bearer your_jwt_token" https://api.kindly.coin/v1/transactions
```

---

## Core APIs

### 1. Submit Transaction

**Endpoint**: `POST /v1/transactions`

**Request**:
```json
{
  "sender": "0x1234...5678",
  "recipient": "0xabcd...ef01",
  "amount": "1000000000000",
  "fee": "10000000",
  "nonce": 42,
  "signature": "0xsig..."
}
```

**Response** (200 OK):
```json
{
  "transaction_id": "0xtx...",
  "status": "pending",
  "submitted_at": "2025-10-07T12:34:56Z",
  "estimated_finality": "2025-10-07T12:34:57Z"
}
```

### 2. Query Balance

**Endpoint**: `GET /v1/accounts/{address}/balance`

**Response** (200 OK):
```json
{
  "address": "0x1234...5678",
  "balance": "50000000000000",
  "nonce": 42,
  "last_updated": "2025-10-07T12:34:56Z"
}
```

### 3. Claim UBI

**Endpoint**: `POST /v1/ubi/claim`

**Request**:
```json
{
  "citizen_id": "0xcitizen...",
  "month": 10,
  "year": 2025,
  "merkle_proof": [
    "0xproof1...",
    "0xproof2...",
    ...
  ],
  "signature": "0xsig..."
}
```

**Response** (200 OK):
```json
{
  "success": true,
  "amount": "100000000000",
  "new_balance": "150000000000",
  "claim_id": "0xclaim..."
}
```

### 4. Query Block

**Endpoint**: `GET /v1/blocks/{height}`

**Response** (200 OK):
```json
{
  "height": 1000000,
  "hash": "0xblock...",
  "parent_hash": "0xparent...",
  "timestamp": "2025-10-07T12:34:56Z",
  "validator": "0xval...",
  "transaction_count": 5000,
  "finalized": true,
  "vote_count": 67,
  "total_validators": 100
}
```

---

## WebSocket Subscriptions

### Connect

```javascript
const ws = new WebSocket('wss://api.kindly.coin/v1/ws');

ws.on('open', () => {
  console.log('Connected to Kindly Coin WebSocket');
});
```

### Subscribe to Transactions

```javascript
ws.send(JSON.stringify({
  "action": "subscribe",
  "channel": "transactions",
  "filter": {
    "address": "0x1234...5678"
  }
}));

ws.on('message', (data) => {
  const tx = JSON.parse(data);
  console.log('New transaction:', tx);
});
```

### Subscribe to Block Finality

```javascript
ws.send(JSON.stringify({
  "action": "subscribe",
  "channel": "blocks",
  "filter": {
    "finalized": true
  }
}));

ws.on('message', (data) => {
  const block = JSON.parse(data);
  console.log('New finalized block:', block);
});
```

---

## Government APIs

### Verify Citizen Identity

**Endpoint**: `POST /v1/government/kyc/verify`

**Request** (requires government API key):
```json
{
  "national_id": "123456789",
  "biometric_hash": "0xbio...",
  "government_id": "US_SSA",
  "signature": "0xgov_sig..."
}
```

**Response** (200 OK):
```json
{
  "citizen_id": "0xcitizen...",
  "verified": true,
  "verification_level": 3,
  "ubi_eligible": true
}
```

### Query Tax Revenue

**Endpoint**: `GET /v1/government/treasury/stats`

**Response** (200 OK):
```json
{
  "total_tax_revenue": "50000000000000",
  "today_revenue": "2000000000000",
  "this_month_revenue": "30000000000000",
  "transactions_today": 100000,
  "projected_monthly": "45000000000000"
}
```

---

## Rate Limits

- **Public API**: 100 requests/minute
- **Authenticated**: 1000 requests/minute
- **Government**: 10,000 requests/minute
- **WebSocket**: 100 subscriptions per connection

---

## Error Codes

| Code | Message | Description |
|------|---------|-------------|
| 400 | Bad Request | Invalid request format |
| 401 | Unauthorized | Missing or invalid API key |
| 403 | Forbidden | Insufficient permissions |
| 404 | Not Found | Resource not found |
| 429 | Too Many Requests | Rate limit exceeded |
| 500 | Internal Server Error | Server error |

---

## Code Examples

### JavaScript/Node.js

```javascript
const axios = require('axios');

async function submitTransaction(sender, recipient, amount) {
  const response = await axios.post('https://api.kindly.coin/v1/transactions', {
    sender,
    recipient,
    amount,
    fee: '10000000',
    nonce: 42,
    signature: '0xsig...'
  }, {
    headers: {
      'X-API-Key': process.env.KINDLY_API_KEY
    }
  });

  return response.data;
}
```

### Python

```python
import requests

def query_balance(address):
    response = requests.get(
        f'https://api.kindly.coin/v1/accounts/{address}/balance',
        headers={'X-API-Key': os.environ['KINDLY_API_KEY']}
    )
    return response.json()
```

### Rust

```rust
use reqwest;
use serde_json::json;

async fn submit_transaction(
    sender: &str,
    recipient: &str,
    amount: u64,
) -> Result<TransactionResponse, reqwest::Error> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.kindly.coin/v1/transactions")
        .header("X-API-Key", std::env::var("KINDLY_API_KEY").unwrap())
        .json(&json!({
            "sender": sender,
            "recipient": recipient,
            "amount": amount.to_string(),
            "fee": "10000000",
            "nonce": 42,
            "signature": "0xsig..."
        }))
        .send()
        .await?
        .json()
        .await?;

    Ok(response)
}
```

---

Next: [ROADMAP.md](ROADMAP.md) - Development timeline
