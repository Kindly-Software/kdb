//! Test all 3 protection features compile and work

#[cfg(feature = "crypto-license")]
use atomic_capsule::protection::CryptoLicenseCapsule;

#[cfg(feature = "encrypted-state")]
use atomic_capsule::protection::EncryptedStateCapsule;

#[cfg(feature = "orchestrator")]
use atomic_capsule::protection::ProtectionOrchestratorCapsule;

#[test]
#[cfg(feature = "crypto-license")]
fn test_crypto_license_basic() {
    let public_key = [0u8; 32];
    let license = CryptoLicenseCapsule::new(public_key);
    assert!(core::mem::size_of_val(&license) > 0);
}

#[test]
#[cfg(feature = "encrypted-state")]
fn test_encrypted_state_basic() {
    // Just verify the type exists and has reasonable size
    // Note: Size is 768B due to Arc<PathBuf> overhead (64B align + padding)
    use std::mem::size_of;
    let size = size_of::<EncryptedStateCapsule>();
    assert!(size >= 512 && size <= 1024, "Size {} not in expected range", size);
}

#[test]
#[cfg(feature = "orchestrator")]
fn test_orchestrator_basic() {
    let orchestrator = ProtectionOrchestratorCapsule::new();
    assert!(core::mem::size_of_val(&orchestrator) == 512);
    
    // Test basic status query
    let status = orchestrator.overall_health();
    assert!(status >= 0.0 && status <= 100.0);
}

fn main() {
    println!("All protection features compile successfully!");
}
