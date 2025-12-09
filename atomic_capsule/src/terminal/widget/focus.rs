//! Focus Manager Capsule - T1 Atomic keyboard navigation
//!
//! Tab-order focus management for widget trees.

use core::sync::atomic::{AtomicU16, AtomicU32, Ordering};

/// T1 Atomic - Focus and keyboard navigation
///
/// Manages focus state and tab-order for widget keyboard navigation.
#[repr(C, align(64))]
pub struct FocusManagerCapsule {
    /// Current focus index (widget index, u32::MAX = none)
    focus_index: AtomicU32,
    /// Previous focus index (for focus history)
    prev_focus: AtomicU32,
    /// Focus ring (ordered widget indices, max 32)
    focus_ring: [AtomicU16; 32],
    /// Focus ring count
    ring_count: AtomicU16,
    /// Flags: focus_visible(1) | tab_cycling(1) | _pad(14)
    flags: AtomicU16,

    _pad: [u8; 116],
}

// #ASSUME: FocusManagerCapsule is 64-byte aligned for cache performance
// #VERIFY: Static assertion below
const _: () = assert!(core::mem::align_of::<FocusManagerCapsule>() == 64);

// #ASSUME: FocusManagerCapsule fits in 256B (4 cache lines)
// #VERIFY: Static assertion below
const _: () = assert!(core::mem::size_of::<FocusManagerCapsule>() == 256);

impl Default for FocusManagerCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusManagerCapsule {
    /// Maximum focusable widgets
    pub const MAX_FOCUSABLE: usize = 32;

    const FLAG_FOCUS_VISIBLE: u16 = 1 << 0;
    const FLAG_TAB_CYCLING: u16 = 1 << 1;

    /// Create new focus manager
    pub const fn new() -> Self {
        // #ASSUME: AtomicU16 can be initialized in const context
        // #VERIFY: Compiles successfully
        const INIT_U16: AtomicU16 = AtomicU16::new(u16::MAX);

        Self {
            focus_index: AtomicU32::new(u32::MAX),
            prev_focus: AtomicU32::new(u32::MAX),
            focus_ring: [INIT_U16; 32],
            ring_count: AtomicU16::new(0),
            flags: AtomicU16::new(Self::FLAG_TAB_CYCLING | Self::FLAG_FOCUS_VISIBLE),
            _pad: [0; 116],
        }
    }

    /// Register a widget as focusable
    ///
    /// # Arguments
    /// * `widget_index` - Widget tree index
    /// * `tab_index` - Tab order (lower = earlier in tab sequence)
    pub fn register_focusable(&self, widget_index: u16, tab_index: u16) {
        let count = self.ring_count.load(Ordering::Acquire);
        if count >= Self::MAX_FOCUSABLE as u16 {
            return;
        }

        // Find insertion position based on tab_index
        let mut insert_pos = count;
        for i in 0..count {
            let entry = self.focus_ring[i as usize].load(Ordering::Acquire);
            let existing_tab = entry >> 8; // Upper 8 bits = tab_index

            if tab_index < existing_tab {
                insert_pos = i;
                break;
            }
        }

        // Shift entries to make room
        for i in (insert_pos..count).rev() {
            let entry = self.focus_ring[i as usize].load(Ordering::Acquire);
            self.focus_ring[(i + 1) as usize].store(entry, Ordering::Release);
        }

        // Insert new entry (tab_index in upper 8 bits, widget_index in lower 8 bits)
        let entry = ((tab_index & 0xFF) << 8) | (widget_index & 0xFF);
        self.focus_ring[insert_pos as usize].store(entry, Ordering::Release);

        self.ring_count.store(count + 1, Ordering::Release);
    }

    /// Unregister a focusable widget
    pub fn unregister_focusable(&self, widget_index: u16) {
        let count = self.ring_count.load(Ordering::Acquire);
        let widget_bits = widget_index & 0xFF;

        // Find widget in ring
        let mut found_pos = None;
        for i in 0..count {
            let entry = self.focus_ring[i as usize].load(Ordering::Acquire);
            if (entry & 0xFF) == widget_bits {
                found_pos = Some(i);
                break;
            }
        }

        if let Some(pos) = found_pos {
            // Shift entries to remove
            for i in pos..count - 1 {
                let entry = self.focus_ring[(i + 1) as usize].load(Ordering::Acquire);
                self.focus_ring[i as usize].store(entry, Ordering::Release);
            }

            self.focus_ring[(count - 1) as usize].store(u16::MAX, Ordering::Release);
            self.ring_count.store(count - 1, Ordering::Release);

            // Clear focus if this widget was focused
            let current_focus = self.focus_index.load(Ordering::Acquire);
            if current_focus == widget_index as u32 {
                self.focus_index.store(u32::MAX, Ordering::Release);
            }
        }
    }

    /// Move focus to next widget
    pub fn focus_next(&self) -> Option<u16> {
        let count = self.ring_count.load(Ordering::Acquire);
        if count == 0 {
            return None;
        }

        let current_focus = self.focus_index.load(Ordering::Acquire);

        // Find current position in ring
        let mut current_pos = None;
        if current_focus != u32::MAX {
            for i in 0..count {
                let entry = self.focus_ring[i as usize].load(Ordering::Acquire);
                if (entry & 0xFF) == (current_focus as u16 & 0xFF) {
                    current_pos = Some(i);
                    break;
                }
            }
        }

        let next_pos = match current_pos {
            Some(pos) => {
                let next = pos + 1;
                if next >= count {
                    if self.is_tab_cycling() {
                        0
                    } else {
                        return None;
                    }
                } else {
                    next
                }
            }
            None => 0,
        };

        let entry = self.focus_ring[next_pos as usize].load(Ordering::Acquire);
        let widget_index = (entry & 0xFF) as u16;

        self.prev_focus.store(current_focus, Ordering::Release);
        self.focus_index.store(widget_index as u32, Ordering::Release);

        Some(widget_index)
    }

    /// Move focus to previous widget
    pub fn focus_prev(&self) -> Option<u16> {
        let count = self.ring_count.load(Ordering::Acquire);
        if count == 0 {
            return None;
        }

        let current_focus = self.focus_index.load(Ordering::Acquire);

        // Find current position in ring
        let mut current_pos = None;
        if current_focus != u32::MAX {
            for i in 0..count {
                let entry = self.focus_ring[i as usize].load(Ordering::Acquire);
                if (entry & 0xFF) == (current_focus as u16 & 0xFF) {
                    current_pos = Some(i);
                    break;
                }
            }
        }

        let prev_pos = match current_pos {
            Some(pos) => {
                if pos == 0 {
                    if self.is_tab_cycling() {
                        count - 1
                    } else {
                        return None;
                    }
                } else {
                    pos - 1
                }
            }
            None => count - 1,
        };

        let entry = self.focus_ring[prev_pos as usize].load(Ordering::Acquire);
        let widget_index = (entry & 0xFF) as u16;

        self.prev_focus.store(current_focus, Ordering::Release);
        self.focus_index.store(widget_index as u32, Ordering::Release);

        Some(widget_index)
    }

    /// Set focus to specific widget
    ///
    /// Returns true if widget is in focus ring, false otherwise.
    pub fn focus(&self, widget_index: u16) -> bool {
        let count = self.ring_count.load(Ordering::Acquire);
        let widget_bits = widget_index & 0xFF;

        // Verify widget is focusable
        let mut found = false;
        for i in 0..count {
            let entry = self.focus_ring[i as usize].load(Ordering::Acquire);
            if (entry & 0xFF) == widget_bits {
                found = true;
                break;
            }
        }

        if found {
            let current_focus = self.focus_index.load(Ordering::Acquire);
            self.prev_focus.store(current_focus, Ordering::Release);
            self.focus_index.store(widget_index as u32, Ordering::Release);
        }

        found
    }

    /// Get current focus
    pub fn current_focus(&self) -> Option<u16> {
        let focus = self.focus_index.load(Ordering::Acquire);
        if focus == u32::MAX {
            None
        } else {
            Some(focus as u16)
        }
    }

    /// Handle tab key press
    ///
    /// # Arguments
    /// * `shift` - True for Shift+Tab (reverse), false for Tab (forward)
    pub fn handle_tab(&self, shift: bool) -> Option<u16> {
        if shift {
            self.focus_prev()
        } else {
            self.focus_next()
        }
    }

    /// Check if focus is visible (affects rendering)
    #[inline]
    pub fn is_focus_visible(&self) -> bool {
        let flags = self.flags.load(Ordering::Acquire);
        flags & Self::FLAG_FOCUS_VISIBLE != 0
    }

    /// Set focus visibility
    #[inline]
    pub fn set_focus_visible(&self, visible: bool) {
        let mut flags = self.flags.load(Ordering::Acquire);
        if visible {
            flags |= Self::FLAG_FOCUS_VISIBLE;
        } else {
            flags &= !Self::FLAG_FOCUS_VISIBLE;
        }
        self.flags.store(flags, Ordering::Release);
    }

    /// Check if tab cycling is enabled (wrap around at edges)
    #[inline]
    pub fn is_tab_cycling(&self) -> bool {
        let flags = self.flags.load(Ordering::Acquire);
        flags & Self::FLAG_TAB_CYCLING != 0
    }

    /// Set tab cycling mode
    #[inline]
    pub fn set_tab_cycling(&self, enabled: bool) {
        let mut flags = self.flags.load(Ordering::Acquire);
        if enabled {
            flags |= Self::FLAG_TAB_CYCLING;
        } else {
            flags &= !Self::FLAG_TAB_CYCLING;
        }
        self.flags.store(flags, Ordering::Release);
    }

    /// Get previous focus (for focus history)
    #[inline]
    pub fn previous_focus(&self) -> Option<u16> {
        let prev = self.prev_focus.load(Ordering::Acquire);
        if prev == u32::MAX {
            None
        } else {
            Some(prev as u16)
        }
    }

    /// Get focus ring count
    #[inline]
    pub fn ring_count(&self) -> u16 {
        self.ring_count.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_single_widget() {
        let mgr = FocusManagerCapsule::new();
        mgr.register_focusable(10, 0);

        assert_eq!(mgr.ring_count(), 1);
        let entry = mgr.focus_ring[0].load(Ordering::Acquire);
        assert_eq!(entry & 0xFF, 10);
    }

    #[test]
    fn test_register_multiple_widgets_sorted() {
        let mgr = FocusManagerCapsule::new();
        mgr.register_focusable(10, 2);
        mgr.register_focusable(20, 0);
        mgr.register_focusable(30, 1);

        assert_eq!(mgr.ring_count(), 3);

        // Should be sorted by tab_index: 20(0), 30(1), 10(2)
        let e0 = mgr.focus_ring[0].load(Ordering::Acquire);
        let e1 = mgr.focus_ring[1].load(Ordering::Acquire);
        let e2 = mgr.focus_ring[2].load(Ordering::Acquire);

        assert_eq!(e0 & 0xFF, 20);
        assert_eq!(e1 & 0xFF, 30);
        assert_eq!(e2 & 0xFF, 10);
    }

    #[test]
    fn test_unregister_widget() {
        let mgr = FocusManagerCapsule::new();
        mgr.register_focusable(10, 0);
        mgr.register_focusable(20, 1);
        mgr.register_focusable(30, 2);

        mgr.unregister_focusable(20);
        assert_eq!(mgr.ring_count(), 2);

        let e0 = mgr.focus_ring[0].load(Ordering::Acquire);
        let e1 = mgr.focus_ring[1].load(Ordering::Acquire);

        assert_eq!(e0 & 0xFF, 10);
        assert_eq!(e1 & 0xFF, 30);
    }

    #[test]
    fn test_focus_next() {
        let mgr = FocusManagerCapsule::new();
        mgr.register_focusable(10, 0);
        mgr.register_focusable(20, 1);
        mgr.register_focusable(30, 2);

        let f1 = mgr.focus_next();
        assert_eq!(f1, Some(10));

        let f2 = mgr.focus_next();
        assert_eq!(f2, Some(20));

        let f3 = mgr.focus_next();
        assert_eq!(f3, Some(30));

        // Cycling enabled by default
        let f4 = mgr.focus_next();
        assert_eq!(f4, Some(10));
    }

    #[test]
    fn test_focus_prev() {
        let mgr = FocusManagerCapsule::new();
        mgr.register_focusable(10, 0);
        mgr.register_focusable(20, 1);
        mgr.register_focusable(30, 2);

        let f1 = mgr.focus_prev();
        assert_eq!(f1, Some(30)); // Wraps to end

        let f2 = mgr.focus_prev();
        assert_eq!(f2, Some(20));
    }

    #[test]
    fn test_focus_direct() {
        let mgr = FocusManagerCapsule::new();
        mgr.register_focusable(10, 0);
        mgr.register_focusable(20, 1);

        let success = mgr.focus(20);
        assert!(success);
        assert_eq!(mgr.current_focus(), Some(20));

        let fail = mgr.focus(99);
        assert!(!fail);
        assert_eq!(mgr.current_focus(), Some(20)); // Unchanged
    }

    #[test]
    fn test_tab_cycling_disabled() {
        let mgr = FocusManagerCapsule::new();
        mgr.set_tab_cycling(false);
        mgr.register_focusable(10, 0);
        mgr.register_focusable(20, 1);

        mgr.focus_next(); // -> 10
        mgr.focus_next(); // -> 20
        let f3 = mgr.focus_next(); // Should be None (no wrap)
        assert_eq!(f3, None);
    }

    #[test]
    fn test_handle_tab() {
        let mgr = FocusManagerCapsule::new();
        mgr.register_focusable(10, 0);
        mgr.register_focusable(20, 1);

        let f1 = mgr.handle_tab(false); // Tab -> forward
        assert_eq!(f1, Some(10));

        let f2 = mgr.handle_tab(true); // Shift+Tab -> backward
        assert_eq!(f2, Some(20)); // Wraps
    }
}

#[cfg(all(test, feature = "proptest"))]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_register_never_exceeds_capacity(
            widgets in prop::collection::vec((0u16..200, 0u16..255), 0..50)
        ) {
            let mgr = FocusManagerCapsule::new();

            for (widget_idx, tab_idx) in widgets {
                mgr.register_focusable(widget_idx, tab_idx);
            }

            assert!(mgr.ring_count() <= FocusManagerCapsule::MAX_FOCUSABLE as u16);
        }

        #[test]
        fn prop_unregister_maintains_sorted_order(
            widgets in prop::collection::vec((0u16..100, 0u16..100), 5..15)
        ) {
            let mgr = FocusManagerCapsule::new();

            // Register all
            for (widget_idx, tab_idx) in &widgets {
                mgr.register_focusable(*widget_idx, *tab_idx);
            }

            // Unregister first one
            if !widgets.is_empty() {
                mgr.unregister_focusable(widgets[0].0);
            }

            // Verify remaining are still sorted by tab_index
            let count = mgr.ring_count();
            for i in 1..count {
                let e_prev = mgr.focus_ring[(i - 1) as usize].load(Ordering::Acquire);
                let e_curr = mgr.focus_ring[i as usize].load(Ordering::Acquire);

                let tab_prev = e_prev >> 8;
                let tab_curr = e_curr >> 8;

                assert!(tab_prev <= tab_curr);
            }
        }

        #[test]
        fn prop_focus_next_cycles_correctly(
            count in 1usize..10
        ) {
            let mgr = FocusManagerCapsule::new();

            for i in 0..count {
                mgr.register_focusable(i as u16, i as u16);
            }

            // Cycle through all widgets twice
            for _ in 0..count * 2 {
                mgr.focus_next();
            }

            // Should be back at start
            let current = mgr.current_focus();
            assert_eq!(current, Some(0));
        }

        #[test]
        fn prop_focus_and_unfocus_consistent(
            widget_idx in 0u16..100,
            tab_idx in 0u16..100,
        ) {
            let mgr = FocusManagerCapsule::new();
            mgr.register_focusable(widget_idx, tab_idx);

            let success = mgr.focus(widget_idx);
            assert!(success);
            assert_eq!(mgr.current_focus(), Some(widget_idx));

            mgr.unregister_focusable(widget_idx);
            assert_eq!(mgr.current_focus(), None);
        }
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod integration_tests {
    use super::*;

    #[test]
    fn test_complex_focus_navigation() {
        let mgr = FocusManagerCapsule::new();

        // Register widgets in non-sequential tab order
        mgr.register_focusable(100, 5);
        mgr.register_focusable(200, 1);
        mgr.register_focusable(300, 3);
        mgr.register_focusable(400, 0);
        mgr.register_focusable(500, 2);

        // Expected order: 400(0), 200(1), 500(2), 300(3), 100(5)
        assert_eq!(mgr.focus_next(), Some(400));
        assert_eq!(mgr.focus_next(), Some(200));
        assert_eq!(mgr.focus_next(), Some(500));
        assert_eq!(mgr.focus_next(), Some(300));
        assert_eq!(mgr.focus_next(), Some(100));
    }

    #[test]
    fn test_focus_history() {
        let mgr = FocusManagerCapsule::new();
        mgr.register_focusable(10, 0);
        mgr.register_focusable(20, 1);

        mgr.focus_next(); // -> 10
        assert_eq!(mgr.previous_focus(), None);

        mgr.focus_next(); // -> 20
        assert_eq!(mgr.previous_focus(), Some(10));

        mgr.focus(10);
        assert_eq!(mgr.previous_focus(), Some(20));
    }

    #[test]
    fn test_unregister_current_focus() {
        let mgr = FocusManagerCapsule::new();
        mgr.register_focusable(10, 0);
        mgr.register_focusable(20, 1);

        mgr.focus(10);
        assert_eq!(mgr.current_focus(), Some(10));

        mgr.unregister_focusable(10);
        assert_eq!(mgr.current_focus(), None);
    }

    #[test]
    fn test_focus_visibility_toggle() {
        let mgr = FocusManagerCapsule::new();
        assert!(mgr.is_focus_visible());

        mgr.set_focus_visible(false);
        assert!(!mgr.is_focus_visible());

        mgr.set_focus_visible(true);
        assert!(mgr.is_focus_visible());
    }
}
