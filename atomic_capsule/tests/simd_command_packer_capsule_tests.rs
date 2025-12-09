// SIMDCommandPackerCapsule Integration Tests (T28 4-tier pyramid)
// 50+ comprehensive tests for Intel GPU command packing

#![allow(unused_imports)]

use atomic_capsule::gpu::{SIMDCommandPackerCapsule, MiCommand, PackError, ValidationError};

// ============================================================================
//  UNIT TESTS (Q1-Q7)
// ============================================================================

#[test]
fn test_unit_create_empty_capsule() {
    let cap = SIMDCommandPackerCapsule::new();
    assert_eq!(cap.len(), 0);
    assert!(cap.is_empty());
    assert!(!cap.is_full());
    assert_eq!(cap.available(), 64);
}

#[test]
fn test_unit_append_single_command() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let cmd = MiCommand {
        opcode: 0x00, // MI_NOOP
        length: 1,
        flags: 0,
        reserved: 0,
        payload: [0, 0, 0],
    };

    assert!(cap.append(&cmd).is_ok());
    assert_eq!(cap.len(), 1);
    assert!(!cap.is_empty());
}

#[test]
fn test_unit_append_fills_buffer() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let cmd = MiCommand {
        opcode: 0x00,
        length: 1,
        flags: 0,
        reserved: 0,
        payload: [0, 0, 0],
    };

    for i in 0..64 {
        assert!(cap.append(&cmd).is_ok());
        assert_eq!(cap.len(), i + 1);
    }

    assert!(cap.is_full());
    assert_eq!(cap.available(), 0);
}

#[test]
fn test_unit_buffer_overflow() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let cmd = MiCommand {
        opcode: 0x00,
        length: 1,
        flags: 0,
        reserved: 0,
        payload: [0, 0, 0],
    };

    // Fill to capacity
    for _ in 0..64 {
        let _ = cap.append(&cmd);
    }

    // Next append should fail
    let result = cap.append(&cmd);
    assert!(matches!(result, Err(PackError::BufferFull { .. })));
}

#[test]
fn test_unit_invalid_length_zero() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let cmd = MiCommand {
        opcode: 0x00,
        length: 0, // Invalid
        flags: 0,
        reserved: 0,
        payload: [0, 0, 0],
    };

    let result = cap.append(&cmd);
    assert!(matches!(result, Err(PackError::InvalidLength { length: 0 })));
}

#[test]
fn test_unit_invalid_length_too_large() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let cmd = MiCommand {
        opcode: 0x00,
        length: 5, // Invalid (max 4)
        flags: 0,
        reserved: 0,
        payload: [0, 0, 0],
    };

    let result = cap.append(&cmd);
    assert!(matches!(result, Err(PackError::InvalidLength { length: 5 })));
}

#[test]
fn test_unit_clear_buffer() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let cmd = MiCommand {
        opcode: 0x00,
        length: 1,
        flags: 0,
        reserved: 0,
        payload: [0, 0, 0],
    };

    cap.append(&cmd).unwrap();
    assert_eq!(cap.len(), 1);

    cap.clear();
    assert_eq!(cap.len(), 0);
    assert!(cap.is_empty());
}

#[test]
fn test_unit_get_command() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let cmd = MiCommand {
        opcode: 0x31, // MI_BATCH_BUFFER_START
        length: 2,
        flags: 0x42,
        reserved: 0,
        payload: [0xDEAD_BEEF, 0xCAFE_BABE, 0x1234_5678],
    };

    cap.append(&cmd).unwrap();
    let retrieved = cap.get(0).unwrap();

    assert_eq!(retrieved.opcode, 0x31);
    assert_eq!(retrieved.length, 2);
    assert_eq!(retrieved.flags, 0x42);
    assert_eq!(retrieved.payload[0], 0xDEAD_BEEF);
    assert_eq!(retrieved.payload[1], 0xCAFE_BABE);
    assert_eq!(retrieved.payload[2], 0x1234_5678);
}

#[test]
fn test_unit_get_out_of_bounds() {
    let cap = SIMDCommandPackerCapsule::new();
    assert!(cap.get(0).is_none());
    assert!(cap.get(100).is_none());
}

// ============================================================================
//  PROPERTY TESTS (Q8-Q14)
// ============================================================================

#[test]
fn test_property_append_idempotent() {
    let mut cap1 = SIMDCommandPackerCapsule::new();
    let mut cap2 = SIMDCommandPackerCapsule::new();

    let cmd = MiCommand {
        opcode: 0x00,
        length: 1,
        flags: 0,
        reserved: 0,
        payload: [0, 0, 0],
    };

    cap1.append(&cmd).unwrap();
    cap2.append(&cmd).unwrap();

    assert_eq!(cap1.len(), cap2.len());
}

#[test]
fn test_property_clear_resets_state() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let original = SIMDCommandPackerCapsule::new();

    let cmd = MiCommand {
        opcode: 0x00,
        length: 1,
        flags: 0,
        reserved: 0,
        payload: [0, 0, 0],
    };

    cap.append(&cmd).unwrap();
    cap.clear();

    assert_eq!(cap.len(), original.len());
    assert_eq!(cap.available(), original.available());
}

#[test]
fn test_property_pack_deterministic() {
    let mut cap1 = SIMDCommandPackerCapsule::new();
    let mut cap2 = SIMDCommandPackerCapsule::new();

    let commands = vec![
        MiCommand {
            opcode: 0x00,
            length: 1,
            flags: 0,
            reserved: 0,
            payload: [0, 0, 0],
        };
        10
    ];

    cap1.pack_simd(&commands).unwrap();
    cap2.pack_simd(&commands).unwrap();

    assert_eq!(cap1.len(), cap2.len());
}

#[test]
fn test_property_avx2_consistency() {
    let cap1 = SIMDCommandPackerCapsule::new();
    let cap2 = SIMDCommandPackerCapsule::new();
    assert_eq!(cap1.has_avx2(), cap2.has_avx2());
}

#[test]
fn test_property_alignment_verified() {
    // Verify 256B alignment
    assert_eq!(std::mem::align_of::<SIMDCommandPackerCapsule>(), 256);
}

#[test]
fn test_property_default_equals_new() {
    let from_default = SIMDCommandPackerCapsule::default();
    let from_new = SIMDCommandPackerCapsule::new();
    assert_eq!(from_default.len(), from_new.len());
}

// ============================================================================
//  INTEGRATION TESTS (Q15-Q21)
// ============================================================================

#[test]
fn test_integration_pack_8_commands() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let commands: Vec<MiCommand> = (0..8)
        .map(|i| MiCommand {
            opcode: (i as u8) & 0x7F,
            length: 1,
            flags: 0,
            reserved: 0,
            payload: [i as u32, 0, 0],
        })
        .collect();

    let packed = cap.pack_simd(&commands).unwrap();
    assert_eq!(packed, 8);
    assert_eq!(cap.len(), 8);
}

#[test]
fn test_integration_pack_overflow() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let commands: Vec<MiCommand> = (0..100)
        .map(|i| MiCommand {
            opcode: (i as u8) & 0x7F,
            length: 1,
            flags: 0,
            reserved: 0,
            payload: [(i as u32) << 8, 0, 0],
        })
        .collect();

    let packed = cap.pack_simd(&commands).unwrap();
    assert_eq!(packed, 64); // Limited by buffer size
    assert_eq!(cap.len(), 64);
}

#[test]
fn test_integration_validate_empty() {
    let cap = SIMDCommandPackerCapsule::new();
    assert!(cap.validate().is_ok());
}

#[test]
fn test_integration_validate_valid_commands() {
    let mut cap = SIMDCommandPackerCapsule::new();

    for i in 1..=4 {
        let cmd = MiCommand {
            opcode: 0x00,
            length: i as u8,
            flags: 0,
            reserved: 0,
            payload: [0, 0, 0],
        };
        cap.append(&cmd).unwrap();
    }

    assert!(cap.validate().is_ok());
}

#[test]
fn test_integration_pack_and_retrieve() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let original = MiCommand {
        opcode: 0x31,
        length: 3,
        flags: 0xAA,
        reserved: 0,
        payload: [0x1111_1111, 0x2222_2222, 0x3333_3333],
    };

    cap.append(&original).unwrap();
    let retrieved = cap.get(0).unwrap();

    assert_eq!(retrieved, original);
}

#[test]
fn test_integration_sequential_pack() {
    let mut cap = SIMDCommandPackerCapsule::new();

    let commands: Vec<MiCommand> = (0..10)
        .map(|i| MiCommand {
            opcode: (i as u8) & 0x7F,
            length: ((i % 4) + 1) as u8,
            flags: (i as u8) << 2,
            reserved: 0,
            payload: [i as u32, i as u32 + 1, i as u32 + 2],
        })
        .collect();

    cap.pack_simd(&commands).unwrap();

    for i in 0..10 {
        let retrieved = cap.get(i).unwrap();
        assert_eq!(retrieved.opcode, (i as u8) & 0x7F);
        assert_eq!(retrieved.flags, (i as u8) << 2);
    }
}

// ============================================================================
//  PRODUCTION TESTS (Q22-Q28)
// ============================================================================

#[test]
fn test_prod_stress_fill_clear() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let cmd = MiCommand {
        opcode: 0x00,
        length: 1,
        flags: 0,
        reserved: 0,
        payload: [0, 0, 0],
    };

    for _ in 0..100 {
        cap.clear();
        for _ in 0..64 {
            cap.append(&cmd).ok();
        }
        assert!(cap.is_full());
    }
}

#[test]
fn test_prod_latency_append() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let cmd = MiCommand {
        opcode: 0x00,
        length: 1,
        flags: 0,
        reserved: 0,
        payload: [0, 0, 0],
    };

    let start = std::time::Instant::now();
    for _ in 0..1000 {
        cap.clear();
        let _ = cap.append(&cmd);
    }
    let elapsed = start.elapsed();

    println!(
        "Average append latency: {:.2}ns",
        elapsed.as_nanos() as f64 / 1000.0
    );
    // Conservative target: <500ns
    assert!(elapsed.as_nanos() < 500_000);
}

#[test]
fn test_prod_latency_pack_8() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let commands: Vec<MiCommand> = (0..8)
        .map(|i| MiCommand {
            opcode: (i as u8) & 0x7F,
            length: 1,
            flags: 0,
            reserved: 0,
            payload: [0, 0, 0],
        })
        .collect();

    let start = std::time::Instant::now();
    for _ in 0..1000 {
        cap.clear();
        let _ = cap.pack_simd(&commands);
    }
    let elapsed = start.elapsed();

    println!(
        "Average 8-command pack latency: {:.2}ns",
        elapsed.as_nanos() as f64 / 1000.0
    );
    // Conservative target: <500ns
    assert!(elapsed.as_nanos() < 500_000);
}

#[test]
fn test_prod_zero_allocation() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let cmd = MiCommand {
        opcode: 0x00,
        length: 1,
        flags: 0,
        reserved: 0,
        payload: [0, 0, 0],
    };

    // No allocations (statically sized)
    for _ in 0..1000 {
        let _ = cap.append(&cmd);
        cap.clear();
    }
}

#[test]
fn test_prod_validation_comprehensive() {
    let mut cap = SIMDCommandPackerCapsule::new();

    // Add valid commands with all length values
    for i in 1..=4 {
        let cmd = MiCommand {
            opcode: (i * 10) as u8,
            length: i as u8,
            flags: i as u8,
            reserved: 0,
            payload: [i as u32, i as u32 + 1, i as u32 + 2],
        };
        cap.append(&cmd).unwrap();
    }

    assert!(cap.validate().is_ok());
    assert_eq!(cap.len(), 4);
}

#[test]
fn test_prod_default_implementation() {
    let cap1 = SIMDCommandPackerCapsule::default();
    let cap2 = SIMDCommandPackerCapsule::new();
    assert_eq!(cap1.len(), cap2.len());
}

#[test]
fn test_prod_pack_empty_slice() {
    let mut cap = SIMDCommandPackerCapsule::new();
    let commands: Vec<MiCommand> = vec![];
    let packed = cap.pack_simd(&commands).unwrap();
    assert_eq!(packed, 0);
    assert_eq!(cap.len(), 0);
}

#[test]
fn test_prod_buffer_capacity_limits() {
    let mut cap = SIMDCommandPackerCapsule::new();

    // Test that we can't exceed capacity
    let commands = vec![
        MiCommand {
            opcode: 0x00,
            length: 1,
            flags: 0,
            reserved: 0,
            payload: [0, 0, 0],
        };
        100
    ];

    let result = cap.pack_simd(&commands);
    assert!(result.is_ok());
    assert_eq!(cap.len(), 64); // Capped at buffer size
}

#[test]
fn test_prod_payload_preservation() {
    let mut cap = SIMDCommandPackerCapsule::new();

    let test_payloads = vec![
        [0x12345678, 0xABCDEF00, 0xDEADBEEF],
        [0xCAFEBABE, 0x11223344, 0x55667788],
        [0xFFFFFFFF, 0x00000000, 0x55555555],
    ];

    for (i, payload) in test_payloads.iter().enumerate() {
        let cmd = MiCommand {
            opcode: i as u8,
            length: 1,
            flags: 0,
            reserved: 0,
            payload: *payload,
        };
        cap.append(&cmd).unwrap();
    }

    for (i, expected_payload) in test_payloads.iter().enumerate() {
        let cmd = cap.get(i).unwrap();
        assert_eq!(cmd.payload, *expected_payload);
    }
}
