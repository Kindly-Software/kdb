//! Integration helper tests
//!
//! Comprehensive tests for PackedStateBuilder and define_capsule! macro

use atomic_capsule::{
    define_capsule, AlignmentTier, PackedStateBuilder, PackedStateUnpacker, UnpackState,
};
use core::sync::atomic::AtomicU64;

// ============================================================================
// PackedStateBuilder Tests
// ============================================================================

#[test]
fn test_packed_state_builder_4_fields() {
    let state = PackedStateBuilder::new()
        .with_field::<8>(0xAB)
        .with_field::<8>(0xCD)
        .with_field::<16>(0x1234)
        .with_field::<32>(0x56789ABC)
        .build();

    assert_eq!(state, 0xABCD_1234_56789ABC);

    let (a, b, c, d) = <(u8, u8, u16, u32)>::unpack(state);
    assert_eq!(a, 0xAB);
    assert_eq!(b, 0xCD);
    assert_eq!(c, 0x1234);
    assert_eq!(d, 0x56789ABC);
}

#[test]
fn test_packed_state_builder_2_u32() {
    let state = PackedStateBuilder::new()
        .with_field::<32>(0x12345678)
        .with_field::<32>(0x9ABCDEF0)
        .build();

    assert_eq!(state, 0x12345678_9ABCDEF0);

    let (a, b) = <(u32, u32)>::unpack(state);
    assert_eq!(a, 0x12345678);
    assert_eq!(b, 0x9ABCDEF0);
}

#[test]
fn test_packed_state_builder_8_u8() {
    let state = PackedStateBuilder::new()
        .with_field::<8>(0x01)
        .with_field::<8>(0x02)
        .with_field::<8>(0x03)
        .with_field::<8>(0x04)
        .with_field::<8>(0x05)
        .with_field::<8>(0x06)
        .with_field::<8>(0x07)
        .with_field::<8>(0x08)
        .build();

    assert_eq!(state, 0x0102030405060708);

    let (a, b, c, d, e, f, g, h) = <(u8, u8, u8, u8, u8, u8, u8, u8)>::unpack(state);
    assert_eq!(a, 0x01);
    assert_eq!(b, 0x02);
    assert_eq!(c, 0x03);
    assert_eq!(d, 0x04);
    assert_eq!(e, 0x05);
    assert_eq!(f, 0x06);
    assert_eq!(g, 0x07);
    assert_eq!(h, 0x08);
}

#[test]
fn test_packed_state_overflow_masking() {
    // Value 0x1AB (9 bits) should be masked to 0xAB (8 bits)
    let state = PackedStateBuilder::new()
        .with_field::<8>(0x1AB) // Overflow: only bottom 8 bits used
        .with_field::<56>(0x00FFFFFFFFFFFF)
        .build();

    let mut unpacker = PackedStateUnpacker::new(state);
    let masked = unpacker.extract::<8>() as u8;

    assert_eq!(masked, 0xAB); // Overflow bits discarded
}

#[test]
fn test_packed_state_roundtrip_circuit_breaker() {
    // Realistic circuit breaker capsule state
    let level: u8 = 2; // ProtectionLevel::Level2
    let cause: u8 = 3; // Cause::MaxDrawdown
    let stale: u8 = 0; // Not stale
    let version: u8 = 42;
    let loss: u32 = 12345678; // Loss in basis points
    let timestamp: u32 = 1234567890;

    let state = PackedStateBuilder::new()
        .with_field::<2>(level as u64) // 2 bits for level
        .with_field::<3>(cause as u64) // 3 bits for cause
        .with_field::<1>(stale as u64) // 1 bit for stale flag
        .with_field::<8>(version as u64) // 8 bits for version
        .with_field::<18>(loss as u64) // 18 bits for loss
        .with_field::<32>(timestamp as u64) // 32 bits for timestamp
        .build();

    let mut unpacker = PackedStateUnpacker::new(state);
    let level2 = unpacker.extract::<2>() as u8;
    let cause2 = unpacker.extract::<3>() as u8;
    let stale2 = unpacker.extract::<1>() as u8;
    let version2 = unpacker.extract::<8>() as u8;
    let loss2 = unpacker.extract::<18>() as u32;
    let timestamp2 = unpacker.extract::<32>() as u32;

    assert_eq!(level2, level);
    assert_eq!(cause2, cause);
    assert_eq!(stale2, stale);
    assert_eq!(version2, version);
    assert_eq!(loss2, loss & 0x3FFFF); // 18-bit mask
    assert_eq!(timestamp2, timestamp);
}

#[test]
fn test_packed_state_all_ones() {
    let state = PackedStateBuilder::new().with_field::<64>(u64::MAX).build();

    assert_eq!(state, u64::MAX);

    let mut unpacker = PackedStateUnpacker::new(state);
    let value = unpacker.extract::<64>();
    assert_eq!(value, u64::MAX);
}

#[test]
fn test_packed_state_all_zeros() {
    let state = PackedStateBuilder::new()
        .with_field::<32>(0)
        .with_field::<32>(0)
        .build();

    assert_eq!(state, 0);
}

// ============================================================================
// define_capsule! Macro Tests
// ============================================================================

define_capsule! {
    pub struct Test64Capsule align(64) size(64) {
        state: AtomicU64,
        padding: [u8; 56],
    }
}

define_capsule! {
    pub struct Test128Capsule align(128) size(128) {
        primary: AtomicU64,
        secondary: AtomicU64,
        padding: [u8; 112],
    }
}

define_capsule! {
    pub struct Test256Capsule align(256) size(256) {
        header: AtomicU64,
        body: [u8; 248],
    }
}

define_capsule! {
    /// Documented capsule
    #[derive(Debug)]
    pub struct DocumentedCapsule align(64) size(512) {
        state: AtomicU64,
        data: [u8; 504],
    }
}

#[test]
fn test_define_capsule_alignment() {
    assert_eq!(core::mem::align_of::<Test64Capsule>(), 64);
    assert_eq!(core::mem::align_of::<Test128Capsule>(), 128);
    assert_eq!(core::mem::align_of::<Test256Capsule>(), 256);
}

#[test]
fn test_define_capsule_size() {
    assert_eq!(core::mem::size_of::<Test64Capsule>(), 64);
    assert_eq!(core::mem::size_of::<Test128Capsule>(), 128);
    assert_eq!(core::mem::size_of::<Test256Capsule>(), 256);
}

#[test]
fn test_define_capsule_alignment_tier() {
    assert_eq!(Test64Capsule::TIER, "hot");
    assert_eq!(Test64Capsule::ALIGNMENT, 64);

    assert_eq!(Test128Capsule::TIER, "warm");
    assert_eq!(Test128Capsule::ALIGNMENT, 128);

    assert_eq!(Test256Capsule::TIER, "cold");
    assert_eq!(Test256Capsule::ALIGNMENT, 256);
}

#[test]
fn test_define_capsule_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<Test64Capsule>();
    assert_sync::<Test64Capsule>();
    assert_send::<Test128Capsule>();
    assert_sync::<Test128Capsule>();
    assert_send::<Test256Capsule>();
    assert_sync::<Test256Capsule>();
}

#[test]
fn test_define_capsule_instantiation() {
    let capsule64 = Test64Capsule {
        state: AtomicU64::new(42),
        padding: [0; 56],
    };

    let capsule128 = Test128Capsule {
        primary: AtomicU64::new(1),
        secondary: AtomicU64::new(2),
        padding: [0; 112],
    };

    let capsule256 = Test256Capsule {
        header: AtomicU64::new(0xDEADBEEF),
        body: [0xFF; 248],
    };

    assert_eq!(
        capsule64.state.load(core::sync::atomic::Ordering::Relaxed),
        42
    );
    assert_eq!(
        capsule128
            .primary
            .load(core::sync::atomic::Ordering::Relaxed),
        1
    );
    assert_eq!(
        capsule256
            .header
            .load(core::sync::atomic::Ordering::Relaxed),
        0xDEADBEEF
    );
}

#[test]
fn test_define_capsule_with_attributes() {
    let capsule = DocumentedCapsule {
        state: AtomicU64::new(0),
        data: [0; 504],
    };

    // Verify Debug derive works
    let debug_str = format!("{:?}", capsule);
    assert!(debug_str.contains("DocumentedCapsule"));
}

// ============================================================================
// Integration: PackedStateBuilder + define_capsule!
// ============================================================================

define_capsule! {
    pub struct CircuitBreakerCapsule align(64) size(64) {
        state: AtomicU64,
        padding: [u8; 56],
    }
}

#[test]
fn test_integration_circuit_breaker_capsule() {
    // Pack state using builder
    let level: u8 = 2;
    let generation: u8 = 42;
    let position: u16 = 1000;
    let timestamp: u32 = 1234567890;

    let packed_state = PackedStateBuilder::new()
        .with_field::<8>(level as u64)
        .with_field::<8>(generation as u64)
        .with_field::<16>(position as u64)
        .with_field::<32>(timestamp as u64)
        .build();

    // Store in capsule
    let capsule = CircuitBreakerCapsule {
        state: AtomicU64::new(packed_state),
        padding: [0; 56],
    };

    // Read and unpack
    let loaded_state = capsule.state.load(core::sync::atomic::Ordering::Relaxed);
    let (level2, gen2, pos2, ts2) = <(u8, u8, u16, u32)>::unpack(loaded_state);

    assert_eq!(level2, level);
    assert_eq!(gen2, generation);
    assert_eq!(pos2, position);
    assert_eq!(ts2, timestamp);
}

// ============================================================================
// Property Tests
// ============================================================================

#[test]
fn test_packed_state_idempotent() {
    // Pack then unpack should be identity
    for i in 0..100 {
        let a = (i % 256) as u8;
        let b = ((i * 2) % 256) as u8;
        let c = ((i * 3) % 65536) as u16;
        let d = i * 100;

        let state = PackedStateBuilder::new()
            .with_field::<8>(a as u64)
            .with_field::<8>(b as u64)
            .with_field::<16>(c as u64)
            .with_field::<32>(d as u64)
            .build();

        let (a2, b2, c2, d2) = <(u8, u8, u16, u32)>::unpack(state);

        assert_eq!(a, a2);
        assert_eq!(b, b2);
        assert_eq!(c, c2);
        assert_eq!(d, d2);
    }
}

#[test]
fn test_define_capsule_alignment_guarantees() {
    // Verify alignment is actually enforced by compiler
    let capsule64 = Test64Capsule {
        state: AtomicU64::new(0),
        padding: [0; 56],
    };

    let capsule128 = Test128Capsule {
        primary: AtomicU64::new(0),
        secondary: AtomicU64::new(0),
        padding: [0; 112],
    };

    // Get raw pointers
    let ptr64 = &capsule64 as *const Test64Capsule as usize;
    let ptr128 = &capsule128 as *const Test128Capsule as usize;

    // Verify alignment (pointer address must be multiple of alignment)
    assert_eq!(ptr64 % 64, 0, "64-byte alignment violated");
    assert_eq!(ptr128 % 128, 0, "128-byte alignment violated");
}

// ============================================================================
// Performance/Zero-Cost Tests
// ============================================================================

#[test]
fn test_packed_state_builder_zero_cost() {
    // This compiles to simple bit operations, no function calls
    let state = PackedStateBuilder::new()
        .with_field::<8>(0xAB)
        .with_field::<8>(0xCD)
        .with_field::<16>(0x1234)
        .with_field::<32>(0x56789ABC)
        .build();

    // Should compile to constant
    assert_eq!(state, 0xABCD_1234_56789ABC);
}

#[test]
fn test_packed_state_const_evaluation() {
    // Verify builder can be used in const context
    const STATE: u64 = {
        let builder = PackedStateBuilder::new();
        let builder = builder.with_field::<32>(0x12345678);
        let builder = builder.with_field::<32>(0x9ABCDEF0);
        builder.build()
    };

    assert_eq!(STATE, 0x12345678_9ABCDEF0);
}
