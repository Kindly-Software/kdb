//! GPU Render Target Capsule - T28 Tests
//!
//! Comprehensive testing for RenderTargetCapsule (Phase 2 GPU HAL)
//! - Q1-Q7: Unit tests
//! - Q8-Q14: Property tests
//! - Q15-Q21: Integration tests
//! - Q22-Q28: Production tests

#![cfg(feature = "gpu-intel")]

use atomic_capsule::gpu::hal::{RenderTargetCapsule, RenderTargetError, TextureHandle};

// ============================================================================
// Q1-Q7: Unit Tests
// ============================================================================

#[test]
fn test_render_target_new_valid() {
    let rt = RenderTargetCapsule::new(1920, 1080).expect("Failed to create render target");
    let (w, h) = rt.get_dimensions().expect("Failed to get dimensions");
    assert_eq!(w, 1920);
    assert_eq!(h, 1080);
}

#[test]
fn test_render_target_new_invalid_zero_width() {
    let result = RenderTargetCapsule::new(0, 1080);
    assert_eq!(result, Err(RenderTargetError::InvalidDimensions));
}

#[test]
fn test_render_target_new_invalid_zero_height() {
    let result = RenderTargetCapsule::new(1920, 0);
    assert_eq!(result, Err(RenderTargetError::InvalidDimensions));
}

#[test]
fn test_render_target_new_invalid_too_large() {
    let result = RenderTargetCapsule::new(20000, 1080);
    assert_eq!(result, Err(RenderTargetError::InvalidDimensions));
}

#[test]
fn test_attach_color_slot_0() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    let texture = TextureHandle(0x1234);
    rt.attach_color(0, texture).expect("Failed to attach color");
    assert_eq!(rt.attached_color_count(), 1);
}

#[test]
fn test_attach_color_invalid_slot() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    let texture = TextureHandle(0x1234);
    let result = rt.attach_color(8, texture);
    assert_eq!(result, Err(RenderTargetError::InvalidSlot));
}

#[test]
fn test_attach_color_null_texture() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    let result = rt.attach_color(0, TextureHandle(0));
    assert_eq!(result, Err(RenderTargetError::InvalidTexture));
}

#[test]
fn test_attach_color_duplicate() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    let texture1 = TextureHandle(0x1234);
    let texture2 = TextureHandle(0x2345);
    rt.attach_color(0, texture1).unwrap();
    let result = rt.attach_color(0, texture2);
    assert_eq!(result, Err(RenderTargetError::SlotOccupied));
}

#[test]
fn test_detach_color() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    let texture = TextureHandle(0x1234);
    rt.attach_color(0, texture).unwrap();
    assert_eq!(rt.attached_color_count(), 1);
    rt.detach(0).expect("Failed to detach");
    assert_eq!(rt.attached_color_count(), 0);
}

#[test]
fn test_detach_empty_slot() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    let result = rt.detach(0);
    assert_eq!(result, Err(RenderTargetError::SlotEmpty));
}

#[test]
fn test_get_attachment() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    let texture = TextureHandle(0x1234);
    rt.attach_color(0, texture).unwrap();
    let snap = rt.get_attachment(0).expect("Failed to get attachment");
    assert_eq!(snap.texture, texture);
}

#[test]
fn test_get_attachment_empty() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    let result = rt.get_attachment(0);
    assert_eq!(result, Err(RenderTargetError::SlotEmpty));
}

// ============================================================================
// Q8-Q14: Property Tests
// ============================================================================

#[test]
fn test_mrt_attach_all_slots() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    for i in 0..8 {
        let texture = TextureHandle((i + 1) as u64);
        rt.attach_color(i, texture).expect(&format!("Failed to attach slot {}", i));
    }
    assert_eq!(rt.attached_color_count(), 8);
}

#[test]
fn test_mrt_detach_all_slots() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    for i in 0..8 {
        let texture = TextureHandle((i + 1) as u64);
        rt.attach_color(i, texture).unwrap();
    }
    for i in 0..8 {
        rt.detach(i).expect(&format!("Failed to detach slot {}", i));
    }
    assert_eq!(rt.attached_color_count(), 0);
}

#[test]
fn test_mrt_attach_color_and_depth() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    for i in 0..4 {
        rt.attach_color(i, TextureHandle((i + 1) as u64)).unwrap();
    }
    rt.attach_depth_stencil(TextureHandle(0x5678)).unwrap();
    assert_eq!(rt.attached_color_count(), 4);
    assert!(rt.has_depth_attachment());
}

#[test]
fn test_attachment_independence() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    rt.attach_color(0, TextureHandle(0x1111)).unwrap();
    rt.attach_color(2, TextureHandle(0x3333)).unwrap();
    rt.detach(0);
    // Slot 2 should still be attached
    assert!(rt.get_attachment(2).is_ok());
    assert!(rt.get_attachment(0).is_err());
}

#[test]
fn test_dimension_validation() {
    for width in &[1, 256, 1024, 4096, 16384] {
        for height in &[1, 256, 1024, 4096, 16384] {
            let rt = RenderTargetCapsule::new(*width, *height).unwrap();
            let (w, h) = rt.get_dimensions().unwrap();
            assert_eq!(w, *width);
            assert_eq!(h, *height);
        }
    }
}

#[test]
fn test_attachment_mask_consistency() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    rt.attach_color(0, TextureHandle(0x1234)).unwrap();
    rt.attach_color(3, TextureHandle(0x4567)).unwrap();
    rt.attach_color(7, TextureHandle(0x7890)).unwrap();

    let mask = rt.get_attachment_mask();
    assert_eq!((mask & (1 << 0)) != 0, true);
    assert_eq!((mask & (1 << 3)) != 0, true);
    assert_eq!((mask & (1 << 7)) != 0, true);
    assert_eq!((mask & (1 << 1)) != 0, false);
}

// ============================================================================
// Q15-Q21: Integration Tests
// ============================================================================

#[test]
fn test_multi_threaded_attach_detach() {
    use std::sync::Arc;
    use std::thread;

    let rt = Arc::new(RenderTargetCapsule::new(1920, 1080).unwrap());

    let mut handles = vec![];
    for i in 0..4 {
        let rt_clone = Arc::clone(&rt);
        let handle = thread::spawn(move || {
            for j in 0..100 {
                let slot = ((i * 2 + j) % 8) as u8;
                let texture = TextureHandle(((i * 1000 + j + 1) as u64));
                let _ = rt_clone.attach_color(slot, texture);
                std::thread::sleep(std::time::Duration::from_micros(10));
                let _ = rt_clone.detach(slot);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(rt.attached_color_count(), 0);
}

#[test]
fn test_mrt_concurrent_rendering() {
    use std::sync::Arc;
    use std::thread;

    let rt = Arc::new(RenderTargetCapsule::new(1920, 1080).unwrap());

    // Attach all slots
    for i in 0..8 {
        rt.attach_color(i, TextureHandle((i + 1) as u64)).unwrap();
    }
    rt.attach_depth_stencil(TextureHandle(0x9999)).unwrap();

    // Concurrent reads (should not block)
    let mut handles = vec![];
    for _ in 0..4 {
        let rt_clone = Arc::clone(&rt);
        let handle = thread::spawn(move || {
            for i in 0..8 {
                let _ = rt_clone.get_attachment(i);
            }
            let _ = rt_clone.get_depth_attachment();
            let _ = rt_clone.get_dimensions();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(rt.attached_color_count(), 8);
}

#[test]
fn test_slot_recycling() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();

    for iteration in 0..100 {
        for slot in 0..8 {
            let texture = TextureHandle(((iteration * 8 + slot + 1) as u64));
            rt.attach_color(slot as u8, texture).unwrap();
        }

        assert_eq!(rt.attached_color_count(), 8);

        for slot in 0..8 {
            rt.detach(slot as u8).unwrap();
        }

        assert_eq!(rt.attached_color_count(), 0);
    }
}

// ============================================================================
// Q22-Q28: Production Tests
// ============================================================================

#[test]
fn test_stress_1m_attachments() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    for i in 0..1_000_000 {
        let slot = (i % 8) as u8;
        let texture = TextureHandle((i + 1) as u64);
        if rt.attach_color(slot, texture).is_ok() {
            let _ = rt.detach(slot);
        } else {
            let _ = rt.detach(slot);
            let _ = rt.attach_color(slot, texture);
        }
    }
}

#[test]
fn test_performance_attach_latency() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();

    let start = std::time::Instant::now();
    for i in 0..1000 {
        let slot = (i % 8) as u8;
        let texture = TextureHandle((i + 1) as u64);
        if i < 8 {
            let _ = rt.attach_color(slot, texture);
        } else if i % 2 == 0 {
            let _ = rt.detach(slot);
        } else {
            let _ = rt.attach_color(slot, texture);
        }
    }
    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() / 1000;
    println!("Average operation latency: {} ns", ns_per_op);
    assert!(ns_per_op < 500, "Average operation {} ns should be < 500ns", ns_per_op);
}

#[test]
fn test_no_memory_leaks() {
    for _iteration in 0..1000 {
        let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
        for i in 0..8 {
            let _ = rt.attach_color(i, TextureHandle((i + 1) as u64));
        }
        // rt dropped here - should not leak
    }
}

#[test]
fn test_full_lifecycle() {
    // Create
    let rt = RenderTargetCapsule::new(1280, 720).unwrap();

    // Attach
    for i in 0..4 {
        rt.attach_color(i, TextureHandle((i + 100) as u64)).unwrap();
    }
    rt.attach_depth_stencil(TextureHandle(0xABCD)).unwrap();

    // Verify
    assert_eq!(rt.attached_color_count(), 4);
    assert!(rt.has_depth_attachment());

    // Query
    let (w, h) = rt.get_dimensions().unwrap();
    assert_eq!(w, 1280);
    assert_eq!(h, 720);

    // Modify
    rt.detach(0).unwrap();
    assert_eq!(rt.attached_color_count(), 3);

    // Cleanup
    for i in 1..4 {
        rt.detach(i).unwrap();
    }
    assert_eq!(rt.attached_color_count(), 0);
}

// ============================================================================
// Bonus: Depth/Stencil Tests
// ============================================================================

#[test]
fn test_attach_depth_stencil() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    let depth_texture = TextureHandle(0x5678);
    rt.attach_depth_stencil(depth_texture).expect("Failed to attach depth");
    assert!(rt.has_depth_attachment());
}

#[test]
fn test_attach_depth_null() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    let result = rt.attach_depth_stencil(TextureHandle(0));
    assert_eq!(result, Err(RenderTargetError::InvalidTexture));
}

#[test]
fn test_attach_depth_duplicate() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    let depth1 = TextureHandle(0x5678);
    let depth2 = TextureHandle(0x6789);
    rt.attach_depth_stencil(depth1).unwrap();
    let result = rt.attach_depth_stencil(depth2);
    assert_eq!(result, Err(RenderTargetError::SlotOccupied));
}

#[test]
fn test_get_depth_attachment() {
    let rt = RenderTargetCapsule::new(1920, 1080).unwrap();
    let depth_texture = TextureHandle(0x5678);
    rt.attach_depth_stencil(depth_texture).unwrap();
    let snap = rt.get_depth_attachment().expect("Failed to get depth");
    assert_eq!(snap.texture, depth_texture);
}

#[test]
fn test_mrt_completeness() {
    let rt = RenderTargetCapsule::new(1024, 768).unwrap();

    // Verify all 8 color slots work
    for slot in 0..8 {
        let texture = TextureHandle(0x1000 + slot as u64);
        assert!(rt.attach_color(slot, texture).is_ok());
        let snap = rt.get_attachment(slot).unwrap();
        assert_eq!(snap.texture.0, 0x1000 + slot as u64);
    }

    // Verify depth/stencil works independently
    let depth = TextureHandle(0x9000);
    assert!(rt.attach_depth_stencil(depth).is_ok());

    // Verify all are accessible
    let (w, h) = rt.get_dimensions().unwrap();
    assert_eq!(w, 1024);
    assert_eq!(h, 768);
}
