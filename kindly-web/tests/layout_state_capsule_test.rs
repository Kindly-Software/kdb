// Integration test for LayoutStateCapsule
// Run with: cargo test --test layout_state_capsule_test

use kindly_web::{LayoutStateCapsule, Breakpoint};

#[test]
fn test_new_initializes_correctly() {
    let layout = LayoutStateCapsule::new(1920, 1080);
    assert_eq!(layout.get_viewport(), (1920, 1080));
    assert_eq!(layout.get_breakpoint(), Breakpoint::XL);
    assert_eq!(layout.get_scroll_y(), 0);
    assert_eq!(layout.get_navbar_blur_level(), 0);
    assert!(!layout.is_mobile());
    assert!(!layout.should_reduce_motion());
}

#[test]
fn test_breakpoint_derivation() {
    // XS: 0-639px
    let layout_xs = LayoutStateCapsule::new(320, 568);
    assert_eq!(layout_xs.get_breakpoint(), Breakpoint::XS);
    assert!(layout_xs.is_mobile());

    // SM: 640-767px
    let layout_sm = LayoutStateCapsule::new(640, 480);
    assert_eq!(layout_sm.get_breakpoint(), Breakpoint::SM);
    assert!(layout_sm.is_mobile());

    // MD: 768-1023px
    let layout_md = LayoutStateCapsule::new(768, 1024);
    assert_eq!(layout_md.get_breakpoint(), Breakpoint::MD);
    assert!(!layout_md.is_mobile());

    // LG: 1024-1279px
    let layout_lg = LayoutStateCapsule::new(1024, 768);
    assert_eq!(layout_lg.get_breakpoint(), Breakpoint::LG);
    assert!(!layout_lg.is_mobile());

    // XL: 1280px+
    let layout_xl = LayoutStateCapsule::new(1920, 1080);
    assert_eq!(layout_xl.get_breakpoint(), Breakpoint::XL);
    assert!(!layout_xl.is_mobile());
}

#[test]
fn test_viewport_updates() {
    let layout = LayoutStateCapsule::new(1920, 1080);
    assert_eq!(layout.get_breakpoint(), Breakpoint::XL);

    // Update to tablet size
    layout.update_viewport(768, 1024);
    assert_eq!(layout.get_breakpoint(), Breakpoint::MD);
    assert_eq!(layout.get_viewport(), (768, 1024));
    assert!(!layout.is_mobile());

    // Update to mobile size
    layout.update_viewport(375, 667);
    assert_eq!(layout.get_breakpoint(), Breakpoint::XS);
    assert_eq!(layout.get_viewport(), (375, 667));
    assert!(layout.is_mobile());
}

#[test]
fn test_scroll_and_navbar_blur() {
    let layout = LayoutStateCapsule::new(1920, 1080);

    // No blur (0-50px)
    layout.update_scroll(0);
    assert_eq!(layout.get_navbar_blur_level(), 0);
    assert_eq!(layout.get_scroll_y(), 0);

    layout.update_scroll(50);
    assert_eq!(layout.get_navbar_blur_level(), 0);

    // Light blur (51-200px)
    layout.update_scroll(51);
    assert_eq!(layout.get_navbar_blur_level(), 1);

    layout.update_scroll(150);
    assert_eq!(layout.get_navbar_blur_level(), 1);

    layout.update_scroll(200);
    assert_eq!(layout.get_navbar_blur_level(), 1);

    // Medium blur (201-500px)
    layout.update_scroll(201);
    assert_eq!(layout.get_navbar_blur_level(), 2);

    layout.update_scroll(350);
    assert_eq!(layout.get_navbar_blur_level(), 2);

    layout.update_scroll(500);
    assert_eq!(layout.get_navbar_blur_level(), 2);

    // Full blur (501px+)
    layout.update_scroll(501);
    assert_eq!(layout.get_navbar_blur_level(), 3);

    layout.update_scroll(1000);
    assert_eq!(layout.get_navbar_blur_level(), 3);
}

#[test]
fn test_reduced_motion_preference() {
    let layout = LayoutStateCapsule::new(1920, 1080);
    assert!(!layout.should_reduce_motion());

    layout.set_reduced_motion_preference(true);
    assert!(layout.should_reduce_motion());

    layout.set_reduced_motion_preference(false);
    assert!(!layout.should_reduce_motion());
}

#[test]
fn test_breakpoint_boundaries() {
    // Test exact breakpoint boundaries
    let layout = LayoutStateCapsule::new(639, 480);
    assert_eq!(layout.get_breakpoint(), Breakpoint::XS);

    layout.update_viewport(640, 480);
    assert_eq!(layout.get_breakpoint(), Breakpoint::SM);

    layout.update_viewport(767, 480);
    assert_eq!(layout.get_breakpoint(), Breakpoint::SM);

    layout.update_viewport(768, 1024);
    assert_eq!(layout.get_breakpoint(), Breakpoint::MD);

    layout.update_viewport(1023, 768);
    assert_eq!(layout.get_breakpoint(), Breakpoint::MD);

    layout.update_viewport(1024, 768);
    assert_eq!(layout.get_breakpoint(), Breakpoint::LG);

    layout.update_viewport(1279, 1024);
    assert_eq!(layout.get_breakpoint(), Breakpoint::LG);

    layout.update_viewport(1280, 720);
    assert_eq!(layout.get_breakpoint(), Breakpoint::XL);
}

#[test]
fn test_capsule_alignment_and_size() {
    // Verify 64-byte alignment and size
    assert_eq!(std::mem::align_of::<LayoutStateCapsule>(), 64);
    assert_eq!(std::mem::size_of::<LayoutStateCapsule>(), 64);
}

#[test]
fn test_concurrent_reads() {
    use std::sync::Arc;
    use std::thread;

    let layout = Arc::new(LayoutStateCapsule::new(1920, 1080));

    // Spawn multiple reader threads
    let mut handles = vec![];
    for _ in 0..10 {
        let layout_clone = Arc::clone(&layout);
        handles.push(thread::spawn(move || {
            for _ in 0..1000 {
                let bp = layout_clone.get_breakpoint();
                let (w, h) = layout_clone.get_viewport();
                let mobile = layout_clone.is_mobile();

                // Verify consistency
                assert_eq!(bp, Breakpoint::XL);
                assert_eq!((w, h), (1920, 1080));
                assert!(!mobile);
            }
        }));
    }

    // Wait for all readers
    for handle in handles {
        handle.join().unwrap();
    }
}
