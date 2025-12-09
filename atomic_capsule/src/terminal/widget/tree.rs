//! Widget Tree Capsule - T4 Batch tier widget hierarchy
//!
//! Arena-allocated tree structure for widget management with lockfree dirty tracking.

use core::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering};

use super::types::Rect;

/// Node in widget tree
#[derive(Copy, Clone, Debug)]
#[repr(C, align(8))]
pub struct WidgetNode {
    /// Widget type ID (from Widget::TYPE_ID)
    pub type_id: u64,
    /// Parent node index (u16::MAX = root)
    pub parent: u16,
    /// First child index (u16::MAX = none)
    pub first_child: u16,
    /// Next sibling index (u16::MAX = none)
    pub next_sibling: u16,
    /// Flags: visible(1) | enabled(1) | dirty(1) | _pad(13)
    pub flags: u16,
    /// Computed bounds
    pub bounds: Rect,
}

impl Default for WidgetNode {
    fn default() -> Self {
        Self {
            type_id: 0,
            parent: u16::MAX,
            first_child: u16::MAX,
            next_sibling: u16::MAX,
            flags: 0,
            bounds: Rect::default(),
        }
    }
}

impl WidgetNode {
    const FLAG_VISIBLE: u16 = 1 << 0;
    const FLAG_ENABLED: u16 = 1 << 1;
    const FLAG_DIRTY: u16 = 1 << 2;

    #[inline]
    pub fn is_visible(&self) -> bool {
        self.flags & Self::FLAG_VISIBLE != 0
    }

    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.flags & Self::FLAG_ENABLED != 0
    }

    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.flags & Self::FLAG_DIRTY != 0
    }

    #[inline]
    pub fn set_visible(&mut self, visible: bool) {
        if visible {
            self.flags |= Self::FLAG_VISIBLE;
        } else {
            self.flags &= !Self::FLAG_VISIBLE;
        }
    }

    #[inline]
    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled {
            self.flags |= Self::FLAG_ENABLED;
        } else {
            self.flags &= !Self::FLAG_ENABLED;
        }
    }

    #[inline]
    pub fn set_dirty(&mut self, dirty: bool) {
        if dirty {
            self.flags |= Self::FLAG_DIRTY;
        } else {
            self.flags &= !Self::FLAG_DIRTY;
        }
    }
}

/// T4 Batch - Widget tree with arena allocation
///
/// Fixed capacity (60 nodes) for lockfree arena allocation.
/// Dirty tracking via bitmap for efficient layout passes.
#[repr(C, align(64))]
pub struct WidgetTreeCapsule {
    /// Generation counter (for versioning)
    generation: AtomicU64,
    /// Node count
    count: AtomicU32,
    /// Root node index
    root: AtomicU32,
    /// Free list head
    free_head: AtomicU32,
    /// Dirty node bitmap (64 bits, covers 60 nodes)
    dirty_bitmap: AtomicU64,

    /// Arena-allocated nodes (60 nodes × 32 bytes = 1920 bytes)
    nodes: [WidgetNode; 60],

    _pad: [u8; 40],
}

// #ASSUME: WidgetTreeCapsule is 64-byte aligned for cache performance
// #VERIFY: Static assertion below
const _: () = assert!(core::mem::align_of::<WidgetTreeCapsule>() == 64);

// #ASSUME: WidgetTreeCapsule fits in 4KB (64 cache lines)
// #VERIFY: Static assertion below
const _: () = assert!(core::mem::size_of::<WidgetTreeCapsule>() == 4096);

impl Default for WidgetTreeCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetTreeCapsule {
    /// Maximum nodes in tree
    pub const MAX_NODES: usize = 60;

    /// Create new widget tree
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            count: AtomicU32::new(0),
            root: AtomicU32::new(u32::MAX),
            free_head: AtomicU32::new(u32::MAX),
            dirty_bitmap: AtomicU64::new(0),
            nodes: [WidgetNode {
                type_id: 0,
                parent: u16::MAX,
                first_child: u16::MAX,
                next_sibling: u16::MAX,
                flags: 0,
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
            }; 60],
            _pad: [0; 40],
        }
    }

    /// Allocate node from arena
    ///
    /// Returns node index on success, None if tree is full.
    pub fn allocate_node(&self, type_id: u64) -> Option<u16> {
        let count = self.count.load(Ordering::Acquire);
        if count >= Self::MAX_NODES as u32 {
            return None;
        }

        // Try to allocate from free list first
        let mut free_head = self.free_head.load(Ordering::Acquire);
        if free_head != u32::MAX {
            // #ASSUME: Free head is valid index
            // #VERIFY: Bounds check below
            if free_head >= Self::MAX_NODES as u32 {
                return None;
            }

            let node = unsafe { &*self.nodes.as_ptr().add(free_head as usize) };
            let next_free = if node.next_sibling == u16::MAX {
                u32::MAX
            } else {
                node.next_sibling as u32
            };

            if self
                .free_head
                .compare_exchange(free_head, next_free, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // Initialize node
                let node_mut =
                    unsafe { &mut *self.nodes.as_ptr().add(free_head as usize).cast_mut() };
                *node_mut = WidgetNode {
                    type_id,
                    parent: u16::MAX,
                    first_child: u16::MAX,
                    next_sibling: u16::MAX,
                    flags: WidgetNode::FLAG_VISIBLE | WidgetNode::FLAG_ENABLED,
                    bounds: Rect::default(),
                };

                self.count.fetch_add(1, Ordering::Release);
                self.generation.fetch_add(1, Ordering::Release);
                return Some(free_head as u16);
            }
        }

        // Allocate new node from end
        let index = self.count.fetch_add(1, Ordering::AcqRel);
        if index >= Self::MAX_NODES as u32 {
            self.count.fetch_sub(1, Ordering::Release);
            return None;
        }

        // Initialize node
        let node_mut = unsafe { &mut *self.nodes.as_ptr().add(index as usize).cast_mut() };
        *node_mut = WidgetNode {
            type_id,
            parent: u16::MAX,
            first_child: u16::MAX,
            next_sibling: u16::MAX,
            flags: WidgetNode::FLAG_VISIBLE | WidgetNode::FLAG_ENABLED,
            bounds: Rect::default(),
        };

        self.generation.fetch_add(1, Ordering::Release);
        Some(index as u16)
    }

    /// Free node back to arena
    pub fn free_node(&self, index: u16) {
        if index >= Self::MAX_NODES as u16 {
            return;
        }

        // Remove from parent's child list
        let node = unsafe { &*self.nodes.as_ptr().add(index as usize) };
        let parent_idx = node.parent;
        if parent_idx != u16::MAX {
            self.remove_child(parent_idx, index);
        }

        // Add to free list
        let node_mut = unsafe { &mut *self.nodes.as_ptr().add(index as usize).cast_mut() };
        let old_free_head = self.free_head.load(Ordering::Acquire);
        node_mut.next_sibling = if old_free_head == u32::MAX {
            u16::MAX
        } else {
            old_free_head as u16
        };

        self.free_head.store(index as u32, Ordering::Release);
        self.count.fetch_sub(1, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Set parent of child node
    pub fn set_parent(&self, child: u16, parent: u16) {
        if child >= Self::MAX_NODES as u16 {
            return;
        }

        let child_node = unsafe { &mut *self.nodes.as_ptr().add(child as usize).cast_mut() };
        child_node.parent = parent;
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Add child to parent's child list
    pub fn add_child(&self, parent: u16, child: u16) {
        if parent >= Self::MAX_NODES as u16 || child >= Self::MAX_NODES as u16 {
            return;
        }

        let parent_node = unsafe { &mut *self.nodes.as_ptr().add(parent as usize).cast_mut() };
        let old_first_child = parent_node.first_child;

        let child_node = unsafe { &mut *self.nodes.as_ptr().add(child as usize).cast_mut() };
        child_node.parent = parent;
        child_node.next_sibling = old_first_child;

        parent_node.first_child = child;
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Remove child from parent's child list
    pub fn remove_child(&self, parent: u16, child: u16) {
        if parent >= Self::MAX_NODES as u16 || child >= Self::MAX_NODES as u16 {
            return;
        }

        let parent_node = unsafe { &mut *self.nodes.as_ptr().add(parent as usize).cast_mut() };

        // Check if child is first child
        if parent_node.first_child == child {
            let child_node = unsafe { &*self.nodes.as_ptr().add(child as usize) };
            parent_node.first_child = child_node.next_sibling;
        } else {
            // Find previous sibling
            let mut prev_sibling = parent_node.first_child;
            while prev_sibling != u16::MAX {
                let prev_node =
                    unsafe { &mut *self.nodes.as_ptr().add(prev_sibling as usize).cast_mut() };
                if prev_node.next_sibling == child {
                    let child_node = unsafe { &*self.nodes.as_ptr().add(child as usize) };
                    prev_node.next_sibling = child_node.next_sibling;
                    break;
                }
                prev_sibling = prev_node.next_sibling;
            }
        }

        let child_node = unsafe { &mut *self.nodes.as_ptr().add(child as usize).cast_mut() };
        child_node.parent = u16::MAX;
        child_node.next_sibling = u16::MAX;
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Mark node as dirty (needs layout)
    pub fn mark_dirty(&self, index: u16) {
        if index >= 60 {
            return;
        }

        let bit = 1u64 << index;
        self.dirty_bitmap.fetch_or(bit, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Clear all dirty flags
    pub fn clear_dirty(&self) {
        self.dirty_bitmap.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get node by index (unsafe but fast)
    ///
    /// # Safety
    /// Caller must ensure index < MAX_NODES
    #[inline]
    pub unsafe fn get_node_unchecked(&self, index: u16) -> &WidgetNode {
        &*self.nodes.as_ptr().add(index as usize)
    }

    /// Get mutable node by index (unsafe but fast)
    ///
    /// # Safety
    /// Caller must ensure index < MAX_NODES and no concurrent access
    #[inline]
    pub unsafe fn get_node_unchecked_mut(&self, index: u16) -> &mut WidgetNode {
        &mut *self.nodes.as_ptr().add(index as usize).cast_mut()
    }

    /// Iterate over dirty node indices
    pub fn iterate_dirty(&self) -> DirtyIterator {
        DirtyIterator {
            bitmap: self.dirty_bitmap.load(Ordering::Acquire),
            current: 0,
        }
    }

    /// Iterate over children of a parent node
    pub fn iterate_children(&self, parent: u16) -> ChildIterator {
        if parent >= Self::MAX_NODES as u16 {
            return ChildIterator {
                tree: self,
                current: u16::MAX,
            };
        }

        let parent_node = unsafe { &*self.nodes.as_ptr().add(parent as usize) };
        ChildIterator {
            tree: self,
            current: parent_node.first_child,
        }
    }

    /// Get current generation
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get node count
    #[inline]
    pub fn count(&self) -> u32 {
        self.count.load(Ordering::Acquire)
    }
}

/// Iterator over dirty node indices
pub struct DirtyIterator {
    bitmap: u64,
    current: u8,
}

impl Iterator for DirtyIterator {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current < 60 {
            let bit = 1u64 << self.current;
            let index = self.current;
            self.current += 1;

            if self.bitmap & bit != 0 {
                return Some(index as u16);
            }
        }
        None
    }
}

/// Iterator over child nodes
pub struct ChildIterator<'a> {
    tree: &'a WidgetTreeCapsule,
    current: u16,
}

impl<'a> Iterator for ChildIterator<'a> {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == u16::MAX {
            return None;
        }

        let result = self.current;
        let node = unsafe { &*self.tree.nodes.as_ptr().add(self.current as usize) };
        self.current = node.next_sibling;
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_single_node() {
        let tree = WidgetTreeCapsule::new();
        let node_idx = tree.allocate_node(42).expect("allocation failed");
        assert_eq!(node_idx, 0);
        assert_eq!(tree.count(), 1);

        let node = unsafe { tree.get_node_unchecked(node_idx) };
        assert_eq!(node.type_id, 42);
        assert!(node.is_visible());
        assert!(node.is_enabled());
    }

    #[test]
    fn test_allocate_multiple_nodes() {
        let tree = WidgetTreeCapsule::new();
        let mut indices = Vec::new();

        for i in 0..10 {
            let idx = tree.allocate_node(i as u64).expect("allocation failed");
            indices.push(idx);
        }

        assert_eq!(tree.count(), 10);
        for (i, &idx) in indices.iter().enumerate() {
            let node = unsafe { tree.get_node_unchecked(idx) };
            assert_eq!(node.type_id, i as u64);
        }
    }

    #[test]
    fn test_free_and_reuse_node() {
        let tree = WidgetTreeCapsule::new();
        let node1 = tree.allocate_node(1).unwrap();
        let node2 = tree.allocate_node(2).unwrap();

        tree.free_node(node1);
        assert_eq!(tree.count(), 1);

        let node3 = tree.allocate_node(3).unwrap();
        assert_eq!(node3, node1); // Should reuse freed slot

        let node = unsafe { tree.get_node_unchecked(node3) };
        assert_eq!(node.type_id, 3);
    }

    #[test]
    fn test_parent_child_relationships() {
        let tree = WidgetTreeCapsule::new();
        let parent = tree.allocate_node(1).unwrap();
        let child1 = tree.allocate_node(2).unwrap();
        let child2 = tree.allocate_node(3).unwrap();

        tree.add_child(parent, child1);
        tree.add_child(parent, child2);

        let children: Vec<_> = tree.iterate_children(parent).collect();
        assert_eq!(children.len(), 2);
        assert!(children.contains(&child1));
        assert!(children.contains(&child2));
    }

    #[test]
    fn test_remove_child() {
        let tree = WidgetTreeCapsule::new();
        let parent = tree.allocate_node(1).unwrap();
        let child1 = tree.allocate_node(2).unwrap();
        let child2 = tree.allocate_node(3).unwrap();

        tree.add_child(parent, child1);
        tree.add_child(parent, child2);
        tree.remove_child(parent, child1);

        let children: Vec<_> = tree.iterate_children(parent).collect();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0], child2);
    }

    #[test]
    fn test_dirty_tracking() {
        let tree = WidgetTreeCapsule::new();
        tree.allocate_node(1).unwrap();
        tree.allocate_node(2).unwrap();
        tree.allocate_node(3).unwrap();

        tree.mark_dirty(0);
        tree.mark_dirty(2);

        let dirty: Vec<_> = tree.iterate_dirty().collect();
        assert_eq!(dirty.len(), 2);
        assert!(dirty.contains(&0));
        assert!(dirty.contains(&2));
    }

    #[test]
    fn test_clear_dirty() {
        let tree = WidgetTreeCapsule::new();
        tree.allocate_node(1).unwrap();
        tree.mark_dirty(0);

        tree.clear_dirty();
        let dirty: Vec<_> = tree.iterate_dirty().collect();
        assert_eq!(dirty.len(), 0);
    }

    #[test]
    fn test_max_capacity() {
        let tree = WidgetTreeCapsule::new();

        // Allocate up to capacity
        for i in 0..WidgetTreeCapsule::MAX_NODES {
            assert!(tree.allocate_node(i as u64).is_some());
        }

        // Next allocation should fail
        assert!(tree.allocate_node(999).is_none());
    }
}

#[cfg(all(test, feature = "proptest"))]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_allocate_never_exceeds_capacity(type_ids in prop::collection::vec(any::<u64>(), 0..100)) {
            let tree = WidgetTreeCapsule::new();
            let mut allocated = 0;

            for type_id in type_ids {
                if tree.allocate_node(type_id).is_some() {
                    allocated += 1;
                }
            }

            assert!(allocated <= WidgetTreeCapsule::MAX_NODES);
            assert_eq!(tree.count() as usize, allocated.min(WidgetTreeCapsule::MAX_NODES));
        }

        #[test]
        fn prop_free_and_allocate_maintains_count(
            ops in prop::collection::vec((any::<bool>(), any::<u64>()), 1..50)
        ) {
            let tree = WidgetTreeCapsule::new();
            let mut allocated_indices = Vec::new();

            for (should_allocate, type_id) in ops {
                if should_allocate || allocated_indices.is_empty() {
                    if let Some(idx) = tree.allocate_node(type_id) {
                        allocated_indices.push(idx);
                    }
                } else if !allocated_indices.is_empty() {
                    let idx = allocated_indices.pop().unwrap();
                    tree.free_node(idx);
                }
            }

            assert_eq!(tree.count() as usize, allocated_indices.len());
        }

        #[test]
        fn prop_child_iteration_matches_added_children(
            children_count in 1usize..10
        ) {
            let tree = WidgetTreeCapsule::new();
            let parent = tree.allocate_node(0).unwrap();
            let mut expected_children = Vec::new();

            for i in 0..children_count {
                let child = tree.allocate_node(i as u64).unwrap();
                tree.add_child(parent, child);
                expected_children.push(child);
            }

            let actual_children: Vec<_> = tree.iterate_children(parent).collect();
            assert_eq!(actual_children.len(), expected_children.len());

            for child in expected_children {
                assert!(actual_children.contains(&child));
            }
        }

        #[test]
        fn prop_dirty_bitmap_correct(
            dirty_indices in prop::collection::vec(0u16..60, 0..20)
        ) {
            let tree = WidgetTreeCapsule::new();

            // Allocate enough nodes
            for i in 0..60 {
                tree.allocate_node(i as u64).unwrap();
            }

            // Mark dirty
            for &idx in &dirty_indices {
                tree.mark_dirty(idx);
            }

            let actual_dirty: Vec<_> = tree.iterate_dirty().collect();
            let expected_dirty: std::collections::HashSet<_> = dirty_indices.into_iter().collect();

            assert_eq!(actual_dirty.len(), expected_dirty.len());
            for idx in actual_dirty {
                assert!(expected_dirty.contains(&idx));
            }
        }
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod integration_tests {
    use super::*;

    #[test]
    fn test_complex_tree_operations() {
        let tree = WidgetTreeCapsule::new();

        // Build tree: root -> [child1, child2] where child1 -> [grandchild1, grandchild2]
        let root = tree.allocate_node(1).unwrap();
        let child1 = tree.allocate_node(2).unwrap();
        let child2 = tree.allocate_node(3).unwrap();
        let grandchild1 = tree.allocate_node(4).unwrap();
        let grandchild2 = tree.allocate_node(5).unwrap();

        tree.add_child(root, child1);
        tree.add_child(root, child2);
        tree.add_child(child1, grandchild1);
        tree.add_child(child1, grandchild2);

        // Verify structure
        let root_children: Vec<_> = tree.iterate_children(root).collect();
        assert_eq!(root_children.len(), 2);

        let child1_children: Vec<_> = tree.iterate_children(child1).collect();
        assert_eq!(child1_children.len(), 2);

        // Remove child1 (should not affect grandchildren yet)
        tree.remove_child(root, child1);
        let root_children: Vec<_> = tree.iterate_children(root).collect();
        assert_eq!(root_children.len(), 1);
    }

    #[test]
    fn test_generation_counter_increments() {
        let tree = WidgetTreeCapsule::new();
        let gen0 = tree.generation();

        tree.allocate_node(1).unwrap();
        let gen1 = tree.generation();
        assert!(gen1 > gen0);

        tree.mark_dirty(0);
        let gen2 = tree.generation();
        assert!(gen2 > gen1);
    }

    #[test]
    fn test_free_node_updates_parent() {
        let tree = WidgetTreeCapsule::new();
        let parent = tree.allocate_node(1).unwrap();
        let child = tree.allocate_node(2).unwrap();

        tree.add_child(parent, child);
        tree.free_node(child);

        let children: Vec<_> = tree.iterate_children(parent).collect();
        assert_eq!(children.len(), 0);
    }

    #[test]
    fn test_stress_allocation_free() {
        let tree = WidgetTreeCapsule::new();
        let mut allocated = Vec::new();

        // Allocate many nodes
        for i in 0..30 {
            if let Some(idx) = tree.allocate_node(i as u64) {
                allocated.push(idx);
            }
        }

        // Free half
        for &idx in allocated.iter().take(15) {
            tree.free_node(idx);
        }

        // Allocate again
        for i in 30..45 {
            tree.allocate_node(i as u64).unwrap();
        }

        assert_eq!(tree.count(), 30);
    }
}
