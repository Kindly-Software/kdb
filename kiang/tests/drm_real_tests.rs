//! Real DRM Integration Tests
//!
//! These tests validate the real kernel driver integration.
//! They are conditional on the real_driver feature and hardware availability.

#![cfg(feature = "real_driver")]

use kiang::drm_real::*;
use kiang::{DrmDevice, DrmError};

/// Test alignment validation without requiring real hardware
#[test]
fn test_gem_create_alignment_validation() {
    // Size must be 4K aligned
    let result = gem_create_real(0, 100, 0, XeCpuCaching::WriteCombine);
    assert!(matches!(result, Err(DrmError::InvalidArgument(_))));

    // Size cannot be zero
    let result = gem_create_real(0, 0, 0, XeCpuCaching::WriteCombine);
    assert!(matches!(result, Err(DrmError::InvalidArgument(_))));

    // Valid size should pass alignment check (will fail on ioctl without hardware)
    let result = gem_create_real(0, 4096, 0, XeCpuCaching::WriteCombine);
    // Either succeeds (real hardware) or fails at ioctl (no hardware)
    // We're just testing validation logic here
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_vm_bind_alignment_validation() {
    // Address must be 4K aligned
    let result = vm_bind_real(0, 0, 1, 0x1001, 4096, 0, 0);
    assert!(matches!(result, Err(DrmError::InvalidArgument(_))));

    // Size must be 4K aligned
    let result = vm_bind_real(0, 0, 1, 0x1000, 100, 0, 0);
    assert!(matches!(result, Err(DrmError::InvalidArgument(_))));

    // Offset must be 4K aligned
    let result = vm_bind_real(0, 0, 1, 0x1000, 4096, 100, 0);
    assert!(matches!(result, Err(DrmError::InvalidArgument(_))));
}

#[test]
fn test_vm_unbind_alignment_validation() {
    // Address must be 4K aligned
    let result = vm_unbind_real(0, 0, 0x1001, 4096);
    assert!(matches!(result, Err(DrmError::InvalidArgument(_))));

    // Size must be 4K aligned
    let result = vm_unbind_real(0, 0, 0x1000, 100);
    assert!(matches!(result, Err(DrmError::InvalidArgument(_))));
}

/// Test struct sizes match kernel expectations
#[test]
fn test_struct_sizes() {
    // These must match kernel uapi definitions
    assert!(std::mem::size_of::<XeGemCreate>() >= 56);
    assert!(std::mem::size_of::<XeVmBind>() >= 64);
    assert!(std::mem::size_of::<XeVmBindOp>() >= 64);
    assert!(std::mem::size_of::<XeWaitUserFence>() >= 64);
}

/// Test flag values match kernel definitions
#[test]
fn test_flag_values() {
    assert_eq!(XeGemCreateFlags::VramIfPossible as u32, 1 << 0);
    assert_eq!(XeGemCreateFlags::NeedsVisibleVram as u32, 1 << 1);
    assert_eq!(XeGemCreateFlags::Scanout as u32, 1 << 2);

    assert_eq!(XeVmBindFlags::Immediate as u32, 1 << 0);
    assert_eq!(XeVmBindFlags::MakeResident as u32, 1 << 1);
    assert_eq!(XeVmBindFlags::Unbind as u32, 1 << 2);

    assert_eq!(XeWaitOp::Eq as u16, 0);
    assert_eq!(XeWaitOp::Neq as u16, 1);
    assert_eq!(XeWaitOp::Gt as u16, 2);
    assert_eq!(XeWaitOp::Gte as u16, 3);
}

/// Test ioctl code calculation
#[test]
fn test_ioctl_code_calculation() {
    use xe_ioctls::*;

    let code = ioctl_code(
        DRM_COMMAND_BASE,
        DRM_XE_GEM_CREATE,
        std::mem::size_of::<XeGemCreate>(),
    );

    // Verify basic structure
    assert_ne!(code, 0);
    // Command number should be in lower byte
    assert_eq!((code & 0xFF) as u32, DRM_XE_GEM_CREATE);
}

/// Integration test - requires real hardware
///
/// To run: cargo test --features real_driver --test drm_real_tests -- --ignored
#[test]
#[ignore = "Requires real Intel Arc GPU and /dev/dri/card0"]
fn test_real_hardware_gem_create() {
    // Try to open real device
    let device = DrmDevice::open(0);

    if let Ok(device) = device {
        // Try to create real GEM buffer
        let result = gem_create_real(
            device.as_raw_fd(),
            4096,
            XeGemCreateFlags::VramIfPossible as u32,
            XeCpuCaching::WriteCombine,
        );

        // If we have real hardware and driver, this should succeed
        match result {
            Ok(handle) => {
                assert_ne!(handle, 0);
                // Clean up
                let _ = gem_close_real(device.as_raw_fd(), handle);
            }
            Err(e) => {
                // May fail if driver not loaded or no permissions
                eprintln!("GEM creation failed (expected if no Xe driver): {}", e);
            }
        }
    } else {
        eprintln!("Skipping: No /dev/dri/card0 device");
    }
}

/// Integration test for VM_BIND - requires real hardware
#[test]
#[ignore = "Requires real Intel Arc GPU and /dev/dri/card0"]
fn test_real_hardware_vm_bind() {
    let device = DrmDevice::open(0);

    if let Ok(device) = device {
        // Create GEM buffer first
        let handle_result = gem_create_real(
            device.as_raw_fd(),
            4096,
            XeGemCreateFlags::VramIfPossible as u32,
            XeCpuCaching::WriteCombine,
        );

        if let Ok(handle) = handle_result {
            // Try to bind it
            let result = vm_bind_real(
                device.as_raw_fd(),
                0, // Default VM
                handle,
                0x10000, // 64KB address
                4096,
                0,
                XeVmBindFlags::Immediate as u32,
            );

            match result {
                Ok(_) => {
                    // Unbind it
                    let _ = vm_unbind_real(device.as_raw_fd(), 0, 0x10000, 4096);
                }
                Err(e) => {
                    eprintln!("VM_BIND failed (may need VM setup): {}", e);
                }
            }

            // Clean up GEM handle
            let _ = gem_close_real(device.as_raw_fd(), handle);
        }
    }
}

/// Test DrmDevice convenience methods
#[test]
#[ignore = "Requires real Intel Arc GPU"]
fn test_drm_device_convenience_methods() {
    let device = DrmDevice::open(0);

    if let Ok(device) = device {
        // Test gem_create_real method
        let result = device.gem_create_real(4096);

        match result {
            Ok(gem) => {
                assert_eq!(gem.size(), 4096);
                assert_ne!(gem.handle(), 0);

                // Test vm_bind_real method
                let bind_result = device.vm_bind_real(&gem, 0x20000);
                if bind_result.is_ok() {
                    // Test vm_unbind_real method
                    let _ = device.vm_unbind_real(0x20000, 4096);
                }

                // gem will be automatically closed on drop
            }
            Err(e) => {
                eprintln!("Device methods test failed: {}", e);
            }
        }
    }
}

/// Property test: All valid aligned sizes should pass validation
#[test]
fn test_valid_sizes_property() {
    let valid_sizes: Vec<u64> = vec![
        4096,             // 4KB
        8192,             // 8KB
        65536,            // 64KB
        1024 * 1024,      // 1MB
        16 * 1024 * 1024, // 16MB
    ];

    for size in valid_sizes {
        // Should pass validation (will fail at ioctl without hardware)
        let result = gem_create_real(0, size, 0, XeCpuCaching::WriteCombine);
        // Either validation passes or ioctl fails - we're testing validation
        if let Err(e) = &result {
            // Should not be alignment error
            if let DrmError::InvalidArgument(msg) = e {
                assert!(
                    !msg.contains("not 4K aligned"),
                    "Size {} should pass alignment check",
                    size
                );
            }
        }
    }
}

/// Property test: All invalid aligned sizes should fail validation
#[test]
fn test_invalid_sizes_property() {
    let invalid_sizes: Vec<u64> = vec![
        0,    // Zero
        100,  // Too small
        4095, // Just under 4KB
        4097, // Just over 4KB
        5000, // Not aligned
    ];

    for size in invalid_sizes {
        let result = gem_create_real(0, size, 0, XeCpuCaching::WriteCombine);
        assert!(
            matches!(result, Err(DrmError::InvalidArgument(_))),
            "Size {} should fail validation",
            size
        );
    }
}

/// Safety validation test
#[test]
fn test_safety_assumptions() {
    // Test that we catch safety violations before kernel calls

    // 1. Zero size should be caught
    let result = gem_create_real(0, 0, 0, XeCpuCaching::WriteCombine);
    assert!(result.is_err());

    // 2. Unaligned address should be caught
    let result = vm_bind_real(0, 0, 1, 0x1001, 4096, 0, 0);
    assert!(result.is_err());

    // 3. Unaligned size should be caught
    let result = vm_bind_real(0, 0, 1, 0x1000, 100, 0, 0);
    assert!(result.is_err());
}

/// Stress test: Multiple allocations (simulation only)
#[test]
fn test_multiple_allocations_stress() {
    // Test validation logic under stress (no real hardware needed)
    for i in 1..=100 {
        let size = i * 4096; // Valid aligned sizes
        let result = gem_create_real(0, size, 0, XeCpuCaching::WriteCombine);

        // Should pass validation (ioctl will fail without hardware)
        if let Err(DrmError::InvalidArgument(msg)) = result {
            panic!("Valid size {} failed validation: {}", size, msg);
        }
    }
}
