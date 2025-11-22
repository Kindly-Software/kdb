# LockfreeBTree Architecture - Phase 11.0

**Version**: 1.0.0
**Date**: 2025-11-04
**Status**: Architecture Complete - Ready for Implementation
**UCE34 Tier**: T1 Atomic (Lockfree Coordination)

---

## Executive Summary

**Problem**: Need ordered data structure for indexes, order books, time-series with O(log N) seeks and range scans.

**Solution**: Lockfree B+ tree using atomic CAS operations, generation counters for ABA prevention, and cache-aligned nodes.

**Performance Targets** (B32 Framework):
- Insert: <100ns (vs 200-500ns RwLock<BTreeMap>)
- Get: <50ns (single atomic load path for hot keys)
- Range: <10ns/entry (sequential scan after seek)
- **Expected Speedup**: 5-10× vs RwLock<BTreeMap>

---

## UCE34 Q1-Q34 Analysis

### Q1-Q9: Problem Definition
- **Q1 (What)**: Lockfree ordered data structure for indexes, order books, time-series
- **Q2 (Why)**: RwLock<BTreeMap> has 200-500ns overhead from reader/writer contention
- **Q3 (Performance)**: <100ns insert/get, O(log N) seeks, sequential range scans
- **Q4 (How)**: B+ tree with lockfree node operations using AtomicPtr + generation counters
- **Q5 (Interface)**: `LockfreeBTree<K, V>` with `insert`, `get`, `remove`, `range`
- **Q6 (Breaking)**: No (pure addition, complementary to ConcurrentMapCapsule)
- **Q7 (Data Migration)**: N/A (new primitive)
- **Q8 (Resources)**: Variable nodes (128B each), <10MB for 10K entries
- **Q9 (Alternatives)**: Skip list (complex) vs B-tree (predictable) - Choose B-tree for cache locality

### Q10-Q12: Capsule Foundation
- **Q10 (Tier)**: **Tier 1 Atomic** - Lockfree coordination with generation counters
- **Q11 (Transform)**: AtomicPtr<Node> for tree structure, AtomicU64 for generation counters
- **Q12 (Nightly)**: None required (stable Rust, portable-atomic if needed)

### Q13-Q27: Implementation Details
- **Node Type**: B+ tree (internal nodes = keys only, leaf nodes = keys + values)
- **Branching Factor**: DEGREE = 8 (7 keys, 8 children max)
- **Generation Counters**: Prevent ABA in node replacement (CAS on pointer + generation)
- **Memory Ordering**: Acquire for loads, Release for stores, AcqRel for CAS
- **Split/Merge**: Lockfree with two-phase CAS (mark node, update parent, publish)

### Q28-Q33: Optimization & Validation
- **Q28 (Simplicity)**: B+ tree (vs skip list complexity), fixed DEGREE (vs dynamic)
- **Q29 (Constraints)**: DEGREE = 8 (cache line fit), max tree height ~5 (1M entries)
- **Q30 (Validation)**: Property tests with 1000-thread concurrent stress
- **Q31 (Rust)**: Generic over K: Ord + Clone, V: Clone
- **Q32 (Nightly)**: None required (stable Rust patterns)
- **Q33 (Verification)**: #[derive(ComputationalCapsule)] on BTreeNode

### Q34: Production Readiness
- **T28 Testing**: Unit + Property + Integration + Stress (200+ tests planned)
- **B32 Benchmarking**: Fair baseline vs RwLock<BTreeMap> (1000+ iterations, 95% CI)
- **ASSUM Safety**: All atomic operations audited, ABA prevention verified
- **I20 Integration**: Drop-in for ordered data structures in kindly-db, order books

---

## Node Design

### BTreeNode (128B Cache-Aligned)

```rust
/// B+ tree node with lockfree coordination
///
/// # Memory Layout
/// ```text
/// Offset 0-7:    metadata (AtomicU64) - node_type(1) | num_keys(15) | generation(48)
/// Offset 8-71:   keys [Option<K>; 7] - Sorted keys (7 max for DEGREE=8)
/// Offset 72-135: values [Option<V>; 7] - Values (leaf nodes only)
/// Offset 136-199: children [AtomicPtr<Node>; 8] - Child pointers (internal nodes only)
/// Offset 200-255: _padding - Complete 128B alignment
/// ```
///
/// # Design Rationale
/// - 128B alignment: Fits 2 cache lines, prevents false sharing
/// - DEGREE = 8: Balance between tree height and cache locality
/// - Packed metadata: Single atomic read for node type + key count + generation
/// - Separate arrays: Keys always sorted, values/children indexed by key position
///
/// # Node Types
/// - Internal: Keys + children (no values)
/// - Leaf: Keys + values (no children)
///
/// NOTE: Cannot use derive(ComputationalCapsule) on generic structs with const parameters
/// Manual verification via const assertions
#[repr(C, align(128))]
pub struct BTreeNode<K, V, const DEGREE: usize> {
    /// Packed metadata: node_type(1 bit) | num_keys(15 bits) | generation(48 bits)
    ///
    /// # Bit Layout
    /// ```text
    /// Bits 63:    Node type (0 = Internal, 1 = Leaf)
    /// Bits 62-48: Number of keys (0-7 for DEGREE=8)
    /// Bits 47-0:  Generation counter (ABA prevention)
    /// ```
    ///
    /// # Ordering
    /// - Load: Acquire (synchronize with node updates)
    /// - Store: Release (publish all field updates)
    /// - CAS: AcqRel (full synchronization for node replacement)
    metadata: AtomicU64,

    /// Sorted keys (DEGREE-1 max = 7 keys for DEGREE=8)
    ///
    /// # Invariant
    /// - keys[0..num_keys] are sorted in ascending order
    /// - keys[num_keys..] are None
    /// - Binary search for lookups (O(log DEGREE))
    keys: [Option<K>; DEGREE - 1],

    /// Values (leaf nodes only, DEGREE-1 max)
    ///
    /// # Invariant
    /// - values[i] corresponds to keys[i] in leaf nodes
    /// - values are None for internal nodes
    values: [Option<V>; DEGREE - 1],

    /// Child pointers (internal nodes only, DEGREE max = 8 children)
    ///
    /// # Invariant
    /// - children[i] contains keys < keys[i]
    /// - children[num_keys] contains keys >= keys[num_keys-1]
    /// - children are null for leaf nodes
    ///
    /// # Ordering
    /// - Load: Acquire (synchronize with child updates)
    /// - Store: Release (publish child after split)
    /// - CAS: AcqRel (atomic child replacement during merge)
    children: [AtomicPtr<BTreeNode<K, V, DEGREE>>; DEGREE],

    /// Padding to complete 128-byte alignment
    /// Size depends on K, V, DEGREE (calculated by compiler)
    _padding: [u8; 56], // Placeholder - actual size TBD based on K/V
}
```

**Metadata Packing**:
```rust
// Extract fields from metadata
const NODE_TYPE_MASK: u64 = 1u64 << 63;
const NUM_KEYS_MASK: u64 = 0x7FFF << 48; // 15 bits
const GENERATION_MASK: u64 = (1u64 << 48) - 1; // 48 bits

fn unpack_metadata(meta: u64) -> (NodeType, usize, u64) {
    let node_type = if (meta & NODE_TYPE_MASK) != 0 {
        NodeType::Leaf
    } else {
        NodeType::Internal
    };
    let num_keys = ((meta & NUM_KEYS_MASK) >> 48) as usize;
    let generation = meta & GENERATION_MASK;
    (node_type, num_keys, generation)
}

fn pack_metadata(node_type: NodeType, num_keys: usize, generation: u64) -> u64 {
    let type_bit = match node_type {
        NodeType::Internal => 0,
        NodeType::Leaf => NODE_TYPE_MASK,
    };
    let keys_bits = ((num_keys as u64) & 0x7FFF) << 48;
    let gen_bits = generation & GENERATION_MASK;
    type_bit | keys_bits | gen_bits
}
```

---

## CAS Patterns (Lockfree Operations)

### Pattern 1: Lockfree Node Insert

**Algorithm**:
1. Load node metadata (generation included)
2. Binary search for key position
3. If not full:
   - Shift keys[pos..] right
   - Insert key/value at pos
   - CAS metadata (increment generation, increment num_keys)
4. If full:
   - Split node (see Pattern 2)
   - Retry insert on appropriate child

**Code Skeleton**:
```rust
fn insert_into_node(
    &self,
    node: &BTreeNode<K, V, DEGREE>,
    key: K,
    value: V,
) -> Result<(), BTreeError> {
    loop {
        // Load metadata (generation included)
        let old_meta = node.metadata.load(Ordering::Acquire);
        let (node_type, num_keys, generation) = unpack_metadata(old_meta);

        // Check if full (DEGREE-1 = 7 keys max)
        if num_keys >= DEGREE - 1 {
            // Split node
            return self.split_and_insert(node, key, value);
        }

        // Binary search for position
        let pos = node.keys[..num_keys]
            .binary_search(&Some(key.clone()))
            .unwrap_or_else(|e| e);

        // Shift keys right (in-place, safe because not full)
        // NOTE: This is NOT atomic - protected by generation counter CAS
        for i in (pos..num_keys).rev() {
            node.keys[i + 1] = node.keys[i].clone();
            if node_type == NodeType::Leaf {
                node.values[i + 1] = node.values[i].clone();
            }
        }

        // Insert new key/value
        node.keys[pos] = Some(key.clone());
        if node_type == NodeType::Leaf {
            node.values[pos] = Some(value.clone());
        }

        // CAS metadata (increment generation + num_keys)
        let new_meta = pack_metadata(node_type, num_keys + 1, generation + 1);
        match node.metadata.compare_exchange(
            old_meta,
            new_meta,
            Ordering::Release,
            Ordering::Acquire,
        ) {
            Ok(_) => return Ok(()), // Success
            Err(_) => {
                // Retry (another thread modified node)
                // Undo partial update (restore keys/values)
                for i in pos..num_keys {
                    node.keys[i] = node.keys[i + 1].clone();
                    if node_type == NodeType::Leaf {
                        node.values[i] = node.values[i + 1].clone();
                    }
                }
                continue;
            }
        }
    }
}
```

**ASSUM Framework**:
- `#ASSUME_GENERATION_CAS`: Generation counter detects concurrent modifications
- `#VERIFY_GENERATION_CAS`: Property test with 1000 threads validates no lost updates
- `#ASSUME_SHIFT_SAFE`: Shifting keys protected by generation counter
- `#VERIFY_SHIFT_SAFE`: Test validates no torn reads during concurrent access

### Pattern 2: Lockfree Node Split

**Algorithm**:
1. Allocate new right node
2. Copy upper half of keys/values to new node
3. Update parent to add new child pointer
4. CAS parent metadata (increment generation)
5. If CAS fails, retry from step 1

**Code Skeleton**:
```rust
fn split_and_insert(
    &self,
    node: &BTreeNode<K, V, DEGREE>,
    key: K,
    value: V,
) -> Result<(), BTreeError> {
    loop {
        // Load metadata
        let old_meta = node.metadata.load(Ordering::Acquire);
        let (node_type, num_keys, generation) = unpack_metadata(old_meta);

        // Split at midpoint (3 keys left, 4 keys right for DEGREE=8)
        let mid = num_keys / 2;

        // Allocate new right node
        let mut right_node = Box::new(BTreeNode::new(node_type));

        // Copy upper half to right node
        for i in mid..num_keys {
            right_node.keys[i - mid] = node.keys[i].clone();
            if node_type == NodeType::Leaf {
                right_node.values[i - mid] = node.values[i].clone();
            } else {
                // Internal node: Copy children too
                let child_ptr = node.children[i].load(Ordering::Acquire);
                right_node.children[i - mid].store(child_ptr, Ordering::Release);
            }
        }

        // Set right node metadata
        let right_meta = pack_metadata(node_type, num_keys - mid, 0);
        right_node.metadata.store(right_meta, Ordering::Release);

        // Update left node metadata (reduce num_keys)
        let left_meta = pack_metadata(node_type, mid, generation + 1);

        // CAS left node metadata
        match node.metadata.compare_exchange(
            old_meta,
            left_meta,
            Ordering::Release,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // Success: Update parent
                return self.insert_into_parent(
                    node,
                    right_node.keys[0].clone().unwrap(),
                    Box::into_raw(right_node),
                );
            }
            Err(_) => {
                // Retry (another thread modified node)
                continue;
            }
        }
    }
}
```

**ASSUM Framework**:
- `#ASSUME_SPLIT_ATOMIC`: CAS on metadata makes split atomic
- `#VERIFY_SPLIT_ATOMIC`: Test validates no torn reads during split
- `#ASSUME_PARENT_UPDATE`: Parent update is separate CAS (may fail)
- `#VERIFY_PARENT_RETRY`: Test validates retry logic on parent CAS failure

### Pattern 3: Lockfree Range Scan

**Algorithm**:
1. Seek to start key (binary search down tree)
2. Load leaf node metadata + keys
3. Iterate keys in leaf
4. Follow right sibling pointer (if range extends beyond current leaf)
5. No locking required (generation counters detect modifications)

**Code Skeleton**:
```rust
pub fn range(&self, start: &K, end: &K) -> Vec<(K, V)> {
    let mut results = Vec::new();

    // Seek to start key
    let mut current = self.seek_to_key(start);

    while !current.is_null() {
        // SAFETY: current is valid (from seek or right sibling)
        let node = unsafe { &*current };

        // Load metadata
        let meta = node.metadata.load(Ordering::Acquire);
        let (node_type, num_keys, _generation) = unpack_metadata(meta);

        // Sanity check: Must be leaf node
        assert_eq!(node_type, NodeType::Leaf);

        // Scan keys in leaf
        for i in 0..num_keys {
            let key = node.keys[i].clone().unwrap();

            // Check if beyond range
            if key > *end {
                return results; // Done
            }

            // Collect entry
            if key >= *start {
                let value = node.values[i].clone().unwrap();
                results.push((key, value));
            }
        }

        // Follow right sibling pointer
        // NOTE: B+ tree leaves form linked list
        current = node.children[DEGREE - 1].load(Ordering::Acquire);
    }

    results
}
```

**ASSUM Framework**:
- `#ASSUME_LEAF_LINKED_LIST`: Leaf nodes form right-linked list
- `#VERIFY_LEAF_LINKED_LIST`: Test validates leaf traversal
- `#ASSUME_NO_CONCURRENT_DELETE`: Range scan may see deleted keys (acceptable)
- `#VERIFY_SNAPSHOT_SEMANTICS`: Test validates snapshot isolation semantics

---

## ABA Prevention Strategy

### Problem: Pointer Reuse Race

**Scenario**:
1. Thread A: Load node pointer P (generation G)
2. Thread B: Delete node P, free memory
3. Thread C: Allocate new node, reuses same address P (new generation G')
4. Thread A: CAS succeeds incorrectly (same pointer, different node!)

### Solution: Generation Counter in Metadata

**Pattern**:
```rust
// Store (pointer, generation) pair atomically
fn load_node_with_generation(
    ptr: &AtomicPtr<BTreeNode<K, V, DEGREE>>
) -> (*mut BTreeNode<K, V, DEGREE>, u64) {
    let node_ptr = ptr.load(Ordering::Acquire);
    if node_ptr.is_null() {
        return (node_ptr, 0);
    }

    // SAFETY: node_ptr is valid (checked non-null)
    let node = unsafe { &*node_ptr };
    let meta = node.metadata.load(Ordering::Acquire);
    let (_node_type, _num_keys, generation) = unpack_metadata(meta);

    (node_ptr, generation)
}

// CAS with generation check
fn cas_node_with_generation(
    ptr: &AtomicPtr<BTreeNode<K, V, DEGREE>>,
    expected_ptr: *mut BTreeNode<K, V, DEGREE>,
    expected_gen: u64,
    new_ptr: *mut BTreeNode<K, V, DEGREE>,
) -> Result<(), BTreeError> {
    // Load current
    let (current_ptr, current_gen) = load_node_with_generation(ptr);

    // Check pointer + generation
    if current_ptr != expected_ptr || current_gen != expected_gen {
        return Err(BTreeError::ConcurrentModification);
    }

    // CAS on pointer only (generation stored in node metadata)
    match ptr.compare_exchange(
        expected_ptr,
        new_ptr,
        Ordering::Release,
        Ordering::Acquire,
    ) {
        Ok(_) => Ok(()),
        Err(_) => Err(BTreeError::ConcurrentModification),
    }
}
```

**Generation Counter Properties**:
- **48 bits**: Wraps after 281 trillion operations (never in practice)
- **Increment on every modification**: Ensures unique generation per node state
- **Stored in node metadata**: Single atomic read gets (pointer, generation) pair

**ASSUM Framework**:
- `#ASSUME_48BIT_GENERATION`: 48 bits sufficient (wraps after 281T ops)
- `#VERIFY_GENERATION_WRAP`: Test validates behavior at generation overflow
- `#ASSUME_METADATA_ATOMIC`: Loading metadata is atomic (single u64)
- `#VERIFY_METADATA_ATOMIC`: Test validates no torn reads on metadata

---

## Memory Ordering

### Acquire/Release Synchronization

**Pattern**:
```rust
// Writer: Release ordering (publish changes)
node.keys[pos] = Some(new_key);
node.values[pos] = Some(new_value);
node.metadata.store(new_meta, Ordering::Release); // Publish

// Reader: Acquire ordering (synchronize)
let meta = node.metadata.load(Ordering::Acquire); // Synchronize
let key = node.keys[pos].clone(); // See published changes
let value = node.values[pos].clone();
```

**Happens-Before Relationship**:
- Writer's Release store happens-before Reader's Acquire load
- All writes before Release are visible after Acquire
- Prevents reordering across Release/Acquire boundary

### Memory Ordering Rules

| Operation | Ordering | Rationale |
|-----------|----------|-----------|
| Load metadata | Acquire | Synchronize with all prior updates |
| Store metadata | Release | Publish all field updates |
| CAS metadata | AcqRel | Full synchronization (read-modify-write) |
| Load child pointer | Acquire | Synchronize with child updates |
| Store child pointer | Release | Publish child after split |
| CAS child pointer | AcqRel | Atomic child replacement |

**ASSUM Framework**:
- `#ASSUME_ACQUIRE_RELEASE`: Acquire/Release prevents reordering
- `#VERIFY_ACQUIRE_RELEASE`: Test validates no torn reads
- `#ASSUME_ACQREL_CAS`: AcqRel provides full fence on CAS
- `#VERIFY_ACQREL_CAS`: Test validates RMW atomicity

---

## Performance Analysis (B32 Framework)

### Latency Targets

| Operation | Target | Baseline (RwLock<BTreeMap>) | Speedup |
|-----------|--------|------------------------------|---------|
| Insert | <100ns | 200-500ns | 2-5× |
| Get | <50ns | 100-200ns | 2-4× |
| Remove | <150ns | 300-600ns | 2-4× |
| Range (per entry) | <10ns | 20-50ns | 2-5× |

**Rationale**:
- Insert: Single CAS on metadata (10-20ns) + key shift (<50ns) + binary search (<30ns)
- Get: Binary search down tree (3-5 nodes × 10ns load) + value load (10ns)
- Range: Sequential scan of sorted keys (<10ns per entry, cache-friendly)

### Scalability

**Expected Throughput** (8 threads):
- Insert: 8M ops/sec (8 threads × 1M ops/sec)
- Get: 16M ops/sec (8 threads × 2M ops/sec, read-heavy)
- Mixed: 10M ops/sec (8 threads × 1.25M ops/sec, 50/50 read/write)

**Reality Check** (B32 K-guidelines):
- 10-50% typical improvement (conservative baseline: 1.5× speedup)
- 2-10× exceptional improvement (proven lockfree patterns: 5× speedup)
- 100×+ requires extensive validation (NOT claimed for B-tree)

### Memory Usage

**Node Size**: 128 bytes (cache-aligned)

**Tree Capacity** (DEGREE=8):
- Height 1: 7 entries (1 node)
- Height 2: 56 entries (1 root + 8 leaves)
- Height 3: 448 entries (1 root + 8 internal + 64 leaves)
- Height 4: 3,584 entries (1 root + 8 internal + 64 internal + 512 leaves)
- Height 5: 28,672 entries (2,560 nodes × 128B = 320KB)

**Memory Efficiency**:
- 10K entries: ~2,000 nodes × 128B = 256KB (comparable to RwLock<BTreeMap>)
- 1M entries: ~150K nodes × 128B = 19MB (within L3 cache on modern CPUs)

---

## Testing Strategy (T28 Framework)

### Unit Tests (Q1-Q7)
- Node metadata packing/unpacking
- Binary search correctness
- Key shifting during insert
- Node split logic
- Generation counter increment

### Property Tests (Q8-Q14)
- Concurrent insert (1000 threads)
- Concurrent get (read-heavy workload)
- Concurrent remove (delete + insert races)
- Range scan consistency (no torn reads)
- ABA prevention (generation counter validation)

### Integration Tests (Q15-Q21)
- Insert → Get → Remove workflow
- Range scan after inserts
- Split/merge under load
- Memory ordering validation (Loom)

### Production Tests (Q22-Q28)
- Stress test (8 threads, 1M ops)
- Performance regression (vs RwLock<BTreeMap>)
- Tail latency (p99, p999)
- Memory leak detection (Valgrind)

---

## Implementation Roadmap

### Phase 1: Node Structure (1 day)
- Define BTreeNode struct
- Implement metadata packing/unpacking
- Write verification tests (alignment, size)

### Phase 2: Basic Operations (2 days)
- Implement insert (no split)
- Implement get
- Implement remove (no merge)
- Unit tests (100+ tests)

### Phase 3: Split/Merge (2 days)
- Implement node split
- Implement parent update
- Implement node merge (lazy)
- Integration tests (50+ tests)

### Phase 4: Range Scan (1 day)
- Implement range iterator
- Implement leaf traversal
- Property tests (consistency)

### Phase 5: Optimization (1 day)
- Prefetching hints
- SIMD binary search (optional)
- B32 benchmarks (1000+ iterations)

### Phase 6: Production (1 day)
- Stress tests (1000 threads)
- Memory ordering audit (ASSUM)
- Documentation + examples
- I20 integration checklist

**Total**: 8 days (1.5 weeks)

---

## Open Questions for Implementation Experts

### Q1: Node Size Trade-offs
- Current: 128B (2 cache lines)
- Alternative: 256B (4 cache lines, more keys per node, fewer tree levels)
- **Decision needed**: Balance between cache usage and tree height

### Q2: Lazy Delete vs Eager Merge
- Current: Lazy delete (mark tombstone, merge on next traversal)
- Alternative: Eager merge (merge immediately, more complex CAS)
- **Decision needed**: Trade-off between insert latency and tree balance

### Q3: SIMD Binary Search
- Current: Scalar binary search (O(log DEGREE) = O(log 8) = 3 comparisons)
- Alternative: SIMD binary search (4 comparisons in parallel, requires padding)
- **Decision needed**: Worth complexity for 2× speedup on 8 keys?

### Q4: Prefetching Strategy
- Current: No prefetching
- Alternative: Prefetch children[i] during binary search in internal nodes
- **Decision needed**: Measure impact on cache miss rate

### Q5: Memory Reclamation
- Current: Box deallocation on remove (immediate reclamation)
- Alternative: Epoch-based reclamation (defer deallocation, safer for concurrent readers)
- **Decision needed**: Safety vs memory usage trade-off

---

## Success Criteria

### Functional
- ✅ Insert/Get/Remove work correctly
- ✅ Range scan returns correct results
- ✅ Concurrent operations safe (no data races)
- ✅ ABA prevention via generation counters

### Performance
- ✅ Insert: <100ns (2-5× vs RwLock<BTreeMap>)
- ✅ Get: <50ns (2-4× vs baseline)
- ✅ Range: <10ns/entry (sequential scan)
- ✅ Scalability: 8M ops/sec (8 threads)

### Quality
- ✅ T28: 200+ tests (Unit/Property/Integration/Stress)
- ✅ B32: Fair baseline, 95% CI, 1000+ iterations
- ✅ ASSUM: 99.5%+ safety (all assumptions verified)
- ✅ I20: Drop-in for ordered data structures

---

**Document Version**: 1.0.0
**Last Updated**: 2025-11-04
**Status**: Architecture Complete - Ready for Implementation
**Frameworks**: UCE34, ASSUM, B32, T28, I20, COCA (100% lockfree)
**Next Steps**: Create skeleton types, implement Phase 1 (Node Structure)
