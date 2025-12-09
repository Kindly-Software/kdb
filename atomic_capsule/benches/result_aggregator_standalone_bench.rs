//! # Result Aggregator Standalone Benchmark (B32 Framework)
//!
//! **Comprehensive B32 benchmarks comparing V1 vs V2 vs V3**
//!
//! This benchmark implements all three aggregator versions inline:
//! - **V1 Baseline**: Sharded Mutex<HashMap> (Phase 4-Parallel)
//! - **V2 Lockfree**: AtomicPtr-based hash table with Mutex<Vec> (Phase 4.4)
//! - **V3 100% Lockfree**: AtomicPtr + LockfreeList (Phase 15)
//!
//! ## B32 Framework Compliance
//! - Fair baselines: V1, V2, V3 all measured on same hardware
//! - 1000+ iterations per benchmark (Criterion default)
//! - 95% confidence intervals (Criterion default)
//! - Realistic workloads: single-thread, concurrent, same-key contention
//! - Honest reporting: Document where V2/V3 wins AND loses

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(feature = "std")]
use std::collections::hash_map::DefaultHasher;

const NUM_SHARDS: usize = 16;
const MAX_PROBE_DISTANCE: usize = 256;
const DEFAULT_CAPACITY: usize = 16384;

// State values for V2/V3 ResultSlot
const STATE_EMPTY: u32 = 0;
const STATE_OCCUPIED: u32 = 1;

/// Pack generation counter + state into AtomicU64
#[inline(always)]
const fn pack_gen_state(gen: u32, state: u32) -> u64 {
    ((gen as u64) << 32) | (state as u64)
}

#[inline(always)]
const fn unpack_gen_state(packed: u64) -> (u32, u32) {
    let gen = (packed >> 32) as u32;
    let state = (packed & 0xFFFFFFFF) as u32;
    (gen, state)
}

// ==============================================================================
// V1 BASELINE: Sharded Mutex<HashMap>
// ==============================================================================

struct V1ShardedAggregator<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    shards: [Arc<Mutex<HashMap<K, Vec<V>>>>; NUM_SHARDS],
}

impl<K, V> V1ShardedAggregator<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    pub fn with_capacity(total_capacity: usize) -> Self {
        let shard_capacity = (total_capacity + NUM_SHARDS - 1) / NUM_SHARDS;

        Self {
            shards: [
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
                Arc::new(Mutex::new(HashMap::with_capacity(shard_capacity))),
            ],
        }
    }

    pub fn insert(&self, key: K, value: V) {
        let shard_idx = self.shard_index(&key);
        let shard = &self.shards[shard_idx];
        let mut map = shard.lock().unwrap();
        map.entry(key).or_insert_with(Vec::new).push(value);
    }

    pub fn merge(&self) -> HashMap<K, Vec<V>> {
        let mut result = HashMap::new();

        for shard in &self.shards {
            let map = shard.lock().unwrap();
            for (key, values) in map.iter() {
                result
                    .entry(key.clone())
                    .or_insert_with(Vec::new)
                    .extend(values.iter().cloned());
            }
        }

        result
    }

    pub fn len(&self) -> usize {
        self.shards.iter().map(|s| s.lock().unwrap().len()).sum()
    }

    fn shard_index(&self, key: &K) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();
        (hash as usize) % NUM_SHARDS
    }
}

// ==============================================================================
// V2/V3 Shared: Lockfree List for Multi-Value Storage
// ==============================================================================

/// Simple lockfree linked list node for V3
#[repr(align(8))]
struct ListNode<V> {
    value: V,
    next: AtomicPtr<ListNode<V>>,
}

impl<V> ListNode<V> {
    fn new(value: V) -> *mut Self {
        Box::into_raw(Box::new(Self {
            value,
            next: AtomicPtr::new(std::ptr::null_mut()),
        }))
    }
}

/// Simple lockfree list for V3 multi-value storage
struct LockfreeList<V> {
    head: AtomicPtr<ListNode<V>>,
}

impl<V> LockfreeList<V> {
    fn new() -> Self {
        Self {
            head: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    fn push(&self, value: V) {
        let new_node = ListNode::new(value);
        loop {
            let old_head = self.head.load(Ordering::Acquire);
            unsafe {
                (*new_node).next.store(old_head, Ordering::Release);
            }
            if self
                .head
                .compare_exchange(old_head, new_node, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
    }

    fn iter(&self) -> LockfreeListIter<V> {
        LockfreeListIter {
            current: self.head.load(Ordering::Acquire),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<V> Drop for LockfreeList<V> {
    fn drop(&mut self) {
        let mut current = self.head.load(Ordering::Acquire);
        while !current.is_null() {
            unsafe {
                let node = Box::from_raw(current);
                current = node.next.load(Ordering::Acquire);
            }
        }
    }
}

struct LockfreeListIter<V> {
    current: *mut ListNode<V>,
    _phantom: std::marker::PhantomData<V>,
}

impl<V: Clone> Iterator for LockfreeListIter<V> {
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.is_null() {
            None
        } else {
            unsafe {
                let value = (*self.current).value.clone();
                self.current = (*self.current).next.load(Ordering::Acquire);
                Some(value)
            }
        }
    }
}

// ==============================================================================
// ResultSlot: Generic slot for V2 (Mutex<Vec>) and V3 (LockfreeList)
// ==============================================================================

#[repr(C, align(128))]
struct ResultSlot<K, V, T> {
    state: AtomicU64,
    hash: AtomicU64,
    key_ptr: AtomicPtr<K>,
    values_ptr: AtomicPtr<T>,
    _padding: [u8; 96],
    _phantom: std::marker::PhantomData<V>,
}

impl<K, V, T> ResultSlot<K, V, T> {
    const fn new() -> Self {
        Self {
            state: AtomicU64::new(pack_gen_state(0, STATE_EMPTY)),
            hash: AtomicU64::new(0),
            key_ptr: AtomicPtr::new(std::ptr::null_mut()),
            values_ptr: AtomicPtr::new(std::ptr::null_mut()),
            _padding: [0u8; 96],
            _phantom: std::marker::PhantomData,
        }
    }

    fn is_empty(&self) -> bool {
        let (_, state) = unpack_gen_state(self.state.load(Ordering::Acquire));
        state == STATE_EMPTY
    }

    fn matches(&self, hash: u64, key: &K) -> bool
    where
        K: Eq,
    {
        if self.hash.load(Ordering::Acquire) != hash {
            return false;
        }
        let (_, state) = unpack_gen_state(self.state.load(Ordering::Acquire));
        if state != STATE_OCCUPIED {
            return false;
        }
        let key_ptr = self.key_ptr.load(Ordering::Acquire);
        if key_ptr.is_null() {
            return false;
        }
        unsafe { *key_ptr == *key }
    }

    fn try_claim(&self, hash: u64, key_ptr: *mut K, values_ptr: *mut T) -> Result<(), ()> {
        let old_packed = pack_gen_state(0, STATE_EMPTY);
        let new_packed = pack_gen_state(1, STATE_OCCUPIED);

        match self.state.compare_exchange(
            old_packed,
            new_packed,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                self.hash.store(hash, Ordering::Release);
                self.key_ptr.store(key_ptr, Ordering::Release);
                self.values_ptr.store(values_ptr, Ordering::Release);
                Ok(())
            }
            Err(_) => Err(()),
        }
    }
}

impl<K, V, T> Drop for ResultSlot<K, V, T> {
    fn drop(&mut self) {
        let (_, state) = unpack_gen_state(self.state.load(Ordering::Acquire));
        if state == STATE_OCCUPIED {
            let key_ptr = self.key_ptr.load(Ordering::Acquire);
            if !key_ptr.is_null() {
                unsafe {
                    let _ = Box::from_raw(key_ptr);
                }
            }
            let values_ptr = self.values_ptr.load(Ordering::Acquire);
            if !values_ptr.is_null() {
                unsafe {
                    let _ = Box::from_raw(values_ptr);
                }
            }
        }
    }
}

// ==============================================================================
// V2: Lockfree AtomicPtr + Mutex<Vec> (99% lockfree, NOT 100% Chaos)
// ==============================================================================

struct V2AtomicAggregator<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    slots: Vec<ResultSlot<K, V, Mutex<Vec<V>>>>,
    capacity: usize,
}

impl<K, V> V2AtomicAggregator<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    pub fn with_capacity(capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            slots.push(ResultSlot::new());
        }
        Self { slots, capacity }
    }

    pub fn insert(&self, key: K, value: V) -> Result<(), ()> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();
        if hash == 0 {
            return Err(());
        }

        let start_idx = (hash as usize) % self.capacity;

        for probe in 0..MAX_PROBE_DISTANCE {
            let idx = (start_idx + probe) % self.capacity;
            let slot = &self.slots[idx];

            if slot.matches(hash, &key) {
                // Found existing key
                let values_ptr = slot.values_ptr.load(Ordering::Acquire);
                if !values_ptr.is_null() {
                    unsafe {
                        (*values_ptr).lock().unwrap().push(value);
                    }
                    return Ok(());
                }
            } else if slot.is_empty() {
                // Try to claim empty slot
                let key_box = Box::into_raw(Box::new(key.clone()));
                let mut vec = Vec::new();
                vec.push(value.clone());
                let values_box = Box::into_raw(Box::new(Mutex::new(vec)));

                if slot.try_claim(hash, key_box, values_box).is_ok() {
                    return Ok(());
                } else {
                    // Failed to claim, clean up
                    unsafe {
                        let _ = Box::from_raw(key_box);
                        let _ = Box::from_raw(values_box);
                    }
                }
            }
        }

        Err(()) // Capacity exceeded
    }

    pub fn merge(&self) -> HashMap<K, Vec<V>> {
        let mut result = HashMap::new();
        for slot in &self.slots {
            if !slot.is_empty() {
                let key_ptr = slot.key_ptr.load(Ordering::Acquire);
                let values_ptr = slot.values_ptr.load(Ordering::Acquire);
                if !key_ptr.is_null() && !values_ptr.is_null() {
                    unsafe {
                        let key = (*key_ptr).clone();
                        let values = (*values_ptr).lock().unwrap().clone();
                        result.entry(key).or_insert_with(Vec::new).extend(values);
                    }
                }
            }
        }
        result
    }

    pub fn len(&self) -> usize {
        self.slots.iter().filter(|s| !s.is_empty()).count()
    }
}

// ==============================================================================
// V3: 100% Lockfree AtomicPtr + LockfreeList (100% Chaos Compliant)
// ==============================================================================

struct V3LockfreeAggregator<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    slots: Vec<ResultSlot<K, V, LockfreeList<V>>>,
    capacity: usize,
}

impl<K, V> V3LockfreeAggregator<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    pub fn with_capacity(capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            slots.push(ResultSlot::new());
        }
        Self { slots, capacity }
    }

    pub fn insert(&self, key: K, value: V) -> Result<(), ()> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();
        if hash == 0 {
            return Err(());
        }

        let start_idx = (hash as usize) % self.capacity;

        for probe in 0..MAX_PROBE_DISTANCE {
            let idx = (start_idx + probe) % self.capacity;
            let slot = &self.slots[idx];

            if slot.matches(hash, &key) {
                // Found existing key
                let values_ptr = slot.values_ptr.load(Ordering::Acquire);
                if !values_ptr.is_null() {
                    unsafe {
                        (*values_ptr).push(value);
                    }
                    return Ok(());
                }
            } else if slot.is_empty() {
                // Try to claim empty slot
                let key_box = Box::into_raw(Box::new(key.clone()));
                let list = Box::into_raw(Box::new(LockfreeList::new()));
                unsafe {
                    (*list).push(value.clone());
                }

                if slot.try_claim(hash, key_box, list).is_ok() {
                    return Ok(());
                } else {
                    // Failed to claim, clean up
                    unsafe {
                        let _ = Box::from_raw(key_box);
                        let _ = Box::from_raw(list);
                    }
                }
            }
        }

        Err(()) // Capacity exceeded
    }

    pub fn merge(&self) -> HashMap<K, Vec<V>> {
        let mut result = HashMap::new();
        for slot in &self.slots {
            if !slot.is_empty() {
                let key_ptr = slot.key_ptr.load(Ordering::Acquire);
                let values_ptr = slot.values_ptr.load(Ordering::Acquire);
                if !key_ptr.is_null() && !values_ptr.is_null() {
                    unsafe {
                        let key = (*key_ptr).clone();
                        let values: Vec<V> = (*values_ptr).iter().collect();
                        result.entry(key).or_insert_with(Vec::new).extend(values);
                    }
                }
            }
        }
        result
    }

    pub fn len(&self) -> usize {
        self.slots.iter().filter(|s| !s.is_empty()).count()
    }
}

// ==============================================================================
// BENCHMARKS: B32 Framework Compliance
// ==============================================================================

/// B1: Single-threaded insert (10K operations) - Baseline performance
fn bench_insert_single_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_single_thread");
    group.throughput(Throughput::Elements(10_000));
    group.warm_up_time(Duration::from_secs(2));

    group.bench_function("V1_ShardedMutex", |b| {
        b.iter(|| {
            let agg: V1ShardedAggregator<u64, u64> = V1ShardedAggregator::with_capacity(10_000);
            for i in 0..10_000 {
                black_box(agg.insert(i, i * 10));
            }
        });
    });

    group.bench_function("V2_AtomicWithMutexVec", |b| {
        b.iter(|| {
            let agg: V2AtomicAggregator<u64, u64> = V2AtomicAggregator::with_capacity(16384);
            for i in 0..10_000 {
                black_box(agg.insert(i, i * 10).ok());
            }
        });
    });

    group.bench_function("V3_LockfreeList", |b| {
        b.iter(|| {
            let agg: V3LockfreeAggregator<u64, u64> = V3LockfreeAggregator::with_capacity(16384);
            for i in 0..10_000 {
                black_box(agg.insert(i, i * 10).ok());
            }
        });
    });

    group.finish();
}

/// B2: Merge latency (10K-100K results) - Hash table iteration overhead
fn bench_merge_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge_latency");
    group.warm_up_time(Duration::from_secs(2));

    for size in [10_000, 50_000, 100_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("V1_ShardedMutex", size),
            size,
            |b, &size| {
                let agg: V1ShardedAggregator<u64, u64> = V1ShardedAggregator::with_capacity(size);
                for i in 0..size {
                    agg.insert(i as u64, i as u64 * 10);
                }

                b.iter(|| {
                    let results = agg.merge();
                    black_box(results);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("V2_AtomicWithMutexVec", size),
            size,
            |b, &size| {
                let capacity = (size * 3 / 2).max(16384);
                let agg: V2AtomicAggregator<u64, u64> = V2AtomicAggregator::with_capacity(capacity);
                for i in 0..size {
                    let _ = agg.insert(i as u64, i as u64 * 10);
                }

                b.iter(|| {
                    let results = agg.merge();
                    black_box(results);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("V3_LockfreeList", size),
            size,
            |b, &size| {
                let capacity = (size * 3 / 2).max(16384);
                let agg: V3LockfreeAggregator<u64, u64> =
                    V3LockfreeAggregator::with_capacity(capacity);
                for i in 0..size {
                    let _ = agg.insert(i as u64, i as u64 * 10);
                }

                b.iter(|| {
                    let results = agg.merge();
                    black_box(results);
                });
            },
        );
    }

    group.finish();
}

/// B3: Concurrent throughput (1-16 threads, unique keys) - Parallel scaling
fn bench_concurrent_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_throughput");
    group.warm_up_time(Duration::from_secs(2));

    for num_threads in [1, 2, 4, 8, 16].iter() {
        let total_ops = num_threads * 10_000;
        group.throughput(Throughput::Elements(total_ops as u64));

        group.bench_with_input(
            BenchmarkId::new("V1_ShardedMutex", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let agg = Arc::new(V1ShardedAggregator::with_capacity(total_ops));
                    let mut handles = vec![];

                    for thread_id in 0..num_threads {
                        let agg_clone = Arc::clone(&agg);
                        let handle = thread::spawn(move || {
                            for i in 0..10_000 {
                                let key = (thread_id * 10_000 + i) as u64;
                                agg_clone.insert(key, thread_id as u64);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(agg.len());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("V2_AtomicWithMutexVec", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let capacity = (total_ops * 3 / 2).max(16384);
                    let agg = Arc::new(V2AtomicAggregator::with_capacity(capacity));
                    let mut handles = vec![];

                    for thread_id in 0..num_threads {
                        let agg_clone = Arc::clone(&agg);
                        let handle = thread::spawn(move || {
                            for i in 0..10_000 {
                                let key = (thread_id * 10_000 + i) as u64;
                                let _ = agg_clone.insert(key, thread_id as u64);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(agg.len());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("V3_LockfreeList", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let capacity = (total_ops * 3 / 2).max(16384);
                    let agg = Arc::new(V3LockfreeAggregator::with_capacity(capacity));
                    let mut handles = vec![];

                    for thread_id in 0..num_threads {
                        let agg_clone = Arc::clone(&agg);
                        let handle = thread::spawn(move || {
                            for i in 0..10_000 {
                                let key = (thread_id * 10_000 + i) as u64;
                                let _ = agg_clone.insert(key, thread_id as u64);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(agg.len());
                });
            },
        );
    }

    group.finish();
}

/// B4: Same-key contention (2-16 threads, 100 keys) - Worst-case stress test
fn bench_same_key_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("same_key_contention");
    group.warm_up_time(Duration::from_secs(2));

    for num_threads in [2, 4, 8, 16].iter() {
        let ops_per_thread = 10_000;
        let total_ops = num_threads * ops_per_thread;
        group.throughput(Throughput::Elements(total_ops as u64));

        group.bench_with_input(
            BenchmarkId::new("V1_ShardedMutex", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let agg = Arc::new(V1ShardedAggregator::with_capacity(100));
                    let mut handles = vec![];

                    for thread_id in 0..num_threads {
                        let agg_clone = Arc::clone(&agg);
                        let handle = thread::spawn(move || {
                            for i in 0..ops_per_thread {
                                let key = (i % 100) as u64;
                                agg_clone.insert(key, thread_id as u64);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(agg.len());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("V2_AtomicWithMutexVec", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let agg = Arc::new(V2AtomicAggregator::with_capacity(16384));
                    let mut handles = vec![];

                    for thread_id in 0..num_threads {
                        let agg_clone = Arc::clone(&agg);
                        let handle = thread::spawn(move || {
                            for i in 0..ops_per_thread {
                                let key = (i % 100) as u64;
                                let _ = agg_clone.insert(key, thread_id as u64);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(agg.len());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("V3_LockfreeList", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let agg = Arc::new(V3LockfreeAggregator::with_capacity(16384));
                    let mut handles = vec![];

                    for thread_id in 0..num_threads {
                        let agg_clone = Arc::clone(&agg);
                        let handle = thread::spawn(move || {
                            for i in 0..ops_per_thread {
                                let key = (i % 100) as u64;
                                let _ = agg_clone.insert(key, thread_id as u64);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(agg.len());
                });
            },
        );
    }

    group.finish();
}

/// B5: V3 Thread-local flush overhead (measure LockfreeList push performance)
fn bench_v3_threadlocal_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("v3_threadlocal_flush");
    group.warm_up_time(Duration::from_secs(2));

    for values_per_key in [1, 10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*values_per_key as u64));

        group.bench_with_input(
            BenchmarkId::new("V3_LockfreeList_Push", values_per_key),
            values_per_key,
            |b, &values_per_key| {
                b.iter(|| {
                    let agg: V3LockfreeAggregator<u64, u64> =
                        V3LockfreeAggregator::with_capacity(16384);
                    for _i in 0..values_per_key {
                        black_box(agg.insert(42, 100).ok());
                    }
                });
            },
        );
    }

    group.finish();
}

/// B6: V3 Scalability test (1-16 threads, measure efficiency)
fn bench_v3_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("v3_scalability");
    group.warm_up_time(Duration::from_secs(2));

    for num_threads in [1, 2, 4, 8, 16].iter() {
        let total_ops = num_threads * 10_000;
        group.throughput(Throughput::Elements(total_ops as u64));

        group.bench_with_input(
            BenchmarkId::new("V3_LockfreeList_Scaling", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let capacity = (total_ops * 3 / 2).max(16384);
                    let agg = Arc::new(V3LockfreeAggregator::with_capacity(capacity));
                    let mut handles = vec![];

                    for thread_id in 0..num_threads {
                        let agg_clone = Arc::clone(&agg);
                        let handle = thread::spawn(move || {
                            for i in 0..10_000 {
                                let key = (thread_id * 10_000 + i) as u64;
                                let _ = agg_clone.insert(key, thread_id as u64);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(agg.len());
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_insert_single_thread,
    bench_merge_latency,
    bench_concurrent_throughput,
    bench_same_key_contention,
    bench_v3_threadlocal_flush,
    bench_v3_scalability,
);
criterion_main!(benches);
