use std::sync::atomic::AtomicU64;

#[repr(C, align(128))]
struct DualAtomicU64 {
    primary: AtomicU64,
    _padding1: [u8; 56],
    secondary: AtomicU64,
    _padding2: [u8; 56],
}

fn main() {
    println!(
        "DualAtomicU64 size: {}",
        std::mem::size_of::<DualAtomicU64>()
    );
    println!("AtomicU64 size: {}", std::mem::size_of::<AtomicU64>());

    // Precommit: AtomicU64(8) + AtomicU64(8) + AtomicU64(8) + AtomicU64(8) + DualAtomicU64(128) = 160 bytes
    // With align(128), size rounds to 256
    println!("\nPrecommitGuardCapsule:");
    println!("  Fields: 8+8+8+8+128 = 160 bytes");
    println!("  Aligned to 128: rounds to 256 bytes");

    // Backup: DualAtomicU64(128) + AtomicU64(8)*6 + DualAtomicU64(128) = 304 bytes
    // With align(256), size rounds to 512
    println!("\nBackupCoordinatorCapsule:");
    println!("  Fields: 128+8+8+8+8+8+8+128 = 304 bytes");
    println!("  Aligned to 256: rounds to 512 bytes");

    // Audit: AtomicHash64(8) + AtomicU64(8)*3 + DualAtomicU64(128) = 160 bytes
    // With align(256), size rounds to 256
    println!("\nAuditTrailCapsule:");
    println!("  Fields: 8+8+8+128+8 = 160 bytes");
    println!("  Aligned to 256: size = 256 bytes");
}
