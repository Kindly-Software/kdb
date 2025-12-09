//! License Management CLI Commands
//! [TRADE SECRET]
//!
//! Command-line interface for Gumroad license activation and management.
//!
//! # Commands
//!
//! - `kindly-av1 license activate <KEY>` - Activate license online
//! - `kindly-av1 license status` - Show current license status
//! - `kindly-av1 license deactivate` - Remove license activation
//!
//! # Architecture
//!
//! Pure functional design - no global state, all dependencies passed explicitly.
//!
//! # Chaos Compliance
//!
//! - UCE34 Q33: Lockfree command execution
//! - No mutex, no RwLock
//! - Explicit error handling

use std::io::{self, Write};

use super::branding::{self, ColorConfig};
use crate::license::{GumroadError, GumroadLicenseCapsule, HardwareFingerprint, TierEnforcementCapsule};

/// License command errors
#[derive(Debug)]
pub enum LicenseCommandError {
    /// Gumroad error
    Gumroad(GumroadError),
    /// IO error
    Io(io::Error),
    /// Invalid license key format
    InvalidKeyFormat,
}

impl std::fmt::Display for LicenseCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gumroad(e) => write!(f, "License error: {}", e),
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::InvalidKeyFormat => write!(f, "Invalid license key format (expected: XXXXX-XXXXX-XXXXX-XXXXX)"),
        }
    }
}

impl std::error::Error for LicenseCommandError {}

impl From<GumroadError> for LicenseCommandError {
    fn from(e: GumroadError) -> Self {
        Self::Gumroad(e)
    }
}

impl From<io::Error> for LicenseCommandError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// License command result
pub type LicenseCommandResult<T> = Result<T, LicenseCommandError>;

/// Activate license online with Gumroad API
///
/// # Workflow
///
/// 1. Validate license key format
/// 2. Generate hardware fingerprint
/// 3. Verify with Gumroad API
/// 4. Generate Ed25519 signature
/// 5. Store signed license locally
/// 6. Update tier enforcement capsule
///
/// # Arguments
///
/// - `license_key`: License key (XXXXX-XXXXX-XXXXX-XXXXX format)
/// - `color_config`: Color configuration for output
pub fn cmd_license_activate(
    license_key: &str,
    color_config: &ColorConfig,
) -> LicenseCommandResult<()> {
    // Print header
    println!("\n=== License Activation ===\n");

    // Validate license key format
    if !validate_key_format(license_key) {
        return Err(LicenseCommandError::InvalidKeyFormat);
    }

    // Generate hardware fingerprint
    print!("Generating hardware fingerprint... ");
    io::stdout().flush()?;
    let fingerprint = HardwareFingerprint::generate();
    println!("✓");

    // Create Gumroad capsule
    let mut gumroad = GumroadLicenseCapsule::new();

    // Activate online
    print!("Verifying license with Gumroad... ");
    io::stdout().flush()?;
    let tier = gumroad.activate_online(license_key, &fingerprint)?;
    println!("✓");

    // Update tier enforcement
    print!("Activating {} tier... ", tier.name());
    io::stdout().flush()?;
    let tier_enforcement = TierEnforcementCapsule::new();
    tier_enforcement.activate(tier).map_err(|e| {
        GumroadError::InvalidResponse(format!("Failed to activate tier: {}", e))
    })?;
    println!("✓");

    // Success
    branding::print_success(
        &format!(
            "License activated successfully!\n  Tier: {}\n  Max Resolution: {}p\n  Device Limit: {}",
            tier.name(),
            tier.max_width(),
            tier.device_limit()
        ),
    );

    Ok(())
}

/// Show current license status
///
/// # Workflow
///
/// 1. Load stored license from disk
/// 2. Verify Ed25519 signature offline
/// 3. Check device fingerprint
/// 4. Display license details
///
/// # Arguments
///
/// - `color_config`: Color configuration for output
pub fn cmd_license_status(color_config: &ColorConfig) -> LicenseCommandResult<()> {
    // Print header
    println!("\n=== License Status ===\n");

    // Create Gumroad capsule
    let gumroad = GumroadLicenseCapsule::new();

    // Generate hardware fingerprint
    let fingerprint = HardwareFingerprint::generate();

    // Verify offline
    match gumroad.verify_offline(&fingerprint) {
        Ok(tier) => {
            // License valid
            println!("Status: ✓ Active");
            println!("Tier: {}", tier.name());
            println!("Max Resolution: {}p", tier.max_width());
            println!("Device Limit: {} devices", tier.device_limit());

            // Create tier enforcement with verified tier (not default)
            // #ASSUME: verify_offline returns the persisted tier from license file
            // #VERIFY: Tier matches what was stored at activation time
            let tier_enforcement = TierEnforcementCapsule::with_tier(tier);
            println!(
                "Current Device Count: {}/{}",
                tier_enforcement.device_count(),
                tier_enforcement.device_limit()
            );
        }
        Err(GumroadError::IoError(_)) => {
            // No license file
            branding::print_error("No license activated");
            println!("\nActivate a license with:");
            println!("  kindly-av1 license activate <LICENSE_KEY>");
        }
        Err(e) => {
            // License error
            branding::print_error(&format!("License validation failed: {}", e));
            println!("\nYour license may be:");
            println!("  - Tampered with (signature verification failed)");
            println!("  - Moved to a different machine (device mismatch)");
            println!("  - Expired (subscription ended)");
            println!("\nDeactivate and re-activate:");
            println!("  kindly-av1 license deactivate");
            println!("  kindly-av1 license activate <LICENSE_KEY>");
        }
    }

    Ok(())
}

/// Deactivate license (remove stored license)
///
/// # Workflow
///
/// 1. Remove stored license file
/// 2. Reset tier enforcement to anonymous free
///
/// # Arguments
///
/// - `color_config`: Color configuration for output
pub fn cmd_license_deactivate(color_config: &ColorConfig) -> LicenseCommandResult<()> {
    // Print header
    println!("\n=== License Deactivation ===\n");

    // Create Gumroad capsule
    let mut gumroad = GumroadLicenseCapsule::new();

    // Deactivate
    print!("Removing license... ");
    io::stdout().flush()?;
    gumroad.deactivate()?;
    println!("✓");

    // Reset tier enforcement (TierEnforcementCapsule::new() already defaults to AnonymousFree)
    print!("Resetting to anonymous free tier... ");
    io::stdout().flush()?;
    // Note: TierEnforcementCapsule::new() defaults to AnonymousFree, no need to activate
    let _tier_enforcement = TierEnforcementCapsule::new();
    println!("✓");

    // Success
    branding::print_success("License deactivated successfully!");
    println!("\nYou can activate a license again with:");
    println!("  kindly-av1 license activate <LICENSE_KEY>");

    Ok(())
}

/// Validate license key format
///
/// Expected format: KDLY-XXXX-XXXX-XXXX-XXXX (5 segments)
/// where first segment is "KDLY" prefix and remaining are 4 chars each
///
/// Delegates to LicenseKey::parse() for authoritative validation.
fn validate_key_format(key: &str) -> bool {
    use crate::license::LicenseKey;
    LicenseKey::parse(key).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_key_format() {
        // The canonical format is KDLY-XXXX-XXXX-XXXX-XXXX (5 segments, 20 chars)
        // First segment must be "KDLY", remaining 4 segments are 4 chars each
        // Characters must be valid Crockford Base32 (0-9, A-Z excluding I, L, O, U)

        // Generate a valid test key using the LicenseKey API
        // This ensures we test against the actual canonical format
        let fingerprint = crate::license::HardwareFingerprint::from_bytes([0xAA; 32]);
        let key = crate::license::LicenseKey::generate(&fingerprint, *b"KAV1", 0, [0x12, 0x34, 0x56]);
        assert!(validate_key_format(key.raw()), "Generated key should be valid");

        // Valid keys (KDLY prefix, 5 segments, valid Base32 chars)
        // Note: These may not pass checksum validation, but format should be valid
        // The validate_key_format delegates to LicenseKey::parse which validates checksum

        // Invalid format tests
        assert!(!validate_key_format("ABCD-1234-5678-9012-3456")); // Wrong prefix (not KDLY)
        assert!(!validate_key_format("KDLY-1234")); // Too short (only 2 segments)
        assert!(!validate_key_format("KDLY-12345-FGHIJ-67890-ABCD")); // Segment too long
        assert!(!validate_key_format("KDLY-123-5678-9012-3456")); // Segment too short
        assert!(!validate_key_format("XXXX-1234-5678-9012-3456")); // Wrong prefix
        assert!(!validate_key_format("")); // Empty
        assert!(!validate_key_format("KDLY12345678901234567890")); // No dashes but wrong length
    }
}
