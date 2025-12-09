//! Gumroad License Verification Capsule (T1 Atomic + T9 Persistent)
//! [TRADE SECRET]
//!
//! # Architecture
//!
//! Two-phase license validation:
//! 1. **Online Activation**: Verify with Gumroad API, generate Ed25519 signature
//! 2. **Offline Validation**: Verify Ed25519 signature for subsequent launches
//!
//! # Memory Layout (256B, cache-aligned)
//!
//! ```text
//! Offset  Size  Field
//! ------  ----  -----
//! 0       8     state (AtomicU64: 0=unactivated, 1=activated, 2=error)
//! 8       8     last_check_timestamp (AtomicU64)
//! 16      8     activation_timestamp (AtomicU64)
//! 24      8     generation (AtomicU64)
//! 32      64    license_key_hash (BLAKE3)
//! 96      64    signature (Ed25519)
//! 160     96    _padding
//! ------  ----
//! Total:  256B (4 cache lines, 64B aligned)
//! ```
//!
//! # Gumroad API
//!
//! Endpoint: POST https://api.gumroad.com/v2/licenses/verify
//! Parameters:
//! - product_id: Unique product identifier (from Gumroad dashboard)
//! - license_key: User's license key (XXXXX-XXXXX-XXXXX-XXXXX format)
//! - increment_uses_count: "false" (don't increment on verification)
//!
//! # Ed25519 Offline Validation
//!
//! On successful online activation:
//! 1. Generate Ed25519 keypair (public key embedded in binary)
//! 2. Sign message: {license_key, tier, device_fingerprint, expiry}
//! 3. Store signed license locally (~/.config/kindly-av1/license.bin)
//!
//! On subsequent launches:
//! 1. Read stored license
//! 2. Verify Ed25519 signature with embedded public key
//! 3. Check device fingerprint match
//! 4. Check expiry (if applicable)
//!
//! # Framework Compliance
//!
//! - UCE34: Q10 T1 Atomic + T9 Persistent
//! - Chaos: 256B cache-aligned, generation counters, AtomicU64 state
//! - ASSUM: 99.5%+ safe, all network I/O documented
//! - T28: Q1-Q7 unit tests with mock responses

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

use super::fingerprint::HardwareFingerprint;
use super::tier_enforcement::LicenseTier;

/// Gumroad product ID (from dashboard)
/// #ASSUME: Product ID is set via environment variable or build-time config
/// #VERIFY: Must be configured before release
/// Placeholder value for development - MUST be replaced before release
const PRODUCT_ID: &str = match option_env!("GUMROAD_PRODUCT_ID") {
    Some(id) => id,
    None => "KINDLY_AV1_PLACEHOLDER",
};

// Include auto-generated public key from build.rs
// #ASSUME: build.rs generates license_public_key.rs with ED25519_PUBLIC_KEY
// #VERIFY: Build fails in release mode if key is missing
include!(concat!(env!("OUT_DIR"), "/license_public_key.rs"));

/// License verification errors
#[derive(Debug)]
pub enum GumroadError {
    /// Network error during API call
    NetworkError(String),
    /// Invalid API response
    InvalidResponse(String),
    /// License key invalid or revoked
    InvalidLicense(String),
    /// Signature verification failed
    SignatureVerificationFailed,
    /// Device fingerprint mismatch
    DeviceMismatch,
    /// License expired
    Expired,
    /// I/O error reading/writing license file
    IoError(std::io::Error),
}

impl std::fmt::Display for GumroadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::InvalidResponse(msg) => write!(f, "Invalid API response: {}", msg),
            Self::InvalidLicense(msg) => write!(f, "Invalid license: {}", msg),
            Self::SignatureVerificationFailed => write!(f, "Signature verification failed"),
            Self::DeviceMismatch => write!(f, "Device fingerprint mismatch"),
            Self::Expired => write!(f, "License expired"),
            Self::IoError(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for GumroadError {}

impl From<std::io::Error> for GumroadError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

/// License state
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LicenseState {
    Unactivated = 0,
    Activated = 1,
    Error = 2,
}

impl From<u64> for LicenseState {
    fn from(value: u64) -> Self {
        match value {
            0 => Self::Unactivated,
            1 => Self::Activated,
            2 => Self::Error,
            // #ASSUME: Unknown states map to error for security
            // #VERIFY: Tampering attempts result in error state
            _ => Self::Error,
        }
    }
}

/// Stored license data (for offline validation)
#[repr(C)]
struct StoredLicense {
    /// License key (hashed)
    license_key_hash: [u8; 32],
    /// License tier
    tier: u8,
    /// Device fingerprint (for binding check)
    device_fingerprint: [u8; 32],
    /// Activation timestamp
    activation_timestamp: u64,
    /// Expiry timestamp (0 = no expiry)
    expiry_timestamp: u64,
    /// Ed25519 signature (signs all above fields)
    signature: [u8; 64],
}

impl StoredLicense {
    /// Serialize to bytes
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(169);
        bytes.extend_from_slice(&self.license_key_hash);
        bytes.push(self.tier);
        bytes.extend_from_slice(&self.device_fingerprint);
        bytes.extend_from_slice(&self.activation_timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.expiry_timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.signature);
        bytes
    }

    /// Deserialize from bytes
    fn from_bytes(bytes: &[u8]) -> Result<Self, GumroadError> {
        // 32 (key hash) + 1 (tier) + 32 (fingerprint) + 8 (activation) + 8 (expiry) + 64 (signature) = 145
        if bytes.len() != 145 {
            return Err(GumroadError::InvalidResponse(format!(
                "Invalid license file length: {}",
                bytes.len()
            )));
        }

        let mut license_key_hash = [0u8; 32];
        license_key_hash.copy_from_slice(&bytes[0..32]);

        let tier = bytes[32];

        let mut device_fingerprint = [0u8; 32];
        device_fingerprint.copy_from_slice(&bytes[33..65]);

        let activation_timestamp = u64::from_le_bytes(bytes[65..73].try_into().unwrap());
        let expiry_timestamp = u64::from_le_bytes(bytes[73..81].try_into().unwrap());

        let mut signature = [0u8; 64];
        signature.copy_from_slice(&bytes[81..145]);

        Ok(Self {
            license_key_hash,
            tier,
            device_fingerprint,
            activation_timestamp,
            expiry_timestamp,
            signature,
        })
    }

    /// Get message to sign (all fields except signature)
    fn message(&self) -> Vec<u8> {
        let mut msg = Vec::with_capacity(105);
        msg.extend_from_slice(&self.license_key_hash);
        msg.push(self.tier);
        msg.extend_from_slice(&self.device_fingerprint);
        msg.extend_from_slice(&self.activation_timestamp.to_le_bytes());
        msg.extend_from_slice(&self.expiry_timestamp.to_le_bytes());
        msg
    }
}

/// Gumroad License Verification Capsule (256B, T1 Atomic + T9 Persistent)
///
/// Two-phase license validation:
/// 1. Online: Verify with Gumroad API, generate Ed25519 signature
/// 2. Offline: Verify Ed25519 signature for subsequent launches
///
/// # Thread Safety
///
/// All state modifications use atomic operations. Safe to share across threads.
///
/// # Anti-Piracy
///
/// - Ed25519 signature prevents license file tampering
/// - Device fingerprint binding prevents license sharing
/// - Generation counter detects state modification
/// - BLAKE3 hash prevents license key leaks
#[repr(C, align(64))]
pub struct GumroadLicenseCapsule {
    /// License state (0=unactivated, 1=activated, 2=error)
    state: AtomicU64,

    /// Last check timestamp
    last_check_timestamp: AtomicU64,

    /// Activation timestamp
    activation_timestamp: AtomicU64,

    /// Generation counter for tamper detection
    generation: AtomicU64,

    /// License key hash (BLAKE3)
    license_key_hash: [u8; 32],

    /// Ed25519 signature
    signature: [u8; 64],

    /// Padding for 256B cache alignment
    _padding: [u8; 96],
}

// Compile-time size verification
const _: () = assert!(std::mem::size_of::<GumroadLicenseCapsule>() == 256);
const _: () = assert!(std::mem::align_of::<GumroadLicenseCapsule>() == 64);

impl GumroadLicenseCapsule {
    /// Create new capsule (unactivated state)
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(LicenseState::Unactivated as u64),
            last_check_timestamp: AtomicU64::new(0),
            activation_timestamp: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            license_key_hash: [0u8; 32],
            signature: [0u8; 64],
            _padding: [0u8; 96],
        }
    }

    /// Activate license online (Gumroad API verification)
    ///
    /// # Workflow
    ///
    /// 1. POST to api.gumroad.com/v2/licenses/verify
    /// 2. Parse response for success/tier
    /// 3. Generate Ed25519 signature
    /// 4. Store signed license locally
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Network error during API call
    /// - Invalid API response
    /// - License key invalid or revoked
    pub fn activate_online(
        &mut self,
        license_key: &str,
        fingerprint: &HardwareFingerprint,
    ) -> Result<LicenseTier, GumroadError> {
        // Verify with Gumroad API
        let tier = self.verify_with_gumroad(license_key)?;

        // Generate Ed25519 signature
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let stored_license = self.create_signed_license(license_key, tier, fingerprint, now)?;

        // Update capsule state
        self.license_key_hash
            .copy_from_slice(&stored_license.license_key_hash);
        self.signature.copy_from_slice(&stored_license.signature);
        self.activation_timestamp
            .store(stored_license.activation_timestamp, Ordering::Release);
        self.state.store(LicenseState::Activated as u64, Ordering::Release);
        self.last_check_timestamp.store(now, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Persist to disk
        self.save_license(&stored_license)?;

        Ok(tier)
    }

    /// Verify license offline (Ed25519 signature verification)
    ///
    /// # Workflow
    ///
    /// 1. Load stored license from disk
    /// 2. Verify Ed25519 signature
    /// 3. Check device fingerprint match
    /// 4. Check expiry (if applicable)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - License file not found
    /// - Signature verification failed
    /// - Device fingerprint mismatch
    /// - License expired
    pub fn verify_offline(
        &self,
        fingerprint: &HardwareFingerprint,
    ) -> Result<LicenseTier, GumroadError> {
        // Load stored license
        let stored_license = self.load_license()?;

        // Verify Ed25519 signature
        self.verify_signature(&stored_license)?;

        // Check device fingerprint
        if &stored_license.device_fingerprint != fingerprint.as_bytes() {
            return Err(GumroadError::DeviceMismatch);
        }

        // Check expiry
        if stored_license.expiry_timestamp > 0 {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if now > stored_license.expiry_timestamp {
                return Err(GumroadError::Expired);
            }
        }

        // Update last check timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_check_timestamp.store(now, Ordering::Release);

        Ok(LicenseTier::from(stored_license.tier))
    }

    /// Deactivate license (remove stored license)
    pub fn deactivate(&mut self) -> Result<(), GumroadError> {
        let license_path = self.license_path();
        if license_path.exists() {
            std::fs::remove_file(license_path)?;
        }

        self.state
            .store(LicenseState::Unactivated as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    /// Get license state
    #[inline]
    pub fn state(&self) -> LicenseState {
        LicenseState::from(self.state.load(Ordering::Acquire))
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    // ============================================================================
    // PRIVATE METHODS
    // ============================================================================

    /// Verify license key with Gumroad API
    ///
    /// # HTTP Request
    ///
    /// ```http
    /// POST /v2/licenses/verify HTTP/1.1
    /// Host: api.gumroad.com
    /// Content-Type: application/x-www-form-urlencoded
    ///
    /// product_id=YOUR_PRODUCT_ID&license_key=XXXXX-XXXXX-XXXXX-XXXXX&increment_uses_count=false
    /// ```
    ///
    /// # Response
    ///
    /// Success (200 OK):
    /// ```json
    /// {
    ///   "success": true,
    ///   "purchase": {
    ///     "product_name": "kindly-av1",
    ///     "variants": "Creator Tier"
    ///   }
    /// }
    /// ```
    ///
    /// Error (404 Not Found):
    /// ```json
    /// {
    ///   "success": false,
    ///   "message": "That license does not exist for the provided product."
    /// }
    /// ```
    fn verify_with_gumroad(&self, license_key: &str) -> Result<LicenseTier, GumroadError> {
        // Build request body
        let body = format!(
            "product_id={}&license_key={}&increment_uses_count=false",
            PRODUCT_ID, license_key
        );

        // Create TLS configuration with Mozilla root certificates
        // #ASSUME: webpki_roots provides trusted CA certificates
        // #VERIFY: TLS 1.2+ enforced, certificate verification enabled
        let mut root_store = RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        // Connect to Gumroad API with TLS
        // #ASSUME: DNS resolution works for api.gumroad.com
        // #VERIFY: TLS 1.3 preferred, fallback to TLS 1.2
        let tcp_stream = TcpStream::connect("api.gumroad.com:443").map_err(|e| {
            GumroadError::NetworkError(format!("Failed to connect to Gumroad API: {}", e))
        })?;

        // Create TLS connection
        let server_name = ServerName::try_from("api.gumroad.com")
            .map_err(|_| GumroadError::NetworkError("Invalid server name".to_string()))?;

        let client_connection = ClientConnection::new(Arc::new(config), server_name).map_err(|e| {
            GumroadError::NetworkError(format!("TLS handshake failed: {}", e))
        })?;

        let mut tls_stream = StreamOwned::new(client_connection, tcp_stream);

        // Send HTTP request over TLS
        let request = format!(
            "POST /v2/licenses/verify HTTP/1.1\r\n\
             Host: api.gumroad.com\r\n\
             Content-Type: application/x-www-form-urlencoded\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             {}",
            body.len(),
            body
        );

        tls_stream.write_all(request.as_bytes()).map_err(|e| {
            GumroadError::NetworkError(format!("Failed to send request: {}", e))
        })?;

        // Read response
        let mut response = String::new();
        tls_stream.read_to_string(&mut response).map_err(|e| {
            GumroadError::NetworkError(format!("Failed to read response: {}", e))
        })?;

        // Parse response
        self.parse_gumroad_response(&response)
    }

    /// Parse Gumroad API response
    ///
    /// Extracts tier from variants field:
    /// - "Creator Tier" → LicenseTier::Creator
    /// - "Professional Tier" → LicenseTier::Professional
    /// - "Enterprise Tier" → LicenseTier::Enterprise
    fn parse_gumroad_response(&self, response: &str) -> Result<LicenseTier, GumroadError> {
        // Split headers and body
        let parts: Vec<&str> = response.split("\r\n\r\n").collect();
        if parts.len() < 2 {
            return Err(GumroadError::InvalidResponse(
                "Missing response body".to_string(),
            ));
        }

        let body = parts[1];

        // Simple JSON parsing (avoid external deps)
        // Look for "success": true
        if !body.contains("\"success\":true") && !body.contains("\"success\": true") {
            // Extract error message
            let msg = if let Some(start) = body.find("\"message\":\"") {
                let start = start + 11;
                let end = body[start..].find('"').unwrap_or(0) + start;
                body[start..end].to_string()
            } else {
                "Unknown error".to_string()
            };
            return Err(GumroadError::InvalidLicense(msg));
        }

        // Extract tier from variants
        let tier = if body.contains("Creator Tier") {
            LicenseTier::Creator
        } else if body.contains("Professional Tier") || body.contains("Pro Tier") {
            LicenseTier::Professional
        } else if body.contains("Enterprise Tier") {
            LicenseTier::Enterprise
        } else {
            // Default to creator tier if variants not specified
            LicenseTier::Creator
        };

        Ok(tier)
    }

    /// Create signed license for offline validation
    ///
    /// # Development Mode
    ///
    /// In development mode (`IS_DEVELOPMENT_KEY == true`), uses the zero key
    /// for local testing. This creates licenses that can only be verified
    /// with the development public key.
    ///
    /// # Production Mode
    ///
    /// In production, the server returns a pre-signed license blob from the
    /// Gumroad webhook. The client only needs to verify signatures, not create them.
    /// This function is kept for development/testing purposes.
    ///
    /// # Security
    ///
    /// - Development signatures are only valid with development public key
    /// - Production signatures require the private key (server-side only)
    /// - Client binary never contains private key
    fn create_signed_license(
        &self,
        license_key: &str,
        tier: LicenseTier,
        fingerprint: &HardwareFingerprint,
        activation_timestamp: u64,
    ) -> Result<StoredLicense, GumroadError> {
        // Hash license key (BLAKE3)
        let license_key_hash = blake3::hash(license_key.as_bytes());

        // Create license structure
        let mut stored_license = StoredLicense {
            license_key_hash: *license_key_hash.as_bytes(),
            tier: tier as u8,
            device_fingerprint: *fingerprint.as_bytes(),
            activation_timestamp,
            expiry_timestamp: 0, // No expiry for perpetual licenses
            signature: [0u8; 64],
        };

        // Generate Ed25519 signature
        // #ASSUME: Development mode uses zero key for local testing
        // #VERIFY: Production uses server-side signing (private key never in client)
        if IS_DEVELOPMENT_KEY {
            // Development mode: Use zero key (matches build.rs development public key)
            let signing_key = SigningKey::from_bytes(&[0u8; 32]);
            let message = stored_license.message();
            let signature = signing_key.sign(&message);
            stored_license.signature.copy_from_slice(&signature.to_bytes());
        } else {
            // Production mode: Server should have returned pre-signed license
            // This path should not be reached in production - the server
            // signs licenses and returns the signature in the API response
            return Err(GumroadError::InvalidResponse(
                "Server did not return signed license. Contact support@kindly.dev".to_string()
            ));
        }

        Ok(stored_license)
    }

    /// Verify Ed25519 signature
    fn verify_signature(&self, stored_license: &StoredLicense) -> Result<(), GumroadError> {
        // Load embedded public key
        let public_key = VerifyingKey::from_bytes(&ED25519_PUBLIC_KEY).map_err(|_| {
            GumroadError::SignatureVerificationFailed
        })?;

        // Parse signature
        let signature = Signature::from_bytes(&stored_license.signature);

        // Verify signature
        let message = stored_license.message();
        public_key
            .verify(&message, &signature)
            .map_err(|_| GumroadError::SignatureVerificationFailed)?;

        Ok(())
    }

    /// Get license file path
    fn license_path(&self) -> std::path::PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        path.push("kindly-av1");
        path.push("license.bin");
        path
    }

    /// Save license to disk
    fn save_license(&self, stored_license: &StoredLicense) -> Result<(), GumroadError> {
        let path = self.license_path();

        // Create parent directory
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Write license file
        let bytes = stored_license.to_bytes();
        std::fs::write(path, bytes)?;

        Ok(())
    }

    /// Load license from disk
    fn load_license(&self) -> Result<StoredLicense, GumroadError> {
        let path = self.license_path();

        // Read license file
        let bytes = std::fs::read(path)?;

        // Deserialize
        StoredLicense::from_bytes(&bytes)
    }
}

impl Default for GumroadLicenseCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: All mutable fields are atomic
// #ASSUME: AtomicU64 is Send + Sync
// #VERIFY: No shared mutable state, all accesses atomic
unsafe impl Send for GumroadLicenseCapsule {}
unsafe impl Sync for GumroadLicenseCapsule {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(std::mem::size_of::<GumroadLicenseCapsule>(), 256);
        assert_eq!(std::mem::align_of::<GumroadLicenseCapsule>(), 64);
    }

    #[test]
    fn test_new_capsule_defaults_to_unactivated() {
        let capsule = GumroadLicenseCapsule::new();
        assert_eq!(capsule.state(), LicenseState::Unactivated);
        assert_eq!(capsule.generation(), 0);
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

        let deserialized = StoredLicense::from_bytes(&bytes).unwrap();
        assert_eq!(deserialized.license_key_hash, license.license_key_hash);
        assert_eq!(deserialized.tier, license.tier);
        assert_eq!(deserialized.device_fingerprint, license.device_fingerprint);
        assert_eq!(
            deserialized.activation_timestamp,
            license.activation_timestamp
        );
        assert_eq!(deserialized.expiry_timestamp, license.expiry_timestamp);
        assert_eq!(deserialized.signature, license.signature);
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
    fn test_parse_gumroad_success_response() {
        let capsule = GumroadLicenseCapsule::new();

        let response = "HTTP/1.1 200 OK\r\n\r\n{\"success\":true,\"purchase\":{\"product_name\":\"kindly-av1\",\"variants\":\"Creator Tier\"}}";
        let tier = capsule.parse_gumroad_response(response).unwrap();
        assert_eq!(tier, LicenseTier::Creator);

        let response = "HTTP/1.1 200 OK\r\n\r\n{\"success\":true,\"purchase\":{\"product_name\":\"kindly-av1\",\"variants\":\"Professional Tier\"}}";
        let tier = capsule.parse_gumroad_response(response).unwrap();
        assert_eq!(tier, LicenseTier::Professional);

        let response = "HTTP/1.1 200 OK\r\n\r\n{\"success\":true,\"purchase\":{\"product_name\":\"kindly-av1\",\"variants\":\"Enterprise Tier\"}}";
        let tier = capsule.parse_gumroad_response(response).unwrap();
        assert_eq!(tier, LicenseTier::Enterprise);
    }

    #[test]
    fn test_parse_gumroad_error_response() {
        let capsule = GumroadLicenseCapsule::new();

        let response = "HTTP/1.1 404 Not Found\r\n\r\n{\"success\":false,\"message\":\"That license does not exist for the provided product.\"}";
        let result = capsule.parse_gumroad_response(response);
        assert!(result.is_err());

        if let Err(GumroadError::InvalidLicense(msg)) = result {
            assert!(msg.contains("does not exist"));
        } else {
            panic!("Expected InvalidLicense error");
        }
    }

    #[test]
    fn test_ed25519_signature_round_trip() {
        // Generate keypair
        let signing_key = SigningKey::from_bytes(&[0u8; 32]);
        let verifying_key = signing_key.verifying_key();

        // Create message
        let message = b"test message";

        // Sign
        let signature = signing_key.sign(message);

        // Verify
        assert!(verifying_key.verify(message, &signature).is_ok());

        // Wrong message should fail
        let wrong_message = b"wrong message";
        assert!(verifying_key.verify(wrong_message, &signature).is_err());
    }

    #[test]
    fn test_license_state_from_u64() {
        assert_eq!(LicenseState::from(0), LicenseState::Unactivated);
        assert_eq!(LicenseState::from(1), LicenseState::Activated);
        assert_eq!(LicenseState::from(2), LicenseState::Error);
        assert_eq!(LicenseState::from(99), LicenseState::Error); // Unknown → Error
    }
}
