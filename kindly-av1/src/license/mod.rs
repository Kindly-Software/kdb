//! Kindly-AV1 License Verification Module
//! [TRADE SECRET] - Proprietary anti-piracy system
//!
//! This module implements capsule-based license verification that is
//! tamper-resistant due to integration with the metacapsule orchestration.
//!
//! # Architecture
//!
//! The license capsule is wired into the metacapsule orchestration - the encoder
//! literally cannot run without valid license state. This makes it extremely
//! difficult to bypass.
//!
//! ## Anti-Piracy Mechanisms
//!
//! 1. **Capsule Integration**: License state is checked by metacapsule before
//!    encoding starts - not a simple boolean check but atomic state verification
//! 2. **Generation Counters**: Binary patches that modify state break the counter
//!    chain, causing integrity verification to fail
//! 3. **Hardware Binding**: Key tied to specific machine's CPU+MAC hash
//! 4. **Signature Verification**: Stored hash prevents key modification
//!
//! # Framework Compliance
//!
//! - UCE34: Q10 T1 Atomic + T0 Auditable
//! - Chaos: 128B cache-aligned, generation counters, AtomicU64 state
//! - ASSUM: 99.5%+ safe, all assumptions documented
//!
//! # License File Locations
//!
//! - Linux: `~/.config/kindly-av1/license.bin`
//! - macOS: `~/Library/Application Support/kindly-av1/license.bin`
//! - Windows: `%APPDATA%\kindly-av1\license.bin`

mod capsule;
mod device_rotation;
mod email_registration;
mod fingerprint;
mod gumroad;
mod key;
mod tier_enforcement;

pub use capsule::{LicenseError, LicenseState, LicenseVerificationCapsule};
pub use device_rotation::{DeviceError, DeviceRotationCapsule, MAX_DEVICES};
pub use email_registration::{EmailError, EmailRegistrationCapsule};
pub use fingerprint::HardwareFingerprint;
pub use gumroad::{GumroadError, GumroadLicenseCapsule};
pub use key::{LicenseKey, LicenseKeyError};
pub use tier_enforcement::{LicenseTier, TierEnforcementCapsule, TierError};

/// License configuration constants
pub mod config {
    /// Product identifier for kindly-av1
    pub const PRODUCT_ID: &[u8; 4] = b"KAV1";

    /// License file name
    pub const LICENSE_FILENAME: &str = "license.bin";

    /// Application name for config directory
    pub const APP_NAME: &str = "kindly-av1";

    /// Magic bytes for license file validation
    pub const LICENSE_MAGIC: [u8; 4] = [0x4B, 0x44, 0x4C, 0x59]; // "KDLY"

    /// License file version
    pub const LICENSE_VERSION: u8 = 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all public types are accessible
        let _state: LicenseState = LicenseState::Invalid;
        let _capsule = LicenseVerificationCapsule::new();
        let _fingerprint = HardwareFingerprint::generate();
        let _tier_capsule = TierEnforcementCapsule::new();
        let _device_capsule = DeviceRotationCapsule::new();
        let _tier = LicenseTier::Creator;
        let _gumroad_capsule = GumroadLicenseCapsule::new();
        let _email_capsule = EmailRegistrationCapsule::new();
    }

    #[test]
    fn test_config_constants() {
        assert_eq!(config::PRODUCT_ID, b"KAV1");
        assert_eq!(config::LICENSE_MAGIC, [0x4B, 0x44, 0x4C, 0x59]);
        assert_eq!(config::LICENSE_VERSION, 1);
    }

    #[test]
    fn test_tier_structure() {
        // Verify tier structure matches spec
        assert_eq!(LicenseTier::AnonymousFree.max_width(), 640);
        assert_eq!(LicenseTier::RegisteredFree.max_width(), 1280);
        assert_eq!(LicenseTier::Creator.max_width(), 1920);
        assert_eq!(LicenseTier::Professional.max_width(), 3840);
        assert_eq!(LicenseTier::Enterprise.max_width(), 7680);

        assert_eq!(LicenseTier::AnonymousFree.device_limit(), 1);
        assert_eq!(LicenseTier::RegisteredFree.device_limit(), 1);
        assert_eq!(LicenseTier::Creator.device_limit(), 2);
        assert_eq!(LicenseTier::Professional.device_limit(), 3);
        assert_eq!(LicenseTier::Enterprise.device_limit(), 5);
    }

    #[test]
    fn test_max_devices_constant() {
        assert_eq!(MAX_DEVICES, 5);
    }
}
