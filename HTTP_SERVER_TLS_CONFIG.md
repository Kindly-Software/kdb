# HTTP Server TLS Configuration for atomic_capsule

**Framework Compliance**: UCE34 Q33 (Verification), Chaos (T8 Network tier), ASSUM (99.5%+ safe)

## Quick Configuration

### Rust Server (atomic_capsule HTTP Module)

```rust
// In atomic_capsule/src/http/server.rs or equivalent

use atomic_capsule::http::server::{HttpServer, TlsConfig};

#[tokio::main]
async fn main() {
    // TLS Configuration
    let tls_config = TlsConfig {
        enabled: true,
        cert_path: "/etc/letsencrypt/live/kindly.software/fullchain.pem".into(),
        key_path: "/etc/letsencrypt/live/kindly.software/privkey.pem".into(),
        min_tls_version: TlsVersion::TLS_1_3,
        // Optional: For self-signed during testing
        // insecure_skip_verify: cfg!(debug_assertions),
    };

    // HTTP Server
    let server = HttpServer::new()
        .listen("0.0.0.0:443")  // HTTPS port
        .tls(tls_config)
        .build()
        .await;

    server.start().await.unwrap();
}
```

### Configuration File (TOML)

```toml
# atomic_capsule/config/server.toml

[server]
host = "0.0.0.0"
https_port = 443
http_port = 80  # Redirect HTTP to HTTPS

[tls]
enabled = true
cert_path = "/etc/letsencrypt/live/kindly.software/fullchain.pem"
key_path = "/etc/letsencrypt/live/kindly.software/privkey.pem"
min_tls_version = "1.3"

# TLS Session management (optional)
[tls.session]
cache_enabled = true
cache_timeout_secs = 300
tickets_enabled = true

# HSTS header (strict transport security)
[security.headers]
hsts_max_age = 31536000  # 1 year
hsts_include_subdomains = true
hsts_preload = true
```

### Alternative: Axum Web Framework

```rust
// If using Axum for HTTP server

use axum::Router;
use tokio_rustls::TlsAcceptor;
use rustls::{ServerConfig, Certificate, PrivateKey};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

pub async fn setup_https_server() -> Result<(), Box<dyn std::error::Error>> {
    // Load certificate chain
    let mut cert_file = BufReader::new(File::open(
        "/etc/letsencrypt/live/kindly.software/fullchain.pem"
    )?);
    let cert_chain = certs(&mut cert_file)?
        .into_iter()
        .map(Certificate)
        .collect();

    // Load private key
    let mut key_file = BufReader::new(File::open(
        "/etc/letsencrypt/live/kindly.software/privkey.pem"
    )?);
    let keys: Vec<PrivateKey> = pkcs8_private_keys(&mut key_file)?
        .into_iter()
        .map(PrivateKey)
        .collect();

    let key = keys.into_iter().next()
        .ok_or("No private key found")?;

    // Build TLS configuration
    let mut tls_config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)?;

    // Enforce TLS 1.3
    tls_config.min_supported_version = Some(rustls::version::TLS13);

    let tls_config = Arc::new(tls_config);
    let tls_acceptor = TlsAcceptor::from(tls_config);

    // Create router
    let app = Router::new()
        .route("/", axum::routing::get(|| async { "Hello HTTPS!" }));

    // Bind to HTTPS
    let listener = tokio::net::TcpListener::bind("0.0.0.0:443").await?;
    let tls_listener = tokio_rustls::TlsListener::new(tls_acceptor, listener);

    axum::serve(tls_listener, app).await?;

    Ok(())
}
```

## Certificate Path Verification

### Verify Files Exist

```bash
# Check Let's Encrypt certificate files
ls -lh /etc/letsencrypt/live/kindly.software/

# Expected output:
# -rw-r--r-- 1 root root 1.7K fullchain.pem
# -rw-r--r-- 1 root root 1.3K privkey.pem
# -rw-r--r-- 1 root root 1.5K chain.pem
# -rw-r--r-- 1 root root  827 cert.pem
```

### Verify File Permissions

```bash
# Verify readable by application user
stat /etc/letsencrypt/live/kindly.software/fullchain.pem
# Expected: Access: (0644) uid=0 (root)

stat /etc/letsencrypt/live/kindly.software/privkey.pem
# Expected: Access: (0600) uid=0 (root)

# If your app runs as 'samuel' user:
sudo usermod -aG ssl-cert samuel
# (SSL cert group has read access)
```

### Test Certificate Loading

```rust
// Quick test to verify certificate can be loaded

use std::fs::File;
use std::io::BufReader;
use rustls_pemfile::certs;

fn main() -> std::io::Result<()> {
    let mut file = BufReader::new(File::open(
        "/etc/letsencrypt/live/kindly.software/fullchain.pem"
    )?);

    match rustls_pemfile::certs(&mut file) {
        Ok(certs) => {
            println!("✅ Certificate chain loaded successfully");
            println!("   Number of certificates: {}", certs.len());
        }
        Err(e) => {
            println!("❌ Failed to load certificates: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
```

## HTTP to HTTPS Redirect

```rust
// Redirect all HTTP traffic to HTTPS

use axum::{
    extract::ConnectInfo,
    http::{StatusCode, Uri},
    response::IntoResponse,
    routing::any,
    Router,
};
use std::net::SocketAddr;

async fn http_redirect(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    uri: Uri,
) -> impl IntoResponse {
    let https_url = format!("https://{}{}", uri.host().unwrap_or("kindly.software"), uri.path_and_query().map(|pq| pq.as_str()).unwrap_or(""));

    (
        StatusCode::MOVED_PERMANENTLY,
        [("Location", https_url.as_str())],
    )
}

pub fn http_redirect_router() -> Router {
    Router::new()
        .route("/*path", any(http_redirect))
}
```

## HSTS and Security Headers

```rust
// Add security headers to all responses

use axum::{
    http::Response,
    middleware::Next,
    extract::Request,
};

pub async fn add_security_headers(
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;

    let headers = response.headers_mut();

    // Enforce TLS for 1 year
    headers.insert(
        "Strict-Transport-Security",
        "max-age=31536000; includeSubDomains; preload".parse().unwrap(),
    );

    // Prevent MIME sniffing
    headers.insert(
        "X-Content-Type-Options",
        "nosniff".parse().unwrap(),
    );

    // Clickjacking protection
    headers.insert(
        "X-Frame-Options",
        "DENY".parse().unwrap(),
    );

    // XSS protection
    headers.insert(
        "X-XSS-Protection",
        "1; mode=block".parse().unwrap(),
    );

    // CSP header
    headers.insert(
        "Content-Security-Policy",
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'".parse().unwrap(),
    );

    // Referrer policy
    headers.insert(
        "Referrer-Policy",
        "strict-origin-when-cross-origin".parse().unwrap(),
    );

    response
}
```

## Certificate Reloading on Renewal

```rust
// Automatically reload certificates when Let's Encrypt renews them

use std::sync::Arc;
use tokio::sync::RwLock;
use rustls::ServerConfig;
use std::fs::File;
use std::io::BufReader;

pub struct ReloadableTlsConfig {
    config: Arc<RwLock<Arc<ServerConfig>>>,
}

impl ReloadableTlsConfig {
    pub async fn reload_certificates(&self) -> std::io::Result<()> {
        // Load new certificates
        let mut cert_file = BufReader::new(File::open(
            "/etc/letsencrypt/live/kindly.software/fullchain.pem"
        )?);
        let certs = rustls_pemfile::certs(&mut cert_file)?
            .into_iter()
            .map(rustls::Certificate)
            .collect();

        let mut key_file = BufReader::new(File::open(
            "/etc/letsencrypt/live/kindly.software/privkey.pem"
        )?);
        let keys: Vec<_> = rustls_pemfile::pkcs8_private_keys(&mut key_file)?
            .into_iter()
            .map(rustls::PrivateKey)
            .collect();

        let key = keys.into_iter().next()
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No private key found"
            ))?;

        // Build new config
        let new_config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;

        // Swap atomically
        let mut config = self.config.write().await;
        *config = Arc::new(new_config);

        println!("✅ TLS certificates reloaded successfully");
        Ok(())
    }
}

// Usage in main:
let tls_config = ReloadableTlsConfig {
    config: Arc::new(RwLock::new(Arc::new(/* initial config */))),
};

// In systemd post-renewal hook:
// /etc/letsencrypt/renewal-hooks/post/reload-tls.sh
// Calls endpoint that triggers reload_certificates()
```

## Systemd Service Configuration

```ini
# /etc/systemd/system/atomic-http-server.service

[Unit]
Description=Atomic Capsule HTTPS Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=samuel
Group=ssl-cert
WorkingDirectory=/home/samuel/Primitives/atomic_capsule

ExecStart=/home/samuel/Primitives/atomic_capsule/target/release/http-server

# Restart on failure
Restart=on-failure
RestartSec=5s

# Security settings
PrivateTmp=yes
NoNewPrivileges=yes
ProtectHome=yes
ProtectSystem=strict
ReadWritePaths=/var/log/http-server

# Capability limitations
AmbientCapabilities=CAP_NET_BIND_SERVICE  # Needed for port 443

# Post-renewal hook (Let's Encrypt)
ExecReload=/bin/kill -HUP $MAINPID

[Install]
WantedBy=multi-user.target
```

### Service Management

```bash
# Enable autostart
sudo systemctl enable atomic-http-server

# Start service
sudo systemctl start atomic-http-server

# Check status
sudo systemctl status atomic-http-server

# View logs
sudo journalctl -u atomic-http-server -n 50 --follow

# Restart (e.g., after certificate renewal)
sudo systemctl restart atomic-http-server
```

## Cargo.toml Dependencies

```toml
[dependencies]
# HTTP/HTTPS
axum = "0.8"
tokio = { version = "1", features = ["full"] }

# TLS/SSL
rustls = "0.23"
rustls-pemfile = "2"
tokio-rustls = "0.25"

# atomic_capsule integration
atomic_capsule = { path = "./atomic_capsule", features = ["std", "http"] }
```

## Testing TLS Configuration

### Local Testing (Self-Signed)

```bash
# Generate self-signed certificate for testing
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes \
    -subj "/CN=localhost"

# Test server locally
RUST_LOG=debug cargo run --bin http-server

# Test from another terminal
curl -k https://localhost:443/
# -k = ignore self-signed certificate warning
```

### Production Testing (Let's Encrypt)

```bash
# Verify certificate chain
curl -I https://kindly.software/

# Check TLS version
curl -I --tlsv1.3 https://kindly.software/

# Full handshake details
openssl s_client -connect kindly.software:443 -showcerts

# Check cipher suites
curl --tlsv1.3 --ciphers DEFAULT -I https://kindly.software/

# Performance test
ab -n 100 -c 10 https://kindly.software/
```

## Troubleshooting

### Certificate Not Found

```bash
# Error: "error reading /etc/letsencrypt/live/kindly.software/fullchain.pem"

# Solution 1: Verify file exists
ls -l /etc/letsencrypt/live/kindly.software/

# Solution 2: Check permissions
sudo chmod 755 /etc/letsencrypt/live/kindly.software/

# Solution 3: Run as root or with group permissions
sudo usermod -aG ssl-cert $(whoami)
```

### Private Key Permissions

```bash
# Error: "Permission denied" reading privkey.pem

# Solution: Private key should be readable only by owner
sudo chmod 600 /etc/letsencrypt/live/kindly.software/privkey.pem

# Or allow group access
sudo chmod 640 /etc/letsencrypt/live/kindly.software/privkey.pem
sudo chgrp ssl-cert /etc/letsencrypt/live/kindly.software/privkey.pem
```

### TLS 1.3 Not Negotiated

```bash
# Error: Server negotiates TLS 1.2 instead of 1.3

# Solution: Verify TLS 1.3 is enabled in config
# In server.toml or code: min_tls_version = "1.3"

# Test available TLS versions
openssl s_client -connect kindly.software:443 -tls1_2
# Should fail: "SSL: TLSV1_ALERT_PROTOCOL_VERSION"

openssl s_client -connect kindly.software:443 -tls1_3
# Should succeed with TLS 1.3
```

## Performance Considerations

### TLS Session Caching

```rust
// Enable TLS session caching to reduce handshake overhead

[tls.session]
cache_enabled = true
cache_size = 1024  // Number of sessions to cache
cache_timeout_secs = 300  // 5 minutes

// For high-traffic servers, session tickets are better:
[tls.session]
tickets_enabled = true
ticket_key_rotation_secs = 3600
```

### Keepalive Tuning

```rust
// Keep HTTP connections alive to reuse TLS session

[http]
keepalive_timeout_secs = 60
max_keepalive_requests = 100
```

### Load Testing with TLS

```bash
# Single connection (test TLS handshake)
time curl https://kindly.software/

# Multiple connections (test throughput)
ab -n 1000 -c 50 https://kindly.software/

# Keep-alive connections (more realistic)
ab -n 1000 -c 50 -k https://kindly.software/
```

## Compliance and Standards

### Q33 Verification (UCE34)

```bash
# Verify certificate details
sudo openssl x509 -in /etc/letsencrypt/live/kindly.software/fullchain.pem \
    -noout -text | grep -E "Subject:|Issuer:|Public-Key:|Not Before:|Not After:"

# Verify TLS 1.3
curl --tlsv1.3 -I https://kindly.software/

# Verify certificate chain
openssl s_client -connect kindly.software:443 -showcerts < /dev/null | \
    openssl verify -CAfile /etc/ssl/certs/ca-certificates.crt
```

### ASSUM Safety (99.5%+)

```
#ASSUME_CERT_ACCESSIBLE: Application can read cert_path and key_path
  → Verified by: ls -l /etc/letsencrypt/live/kindly.software/

#ASSUME_PORTS_AVAILABLE: Ports 80 and 443 are available and not firewalled
  → Verified by: telnet kindly.software 443

#ASSUME_RENEWAL_AUTOMATIC: Certbot systemd timer runs renewal
  → Verified by: systemctl status certbot.timer

#ASSUME_TLS_1_3_AVAILABLE: Client supports TLS 1.3
  → Verified by: curl --tlsv1.3 https://kindly.software/
```

---

## References

- **Certbot HTTPS**: https://certbot.eff.org/docs/
- **Rustls**: https://docs.rs/rustls/
- **Tokio-Rustls**: https://docs.rs/tokio-rustls/
- **RFC 8446 (TLS 1.3)**: https://tools.ietf.org/html/rfc8446
- **Axum HTTPS**: https://github.com/tokio-rs/axum/tree/main/examples
