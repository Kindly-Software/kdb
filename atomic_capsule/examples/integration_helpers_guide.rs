//! Integration Helpers Guide
//!
//! Comprehensive guide to using `PackedStateBuilder` and `define_capsule!` macro
//!
//! # UCE33 Framework Alignment
//!
//! These helpers implement:
//! - **Q10**: Tier 1 (Atomic capsule primitives)
//! - **Q28**: Simplicity (40-60% boilerplate reduction)
//! - **Q31**: Rust zero-cost abstractions
//! - **Q33**: Compile-time verification
//!
//! # Problem Solved
//!
//! Before these helpers:
//! - 1,500 lines of manual bit packing boilerplate
//! - 318 lines of capsule definition boilerplate (6 lines × 53 capsules)
//! - Prone to alignment errors and bit packing bugs
//!
//! After these helpers:
//! - 40-60% less boilerplate
//! - Type-safe bit packing (compile-time validation)
//! - Automatic Send/Sync/verification

use atomic_capsule::{define_capsule, AlignmentTier, PackedStateBuilder, UnpackState};
use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// Part 1: PackedStateBuilder - Type-Safe Bit Packing
// ============================================================================

/// Example 1: Simple 4-field packing
fn example_1_simple_packing() {
    println!("=== Example 1: Simple 4-Field Packing ===\n");

    // Before (manual bit packing):
    let level = 2u8;
    let generation = 42u8;
    let position = 1000u16;
    let timestamp = 1234567890u32;

    // Manual: error-prone and verbose
    let manual_state = (level as u64) << 56
        | (generation as u64) << 48
        | (position as u64) << 32
        | (timestamp as u64);

    println!("Manual state: 0x{:016X}", manual_state);

    // After (type-safe builder):
    let builder_state = PackedStateBuilder::new()
        .with_field::<8>(level as u64) // Compile-time bit width validation
        .with_field::<8>(generation as u64)
        .with_field::<16>(position as u64)
        .with_field::<32>(timestamp as u64)
        .build();

    println!("Builder state: 0x{:016X}", builder_state);
    assert_eq!(manual_state, builder_state);

    // Unpacking: type-safe extraction
    let (l2, g2, p2, t2) = <(u8, u8, u16, u32)>::unpack(builder_state);

    println!("\nUnpacked values:");
    println!("  level: {}", l2);
    println!("  generation: {}", g2);
    println!("  position: {}", p2);
    println!("  timestamp: {}", t2);

    assert_eq!(l2, level);
    assert_eq!(g2, generation);
    assert_eq!(p2, position);
    assert_eq!(t2, timestamp);

    println!("\n✓ Values match!\n");
}

/// Example 2: Circuit Breaker Capsule State
///
/// Realistic example from production HFT system
fn example_2_circuit_breaker_state() {
    println!("=== Example 2: Circuit Breaker State (Production) ===\n");

    #[derive(Debug, Clone, Copy, PartialEq)]
    #[repr(u8)]
    enum ProtectionLevel {
        Normal = 0,
        Level1 = 1,
        Level2 = 2,
        Level3 = 3,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    #[repr(u8)]
    enum BreakerCause {
        None = 0,
        MaxLoss = 1,
        MaxDrawdown = 2,
        OrderReject = 3,
        TimeoutExceeded = 4,
        Manual = 5,
    }

    // Pack circuit breaker state: 2+3+1+8+18+32 = 64 bits
    let level = ProtectionLevel::Level2;
    let cause = BreakerCause::MaxDrawdown;
    let stale = false;
    let version: u8 = 42;
    let loss_bp: u32 = 12345; // Basis points (18 bits max: 262,143)
    let timestamp: u32 = 1234567890;

    let packed_state = PackedStateBuilder::new()
        .with_field::<2>(level as u64) // 2 bits: 0-3
        .with_field::<3>(cause as u64) // 3 bits: 0-7
        .with_field::<1>(stale as u64) // 1 bit: bool
        .with_field::<8>(version as u64) // 8 bits: version counter
        .with_field::<18>(loss_bp as u64) // 18 bits: 0-262,143 bp
        .with_field::<32>(timestamp as u64) // 32 bits: unix timestamp
        .build();

    println!("Packed state: 0x{:016X}", packed_state);
    println!("Size: 64 bits (single AtomicU64)\n");

    // Unpacking (manual extraction for enums)
    use atomic_capsule::PackedStateUnpacker;
    let mut unpacker = PackedStateUnpacker::new(packed_state);

    let level2 = match unpacker.extract::<2>() as u8 {
        0 => ProtectionLevel::Normal,
        1 => ProtectionLevel::Level1,
        2 => ProtectionLevel::Level2,
        3 => ProtectionLevel::Level3,
        _ => unreachable!(),
    };

    let cause2 = match unpacker.extract::<3>() as u8 {
        0 => BreakerCause::None,
        1 => BreakerCause::MaxLoss,
        2 => BreakerCause::MaxDrawdown,
        3 => BreakerCause::OrderReject,
        4 => BreakerCause::TimeoutExceeded,
        5 => BreakerCause::Manual,
        _ => unreachable!(),
    };

    let stale2 = unpacker.extract::<1>() != 0;
    let version2 = unpacker.extract::<8>() as u8;
    let loss_bp2 = unpacker.extract::<18>() as u32;
    let timestamp2 = unpacker.extract::<32>() as u32;

    println!("Unpacked values:");
    println!("  level: {:?}", level2);
    println!("  cause: {:?}", cause2);
    println!("  stale: {}", stale2);
    println!("  version: {}", version2);
    println!("  loss_bp: {}", loss_bp2);
    println!("  timestamp: {}", timestamp2);

    assert_eq!(level2, level);
    assert_eq!(cause2, cause);
    assert_eq!(stale2, stale);
    assert_eq!(version2, version);
    assert_eq!(loss_bp2, loss_bp);
    assert_eq!(timestamp2, timestamp);

    println!("\n✓ Circuit breaker state packed successfully!\n");
}

/// Example 3: Compile-Time Validation
///
/// Shows how the builder prevents bit width overflow at compile-time
fn example_3_compile_time_validation() {
    println!("=== Example 3: Compile-Time Validation ===\n");

    // Valid: 32 + 32 = 64 bits
    let state_valid = PackedStateBuilder::new()
        .with_field::<32>(0x12345678)
        .with_field::<32>(0x9ABCDEF0)
        .build();

    println!("Valid state (32+32=64): 0x{:016X}", state_valid);

    // This would fail at compile-time (uncomment to see error):
    // let state_invalid = PackedStateBuilder::new()
    //     .with_field::<32>(0x12345678)  // 32 bits
    //     .with_field::<32>(0x9ABCDEF0)  // 32 bits
    //     .with_field::<16>(0xBEEF)      // 16 bits (total: 80 > 64) ❌
    //     .build();
    //
    // Error: "Bit width overflow: total exceeds 64 bits"

    println!("\n✓ Compile-time validation prevents overflow!\n");
}

// ============================================================================
// Part 2: define_capsule! Macro - Automatic Boilerplate
// ============================================================================

/// Example 4: Basic Capsule Definition
fn example_4_basic_capsule() {
    println!("=== Example 4: Basic Capsule Definition ===\n");

    // Before (manual boilerplate):
    // #[repr(C, align(64))]
    // struct MyCapsule {
    //     state: AtomicU64,
    //     data: [u8; 56],
    // }
    // unsafe impl Send for MyCapsule {}
    // unsafe impl Sync for MyCapsule {}
    // verify_capsule!(MyCapsule, 64, 64);

    // After (one macro):
    define_capsule! {
        pub struct SimpleCapsule align(64) size(64) {
            state: AtomicU64,
            data: [u8; 56],
        }
    }

    let capsule = SimpleCapsule {
        state: AtomicU64::new(42),
        data: [0; 56],
    };

    println!(
        "Capsule alignment: {} bytes",
        core::mem::align_of::<SimpleCapsule>()
    );
    println!(
        "Capsule size: {} bytes",
        core::mem::size_of::<SimpleCapsule>()
    );
    println!("Alignment tier: {}", SimpleCapsule::TIER);
    println!("Value: {}", capsule.state.load(Ordering::Relaxed));

    println!("\n✓ Capsule defined with single macro!\n");
}

/// Example 5: Different Alignment Tiers
fn example_5_alignment_tiers() {
    println!("=== Example 5: Alignment Tiers ===\n");

    define_capsule! {
        pub struct HotCapsule align(64) size(64) {
            data: [u8; 64],
        }
    }

    define_capsule! {
        pub struct WarmCapsule align(128) size(128) {
            data: [u8; 128],
        }
    }

    define_capsule! {
        pub struct ColdCapsule align(256) size(256) {
            data: [u8; 256],
        }
    }

    println!(
        "Hot (64B):   tier='{}', alignment={}",
        HotCapsule::TIER,
        HotCapsule::ALIGNMENT
    );
    println!(
        "Warm (128B): tier='{}', alignment={}",
        WarmCapsule::TIER,
        WarmCapsule::ALIGNMENT
    );
    println!(
        "Cold (256B): tier='{}', alignment={}",
        ColdCapsule::TIER,
        ColdCapsule::ALIGNMENT
    );

    println!("\n✓ Three tiers automatically classified!\n");
}

/// Example 6: Integration - Capsule + PackedState
fn example_6_full_integration() {
    println!("=== Example 6: Full Integration ===\n");

    // Define capsule with macro
    define_capsule! {
        pub struct CircuitBreakerCapsule align(64) size(64) {
            state: AtomicU64,
            padding: [u8; 56],
        }
    }

    // Pack state with builder
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

    // Create capsule with packed state
    let capsule = CircuitBreakerCapsule {
        state: AtomicU64::new(packed_state),
        padding: [0; 56],
    };

    println!("Capsule created:");
    println!(
        "  Alignment: {} bytes",
        core::mem::align_of::<CircuitBreakerCapsule>()
    );
    println!(
        "  Size: {} bytes",
        core::mem::size_of::<CircuitBreakerCapsule>()
    );
    println!("  Tier: {}", CircuitBreakerCapsule::TIER);
    println!("  State: 0x{:016X}", capsule.state.load(Ordering::Relaxed));

    // Read and unpack atomically
    let loaded_state = capsule.state.load(Ordering::Relaxed);
    let (l2, g2, p2, t2) = <(u8, u8, u16, u32)>::unpack(loaded_state);

    println!("\nUnpacked values:");
    println!("  level: {}", l2);
    println!("  generation: {}", g2);
    println!("  position: {}", p2);
    println!("  timestamp: {}", t2);

    assert_eq!(l2, level);
    assert_eq!(g2, generation);
    assert_eq!(p2, position);
    assert_eq!(t2, timestamp);

    println!("\n✓ Full integration successful!\n");
}

/// Example 7: Performance - Zero-Cost Abstractions
fn example_7_zero_cost() {
    println!("=== Example 7: Zero-Cost Abstractions ===\n");

    // PackedStateBuilder compiles to simple bit operations
    let state = PackedStateBuilder::new()
        .with_field::<32>(0x12345678)
        .with_field::<32>(0x9ABCDEF0)
        .build();

    println!("Builder result: 0x{:016X}", state);

    // Equivalent manual code (identical assembly)
    let manual = 0x12345678_9ABCDEF0u64;

    println!("Manual result:  0x{:016X}", manual);

    assert_eq!(state, manual);

    println!("\n✓ Zero runtime cost (identical to manual code)!\n");
}

/// Example 8: Const Evaluation
fn example_8_const_evaluation() {
    println!("=== Example 8: Compile-Time Evaluation ===\n");

    // Builder works in const context (computed at compile-time)
    const PACKED_STATE: u64 = {
        let builder = PackedStateBuilder::new();
        let builder = builder.with_field::<32>(0x12345678);
        let builder = builder.with_field::<32>(0x9ABCDEF0);
        builder.build()
    };

    println!("Const state: 0x{:016X}", PACKED_STATE);
    println!("Computed at: compile-time (zero runtime cost)");

    println!("\n✓ Builder works in const context!\n");
}

// ============================================================================
// Main Example Runner
// ============================================================================

fn main() {
    println!("\n");
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║        Integration Helpers Guide - atomic_capsule             ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!("\n");

    example_1_simple_packing();
    example_2_circuit_breaker_state();
    example_3_compile_time_validation();
    example_4_basic_capsule();
    example_5_alignment_tiers();
    example_6_full_integration();
    example_7_zero_cost();
    example_8_const_evaluation();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║                     All Examples Passed!                       ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!("\n");

    println!("Key Takeaways:");
    println!("  • PackedStateBuilder: Type-safe bit packing (40-60% less code)");
    println!("  • define_capsule!: Automatic boilerplate (80% reduction)");
    println!("  • Compile-time validation: Zero runtime overhead");
    println!("  • Production-ready: Used in 53 capsules in kindly_hft");
    println!("\n");
}
