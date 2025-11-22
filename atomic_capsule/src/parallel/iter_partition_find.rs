//! # Phase 4: partition() and find() Operations
//!
//! Additional parallel iterator operations for VecParIter.

use super::iter::{ParallelIterator, SyncUnsafeCell, VecParIter};
use super::ParallelError;
use crate::parallel::scoped::get_global_pool;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;

impl<T: Send + Sync> VecParIter<T> {
    /// Partition elements into two collections based on predicate
    ///
    /// Implementation strategy:
    /// 1. Workers evaluate predicate in parallel on chunks
    /// 2. Each worker writes to matching/non-matching slots (Option<T>)
    /// 3. After scope, collect both Vecs from results
    ///
    /// #ASSUME_PARTITION_TWO_PASS: Two-pass collect (matching, then non-matching)
    /// #VERIFY_PARTITION_TWO_PASS: Unit test validates both Vecs maintain order
    pub fn partition_impl<P>(self, pred: P) -> (Vec<Self::Item>, Vec<Self::Item>)
    where
        P: Fn(&Self::Item) -> bool + Sync + Send,
        Self::Item: Send,
    {
        // Early exit for empty iterator
        if self.items.is_empty() {
            return (Vec::new(), Vec::new());
        }

        let len = self.items.len();

        // Pre-allocate result flags (Option<T> for each item)
        // We store the item and a bool indicating which partition it belongs to
        let results: Vec<SyncUnsafeCell<(Option<Self::Item>, bool)>> = (0..len)
            .map(|_| SyncUnsafeCell::new((None, false)))
            .collect();
        let results = Arc::new(results);

        let pool = match get_global_pool() {
            Ok(p) => p,
            Err(_) => {
                // Sequential fallback
                let mut matching = Vec::new();
                let mut non_matching = Vec::new();
                for item in self.items {
                    if pred(&item) {
                        matching.push(item);
                    } else {
                        non_matching.push(item);
                    }
                }
                return (matching, non_matching);
            }
        };

        let num_workers = pool.num_workers();
        let chunk_size = len.div_ceil(num_workers).max(1);

        let items = Arc::new(self.items);

        pool.scope(|s| {
            for chunk_idx in 0..num_workers {
                let start = chunk_idx * chunk_size;
                if start >= len {
                    break;
                }
                let end = (start + chunk_size).min(len);

                let pred_ref = &pred;

                let mut backoff = 1;
                loop {
                    let items_clone = Arc::clone(&items);
                    let results_clone = Arc::clone(&results);
                    match s.spawn(move || {
                        for i in start..end {
                            let item_ref = &items_clone[i];

                            // Evaluate predicate
                            let matches = pred_ref(item_ref);

                            // Move item to results with match flag
                            let item = unsafe { std::ptr::read(item_ref as *const Self::Item) };
                            unsafe {
                                let slot_ptr = results_clone[i].get();
                                (*slot_ptr) = (Some(item), matches);
                            }
                        }
                    }) {
                        Ok(_) => break,
                        Err(ParallelError::QueueFull) => {
                            for _ in 0..backoff {
                                std::hint::spin_loop();
                            }
                            backoff = (backoff * 2).min(1024);

                            if backoff >= 1024 {
                                // Re-clone for fallback path
                                let items_fallback = Arc::clone(&items);
                                let results_fallback = Arc::clone(&results);
                                for i in start..end {
                                    let item_ref = &items_fallback[i];
                                    let matches = pred_ref(item_ref);
                                    let item = unsafe {
                                        std::ptr::read(item_ref as *const Self::Item)
                                    };
                                    unsafe {
                                        let slot_ptr = results_fallback[i].get();
                                        (*slot_ptr) = (Some(item), matches);
                                    }
                                }
                                break;
                            }
                        }
                        Err(_) => {
                            // Re-clone for error fallback path
                            let items_fallback = Arc::clone(&items);
                            let results_fallback = Arc::clone(&results);
                            for i in start..end {
                                let item_ref = &items_fallback[i];
                                let matches = pred_ref(item_ref);
                                let item =
                                    unsafe { std::ptr::read(item_ref as *const Self::Item) };
                                unsafe {
                                    let slot_ptr = results_fallback[i].get();
                                    (*slot_ptr) = (Some(item), matches);
                                }
                            }
                            break;
                        }
                    }
                }
            }
        });

        // Collect both partitions from results
        let results_arc = Arc::try_unwrap(results).unwrap_or_else(|_| {
            panic!("Internal error: Arc refcount > 1 after scope completion")
        });

        let mut matching = Vec::new();
        let mut non_matching = Vec::new();

        for cell in results_arc {
            let (item_opt, matches) = cell.into_inner();
            if let Some(item) = item_opt {
                if matches {
                    matching.push(item);
                } else {
                    non_matching.push(item);
                }
            }
        }

        std::mem::forget(items);

        (matching, non_matching)
    }

    /// Find first element matching predicate (parallel early exit)
    ///
    /// Implementation strategy:
    /// 1. Workers check predicate in parallel on chunks
    /// 2. First worker to find a match sets AtomicBool flag + stores result index
    /// 3. Other workers exit early when flag is set
    /// 4. After scope, extract lowest-index match (deterministic)
    ///
    /// #ASSUME_FIND_FIRST: Returns lowest-index match (deterministic)
    /// #VERIFY_FIND_FIRST: Unit test validates determinism with multiple matches
    pub fn find_impl<P>(self, pred: P) -> Option<Self::Item>
    where
        P: Fn(&Self::Item) -> bool + Sync + Send,
        Self::Item: Send,
    {
        // Early exit for empty iterator
        if self.items.is_empty() {
            return None;
        }

        let len = self.items.len();

        // Early exit flag (lockfree coordination)
        let found = Arc::new(AtomicBool::new(false));

        // Store matching index and item (Option<(usize, T)>)
        // We use AtomicUsize for index to track the FIRST match
        let match_index = Arc::new(AtomicUsize::new(usize::MAX));
        let match_item: Arc<SyncUnsafeCell<Option<Self::Item>>> =
            Arc::new(SyncUnsafeCell::new(None));

        let pool = match get_global_pool() {
            Ok(p) => p,
            Err(_) => {
                // Sequential fallback
                for item in self.items {
                    if pred(&item) {
                        return Some(item);
                    }
                }
                return None;
            }
        };

        let num_workers = pool.num_workers();
        let chunk_size = len.div_ceil(num_workers).max(1);

        let items = Arc::new(self.items);

        pool.scope(|s| {
            for chunk_idx in 0..num_workers {
                let start = chunk_idx * chunk_size;
                if start >= len {
                    break;
                }
                let end = (start + chunk_size).min(len);

                let pred_ref = &pred;
                let found_ref = Arc::clone(&found);
                let match_index_ref = Arc::clone(&match_index);
                let match_item_ref = Arc::clone(&match_item);

                let mut backoff = 1;
                loop {
                    let items_clone = Arc::clone(&items);
                    let found_clone = Arc::clone(&found_ref);
                    let match_index_clone = Arc::clone(&match_index_ref);
                    let match_item_clone = Arc::clone(&match_item_ref);

                    match s.spawn(move || {
                        for i in start..end {
                            // Early exit if another worker found a match
                            if found_clone.load(AtomicOrdering::Acquire) {
                                return;
                            }

                            let item_ref = &items_clone[i];

                            // Check predicate
                            if pred_ref(item_ref) {
                                // Found a match! Try to claim it if it's the first one
                                // Use compare_exchange to ensure we only store the LOWEST index
                                let mut current_min = match_index_clone.load(AtomicOrdering::Acquire);
                                loop {
                                    if i >= current_min {
                                        // Another worker found an earlier match, skip
                                        break;
                                    }

                                    match match_index_clone.compare_exchange(
                                        current_min,
                                        i,
                                        AtomicOrdering::Release,
                                        AtomicOrdering::Acquire,
                                    ) {
                                        Ok(_) => {
                                            // We successfully claimed this index as the new minimum
                                            // Move item to result
                                            let item = unsafe {
                                                std::ptr::read(item_ref as *const Self::Item)
                                            };
                                            unsafe {
                                                let slot_ptr = match_item_clone.get();
                                                // SAFETY: Only one thread writes to this index
                                                // (we just claimed it via CAS above)
                                                (*slot_ptr) = Some(item);
                                            }

                                            // Set found flag (other workers will exit early)
                                            found_clone.store(true, AtomicOrdering::Release);
                                            return;
                                        }
                                        Err(new_min) => {
                                            // Another thread claimed a lower index, retry
                                            current_min = new_min;
                                            continue;
                                        }
                                    }
                                }

                                // Early exit after finding any match
                                return;
                            }
                        }
                    }) {
                        Ok(_) => break,
                        Err(ParallelError::QueueFull) => {
                            for _ in 0..backoff {
                                std::hint::spin_loop();
                            }
                            backoff = (backoff * 2).min(1024);

                            if backoff >= 1024 {
                                // Sequential fallback for this chunk
                                for i in start..end {
                                    if found_ref.load(AtomicOrdering::Acquire) {
                                        break;
                                    }

                                    let item_ref = &items[i];
                                    if pred_ref(item_ref) {
                                        let mut current_min =
                                            match_index_ref.load(AtomicOrdering::Acquire);
                                        loop {
                                            if i >= current_min {
                                                break;
                                            }

                                            match match_index_ref.compare_exchange(
                                                current_min,
                                                i,
                                                AtomicOrdering::Release,
                                                AtomicOrdering::Acquire,
                                            ) {
                                                Ok(_) => {
                                                    let item = unsafe {
                                                        std::ptr::read(item_ref as *const Self::Item)
                                                    };
                                                    unsafe {
                                                        let slot_ptr = match_item_ref.get();
                                                        (*slot_ptr) = Some(item);
                                                    }
                                                    found_ref.store(true, AtomicOrdering::Release);
                                                    break;
                                                }
                                                Err(new_min) => {
                                                    current_min = new_min;
                                                    continue;
                                                }
                                            }
                                        }
                                        break;
                                    }
                                }
                                break;
                            }
                        }
                        Err(_) => {
                            // Sequential fallback for this chunk on error
                            for i in start..end {
                                if found_ref.load(AtomicOrdering::Acquire) {
                                    break;
                                }

                                let item_ref = &items[i];
                                if pred_ref(item_ref) {
                                    let mut current_min =
                                        match_index_ref.load(AtomicOrdering::Acquire);
                                    loop {
                                        if i >= current_min {
                                            break;
                                        }

                                        match match_index_ref.compare_exchange(
                                            current_min,
                                            i,
                                            AtomicOrdering::Release,
                                            AtomicOrdering::Acquire,
                                        ) {
                                            Ok(_) => {
                                                let item = unsafe {
                                                    std::ptr::read(item_ref as *const Self::Item)
                                                };
                                                unsafe {
                                                    let slot_ptr = match_item_ref.get();
                                                    (*slot_ptr) = Some(item);
                                                }
                                                found_ref.store(true, AtomicOrdering::Release);
                                                break;
                                            }
                                            Err(new_min) => {
                                                current_min = new_min;
                                                continue;
                                            }
                                        }
                                    }
                                    break;
                                }
                            }
                            break;
                        }
                    }
                }
            }
        });

        // Extract result
        let result_item = Arc::try_unwrap(match_item)
            .unwrap_or_else(|_| panic!("Internal error: Arc refcount > 1 after scope completion"))
            .into_inner();

        std::mem::forget(items);

        result_item
    }
}
