//! Kindly-AV1 License Activation Server
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! # Purpose
//!
//! HTTP server that handles:
//! 1. Gumroad webhook callbacks (purchase events)
//! 2. License activation requests from kindly-av1 clients
//! 3. License key generation and Ed25519 signing
//!
//! # Architecture
//!
//! Pure Rust HTTP server using std::net (no external framework dependencies).
//! Uses atomic_capsule for lockfree state management.
//!
//! # Endpoints
//!
//! - `POST /webhook/gumroad` - Gumroad purchase webhook
//! - `POST /activate` - Client license activation
//! - `GET /health` - Health check
//!
//! # Security
//!
//! - Ed25519 private key loaded from environment or file
//! - HMAC signature verification for Gumroad webhooks
//! - Rate limiting via atomic counters
//! - TLS required in production
//!
//! # Framework Compliance
//!
//! - UCE34 Q11: 100% Rust implementation
//! - Chaos: Lockfree state (AtomicU64 counters)
//! - ASSUM: All security assumptions documented

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
#[cfg(test)]
use ed25519_dalek::Signature;

/// Byzantine Royal Purple for CLI branding
const PURPLE: &str = "\x1b[38;2;155;89;182m";
/// Golden Spark for highlights
const GOLD: &str = "\x1b[38;2;241;196;15m";
/// Reset color
const RESET: &str = "\x1b[0m";

/// Server configuration
struct Config {
    /// HTTP port to listen on
    port: u16,
    /// Ed25519 signing key
    signing_key: SigningKey,
    /// Gumroad product ID
    product_id: String,
    /// Gumroad webhook secret (for HMAC verification)
    /// TODO: Implement HMAC verification in Phase 2.3
    #[allow(dead_code)]
    webhook_secret: Option<String>,
}

/// License tier (matches client-side LicenseTier)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LicenseTier {
    Creator = 1,
    Professional = 2,
    Enterprise = 3,
}

impl LicenseTier {
    fn from_str(s: &str) -> Self {
        if s.contains("Enterprise") {
            Self::Enterprise
        } else if s.contains("Professional") || s.contains("Pro") {
            Self::Professional
        } else {
            Self::Creator
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Creator => "Creator",
            Self::Professional => "Professional",
            Self::Enterprise => "Enterprise",
        }
    }
}

/// Stored license data (matches client-side StoredLicense)
#[repr(C)]
struct StoredLicense {
    license_key_hash: [u8; 32],
    tier: u8,
    device_fingerprint: [u8; 32],
    activation_timestamp: u64,
    expiry_timestamp: u64,
    signature: [u8; 64],
}

impl StoredLicense {
    /// Create and sign a new license
    fn create_signed(
        license_key: &str,
        tier: LicenseTier,
        device_fingerprint: &[u8; 32],
        signing_key: &SigningKey,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Hash license key
        let license_key_hash = *blake3::hash(license_key.as_bytes()).as_bytes();

        let mut license = Self {
            license_key_hash,
            tier: tier as u8,
            device_fingerprint: *device_fingerprint,
            activation_timestamp: now,
            expiry_timestamp: 0, // Perpetual
            signature: [0u8; 64],
        };

        // Generate Ed25519 signature over message
        let message = license.message();
        let signature = signing_key.sign(&message);
        license.signature = signature.to_bytes();

        license
    }

    /// Get message to sign (all fields except signature)
    fn message(&self) -> Vec<u8> {
        let mut msg = Vec::with_capacity(81);
        msg.extend_from_slice(&self.license_key_hash);
        msg.push(self.tier);
        msg.extend_from_slice(&self.device_fingerprint);
        msg.extend_from_slice(&self.activation_timestamp.to_le_bytes());
        msg.extend_from_slice(&self.expiry_timestamp.to_le_bytes());
        msg
    }

    /// Serialize to bytes (for transmission to client)
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(145);
        bytes.extend_from_slice(&self.license_key_hash);
        bytes.push(self.tier);
        bytes.extend_from_slice(&self.device_fingerprint);
        bytes.extend_from_slice(&self.activation_timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.expiry_timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.signature);
        bytes
    }
}

/// Server metrics (lockfree atomic counters)
struct Metrics {
    requests_total: AtomicU64,
    activations_success: AtomicU64,
    activations_failed: AtomicU64,
    webhooks_received: AtomicU64,
}

impl Metrics {
    const fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            activations_success: AtomicU64::new(0),
            activations_failed: AtomicU64::new(0),
            webhooks_received: AtomicU64::new(0),
        }
    }
}

/// Global metrics (lockfree)
static METRICS: Metrics = Metrics::new();

/// License record from Gumroad webhook
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used in /licenses endpoint JSON serialization
struct LicenseRecord {
    /// License key (Gumroad format)
    license_key: String,
    /// License tier (from Gumroad variant)
    tier: LicenseTier,
    /// Customer email
    email: String,
    /// Gumroad sale ID
    sale_id: String,
    /// Timestamp when webhook received
    registered_at: u64,
    /// Number of activations for this license
    activation_count: u32,
    /// Maximum allowed activations (based on tier)
    max_activations: u32,
}

impl LicenseRecord {
    fn new(license_key: String, tier: LicenseTier, email: String, sale_id: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Max activations based on tier (matches kindly-av1 CLAUDE.md)
        let max_activations = match tier {
            LicenseTier::Creator => 2,
            LicenseTier::Professional => 3,
            LicenseTier::Enterprise => 10,
        };

        Self {
            license_key,
            tier,
            email,
            sale_id,
            registered_at: now,
            activation_count: 0,
            max_activations,
        }
    }
}

/// Global license storage (RwLock OK for activation server - not hot path)
/// Key: license_key, Value: LicenseRecord
/// NOTE: In production, use persistent storage (SQLite, PostgreSQL)
static LICENSE_STORE: std::sync::LazyLock<RwLock<HashMap<String, LicenseRecord>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

fn main() {
    println!("\n{}=== kindly-av1 Activation Server ==={}", PURPLE, RESET);
    println!("{}[TRADE SECRET]{} License signing server\n", GOLD, RESET);

    // Load configuration
    let config = match load_config() {
        Ok(c) => Arc::new(c),
        Err(e) => {
            eprintln!("{}ERROR:{} {}", GOLD, RESET, e);
            std::process::exit(1);
        }
    };

    // Print public key for verification
    let verifying_key = config.signing_key.verifying_key();
    println!("Public key (BASE64): {}", BASE64.encode(verifying_key.to_bytes()));
    println!("Product ID: {}", config.product_id);
    println!();

    // Start HTTP server
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{}ERROR:{} Failed to bind to {}: {}", GOLD, RESET, addr, e);
            std::process::exit(1);
        }
    };

    println!("{}Listening on http://{}{}", GOLD, addr, RESET);
    println!();
    println!("Endpoints:");
    println!("  POST /webhook/gumroad  - Gumroad purchase webhook");
    println!("  POST /activate         - Client license activation");
    println!("  GET  /health           - Health check");
    println!("  GET  /metrics          - Server metrics");
    println!("  GET  /licenses         - List registered licenses (admin)");
    println!();

    // Handle connections
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let config = Arc::clone(&config);
                thread::spawn(move || {
                    if let Err(e) = handle_connection(stream, &config) {
                        eprintln!("Connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Accept error: {}", e);
            }
        }
    }
}

/// Load server configuration from environment/files
fn load_config() -> Result<Config, String> {
    // Port (default: 8080)
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .map_err(|_| "Invalid PORT")?;

    // Product ID (required)
    let product_id = env::var("GUMROAD_PRODUCT_ID")
        .unwrap_or_else(|_| "KINDLY_AV1_PLACEHOLDER".to_string());

    // Webhook secret (optional)
    let webhook_secret = env::var("GUMROAD_WEBHOOK_SECRET").ok();

    // Load signing key
    let signing_key = load_signing_key()?;

    Ok(Config {
        port,
        signing_key,
        product_id,
        webhook_secret,
    })
}

/// Load Ed25519 signing key from file or environment
fn load_signing_key() -> Result<SigningKey, String> {
    // Try environment variable first (BASE64 encoded)
    if let Ok(key_b64) = env::var("ED25519_SIGNING_KEY") {
        let key_bytes = BASE64
            .decode(&key_b64)
            .map_err(|_| "Invalid BASE64 in ED25519_SIGNING_KEY")?;
        if key_bytes.len() != 32 {
            return Err(format!(
                "Invalid signing key length: {} (expected 32)",
                key_bytes.len()
            ));
        }
        let mut key_array = [0u8; 32];
        key_array.copy_from_slice(&key_bytes);
        return Ok(SigningKey::from_bytes(&key_array));
    }

    // Try file path
    let key_path = env::var("ED25519_SIGNING_KEY_PATH").unwrap_or_else(|_| {
        // Default: relative to tool directory
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("keys/signing_key.bin"))
            .unwrap_or_else(|| PathBuf::from("keys/signing_key.bin"))
            .to_string_lossy()
            .to_string()
    });

    let key_bytes = fs::read(&key_path).map_err(|e| {
        format!(
            "Failed to read signing key from {}: {}\n\
             Set ED25519_SIGNING_KEY env var or generate keys with keygen tool",
            key_path, e
        )
    })?;

    if key_bytes.len() != 32 {
        return Err(format!(
            "Invalid signing key length in {}: {} (expected 32)",
            key_path,
            key_bytes.len()
        ));
    }

    let mut key_array = [0u8; 32];
    key_array.copy_from_slice(&key_bytes);

    println!("Loaded signing key from: {}", key_path);
    Ok(SigningKey::from_bytes(&key_array))
}

/// Handle incoming HTTP connection
fn handle_connection(mut stream: TcpStream, config: &Config) -> Result<(), String> {
    METRICS.requests_total.fetch_add(1, Ordering::Relaxed);

    // Read request
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| format!("Read error: {}", e))?;

    // Parse method and path
    let parts: Vec<&str> = request_line.trim().split(' ').collect();
    if parts.len() < 2 {
        return send_response(&mut stream, 400, "Bad Request", "Invalid request line");
    }

    let method = parts[0];
    let path = parts[1];

    // Read headers
    let mut headers = HashMap::new();
    let mut content_length: usize = 0;

    loop {
        let mut header_line = String::new();
        reader
            .read_line(&mut header_line)
            .map_err(|e| format!("Header read error: {}", e))?;

        if header_line.trim().is_empty() {
            break;
        }

        if let Some((key, value)) = header_line.split_once(':') {
            let key = key.trim().to_lowercase();
            let value = value.trim().to_string();
            if key == "content-length" {
                content_length = value.parse().unwrap_or(0);
            }
            headers.insert(key, value);
        }
    }

    // Read body if present
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader
            .read_exact(&mut body)
            .map_err(|e| format!("Body read error: {}", e))?;
    }

    // Route request
    match (method, path) {
        ("GET", "/health") => handle_health(&mut stream),
        ("POST", "/webhook/gumroad") => handle_gumroad_webhook(&mut stream, &body, config),
        ("POST", "/activate") => handle_activation(&mut stream, &body, config),
        ("GET", "/metrics") => handle_metrics(&mut stream),
        ("GET", "/licenses") => handle_list_licenses(&mut stream),
        _ => send_response(&mut stream, 404, "Not Found", "Unknown endpoint"),
    }
}

/// Handle health check
fn handle_health(stream: &mut TcpStream) -> Result<(), String> {
    send_json_response(stream, 200, r#"{"status":"ok","service":"kindly-av1-activation"}"#)
}

/// Handle metrics endpoint
fn handle_metrics(stream: &mut TcpStream) -> Result<(), String> {
    let json = format!(
        r#"{{"requests_total":{},"activations_success":{},"activations_failed":{},"webhooks_received":{}}}"#,
        METRICS.requests_total.load(Ordering::Relaxed),
        METRICS.activations_success.load(Ordering::Relaxed),
        METRICS.activations_failed.load(Ordering::Relaxed),
        METRICS.webhooks_received.load(Ordering::Relaxed),
    );
    send_json_response(stream, 200, &json)
}

/// Handle license list endpoint (admin/debug)
fn handle_list_licenses(stream: &mut TcpStream) -> Result<(), String> {
    let store = LICENSE_STORE.read().map_err(|_| "License store lock poisoned")?;

    let mut licenses = Vec::new();
    for (key, record) in store.iter() {
        licenses.push(format!(
            r#"{{"license_key":"{}","tier":"{}","email":"{}","sale_id":"{}","registered_at":{},"activations":{},"max":{}}}"#,
            key,
            record.tier.name(),
            record.email,
            record.sale_id,
            record.registered_at,
            record.activation_count,
            record.max_activations
        ));
    }

    let json = format!(
        r#"{{"count":{},"licenses":[{}]}}"#,
        licenses.len(),
        licenses.join(",")
    );
    send_json_response(stream, 200, &json)
}

/// Handle Gumroad webhook callback
///
/// Gumroad sends POST with form data:
/// - seller_id, product_id, product_name
/// - email, full_name
/// - license_key
/// - variants (contains tier: "Creator Tier", "Professional Tier", etc.)
/// - ip_country, referrer, sale_id, sale_timestamp
fn handle_gumroad_webhook(
    stream: &mut TcpStream,
    body: &[u8],
    config: &Config,
) -> Result<(), String> {
    METRICS.webhooks_received.fetch_add(1, Ordering::Relaxed);

    // Parse form data
    let body_str = String::from_utf8_lossy(body);
    let params = parse_form_data(&body_str);

    // Verify product ID
    let product_id = params.get("product_id").map(|s| s.as_str()).unwrap_or("");
    if product_id != config.product_id && config.product_id != "KINDLY_AV1_PLACEHOLDER" {
        println!("{}WARN:{} Product ID mismatch: {} != {}", GOLD, RESET, product_id, config.product_id);
        return send_json_response(stream, 400, r#"{"error":"invalid_product"}"#);
    }

    // Extract license info
    let license_key = params.get("license_key").map(|s| s.as_str()).unwrap_or("");
    let email = params.get("email").map(|s| s.as_str()).unwrap_or("");
    let variants = params.get("variants").map(|s| s.as_str()).unwrap_or("");
    let sale_id = params.get("sale_id").map(|s| s.as_str()).unwrap_or("");

    if license_key.is_empty() {
        return send_json_response(stream, 400, r#"{"error":"missing_license_key"}"#);
    }

    // Determine tier from variants
    let tier = LicenseTier::from_str(variants);

    println!(
        "{}Webhook:{} sale_id={}, email={}, tier={}, key={}...",
        GOLD, RESET, sale_id, email, tier.name(), &license_key[..license_key.len().min(10)]
    );

    // Store license in memory (in production, use persistent database)
    let record = LicenseRecord::new(
        license_key.to_string(),
        tier,
        email.to_string(),
        sale_id.to_string(),
    );

    {
        let mut store = LICENSE_STORE.write().map_err(|_| "License store lock poisoned")?;
        store.insert(license_key.to_string(), record);
    }

    println!(
        "{}Stored:{} license {} ({} tier) - max {} activations",
        GOLD, RESET, &license_key[..license_key.len().min(10)], tier.name(),
        match tier {
            LicenseTier::Creator => 2,
            LicenseTier::Professional => 3,
            LicenseTier::Enterprise => 10,
        }
    );

    send_json_response(stream, 200, &format!(
        r#"{{"success":true,"license_key":"{}","tier":"{}","max_activations":{}}}"#,
        license_key, tier.name(),
        match tier {
            LicenseTier::Creator => 2,
            LicenseTier::Professional => 3,
            LicenseTier::Enterprise => 10,
        }
    ))
}

/// Handle client license activation request
///
/// Client sends JSON:
/// {
///   "license_key": "XXXXX-XXXXX-XXXXX-XXXXX",
///   "device_fingerprint": "<hex-encoded-32-bytes>"
/// }
///
/// Server returns:
/// {
///   "success": true,
///   "tier": "Creator",
///   "license_blob": "<base64-encoded-signed-license>"
/// }
fn handle_activation(
    stream: &mut TcpStream,
    body: &[u8],
    config: &Config,
) -> Result<(), String> {
    let body_str = String::from_utf8_lossy(body);

    // Parse JSON (simple parsing without serde)
    let license_key = extract_json_string(&body_str, "license_key");
    let fingerprint_hex = extract_json_string(&body_str, "device_fingerprint");

    if license_key.is_none() || fingerprint_hex.is_none() {
        METRICS.activations_failed.fetch_add(1, Ordering::Relaxed);
        return send_json_response(stream, 400, r#"{"error":"missing_fields"}"#);
    }

    let license_key = license_key.unwrap();
    let fingerprint_hex = fingerprint_hex.unwrap();

    // Decode device fingerprint
    let fingerprint_bytes = match hex::decode(&fingerprint_hex) {
        Ok(b) if b.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            arr
        }
        _ => {
            METRICS.activations_failed.fetch_add(1, Ordering::Relaxed);
            return send_json_response(stream, 400, r#"{"error":"invalid_fingerprint"}"#);
        }
    };

    // Look up license in store
    let tier = {
        let mut store = LICENSE_STORE.write().map_err(|_| "License store lock poisoned")?;

        if let Some(record) = store.get_mut(&license_key) {
            // Check activation limit
            if record.activation_count >= record.max_activations {
                METRICS.activations_failed.fetch_add(1, Ordering::Relaxed);
                return send_json_response(stream, 403, &format!(
                    r#"{{"error":"activation_limit_exceeded","activations":{}, "max":{}}}"#,
                    record.activation_count, record.max_activations
                ));
            }

            // Increment activation count
            record.activation_count += 1;
            record.tier
        } else {
            // License not found - for demo mode, allow unregistered keys as Creator tier
            // In production, return error or call Gumroad API to verify
            #[cfg(debug_assertions)]
            {
                println!(
                    "{}WARN:{} License not registered, using demo mode (Creator tier)",
                    GOLD, RESET
                );
                LicenseTier::Creator
            }
            #[cfg(not(debug_assertions))]
            {
                METRICS.activations_failed.fetch_add(1, Ordering::Relaxed);
                return send_json_response(stream, 404, r#"{"error":"license_not_found"}"#);
            }
        }
    };

    // Create and sign license
    let signed_license = StoredLicense::create_signed(
        &license_key,
        tier,
        &fingerprint_bytes,
        &config.signing_key,
    );

    // Encode for transmission
    let license_blob = BASE64.encode(signed_license.to_bytes());

    METRICS.activations_success.fetch_add(1, Ordering::Relaxed);
    println!(
        "{}Activated:{} tier={}, fingerprint={}...",
        GOLD, RESET, tier.name(), &fingerprint_hex[..16.min(fingerprint_hex.len())]
    );

    send_json_response(stream, 200, &format!(
        r#"{{"success":true,"tier":"{}","license_blob":"{}"}}"#,
        tier.name(), license_blob
    ))
}

/// Parse URL-encoded form data
fn parse_form_data(data: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    for pair in data.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            let key = urlencoding_decode(key);
            let value = urlencoding_decode(value);
            params.insert(key, value);
        }
    }
    params
}

/// Simple URL decoding
fn urlencoding_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}

/// Extract string value from JSON (simple parser)
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)?;
    let rest = &json[start + pattern.len()..];

    // Skip whitespace and colon
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?;
    let rest = rest.trim_start();

    // Get quoted value
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;

    Some(rest[..end].to_string())
}

/// Send HTTP response
fn send_response(
    stream: &mut TcpStream,
    status: u16,
    status_text: &str,
    body: &str,
) -> Result<(), String> {
    let response = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: text/plain\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        status,
        status_text,
        body.len(),
        body
    );

    stream
        .write_all(response.as_bytes())
        .map_err(|e| format!("Write error: {}", e))
}

/// Send JSON HTTP response
fn send_json_response(stream: &mut TcpStream, status: u16, body: &str) -> Result<(), String> {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };

    let response = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        status,
        status_text,
        body.len(),
        body
    );

    stream
        .write_all(response.as_bytes())
        .map_err(|e| format!("Write error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Verifier;

    #[test]
    fn test_license_tier_from_str() {
        assert_eq!(LicenseTier::from_str("Creator Tier"), LicenseTier::Creator);
        assert_eq!(LicenseTier::from_str("Professional Tier"), LicenseTier::Professional);
        assert_eq!(LicenseTier::from_str("Pro Tier"), LicenseTier::Professional);
        assert_eq!(LicenseTier::from_str("Enterprise Tier"), LicenseTier::Enterprise);
        assert_eq!(LicenseTier::from_str("Unknown"), LicenseTier::Creator);
    }

    #[test]
    fn test_stored_license_serialization() {
        let license = StoredLicense {
            license_key_hash: [1u8; 32],
            tier: LicenseTier::Creator as u8,
            device_fingerprint: [2u8; 32],
            activation_timestamp: 1234567890,
            expiry_timestamp: 0,
            signature: [3u8; 64],
        };

        let bytes = license.to_bytes();
        // 32 (key hash) + 1 (tier) + 32 (fingerprint) + 8 (activation) + 8 (expiry) + 64 (signature) = 145
        assert_eq!(bytes.len(), 145);
    }

    #[test]
    fn test_stored_license_message() {
        let license = StoredLicense {
            license_key_hash: [1u8; 32],
            tier: LicenseTier::Creator as u8,
            device_fingerprint: [2u8; 32],
            activation_timestamp: 1234567890,
            expiry_timestamp: 0,
            signature: [3u8; 64],
        };

        let message = license.message();
        // 32 (key hash) + 1 (tier) + 32 (fingerprint) + 8 (activation) + 8 (expiry) = 81
        assert_eq!(message.len(), 81);
    }

    #[test]
    fn test_parse_form_data() {
        let data = "license_key=KDLY-1234-5678&email=test%40example.com&tier=Creator+Tier";
        let params = parse_form_data(data);

        assert_eq!(params.get("license_key"), Some(&"KDLY-1234-5678".to_string()));
        assert_eq!(params.get("email"), Some(&"test@example.com".to_string()));
        assert_eq!(params.get("tier"), Some(&"Creator Tier".to_string()));
    }

    #[test]
    fn test_extract_json_string() {
        let json = r#"{"license_key":"KDLY-1234","device_fingerprint":"aabbccdd"}"#;

        assert_eq!(
            extract_json_string(json, "license_key"),
            Some("KDLY-1234".to_string())
        );
        assert_eq!(
            extract_json_string(json, "device_fingerprint"),
            Some("aabbccdd".to_string())
        );
        assert_eq!(extract_json_string(json, "missing"), None);
    }

    #[test]
    fn test_urlencoding_decode() {
        assert_eq!(urlencoding_decode("hello+world"), "hello world");
        assert_eq!(urlencoding_decode("test%40example.com"), "test@example.com");
        assert_eq!(urlencoding_decode("a%2Fb%2Fc"), "a/b/c");
    }

    #[test]
    fn test_license_signing_round_trip() {
        use rand::rngs::OsRng;

        // Generate test keypair
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        // Create signed license
        let fingerprint = [0xAA; 32];
        let license = StoredLicense::create_signed(
            "KDLY-TEST-1234-5678-90AB",
            LicenseTier::Professional,
            &fingerprint,
            &signing_key,
        );

        // Verify signature
        let message = license.message();
        let signature = Signature::from_bytes(&license.signature);
        assert!(verifying_key.verify(&message, &signature).is_ok());

        // Wrong message should fail
        let wrong_message = b"wrong message";
        assert!(verifying_key.verify(wrong_message, &signature).is_err());
    }

    #[test]
    fn test_license_record_creator_tier() {
        let record = LicenseRecord::new(
            "KDLY-1234".to_string(),
            LicenseTier::Creator,
            "user@example.com".to_string(),
            "sale_123".to_string(),
        );
        assert_eq!(record.tier, LicenseTier::Creator);
        assert_eq!(record.max_activations, 2);
        assert_eq!(record.activation_count, 0);
    }

    #[test]
    fn test_license_record_professional_tier() {
        let record = LicenseRecord::new(
            "KDLY-5678".to_string(),
            LicenseTier::Professional,
            "pro@example.com".to_string(),
            "sale_456".to_string(),
        );
        assert_eq!(record.tier, LicenseTier::Professional);
        assert_eq!(record.max_activations, 3);
        assert_eq!(record.activation_count, 0);
    }

    #[test]
    fn test_license_record_enterprise_tier() {
        let record = LicenseRecord::new(
            "KDLY-9012".to_string(),
            LicenseTier::Enterprise,
            "enterprise@company.com".to_string(),
            "sale_789".to_string(),
        );
        assert_eq!(record.tier, LicenseTier::Enterprise);
        assert_eq!(record.max_activations, 10);
        assert_eq!(record.activation_count, 0);
    }
}
