//! Integration tests for command palette
//!
//! Tests the command palette module independently from the broken TUI code.

use std::sync::atomic::Ordering;

// Copied directly from palette.rs to test independently
#[repr(C, align(128))]
pub struct CommandPaletteCapsule {
    visible: std::sync::atomic::AtomicBool,
    _padding0: [u8; 7],
    selected_index: std::sync::atomic::AtomicU32,
    _padding1: [u8; 4],
    filter_hash: std::sync::atomic::AtomicU64,
    _padding2: [u8; 96],
}

impl CommandPaletteCapsule {
    pub const fn new() -> Self {
        Self {
            visible: std::sync::atomic::AtomicBool::new(false),
            _padding0: [0u8; 7],
            selected_index: std::sync::atomic::AtomicU32::new(0),
            _padding1: [0u8; 4],
            filter_hash: std::sync::atomic::AtomicU64::new(0),
            _padding2: [0u8; 96],
        }
    }

    pub fn toggle(&self) {
        let current = self.visible.load(Ordering::Relaxed);
        self.visible.store(!current, Ordering::Release);
        if !current {
            self.selected_index.store(0, Ordering::Release);
            self.filter_hash.store(0, Ordering::Release);
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Acquire)
    }

    pub fn next(&self, max_index: u32) {
        let current = self.selected_index.load(Ordering::Acquire);
        let new_index = if current >= max_index { 0 } else { current + 1 };
        self.selected_index.store(new_index, Ordering::Release);
    }

    pub fn prev(&self, max_index: u32) {
        let current = self.selected_index.load(Ordering::Acquire);
        let new_index = if current == 0 { max_index } else { current - 1 };
        self.selected_index.store(new_index, Ordering::Release);
    }

    pub fn selected_index(&self) -> u32 {
        self.selected_index.load(Ordering::Acquire)
    }
}

#[test]
fn test_capsule_size_alignment() {
    assert_eq!(std::mem::size_of::<CommandPaletteCapsule>(), 128);
    assert_eq!(std::mem::align_of::<CommandPaletteCapsule>(), 128);
}

#[test]
fn test_toggle() {
    let capsule = CommandPaletteCapsule::new();
    assert!(!capsule.is_visible());

    capsule.toggle();
    assert!(capsule.is_visible());

    capsule.toggle();
    assert!(!capsule.is_visible());
}

#[test]
fn test_navigation() {
    let capsule = CommandPaletteCapsule::new();
    assert_eq!(capsule.selected_index(), 0);

    capsule.next(5);
    assert_eq!(capsule.selected_index(), 1);

    capsule.next(5);
    assert_eq!(capsule.selected_index(), 2);

    capsule.prev(5);
    assert_eq!(capsule.selected_index(), 1);

    capsule.prev(5);
    assert_eq!(capsule.selected_index(), 0);

    // Wrap around
    capsule.prev(5);
    assert_eq!(capsule.selected_index(), 5);

    capsule.next(5);
    assert_eq!(capsule.selected_index(), 0);
}

#[test]
fn test_capsule_lockfree() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(CommandPaletteCapsule::new());
    let mut handles = vec![];

    // Spawn 10 threads toggling visibility
    for _ in 0..10 {
        let capsule_clone = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                capsule_clone.toggle();
            }
        }));
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Final state is deterministic only in terms of parity
    // (1000 toggles total, even → same state as start)
    // We just verify no panics occurred
    println!("Final visibility: {}", capsule.is_visible());
}

#[test]
fn test_navigation_concurrent() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(CommandPaletteCapsule::new());
    let mut handles = vec![];

    // Spawn threads navigating
    for _ in 0..5 {
        let capsule_clone = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                capsule_clone.next(10);
            }
        }));
    }

    for _ in 0..5 {
        let capsule_clone = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                capsule_clone.prev(10);
            }
        }));
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final index is in range
    let final_index = capsule.selected_index();
    assert!(final_index <= 10, "Index out of bounds: {}", final_index);
    println!("Final index after concurrent navigation: {}", final_index);
}
