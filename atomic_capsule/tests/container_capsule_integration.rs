// Copyright (C) 2025 Kindly Platform, Inc.
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Integration tests for ContainerCapsule

#[cfg(feature = "std")]
mod container_tests {
    use atomic_capsule::gui::{ContainerCapsule, Overflow, Rect, Size};

    #[test]
    fn test_container_complete_workflow() {
        // Create container
        let bounds = Rect::new(0, 0, 800, 600).unwrap();
        let mut container = ContainerCapsule::new(42, bounds);

        // Verify initial state
        assert_eq!(container.id(), 42);
        assert_eq!(container.child_count(), 0);
        assert_eq!(container.scroll_x(), 0.0);
        assert_eq!(container.scroll_y(), 0.0);

        // Set content size
        let content = Size::new(1600, 1200).unwrap();
        container.set_content_size(content);
        assert_eq!(container.content_size().width.to_int(), 1600);

        // Configure overflow
        container.set_overflow(Overflow::Scroll, Overflow::Auto);
        assert_eq!(container.overflow_x(), Overflow::Scroll);
        assert_eq!(container.overflow_y(), Overflow::Auto);

        // Test scrolling
        container.set_scroll(10.5, 20.75);
        assert!((container.scroll_x() - 10.5).abs() < 0.01);
        assert!((container.scroll_y() - 20.75).abs() < 0.01);

        // Add children
        for i in 100..105 {
            assert!(container.add_child(i));
        }
        assert_eq!(container.child_count(), 5);

        // Remove child
        assert!(container.remove_child(102));
        assert_eq!(container.child_count(), 4);

        // Verify children
        let children = container.children();
        assert_eq!(children.len(), 4);
        assert_eq!(children[0], 100);
        assert_eq!(children[1], 101);
        assert_eq!(children[2], 103); // 102 removed
        assert_eq!(children[3], 104);
    }

    #[test]
    fn test_container_generation_counter() {
        let bounds = Rect::new(0, 0, 800, 600).unwrap();
        let mut container = ContainerCapsule::new(1, bounds);

        let gen0 = container.generation();

        container.set_scroll(10.0, 20.0);
        assert!(container.generation() > gen0);

        let gen1 = container.generation();
        container.add_child(100);
        assert!(container.generation() > gen1);
    }

    #[test]
    fn test_container_size_and_alignment() {
        assert_eq!(core::mem::size_of::<ContainerCapsule>(), 128);
        assert_eq!(core::mem::align_of::<ContainerCapsule>(), 64);
    }

    #[test]
    fn test_container_max_children() {
        let bounds = Rect::new(0, 0, 800, 600).unwrap();
        let mut container = ContainerCapsule::new(1, bounds);

        // Fill to capacity
        for i in 0..ContainerCapsule::MAX_CHILDREN {
            assert!(container.add_child(i as u16));
        }
        assert_eq!(container.child_count(), ContainerCapsule::MAX_CHILDREN);

        // Try adding beyond capacity
        assert!(!container.add_child(999));
        assert_eq!(container.child_count(), ContainerCapsule::MAX_CHILDREN);
    }
}
