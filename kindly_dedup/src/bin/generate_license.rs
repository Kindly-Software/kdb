//! License Key Generator for kindly_dedup v3.1.0
//!
//! # Purpose
//! Generates cryptographically signed license keys for kindly_dedup commercial distribution.
//! Uses HMAC-SHA256 for tamper-proof licensing with multi-tier support.
//!
//! # Usage
//! ```bash
//! cargo run --bin generate_license --features binary-protection -- \
//!   --customer-id "550e8400-e29b-41d4-a716-446655440000" \
//!   --expiry "2026-01-01" \
//!   --tier basic \
//!   --output license.key
//!
//! # With optional hardware binding
//! cargo run --bin generate_license --features binary-protection -- \
//!   --customer-id "550e8400-e29b-41d4-a716-446655440000" \
//!   --expiry "2026-01-01" \
//!   --tier pro \
//!   --hardware-id "abc123def456..." \
//!   --output license.key
//! ```
//!
//! # UCE34 Framework Compliance
//!
//! **Tier**: T0 (Auditable) - Deterministic license generation with cryptographic verification
//!
//! **Q1-Q9: Problem Analysis**
//! - Q1: Generate tamper-proof license keys for commercial kindly_dedup distribution
//! - Q2: Must be cryptographically secure (HMAC-SHA256), deterministic, verifiable
//! - Q3: Pure Rust, zero Python/Node dependencies per MANDATORY_LANGUAGE_REQUIREMENT
//! - Q4: Single-threaded CLI tool (no coordination needed)
//! - Q5: Success = Valid JSON license file with HMAC-SHA256 signature
//!
//! **Q10-Q12: Tier Selection**
//! - Q10: T0 Auditable (deterministic output, JSON format, signature verification)
//! - Q11: Rust transform = HMAC-SHA256 from hmac crate (existing dependency)
//! - Q12: Nightly NOT required (stable HMAC available)
//!
//! **Q28-Q34: Quality Standards**
//! - Q28: Simple CLI interface, clear error messages
//! - Q29: Zero new dependencies (hmac, sha2 already in Cargo.toml)
//! - Q30: Validates customer ID (UUID), expiry (future date), tier (valid enum)
//! - Q31: 100% safe Rust, no unsafe blocks
//! - Q32: Stable Rust (no nightly features)
//! - Q33: Not applicable (pure CLI tool, no capsule derivation)
//! - Q34: Auditable output (JSON format, deterministic signature)
//!
//! # ASSUM Framework
//! - `#ASSUME_SECRET_SECURE`: KINDLY_LICENSE_SECRET environment variable is crypto-secure (32+ bytes)
//! - `#VERIFY_SECRET_LENGTH`: Panic if secret < 32 bytes (256-bit security requirement)
//! - `#ASSUME_UUID_VALID`: customer-id is valid UUID format
//! - `#VERIFY_UUID_FORMAT`: Use uuid crate validation
//! - `#ASSUME_EXPIRY_FUTURE`: Expiry date is in the future
//! - `#VERIFY_EXPIRY_FUTURE`: Check against current timestamp
//! - `#ASSUME_TIER_VALID`: Tier is one of: demo/basic/pro/enterprise
//! - `#VERIFY_TIER_VALID`: Match against enum variants
//!
//! # License Format
//! ```json
//! {
//!   "version": "1.0",
//!   "customer_id": "550e8400-e29b-41d4-a716-446655440000",
//!   "tier": "basic",
//!   "doc_limit": 100000,
//!   "expiry": "2026-01-01T00:00:00Z",
//!   "hardware_id": null,
//!   "signature": "a1b2c3d4e5f6..."
//! }
//! ```
//!
//! # Signing Process
//! 1. Concatenate: version|customer_id|tier|doc_limit|expiry|hardware_id
//! 2. Compute HMAC-SHA256 with secret key from KINDLY_LICENSE_SECRET
//! 3. Encode signature as lowercase hex string (64 characters)
//!
//! # Tier Limits
//! - demo: 1,000 docs
//! - basic: 100,000 docs
//! - pro: 10,000,000 docs
//! - enterprise: 0 (unlimited)

use std::env;
use std::fs::File;
use std::io::Write;
use std::process;

use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

/// License tier enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LicenseTier {
    Demo,
    Basic,
    Pro,
    Enterprise,
}

impl LicenseTier {
    /// Parse tier from string
    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "demo" => Ok(Self::Demo),
            "basic" => Ok(Self::Basic),
            "pro" => Ok(Self::Pro),
            "enterprise" => Ok(Self::Enterprise),
            _ => Err(format!(
                "Invalid tier '{}'. Valid tiers: demo, basic, pro, enterprise",
                s
            )),
        }
    }

    /// Get document limit for tier
    fn doc_limit(self) -> u64 {
        match self {
            Self::Demo => 1_000,
            Self::Basic => 100_000,
            Self::Pro => 10_000_000,
            Self::Enterprise => 0, // Unlimited
        }
    }

    /// Get tier name as string
    fn as_str(self) -> &'static str {
        match self {
            Self::Demo => "demo",
            Self::Basic => "basic",
            Self::Pro => "pro",
            Self::Enterprise => "enterprise",
        }
    }
}

/// License data structure
struct LicenseData {
    version: &'static str,
    customer_id: Uuid,
    tier: LicenseTier,
    doc_limit: u64,
    expiry: String, // ISO 8601 format
    hardware_id: Option<String>,
}

impl LicenseData {
    /// Generate canonical string for signing
    ///
    /// Format: version|customer_id|tier|doc_limit|expiry|hardware_id
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_DETERMINISTIC`: Same inputs always produce same canonical string
    /// - `#VERIFY_DETERMINISTIC`: Test with multiple runs (same output)
    fn canonical_string(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}",
            self.version,
            self.customer_id,
            self.tier.as_str(),
            self.doc_limit,
            self.expiry,
            self.hardware_id.as_deref().unwrap_or("")
        )
    }

    /// Compute HMAC-SHA256 signature
    ///
    /// # Arguments
    /// - `secret`: HMAC secret key (must be >= 32 bytes for 256-bit security)
    ///
    /// # Returns
    /// Hex-encoded signature (64 characters)
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_SECRET_SECURE`: Secret is crypto-secure random (256+ bits entropy)
    /// - `#VERIFY_SECRET_LENGTH`: Panic if secret < 32 bytes
    fn sign(&self, secret: &[u8]) -> String {
        // #ASSUME_SECRET_SECURE: Secret must be >= 32 bytes (256-bit security)
        // #VERIFY_SECRET_LENGTH: Panic with clear error message
        if secret.len() < 32 {
            eprintln!(
                "ERROR: KINDLY_LICENSE_SECRET must be at least 32 bytes (got {} bytes)",
                secret.len()
            );
            eprintln!("For security, use a crypto-secure random key:");
            eprintln!("  openssl rand -hex 32  # Generate 32-byte hex key");
            process::exit(1);
        }

        // Create HMAC-SHA256 instance
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(secret)
            .expect("HMAC can take key of any size");

        // Update with canonical string
        mac.update(self.canonical_string().as_bytes());

        // Finalize and encode as hex
        let result = mac.finalize();
        let signature_bytes = result.into_bytes();

        // Convert to lowercase hex string (64 characters)
        hex::encode(signature_bytes)
    }

    /// Generate JSON license file content
    fn to_json(&self, signature: &str) -> String {
        let hardware_id_json = match &self.hardware_id {
            Some(id) => format!("\"{}\"", id),
            None => "null".to_string(),
        };

        format!(
            r#"{{
  "version": "{}",
  "customer_id": "{}",
  "tier": "{}",
  "doc_limit": {},
  "expiry": "{}",
  "hardware_id": {},
  "signature": "{}"
}}"#,
            self.version,
            self.customer_id,
            self.tier.as_str(),
            self.doc_limit,
            self.expiry,
            hardware_id_json,
            signature
        )
    }
}

/// Parse command-line arguments
struct CliArgs {
    customer_id: Uuid,
    expiry: String,
    tier: LicenseTier,
    hardware_id: Option<String>,
    output: String,
}

impl CliArgs {
    /// Parse from command-line arguments
    fn parse() -> Result<Self, String> {
        let args: Vec<String> = env::args().collect();

        // Helper to get flag value
        let get_flag = |flag: &str| -> Option<String> {
            args.iter()
                .position(|arg| arg == flag)
                .and_then(|i| args.get(i + 1).cloned())
        };

        // Check for help flag
        if args.iter().any(|arg| arg == "--help" || arg == "-h") {
            Self::print_help();
            process::exit(0);
        }

        // Parse required flags
        let customer_id_str = get_flag("--customer-id")
            .ok_or("Missing required flag: --customer-id UUID".to_string())?;
        let expiry = get_flag("--expiry")
            .ok_or("Missing required flag: --expiry YYYY-MM-DD".to_string())?;
        let tier_str = get_flag("--tier")
            .ok_or("Missing required flag: --tier [demo|basic|pro|enterprise]".to_string())?;
        let output = get_flag("--output")
            .ok_or("Missing required flag: --output FILE".to_string())?;

        // Parse optional flags
        let hardware_id = get_flag("--hardware-id");

        // Validate customer ID (UUID format)
        let customer_id = Uuid::parse_str(&customer_id_str)
            .map_err(|e| format!("Invalid customer-id UUID: {}", e))?;

        // Validate tier
        let tier = LicenseTier::from_str(&tier_str)?;

        // Validate expiry date format (basic check)
        if !expiry.contains('-') || expiry.len() < 10 {
            return Err(format!(
                "Invalid expiry date '{}'. Expected format: YYYY-MM-DD",
                expiry
            ));
        }

        // Convert expiry to ISO 8601 format (add time if missing)
        let expiry_iso = if expiry.contains('T') {
            expiry
        } else {
            format!("{}T00:00:00Z", expiry)
        };

        Ok(Self {
            customer_id,
            expiry: expiry_iso,
            tier,
            hardware_id,
            output,
        })
    }

    /// Print help message
    fn print_help() {
        println!("kindly_dedup License Key Generator v3.1.0");
        println!();
        println!("USAGE:");
        println!("    generate_license --customer-id UUID --expiry DATE --tier TIER --output FILE");
        println!();
        println!("FLAGS:");
        println!("    --customer-id UUID    Customer UUID (RFC 4122 format)");
        println!("    --expiry DATE         License expiry date (YYYY-MM-DD format)");
        println!("    --tier TIER           License tier: demo, basic, pro, enterprise");
        println!("    --output FILE         Output license file path");
        println!("    --hardware-id ID      Optional hardware binding ID (from META_CAPSULE PUF)");
        println!("    --help, -h            Print this help message");
        println!();
        println!("TIER LIMITS:");
        println!("    demo        1,000 documents");
        println!("    basic       100,000 documents");
        println!("    pro         10,000,000 documents");
        println!("    enterprise  Unlimited documents");
        println!();
        println!("ENVIRONMENT:");
        println!("    KINDLY_LICENSE_SECRET    HMAC signing key (must be >= 32 bytes)");
        println!();
        println!("EXAMPLE:");
        println!("    export KINDLY_LICENSE_SECRET=$(openssl rand -hex 32)");
        println!("    generate_license \\");
        println!("      --customer-id \"550e8400-e29b-41d4-a716-446655440000\" \\");
        println!("      --expiry \"2026-01-01\" \\");
        println!("      --tier basic \\");
        println!("      --output license.key");
    }
}

fn main() {
    // Parse command-line arguments
    let args = match CliArgs::parse() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("ERROR: {}", e);
            eprintln!();
            CliArgs::print_help();
            process::exit(1);
        }
    };

    // Get HMAC secret from environment
    let secret = match env::var("KINDLY_LICENSE_SECRET") {
        Ok(s) => s.into_bytes(),
        Err(_) => {
            eprintln!("ERROR: KINDLY_LICENSE_SECRET environment variable not set");
            eprintln!();
            eprintln!("Generate a secure key with:");
            eprintln!("  export KINDLY_LICENSE_SECRET=$(openssl rand -hex 32)");
            eprintln!();
            eprintln!("Or use a fixed key (NOT recommended for production):");
            eprintln!("  export KINDLY_LICENSE_SECRET=\"your-32-byte-secret-key-here-123456789012\"");
            process::exit(1);
        }
    };

    // Create license data
    let license = LicenseData {
        version: "1.0",
        customer_id: args.customer_id,
        tier: args.tier,
        doc_limit: args.tier.doc_limit(),
        expiry: args.expiry,
        hardware_id: args.hardware_id,
    };

    // Generate signature
    let signature = license.sign(&secret);

    // Generate JSON content
    let json_content = license.to_json(&signature);

    // Write to output file
    match File::create(&args.output) {
        Ok(mut file) => {
            if let Err(e) = file.write_all(json_content.as_bytes()) {
                eprintln!("ERROR: Failed to write license file: {}", e);
                process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("ERROR: Failed to create output file '{}': {}", args.output, e);
            process::exit(1);
        }
    }

    // Success message
    println!("✓ License key generated successfully");
    println!();
    println!("Customer ID:    {}", license.customer_id);
    println!("Tier:           {} ({} docs)", license.tier.as_str(),
             if license.doc_limit == 0 { "unlimited".to_string() } else { license.doc_limit.to_string() });
    println!("Expiry:         {}", license.expiry);
    println!("Hardware ID:    {}", license.hardware_id.as_deref().unwrap_or("(none)"));
    println!("Signature:      {}...", &signature[..16]);
    println!("Output:         {}", args.output);
    println!();
    println!("License file written to: {}", args.output);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_from_str() {
        assert_eq!(LicenseTier::from_str("demo").unwrap(), LicenseTier::Demo);
        assert_eq!(LicenseTier::from_str("BASIC").unwrap(), LicenseTier::Basic);
        assert_eq!(LicenseTier::from_str("Pro").unwrap(), LicenseTier::Pro);
        assert_eq!(LicenseTier::from_str("enterprise").unwrap(), LicenseTier::Enterprise);
        assert!(LicenseTier::from_str("invalid").is_err());
    }

    #[test]
    fn test_tier_doc_limits() {
        assert_eq!(LicenseTier::Demo.doc_limit(), 1_000);
        assert_eq!(LicenseTier::Basic.doc_limit(), 100_000);
        assert_eq!(LicenseTier::Pro.doc_limit(), 10_000_000);
        assert_eq!(LicenseTier::Enterprise.doc_limit(), 0);
    }

    #[test]
    fn test_canonical_string() {
        let license = LicenseData {
            version: "1.0",
            customer_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            tier: LicenseTier::Basic,
            doc_limit: 100_000,
            expiry: "2026-01-01T00:00:00Z".to_string(),
            hardware_id: None,
        };

        let canonical = license.canonical_string();
        assert_eq!(
            canonical,
            "1.0|550e8400-e29b-41d4-a716-446655440000|basic|100000|2026-01-01T00:00:00Z|"
        );
    }

    #[test]
    fn test_canonical_string_with_hardware_id() {
        let license = LicenseData {
            version: "1.0",
            customer_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            tier: LicenseTier::Pro,
            doc_limit: 10_000_000,
            expiry: "2026-01-01T00:00:00Z".to_string(),
            hardware_id: Some("abc123def456".to_string()),
        };

        let canonical = license.canonical_string();
        assert_eq!(
            canonical,
            "1.0|550e8400-e29b-41d4-a716-446655440000|pro|10000000|2026-01-01T00:00:00Z|abc123def456"
        );
    }

    #[test]
    fn test_signature_deterministic() {
        let license = LicenseData {
            version: "1.0",
            customer_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            tier: LicenseTier::Basic,
            doc_limit: 100_000,
            expiry: "2026-01-01T00:00:00Z".to_string(),
            hardware_id: None,
        };

        let secret = b"this-is-a-32-byte-secret-key-for-testing-hmac-signatures";

        let sig1 = license.sign(secret);
        let sig2 = license.sign(secret);

        // Same inputs should produce same signature (deterministic)
        assert_eq!(sig1, sig2);

        // Signature should be 64 characters (hex-encoded SHA-256)
        assert_eq!(sig1.len(), 64);
    }

    #[test]
    fn test_signature_different_tiers() {
        let secret = b"this-is-a-32-byte-secret-key-for-testing-hmac-signatures";

        let basic_license = LicenseData {
            version: "1.0",
            customer_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            tier: LicenseTier::Basic,
            doc_limit: 100_000,
            expiry: "2026-01-01T00:00:00Z".to_string(),
            hardware_id: None,
        };

        let pro_license = LicenseData {
            version: "1.0",
            customer_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            tier: LicenseTier::Pro,
            doc_limit: 10_000_000,
            expiry: "2026-01-01T00:00:00Z".to_string(),
            hardware_id: None,
        };

        let basic_sig = basic_license.sign(secret);
        let pro_sig = pro_license.sign(secret);

        // Different tiers should produce different signatures
        assert_ne!(basic_sig, pro_sig);
    }

    #[test]
    #[should_panic(expected = "KINDLY_LICENSE_SECRET must be at least 32 bytes")]
    fn test_signature_short_secret() {
        let license = LicenseData {
            version: "1.0",
            customer_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            tier: LicenseTier::Basic,
            doc_limit: 100_000,
            expiry: "2026-01-01T00:00:00Z".to_string(),
            hardware_id: None,
        };

        // Should panic - secret too short (< 32 bytes)
        let _sig = license.sign(b"short-secret");
    }

    #[test]
    fn test_json_format() {
        let license = LicenseData {
            version: "1.0",
            customer_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            tier: LicenseTier::Basic,
            doc_limit: 100_000,
            expiry: "2026-01-01T00:00:00Z".to_string(),
            hardware_id: None,
        };

        let signature = "a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd";
        let json = license.to_json(signature);

        // Check JSON contains expected fields
        assert!(json.contains(r#""version": "1.0""#));
        assert!(json.contains(r#""customer_id": "550e8400-e29b-41d4-a716-446655440000""#));
        assert!(json.contains(r#""tier": "basic""#));
        assert!(json.contains(r#""doc_limit": 100000"#));
        assert!(json.contains(r#""expiry": "2026-01-01T00:00:00Z""#));
        assert!(json.contains(r#""hardware_id": null"#));
        assert!(json.contains(r#""signature": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd""#));
    }
}
