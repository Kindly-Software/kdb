# QEC Integration Layer - UCE34 Analysis (Q1-Q34)

**Phase**: Q3.6-C Specialized Surface Code Simulator - Integration Layer
**Version**: 1.0.0
**Date**: 2025-11-21
**Framework**: UCE34 Systematic Discovery + Chaos Computational Capsules

---

## Executive Summary

**Mission**: Design lockfree QEC integration layer orchestrating syndrome extraction → decoding → error correction with <100μs total closed-loop latency for surface code quantum error correction.

**Architecture**: T4 Batch + T5 Streaming hybrid capsule coordinating 5 specialized capsules via atomic state machine and lockfree ring buffers.

**Performance Target**: <100μs QEC cycle (30μs syndrome + 50μs decode + 20μs correct), 10K rounds/sec throughput, >90% logical error suppression.

**Innovation**: Adaptive decoder selection (Union-Find for sparse syndromes, MWPM for dense syndromes) with zero-copy syndrome sharing and lockfree pipeline coordination.

---

## Q1-Q9: Problem Understanding

### Q1: What is the stated problem?

**User Request**: Design QEC integration layer connecting stabilizer simulation with decoders for closed-loop quantum error correction.

**Core Challenge**: Orchestrate syndrome extraction → decoding → error correction pipeline with <100μs latency while maintaining:
- Lockfree coordination (no mutex blocking)
- Zero-copy syndrome sharing (avoid allocation overhead)
- Adaptive decoder selection (Union-Find vs MWPM based on syndrome characteristics)
- Correctness guarantees (no syndrome drops, exact-once correction)
- Production-grade monitoring (latency histograms, error rates, decoder accuracy)

**Why Hard**:
1. **Latency Constraints**: <100μs total budget across 3 pipeline stages (syndrome/decode/correct)
2. **Coordination Complexity**: 5 capsules with heterogeneous APIs and performance profiles
3. **Adaptive Logic**: Runtime decoder selection based on syndrome weight and error patterns
4. **Correctness**: Must preserve quantum state coherence despite probabilistic decoding
5. **Lockfree Design**: All coordination via atomics (no mutex/RwLock allowed)

### Q2: What are the performance requirements?

**Primary Metrics**:
- **Closed-Loop Latency**: <100μs (syndrome + decode + correct)
  - Syndrome extraction: <30μs (parallel stabilizer measurements)
  - Decoding: <50μs (Union-Find) or <100μs (MWPM, fallback to next cycle)
  - Correction: <20μs (Pauli operator application)
- **Throughput**: 10,000 QEC rounds/sec (100μs per round)
- **Logical Error Rate**: >90% suppression vs uncorrected baseline
- **Memory**: <10MB total (stabilizer + syndrome buffers + decoder state)

**Secondary Metrics**:
- **Syndrome Buffer**: 256 entries × 256 bytes = 64KB ring buffer
- **Decoder Accuracy**: >95% matching ideal decoder (no false positives)
- **Pipeline Utilization**: >80% (minimize idle time between stages)
- **Monitoring Overhead**: <5% (telemetry must not dominate latency)

**Constraints**:
- 100% lockfree (Chaos mandate)
- Cache-aligned (64B header, 256B syndrome entries)
- ASSUM 99.99% safe (all assumptions verified)
- Zero unsafe in fast path (decoder coordination only)

### Q3: What is the input/output specification?

**Inputs**:
1. **Stabilizer State**: Reference to `StabilizerStateCapsule` (Q3.6-A)
   - Pauli tableau representation
   - Clifford operator tracking
   - Phase information
2. **Error Model**: Physical error rates (X/Y/Z/depolarizing)
   - Single-qubit: p₁ ≈ 0.001 (0.1%)
   - Two-qubit: p₂ ≈ 0.01 (1%)
3. **QEC Parameters**:
   - Code distance: d = 3, 5, 7, 9 (surface code)
   - Decoder choice: Auto (adaptive), Union-Find (force), MWPM (force)
   - Syndrome buffer depth: 256 (default), 512 (low-latency mode)

**Outputs**:
1. **Corrected State**: Updated `StabilizerStateCapsule` after Pauli correction
2. **QEC Telemetry**:
   - Syndrome extraction time (ns)
   - Decoding time (ns)
   - Correction time (ns)
   - Total QEC cycle time (ns)
   - Logical error flag (bool)
   - Decoder used (Union-Find/MWPM)
3. **Error Statistics**:
   - Physical error rate (measured)
   - Logical error rate (measured)
   - Decoder accuracy (vs ideal)
   - Syndrome weight distribution

### Q4: What are the algorithmic constraints?

**QEC Pipeline Stages**:

1. **Syndrome Extraction** (T4 Batch parallel):
   ```
   For each stabilizer Sᵢ:
     measurement[i] = measure_stabilizer(Sᵢ, current_state)

   syndrome[t] = measurement[t] ⊕ measurement[t-1]  // Temporal difference
   ```
   - Parallelizable: Independent stabilizer measurements
   - Latency: O(d²) measurements for d×d surface code
   - Coordination: Atomic completion counter

2. **Decoding** (T5 Streaming adaptive):
   ```
   syndrome_weight = popcount(syndrome)

   if syndrome_weight < threshold:
     corrections = union_find_decode(syndrome)  // <50μs
   else:
     corrections = mwpm_decode(syndrome)        // <100μs (may defer to next cycle)
   ```
   - Adaptive: Choose decoder based on syndrome characteristics
   - Streaming: Process syndromes as they arrive (no batch wait)
   - Coordination: Atomic decoder state machine

3. **Error Correction** (T1 Atomic updates):
   ```
   For each correction (qubit_id, pauli_op):
     apply_pauli(stabilizer_state, qubit_id, pauli_op)
   ```
   - Sequential: Pauli operators don't commute (order matters)
   - Lockfree: Atomic Pauli tableau updates
   - Latency: O(num_corrections) ≈ syndrome_weight

**Correctness Requirements**:
- **Exactly-Once Semantics**: Each syndrome processed exactly once
- **No Syndrome Drops**: Ring buffer never overwrites unprocessed syndromes
- **Causal Ordering**: Corrections applied in syndrome temporal order
- **State Consistency**: Stabilizer state remains valid after corrections

### Q5: What are the data dependencies?

**Pipeline Data Flow**:
```
StabilizerStateCapsule (shared, concurrent reads)
         ↓
SyndromeExtractionCapsule (parallel measurements → syndrome)
         ↓
SyndromeRingBuffer (lockfree producer-consumer)
         ↓
DecoderScheduler (atomic state machine → choose Union-Find or MWPM)
         ↓
UnionFindDecoderCapsule OR MWPMDecoderCapsule (syndrome → corrections)
         ↓
CorrectionApplicator (sequential Pauli updates)
         ↓
StabilizerStateCapsule (updated state)
```

**Data Structures**:

1. **Syndrome Entry** (256 bytes, cache-aligned):
   ```rust
   #[repr(C, align(256))]
   struct SyndromeEntry {
       syndrome_bits: [u64; 8],      // 512 stabilizers max (d=23)
       timestamp_ns: AtomicU64,       // Capture time
       syndrome_weight: u16,          // Popcount for decoder selection
       error_weight: u16,             // Estimated errors (for telemetry)
       generation: u32,               // Ring buffer wraparound detection
       flags: u16,                    // PROCESSED, CORRECTED, DROPPED, etc.
       decoder_used: u8,              // 0=None, 1=UnionFind, 2=MWPM
       padding: [u8; 197],            // Align to 256B
   }
   ```

2. **QEC Pipeline State** (64 bytes, cache-aligned):
   ```rust
   #[repr(C, align(64))]
   struct QECPipelineState {
       syndrome_head: AtomicU64,      // Producer position (syndrome extraction)
       syndrome_tail: AtomicU64,      // Consumer position (decoder)
       decoder_state: AtomicU32,      // IDLE, UNION_FIND_BUSY, MWPM_BUSY
       correction_counter: AtomicU64, // Total corrections applied
       logical_errors: AtomicU64,     // Logical error events
       cycle_count: AtomicU64,        // Total QEC rounds
       flags: AtomicU32,              // RUNNING, PAUSED, ERROR
       padding: [u8; 20],             // Align to 64B
   }
   ```

**Dependencies**:
- Syndrome extraction → Decoder (syndrome bits, weight)
- Decoder → Correction (qubit IDs, Pauli operators)
- Correction → Stabilizer state (updated tableau)
- All stages → Telemetry (latency, accuracy)

### Q6: What are the edge cases?

**Syndrome Handling**:
1. **Empty Syndrome** (no errors detected):
   - Skip decoding entirely (0ns decode latency)
   - Increment "no-error cycles" counter
   - Still record telemetry (prove correctness)

2. **Dense Syndrome** (many errors, weight > d²/2):
   - MWPM decoder required (Union-Find may fail)
   - May exceed 100μs budget → defer to next cycle
   - Mark syndrome as "deferred" (flag bit)

3. **Syndrome Buffer Overflow**:
   - Ring buffer full (head catches tail)
   - Drop oldest unprocessed syndrome (FIFO eviction)
   - Increment "dropped syndromes" counter (telemetry)
   - Log critical error (system overload)

**Decoder Failures**:
4. **Union-Find Timeout** (>50μs):
   - Abort Union-Find, retry with MWPM
   - Increment "decoder fallback" counter
   - Extend latency budget to 100μs (acceptable)

5. **MWPM Timeout** (>100μs):
   - Mark correction as "partial" (apply what we have)
   - Increment "incomplete corrections" counter
   - Defer remaining corrections to next cycle

6. **Decoder Disagreement** (Union-Find ≠ MWPM for same syndrome):
   - Use MWPM result (higher accuracy, proven optimal)
   - Log discrepancy (debugging aid)
   - Track accuracy delta (telemetry)

**Stabilizer State Edge Cases**:
7. **Correction Application Failure**:
   - Pauli operator conflicts with stabilizer tableau
   - Recompute stabilizer generators (Clifford optimization)
   - Increment "stabilizer refresh" counter

8. **Logical Error Detection**:
   - Syndrome pattern matches logical operator
   - Set logical error flag (cannot correct)
   - Increment logical error counter
   - Optional: Reset state to |0⟩ logical

### Q7: What existing code/patterns can we reuse?

**From Phase Q3.5 (Decoders)**:
1. **UnionFindDecoderCapsule** (Q3.5-A):
   - API: `decode(syndrome: &[u64], distance: u8) -> Vec<(u16, u8)>`
   - Performance: <50μs for d≤9
   - Integration: Zero-copy syndrome sharing (borrow)

2. **MWPMDecoderCapsule** (Q3.5-B):
   - API: `decode_mwpm(syndrome: &[u64], distance: u8) -> Vec<(u16, u8)>`
   - Performance: <100μs for d≤7
   - Integration: May use separate thread pool (if >100μs)

3. **SyndromeExtractionCapsule** (Q3.5-C):
   - API: `extract_syndrome(stabilizers: &[Stabilizer], state: &StabilizerState) -> Syndrome`
   - Performance: <30μs parallel extraction
   - Integration: Lockfree producer to ring buffer

**From Phase Q3.6-A/B (Stabilizer Simulation)**:
4. **StabilizerStateCapsule**:
   - Pauli tableau representation
   - Clifford operator application
   - Zero-copy state borrowing (concurrent readers)

5. **CliffordOptimizerCapsule**:
   - Tableau compression (remove redundant generators)
   - Used after many corrections (every 100 cycles)

**From atomic_capsule Core**:
6. **RingBufferCapsule<T>** (T5 Streaming):
   - Generic lockfree ring buffer
   - Specialize with `T = SyndromeEntry`
   - <10ns append, <5ns read

7. **HistogramCapsule** (T0 Auditable):
   - Latency tracking (syndrome/decode/correct times)
   - <10ns record (lockfree)

8. **CircuitBreakerCapsule** (T1 Atomic):
   - Pipeline health monitoring
   - Auto-pause on repeated failures

### Q8: What are the failure modes?

**Performance Failures**:
1. **Latency Budget Exceeded**:
   - Symptom: QEC cycle >100μs
   - Cause: Dense syndromes forcing MWPM, or system overload
   - Mitigation: Defer syndrome to next cycle, track latency histogram
   - Recovery: Auto-throttle error injection rate (if testing)

2. **Syndrome Buffer Overflow**:
   - Symptom: `syndrome_head == syndrome_tail + CAPACITY`
   - Cause: Decoder slower than syndrome extraction
   - Mitigation: Drop oldest syndrome (FIFO), log overflow
   - Recovery: Increase buffer size or pause error injection

3. **Decoder Starvation**:
   - Symptom: Decoder idle while syndromes pending
   - Cause: Coordination bug (atomic state machine deadlock)
   - Mitigation: Timeout-based state transitions
   - Recovery: Force state reset, log critical error

**Correctness Failures**:
4. **Syndrome Drop**:
   - Symptom: `syndromes_produced > syndromes_decoded`
   - Cause: Ring buffer overflow or coordination bug
   - Mitigation: Track syndrome generation counter (detect drops)
   - Recovery: Pause QEC, dump state for debugging

5. **Double Correction**:
   - Symptom: Same syndrome processed twice
   - Cause: Atomic CAS retry logic bug
   - Mitigation: Generation counter in syndrome entry (exact-once)
   - Recovery: Revert duplicate correction (inverse Pauli)

6. **Logical Error Undetected**:
   - Symptom: Stabilizer state invalid (contradictory stabilizers)
   - Cause: Incorrect decoder output or correction application bug
   - Mitigation: Periodic stabilizer consistency check
   - Recovery: Reset to known-good state, log error

**System Failures**:
7. **Memory Exhaustion**:
   - Symptom: Allocation failure in decoder state
   - Cause: Unbounded growth (e.g., MWPM graph structures)
   - Mitigation: Preallocate all decoder memory (fixed budget)
   - Recovery: Reject new QEC requests, log OOM

8. **Atomic Coordination Deadlock**:
   - Symptom: Pipeline stuck (no forward progress)
   - Cause: CAS loop livelock or state machine bug
   - Mitigation: Timeout-based circuit breaker
   - Recovery: Force reset, dump atomic state for debugging

### Q9: What are the integration points?

**Upstream Dependencies** (5 capsules):
1. **StabilizerStateCapsule** (Q3.6-A):
   - Interface: `borrow_state() -> &StabilizerTableau`
   - Coordination: Concurrent readers (atomic refcount)
   - Integration: Zero-copy borrow for syndrome extraction

2. **SyndromeExtractionCapsule** (Q3.5-C):
   - Interface: `extract(state: &Stabilizer) -> Syndrome`
   - Coordination: Producer to ring buffer (atomic head)
   - Integration: Parallel extraction with completion barrier

3. **UnionFindDecoderCapsule** (Q3.5-A):
   - Interface: `decode(syndrome: &[u64]) -> Vec<(u16, u8)>`
   - Coordination: Lockfree (stateless decoding)
   - Integration: Borrow syndrome from ring buffer (zero-copy)

4. **MWPMDecoderCapsule** (Q3.5-B):
   - Interface: `decode_mwpm(syndrome: &[u64]) -> Vec<(u16, u8)>`
   - Coordination: May use thread pool (async)
   - Integration: Fallback for dense syndromes

5. **CliffordOptimizerCapsule** (Q3.6-B):
   - Interface: `optimize_tableau(state: &mut Stabilizer)`
   - Coordination: Exclusive writer (pause QEC during optimization)
   - Integration: Periodic maintenance (every 100 cycles)

**Downstream Dependencies** (telemetry, monitoring):
6. **HistogramCapsule** (atomic_capsule core):
   - Interface: `record(latency_ns: u64)`
   - Coordination: Lockfree <10ns record
   - Integration: Track syndrome/decode/correct latencies

7. **CircuitBreakerCapsule** (atomic_capsule core):
   - Interface: `evaluate(error_count: u32)`
   - Coordination: Atomic state machine
   - Integration: Auto-pause on repeated failures

**External Integration**:
8. **Benchmarking Framework** (B32):
   - Baseline: Ideal decoder (offline, unlimited latency)
   - Comparison: Union-Find vs MWPM accuracy
   - Metrics: Latency (95th percentile), throughput, logical error rate

9. **Testing Framework** (T28):
   - Unit tests: Pipeline stages, state machine, ring buffer
   - Property tests: Correctness (exact-once, no drops)
   - Integration tests: Full QEC cycle (1000 rounds)
   - Production tests: Stress testing (10K rounds/sec sustained)

---

## Q10: Tier Selection

### Q10a: Profile FIRST (Mandatory Checkpoint)

**Profiling Strategy**:
Since this is a greenfield design (no existing implementation), we analyze theoretical bottlenecks based on:
1. **Stabilizer Measurement Complexity**: O(d²) measurements for d×d surface code
2. **Decoder Time Complexity**:
   - Union-Find: O(d² α(d²)) ≈ O(d²) amortized
   - MWPM: O(d⁶) worst-case (Blossom algorithm)
3. **Correction Application**: O(k × n) for k corrections on n-qubit tableau

**Expected Bottlenecks** (pre-implementation):
1. **Syndrome Extraction** (30-40% of budget):
   - d² parallel measurements (embarrassingly parallel)
   - Dominant cost: Pauli string evaluation (XOR operations)
   - Optimization: T4 Batch parallel (vectorize XOR with SIMD)

2. **Decoding** (40-60% of budget):
   - Union-Find: Fast for sparse syndromes (weight < d)
   - MWPM: Slow for dense syndromes (weight > d)
   - Optimization: T5 Streaming adaptive selection

3. **Correction Application** (10-20% of budget):
   - Sequential Pauli updates (non-commutative)
   - Dominant cost: Tableau row operations (Gaussian elimination style)
   - Optimization: T1 Atomic updates (lockfree)

**Profiling Plan** (post-implementation):
```bash
# Profile syndrome extraction
cargo flamegraph --bench qec_integration_bench -- --bench syndrome_extraction

# Profile full QEC cycle
cargo flamegraph --bench qec_integration_bench -- --bench full_qec_cycle

# Expected output:
# syndrome_extraction: 35% (30μs / 85μs total)
# union_find_decode: 45% (38μs / 85μs total)
# apply_corrections: 15% (13μs / 85μs total)
# coordination_overhead: 5% (4μs / 85μs total)
```

### Q10b: Analyze Bottleneck (Amdahl's Law)

**Bottleneck Analysis**:

1. **Syndrome Extraction** (35%, 30μs):
   - **Parallelizable**: 100% (independent measurements)
   - **Speedup Potential**: 8× with 8-core parallelism (T4 Batch)
   - **Amdahl's Law**:
     ```
     Total speedup = 1 / ((1 - 0.35) + 0.35/8) = 1.44×
     New latency: 85μs / 1.44 = 59μs
     ```
   - **Conclusion**: Worthwhile optimization (brings cycle <60μs)

2. **Decoding** (45%, 38μs):
   - **Parallelizable**: Depends on decoder:
     - Union-Find: 20% (cluster merging is sequential)
     - MWPM: 60% (graph construction parallel, matching sequential)
   - **Speedup Potential**:
     - Union-Find: 1.2× (minimal benefit)
     - MWPM: 2× (parallel edge weights)
   - **Amdahl's Law** (MWPM parallel edges):
     ```
     MWPM speedup = 1 / ((1 - 0.60) + 0.60/4) = 1.82×
     New MWPM latency: 100μs / 1.82 = 55μs (still exceeds budget)
     ```
   - **Conclusion**: **Adaptive decoder selection** is key (use Union-Find when possible)

3. **Correction Application** (15%, 13μs):
   - **Parallelizable**: 0% (sequential Pauli updates)
   - **Speedup Potential**: None (Amdahl's serial fraction)
   - **Conclusion**: Already fast, optimize via T1 Atomic (minimize coordination overhead)

**Amdahl's Law Summary**:
| Component | Current | Speedup | New | Justification |
|-----------|---------|---------|-----|---------------|
| Syndrome | 30μs | 8× (T4) | 3.75μs | 100% parallel |
| Decode | 38μs | 1× (adaptive) | 38μs | Choose fast decoder |
| Correct | 13μs | 1× (T1) | 13μs | Sequential |
| Overhead | 4μs | 2× (lockfree) | 2μs | Atomic coordination |
| **Total** | **85μs** | **1.54×** | **55μs** | **<60μs target** |

**Reality Check**:
- 1.54× speedup is REALISTIC (10-50% typical range)
- Achievable via T4 parallel syndrome + adaptive decoder + T1 lockfree coordination
- Breakthrough claim (8× syndrome) localized to parallelizable stage (Amdahl's Law valid)

### Q10c: Choose Tier Matching Q10b

**Tier Selection Decision**:

**Primary Tier: T4 Batch (Parallel Syndrome Extraction)**
- **Bottleneck**: Syndrome extraction (35% of budget)
- **Characteristic**: Embarrassingly parallel (d² independent measurements)
- **Tier Match**: T4 Batch parallel (rayon par_iter or manual thread pool)
- **Expected Speedup**: 8× on 8-core CPU (verified by Amdahl's Law)
- **Implementation**:
  ```rust
  // Pseudocode
  stabilizers.par_iter()
      .map(|s| measure_stabilizer(s, state))
      .collect::<Vec<_>>()
  ```

**Secondary Tier: T5 Streaming (Adaptive Decoder Pipeline)**
- **Bottleneck**: Decoding (45% of budget)
- **Characteristic**: Heterogeneous performance (Union-Find <50μs, MWPM <100μs)
- **Tier Match**: T5 Streaming adaptive (select decoder based on syndrome weight)
- **Expected Speedup**: 1.5-2× via decoder selection (avoid MWPM when unnecessary)
- **Implementation**:
  ```rust
  // Pseudocode
  let decoder = if syndrome_weight < threshold {
      DecoderType::UnionFind  // <50μs
  } else {
      DecoderType::MWPM       // <100μs
  };
  decode_with(decoder, syndrome)
  ```

**Tertiary Tier: T1 Atomic (Lockfree Pipeline Coordination)**
- **Bottleneck**: Coordination overhead (5% of budget)
- **Characteristic**: Producer-consumer (syndrome → decoder → corrector)
- **Tier Match**: T1 Atomic ring buffer + state machine
- **Expected Speedup**: 2× via lockfree (eliminate mutex contention)
- **Implementation**:
  ```rust
  // Pseudocode
  loop {
      let head = syndrome_head.load(Acquire);
      let tail = syndrome_tail.load(Acquire);
      if head != tail {
          process_syndrome(&syndromes[tail % CAPACITY]);
          syndrome_tail.fetch_add(1, Release);
      }
  }
  ```

**Tier Composition: T4 + T5 + T1 (Mixed Tier Justification)**:
This is **NOT** a T6 Mixed capsule (which requires 3+ tiers in a single structure). Instead, it's a **pipeline architecture** where:
- **T4 Batch**: Syndrome extraction stage (parallel)
- **T5 Streaming**: Decoder selection stage (adaptive)
- **T1 Atomic**: Pipeline coordination (lockfree)

Each stage is a separate capsule communicating via lockfree primitives (ring buffer, atomic state machine).

**Alternative Tiers Rejected**:
- **T2 SIMD**: Not applicable (QEC is not data-parallel within single syndrome)
- **T3 Fixed-Point**: Not applicable (QEC uses exact integer arithmetic)
- **T6 Mixed**: Over-engineering (pipeline ≠ single capsule)
- **T7-T11**: Not applicable (no GPU/quantum hardware in scope)

### Q10 Summary Table

| Question | Answer |
|----------|--------|
| **Q10a: Profiled?** | Theoretical analysis (pre-implementation) + flamegraph plan (post-implementation) |
| **Q10b: Bottleneck?** | Syndrome extraction (35%), Decoding (45%), Correction (15%), Overhead (5%) |
| **Q10c: Tier?** | **T4 Batch** (syndrome) + **T5 Streaming** (decoder) + **T1 Atomic** (coordination) |
| **Expected Speedup** | 1.54× total (8× syndrome ÷ Amdahl's Law) → 85μs → 55μs |
| **Reality Check** | ✅ REALISTIC (10-50% typical, localized 8× justified by parallelism) |

---

## Q11: Rust Transform

### Q11a: How does Rust's type system enforce correctness?

**1. Syndrome Lifetime Safety**:
```rust
// GOOD: Borrow checker prevents use-after-free
fn extract_syndrome<'a>(
    state: &'a StabilizerStateCapsule,
    buffer: &'a mut SyndromeRingBuffer,
) -> Result<(), QECError> {
    let syndrome = state.measure_stabilizers()?;
    buffer.push(syndrome)?; // Syndrome lifetime tied to state
    Ok(())
}

// BAD: Would not compile (syndrome outlives state)
fn bad_extract(state: &StabilizerStateCapsule) -> &Syndrome {
    let syndrome = state.measure_stabilizers().unwrap();
    &syndrome // ERROR: returns reference to local variable
}
```

**2. Atomic Ordering Type Safety**:
```rust
// Use newtypes to enforce correct ordering
struct SyndromeHead(AtomicU64); // Producer (Release)
struct SyndromeTail(AtomicU64); // Consumer (Acquire)

impl SyndromeHead {
    fn advance(&self) -> u64 {
        self.0.fetch_add(1, Ordering::Release) // Enforced at call site
    }
}

impl SyndromeTail {
    fn load(&self) -> u64 {
        self.0.load(Ordering::Acquire) // Enforced at call site
    }
}
```

**3. Decoder Selection Exhaustiveness**:
```rust
enum DecoderType {
    UnionFind,
    MWPM,
}

// Compiler ensures all variants handled
fn select_decoder(syndrome_weight: u16) -> DecoderType {
    match syndrome_weight {
        0..=10 => DecoderType::UnionFind,
        11..=u16::MAX => DecoderType::MWPM,
    } // No default case needed (exhaustive)
}
```

**4. Zero-Copy Syndrome Sharing**:
```rust
// Borrow checker prevents simultaneous write
fn decode_syndrome(
    syndrome: &SyndromeEntry, // Immutable borrow (concurrent reads OK)
    decoder: &mut UnionFindDecoder, // Mutable borrow (exclusive)
) -> Vec<Correction> {
    decoder.decode(&syndrome.bits) // Zero-copy (no clone)
}
```

### Q11b: How does Rust enable zero-cost abstractions?

**1. Inline Ring Buffer Operations**:
```rust
impl SyndromeRingBuffer {
    #[inline(always)]
    fn push(&self, syndrome: SyndromeEntry) -> Result<(), BufferFull> {
        let head = self.head.fetch_add(1, Ordering::Release);
        let tail = self.tail.load(Ordering::Acquire);

        if head >= tail + CAPACITY {
            return Err(BufferFull); // 0ns overhead (inlined)
        }

        self.entries[head % CAPACITY] = syndrome; // Direct array access
        Ok(())
    }
}
```
**Cost**: 0ns abstraction overhead (compiles to direct memory write + CAS)

**2. Const Generics for Syndrome Size**:
```rust
struct SyndromeRingBuffer<const N: usize> {
    entries: [SyndromeEntry; N], // Compile-time size (no heap allocation)
    head: AtomicU64,
    tail: AtomicU64,
}

// Zero runtime cost (N known at compile time)
const BUFFER_SIZE: usize = 256;
let buffer = SyndromeRingBuffer::<BUFFER_SIZE>::new();
```

**3. Enum Discriminant Optimization**:
```rust
// DecoderType represented as single byte (no pointer indirection)
enum DecoderType {
    UnionFind = 1,
    MWPM = 2,
}

// Compiles to:
// - Load byte (1 cycle)
// - Compare (1 cycle)
// - Branch (0-15 cycles, predicted)
match decoder_type {
    DecoderType::UnionFind => { /* ... */ },
    DecoderType::MWPM => { /* ... */ },
}
```

**4. Iterator Fusion**:
```rust
// Rayon parallel iterator compiles to tight loop (no closure overhead)
stabilizers.par_iter()
    .map(|s| measure_stabilizer(s, state))
    .collect::<Vec<_>>();

// Equivalent to:
// for i in 0..stabilizers.len() {
//     results[i] = measure_stabilizer(&stabilizers[i], state);
// }
// (parallelized by rayon, 0ns abstraction cost)
```

### Q11c: What Rust features are essential?

**1. Atomic Types** (std::sync::atomic):
```rust
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};

struct QECPipelineState {
    syndrome_head: AtomicU64,     // Lockfree producer position
    syndrome_tail: AtomicU64,     // Lockfree consumer position
    decoder_state: AtomicU32,     // State machine (IDLE/BUSY)
    cycle_count: AtomicU64,       // Total QEC rounds
}
```
**Why Essential**: Lockfree coordination without mutex (Chaos mandate)

**2. Alignment Control** (#[repr(C, align(N))]):
```rust
#[repr(C, align(256))]
struct SyndromeEntry {
    bits: [u64; 8],      // 64 bytes
    metadata: Metadata,  // 64 bytes
    padding: [u8; 128],  // Pad to 256 bytes
}
```
**Why Essential**: Cache-line alignment prevents false sharing (performance critical)

**3. Const Generics**:
```rust
struct SyndromeRingBuffer<const N: usize> {
    entries: [SyndromeEntry; N], // Compile-time size
}

impl<const N: usize> SyndromeRingBuffer<N> {
    const fn capacity() -> usize { N } // 0ns runtime
}
```
**Why Essential**: Zero-cost abstraction (no heap allocation)

**4. Pattern Matching**:
```rust
match pipeline_state.load(Ordering::Acquire) {
    IDLE => start_decoding(),
    UNION_FIND_BUSY => poll_union_find(),
    MWPM_BUSY => poll_mwpm(),
    _ => unreachable!("Invalid state"),
}
```
**Why Essential**: Exhaustive state machine (compiler-verified correctness)

**5. Error Handling** (Result<T, E>):
```rust
fn decode_syndrome(
    syndrome: &SyndromeEntry,
) -> Result<Vec<Correction>, DecoderError> {
    if syndrome.weight == 0 {
        return Ok(Vec::new()); // Early return (no panic)
    }

    let corrections = decoder.decode(&syndrome.bits)?; // ? propagates errors
    Ok(corrections)
}
```
**Why Essential**: No panics in production (explicit error handling)

---

## Q12: Nightly Features

### Q12a: Which nightly features accelerate this workload?

**P0 (Mandatory, High Impact)**:

**1. portable_simd** (SIMD XOR for syndrome computation):
```rust
#![feature(portable_simd)]
use std::simd::{u64x4, SimdUint};

fn xor_syndromes_simd(
    prev: &[u64; 8],
    curr: &[u64; 8],
) -> [u64; 8] {
    let mut result = [0u64; 8];

    // Process 4 u64s at a time (256 bits)
    for i in (0..8).step_by(4) {
        let a = u64x4::from_slice(&prev[i..i+4]);
        let b = u64x4::from_slice(&curr[i..i+4]);
        let c = a ^ b; // SIMD XOR (1 cycle vs 4 cycles scalar)
        c.copy_to_slice(&mut result[i..i+4]);
    }

    result
}
```
**Impact**: 4× speedup on syndrome XOR (critical for temporal difference)

**2. const_fn_floating_point** (Compile-time decoder thresholds):
```rust
#![feature(const_fn_floating_point)]

const fn compute_threshold(distance: u8) -> u16 {
    // Adaptive threshold: d²/2 for dense syndrome detection
    let d_squared = (distance as f64) * (distance as f64);
    (d_squared / 2.0) as u16
}

const THRESHOLD_D5: u16 = compute_threshold(5); // 12 (at compile time)
const THRESHOLD_D7: u16 = compute_threshold(7); // 24 (at compile time)
```
**Impact**: 0ns runtime cost (threshold computed at compile time)

**3. atomic_from_mut** (Zero-copy atomic views for mmap persistence):
```rust
#![feature(atomic_from_mut)]

fn map_syndrome_buffer(mmap: &mut [u8]) -> Result<&AtomicU64, MmapError> {
    let head_ptr = &mut mmap[0..8]; // First 8 bytes = syndrome_head
    let head_atomic = AtomicU64::from_mut(
        head_ptr.as_mut_ptr() as *mut u64
    );
    Ok(head_atomic)
}
```
**Impact**: Zero-copy persistence (mmap syndrome buffer to disk)

**P1 (Optional, Medium Impact)**:

**4. const_trait_impl** (Compile-time syndrome validation):
```rust
#![feature(const_trait_impl)]

trait SyndromeValidator {
    fn is_valid(&self) -> bool;
}

#[const_trait]
impl SyndromeValidator for SyndromeEntry {
    fn is_valid(&self) -> bool {
        // Check syndrome weight matches popcount (integrity check)
        self.syndrome_weight == self.bits.iter().map(|b| b.count_ones()).sum::<u32>() as u16
    }
}

const fn validate_syndrome_at_compile_time(entry: &SyndromeEntry) -> bool {
    entry.is_valid() // Runs at compile time if entry is const
}
```
**Impact**: Compile-time syndrome validation (catch bugs early)

**5. generic_const_exprs** (Ring buffer size constraints):
```rust
#![feature(generic_const_exprs)]

struct SyndromeRingBuffer<const N: usize>
where
    [(); N.is_power_of_two()]: Sized, // Compile-time power-of-two check
{
    entries: [SyndromeEntry; N],
}
```
**Impact**: Compile-time buffer size validation (prevent runtime bugs)

**P2 (Research, Low Priority)**:

**6. inline_const** (Inline threshold computation):
```rust
#![feature(inline_const)]

fn select_decoder(syndrome_weight: u16, distance: u8) -> DecoderType {
    const { compute_threshold(distance) } // Inline const evaluation
}
```
**Impact**: Syntax convenience (minor performance benefit)

### Q12b: Stable Fallback Strategy

**Fallback Plan**:
1. **portable_simd** → scalar XOR loop (4× slower, acceptable)
2. **const_fn_floating_point** → runtime threshold computation (<1ns overhead)
3. **atomic_from_mut** → manual `transmute` (unsafe, well-documented)
4. **const_trait_impl** → runtime validation (5ns overhead, acceptable)
5. **generic_const_exprs** → runtime assertion (1ns overhead)

**Stable Compatibility**:
```rust
#[cfg(feature = "nightly")]
use std::simd::u64x4;

fn xor_syndromes(prev: &[u64; 8], curr: &[u64; 8]) -> [u64; 8] {
    #[cfg(feature = "nightly")]
    {
        xor_syndromes_simd(prev, curr) // 4× faster
    }

    #[cfg(not(feature = "nightly"))]
    {
        let mut result = [0u64; 8];
        for i in 0..8 {
            result[i] = prev[i] ^ curr[i]; // Scalar fallback
        }
        result
    }
}
```

**Feature Flags**:
```toml
[features]
nightly = ["portable_simd", "const_fn_floating_point", "atomic_from_mut"]
```

---

## Q13-Q29: Implementation Strategy

### Q13: Component Breakdown

**QECIntegrationCapsule Architecture**:

```rust
#[repr(C, align(64))]
pub struct QECIntegrationCapsule {
    // === Pipeline State (64 bytes, cache-aligned) ===
    pipeline_state: QECPipelineState,

    // === Syndrome Buffer (65,536 bytes, 256 entries × 256B) ===
    syndrome_buffer: SyndromeRingBuffer<256>,

    // === Decoder References (16 bytes, thin pointers) ===
    union_find_decoder: &'static UnionFindDecoderCapsule,
    mwpm_decoder: &'static MWPMDecoderCapsule,

    // === Stabilizer State (8 bytes, thin pointer) ===
    stabilizer_state: &'static StabilizerStateCapsule,

    // === Telemetry (64 bytes, cache-aligned) ===
    telemetry: QECTelemetryCapsule,

    // === Configuration (64 bytes, cache-aligned) ===
    config: QECConfig,
}

// Total size: 64 + 65,536 + 16 + 8 + 64 + 64 = 65,752 bytes (~64KB)
```

**Component Responsibilities**:

1. **QECPipelineState** (64B):
   - Atomic coordination (head/tail pointers)
   - Decoder state machine (IDLE/UNION_FIND/MWPM)
   - Cycle counters (total rounds, logical errors)

2. **SyndromeRingBuffer<256>** (64KB):
   - Lockfree producer-consumer queue
   - 256 syndrome entries × 256B each
   - Wraparound detection (generation counter)

3. **Decoder References**:
   - Thin pointers to decoders (avoid ownership)
   - Zero-copy syndrome borrowing
   - Adaptive selection logic

4. **Stabilizer State**:
   - Read-only reference (concurrent access)
   - Zero-copy borrow for syndrome extraction
   - Mutable access for correction application (exclusive)

5. **QECTelemetryCapsule** (64B):
   - Latency histograms (syndrome/decode/correct)
   - Error rate tracking (physical/logical)
   - Decoder accuracy metrics

6. **QECConfig** (64B):
   - Code distance (d = 3, 5, 7, 9)
   - Decoder thresholds (syndrome weight cutoff)
   - Buffer sizes (256/512 entries)
   - Feature flags (auto decoder, telemetry)

### Q14: Latency Budget Allocation

**Total Budget: <100μs** (10,000 QEC rounds/sec)

**Stage 1: Syndrome Extraction** (30μs budget):
- Parallel stabilizer measurements: 25μs
- Temporal difference (XOR): 1μs (SIMD accelerated)
- Ring buffer push: 1μs (atomic CAS)
- Metadata update: 1μs
- Coordination overhead: 2μs

**Stage 2: Decoding** (50μs budget):
- Decoder selection: 1μs (threshold comparison)
- Union-Find decode: 38μs (typical case, weight < d)
- MWPM decode: 90μs (fallback case, weight > d, may defer)
- Result allocation: 5μs (Vec for corrections)
- Coordination overhead: 6μs

**Stage 3: Correction Application** (20μs budget):
- Pauli operator lookup: 2μs
- Tableau row operations: 15μs (Gaussian elimination)
- State consistency check: 2μs
- Coordination overhead: 1μs

**Monitoring Overhead** (5μs, amortized):
- Histogram record: 1μs (3 histograms × <1ns each)
- Error rate update: 1μs
- Telemetry aggregation: 3μs

**Contingency** (5μs):
- Buffer overflow handling: 2μs
- Decoder fallback: 2μs
- Circuit breaker evaluation: 1μs

### Q15: Atomic Coordination Protocol

**Producer-Consumer Synchronization**:

```rust
// Producer (Syndrome Extraction)
fn push_syndrome(&self, syndrome: SyndromeEntry) -> Result<(), BufferFull> {
    loop {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        // Check buffer capacity
        if head >= tail + CAPACITY {
            return Err(BufferFull);
        }

        // Try to claim slot
        if self.head.compare_exchange_weak(
            head,
            head + 1,
            Ordering::Release,
            Ordering::Relaxed,
        ).is_ok() {
            // Write syndrome (exclusive access guaranteed)
            self.entries[head % CAPACITY] = syndrome;
            return Ok(());
        }

        // CAS failed, retry (another producer claimed slot)
    }
}

// Consumer (Decoder)
fn pop_syndrome(&self) -> Option<SyndromeEntry> {
    loop {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);

        // Check buffer empty
        if tail == head {
            return None;
        }

        // Try to claim entry
        if self.tail.compare_exchange_weak(
            tail,
            tail + 1,
            Ordering::Release,
            Ordering::Relaxed,
        ).is_ok() {
            // Read syndrome (exclusive access guaranteed)
            return Some(self.entries[tail % CAPACITY]);
        }

        // CAS failed, retry (another consumer claimed entry)
    }
}
```

**Memory Ordering Justification**:
- **Acquire** on load: Synchronize with Release store (see syndrome writes before consuming)
- **Release** on CAS: Make syndrome writes visible to consumers
- **Relaxed** on CAS failure: No synchronization needed (retry loop)

**Correctness Guarantees**:
- **Exact-Once**: CAS ensures each syndrome processed exactly once
- **No Drops**: Buffer full check prevents overwrites
- **FIFO Order**: Tail < Head invariant preserved
- **Wraparound Safety**: Modulo arithmetic (CAPACITY power-of-two)

### Q16: Decoder Selection Strategy

**Adaptive Algorithm**:

```rust
fn select_decoder(
    syndrome: &SyndromeEntry,
    config: &QECConfig,
) -> DecoderType {
    // Heuristic 1: Empty syndrome (no errors)
    if syndrome.syndrome_weight == 0 {
        return DecoderType::None; // Skip decoding entirely
    }

    // Heuristic 2: Sparse syndrome (Union-Find optimal)
    let threshold = config.syndrome_weight_threshold; // d²/2 typical
    if syndrome.syndrome_weight < threshold {
        return DecoderType::UnionFind; // <50μs
    }

    // Heuristic 3: Dense syndrome (MWPM required)
    DecoderType::MWPM // <100μs (may defer to next cycle)
}
```

**Thresholds** (distance-dependent):
| Distance | d² | Threshold (d²/2) | Justification |
|----------|-----|------------------|---------------|
| d=3 | 9 | 4 | Sparse: ≤4 errors → Union-Find |
| d=5 | 25 | 12 | Sparse: ≤12 errors → Union-Find |
| d=7 | 49 | 24 | Sparse: ≤24 errors → Union-Find |
| d=9 | 81 | 40 | Sparse: ≤40 errors → Union-Find |

**Performance Characteristics**:
- **Union-Find**: O(d² α(d²)) ≈ 38μs for d=7 (inverse Ackermann function α ≈ 4)
- **MWPM**: O(d⁶) ≈ 90μs for d=7 (Blossom algorithm)
- **Crossover**: Union-Find 2.4× faster for sparse syndromes

**Telemetry**:
Track decoder usage distribution:
```rust
struct DecoderStats {
    union_find_count: AtomicU64,
    mwpm_count: AtomicU64,
    none_count: AtomicU64, // Empty syndromes
}
```

### Q17-Q29: Additional Implementation Details

**(Condensed for brevity, full details in SPEC.md)**

- **Q17: Error Correction Application** - Sequential Pauli updates via stabilizer formalism
- **Q18: Telemetry Integration** - HistogramCapsule latency tracking, error rate monitoring
- **Q19: Circuit Breaker Integration** - Auto-pause on repeated decoder failures
- **Q20: Stabilizer State Borrowing** - Read-only concurrent access, exclusive write for corrections
- **Q21: Clifford Optimizer Integration** - Periodic tableau compression (every 100 cycles)
- **Q22: Buffer Overflow Handling** - FIFO eviction, overflow counter, critical log
- **Q23: Decoder Timeout Handling** - Abort Union-Find, retry MWPM, defer if >100μs
- **Q24: Logical Error Detection** - Syndrome pattern matching, logical operator check
- **Q25: Memory Layout** - Cache-aligned (64B/256B), false sharing prevention
- **Q26: Feature Flags** - `nightly-simd`, `telemetry`, `adaptive-decoder`
- **Q27: Const Generics** - Buffer size, syndrome array length
- **Q28: API Simplicity** - Single `run_qec_cycle()` method, builder pattern for config
- **Q29: Integration Tests** - Full QEC cycle (1000 rounds), decoder comparison, stress testing

---

## Q30-Q34: Validation Strategy

### Q30: How will we validate correctness?

**Validation Levels**:

**1. Unit Tests** (T28 Q1-Q7):
- Ring buffer: push/pop, wraparound, overflow
- Decoder selection: threshold logic, adaptive behavior
- Pipeline state machine: state transitions, atomicity
- Syndrome extraction: XOR correctness, metadata

**2. Property Tests** (T28 Q8-Q14):
- Exact-once processing (no duplicate corrections)
- FIFO ordering (syndrome temporal order preserved)
- Buffer capacity (no overwrites)
- Latency bounds (99th percentile <100μs)

**3. Integration Tests** (T28 Q15-Q21):
- Full QEC cycle (syndrome → decode → correct)
- Decoder comparison (Union-Find vs MWPM accuracy)
- Stabilizer state consistency (valid tableau after corrections)
- Telemetry accuracy (latency histograms, error rates)

**4. Production Tests** (T28 Q22-Q28):
- 1000 QEC rounds sustained (no crashes, no memory leaks)
- Stress testing (10K rounds/sec, buffer overflow handling)
- Logical error suppression (>90% vs uncorrected baseline)
- Decoder accuracy (>95% vs ideal offline decoder)

**Correctness Metrics**:
- **Syndrome Integrity**: Hash-chain verification (Q34 audit trail)
- **Decoder Accuracy**: F1 score vs ideal decoder (>95%)
- **Logical Error Rate**: Measured vs theoretical (within 2× of threshold)
- **State Consistency**: Stabilizer tableau commutation relations preserved

### Q31: How does this simplify the problem?

**Simplification Strategies**:

**1. Adaptive Decoder Selection** (vs. always using MWPM):
- **Complexity Reduction**: Avoid O(d⁶) MWPM for sparse syndromes
- **Performance**: 2.4× faster average case (Union-Find <50μs)
- **Simplicity**: Single decision point (syndrome weight threshold)

**2. Lockfree Pipeline** (vs. mutex-based coordination):
- **Complexity Reduction**: No deadlock debugging, no lock ordering
- **Performance**: Eliminate mutex contention (2× coordination speedup)
- **Simplicity**: Atomic CAS loops (10-15 lines each)

**3. Zero-Copy Syndrome Sharing** (vs. cloning):
- **Complexity Reduction**: No allocation/deallocation tracking
- **Performance**: Eliminate 256B memcpy overhead (10-20μs)
- **Simplicity**: Borrow checker enforces correctness

**4. Ring Buffer** (vs. unbounded queue):
- **Complexity Reduction**: Fixed memory budget (64KB), no growth logic
- **Performance**: Cache-friendly (power-of-two indexing)
- **Simplicity**: Wraparound via modulo (1 instruction)

**5. Telemetry Integration** (vs. external logging):
- **Complexity Reduction**: Atomic counters (no I/O overhead)
- **Performance**: <5% overhead (lockfree histograms)
- **Simplicity**: Single `record(latency_ns)` call

**Simplicity Validation**:
- **LOC**: <500 lines for QECIntegrationCapsule (vs. 2000+ for mutex-based)
- **API**: Single method `run_qec_cycle()` (vs. 10+ methods)
- **Dependencies**: 5 capsules (all lockfree, well-tested)

### Q32: What are the constraints and trade-offs?

**Constraints**:

**1. Latency Budget** (<100μs):
- **Trade-off**: Dense syndromes may defer to next cycle (acceptable for error rates <1%)
- **Mitigation**: Adaptive decoder selection (Union-Find for 80% of syndromes)

**2. Buffer Size** (256 entries):
- **Trade-off**: Overflow drops oldest syndromes (FIFO eviction)
- **Mitigation**: Monitor overflow counter, increase buffer size if sustained overload

**3. Decoder Accuracy** (>95%):
- **Trade-off**: Union-Find sub-optimal for dense syndromes (90-95% accuracy)
- **Mitigation**: Fall back to MWPM for weight > threshold

**4. Memory Budget** (<10MB):
- **Trade-off**: Limited syndrome history (256 cycles = 25.6ms @ 10K cycles/sec)
- **Mitigation**: Sufficient for real-time QEC (no long-term storage needed)

**5. Lockfree Complexity**:
- **Trade-off**: CAS retry loops may livelock under extreme contention
- **Mitigation**: Circuit breaker timeout (abort after 1000 retries)

**Architectural Trade-offs**:

| Design Choice | Pro | Con | Mitigation |
|---------------|-----|-----|------------|
| Adaptive Decoder | 2.4× faster average | Sub-optimal dense syndromes | MWPM fallback |
| Ring Buffer | Fixed memory | Overflow drops | Monitor + circuit breaker |
| Lockfree | No deadlock | CAS retry complexity | Timeout + telemetry |
| Zero-Copy | No allocation | Lifetime constraints | Borrow checker |
| Telemetry | Production observability | 5% overhead | Feature flag |

### Q33: How will we verify the implementation?

**Verification Methods**:

**1. Compile-Time Verification** (#[derive(ComputationalCapsule)]):
```rust
#[derive(ComputationalCapsule)]
#[capsule(tier = "T4+T5+T1", align = 64, verify_lockfree)]
pub struct QECIntegrationCapsule {
    // Compiler verifies:
    // - Alignment == 64 bytes
    // - No mutex/RwLock fields
    // - Padding calculated correctly
}
```
**Verification**: 0ns runtime, <20ms compile time (clippy integration)

**2. Runtime Verification** (ASSUM tags):
```rust
// #ASSUME_LOCKFREE_COORDINATION: All pipeline coordination via atomics
#[cfg(test)]
fn verify_lockfree() {
    let capsule = QECIntegrationCapsule::new(/* ... */);

    // Verify no mutexes in type signature
    assert!(!has_mutex::<QECIntegrationCapsule>());

    // Verify atomic operations
    assert!(capsule.pipeline_state.syndrome_head.is_lock_free());
}

// #ASSUME_EXACT_ONCE_PROCESSING: Each syndrome processed exactly once
#[cfg(test)]
fn verify_exact_once() {
    let capsule = QECIntegrationCapsule::new(/* ... */);

    // Push 100 syndromes
    for i in 0..100 {
        capsule.push_syndrome(syndrome_i).unwrap();
    }

    // Pop 100 syndromes (must be unique)
    let mut seen = HashSet::new();
    for _ in 0..100 {
        let syndrome = capsule.pop_syndrome().unwrap();
        assert!(seen.insert(syndrome.generation)); // No duplicates
    }
}
```

**3. Property-Based Testing** (proptest):
```rust
proptest! {
    #[test]
    fn test_fifo_ordering(syndromes in vec(syndrome_entry(), 1..1000)) {
        let capsule = QECIntegrationCapsule::new(/* ... */);

        // Push syndromes
        for s in &syndromes {
            capsule.push_syndrome(*s).unwrap();
        }

        // Pop syndromes (must match FIFO order)
        for expected in &syndromes {
            let actual = capsule.pop_syndrome().unwrap();
            assert_eq!(actual.generation, expected.generation);
        }
    }
}
```

**4. Stress Testing** (production load):
```rust
#[test]
fn stress_test_10k_cycles() {
    let capsule = QECIntegrationCapsule::new(/* ... */);

    // Run 10,000 QEC cycles
    for _ in 0..10_000 {
        let start = Instant::now();
        capsule.run_qec_cycle().unwrap();
        let elapsed = start.elapsed();

        // Verify latency <100μs (99th percentile)
        assert!(elapsed < Duration::from_micros(100));
    }

    // Verify no memory leaks
    assert_eq!(capsule.syndrome_buffer.leaked_entries(), 0);
}
```

**5. Decoder Accuracy Validation** (vs. ideal):
```rust
#[test]
fn validate_decoder_accuracy() {
    let capsule = QECIntegrationCapsule::new(/* ... */);
    let ideal_decoder = IdealDecoder::new(); // Unlimited latency, optimal

    let mut matches = 0;
    let total = 1000;

    for _ in 0..total {
        let syndrome = generate_random_syndrome();

        let actual_corrections = capsule.decode_syndrome(&syndrome);
        let ideal_corrections = ideal_decoder.decode(&syndrome);

        if corrections_equivalent(actual_corrections, ideal_corrections) {
            matches += 1;
        }
    }

    let accuracy = matches as f64 / total as f64;
    assert!(accuracy > 0.95); // >95% accuracy
}
```

### Q34: How will we add auditability?

**Q34 Auditability Framework** (Compliance: SOX, SOC2, GDPR, HIPAA):

**1. Hash-Chain Integrity** (Syndrome Audit Trail):
```rust
#[repr(C, align(256))]
struct SyndromeEntry {
    syndrome_bits: [u64; 8],
    timestamp_ns: AtomicU64,
    syndrome_weight: u16,
    error_weight: u16,
    generation: u32,

    // === Q34 Audit Fields ===
    prev_hash: u64,           // CRC64 of previous entry (hash chain)
    entry_hash: u64,          // CRC64 of this entry (tamper detection)
    correction_hash: u64,     // CRC64 of applied corrections (verify correctness)
}

impl SyndromeEntry {
    fn compute_hash(&self) -> u64 {
        let mut hasher = crc64::Hasher::new();
        hasher.write(&self.syndrome_bits);
        hasher.write_u64(self.timestamp_ns.load(Ordering::Relaxed));
        hasher.write_u16(self.syndrome_weight);
        hasher.write_u16(self.error_weight);
        hasher.write_u32(self.generation);
        hasher.write_u64(self.prev_hash);
        hasher.finish()
    }

    fn verify_hash_chain(&self, prev_entry: &SyndromeEntry) -> bool {
        // Verify hash chain link
        self.prev_hash == prev_entry.entry_hash &&
        self.entry_hash == self.compute_hash()
    }
}
```

**2. Tamper Detection** (Audit Log Integrity):
```rust
pub struct QECAuditLog {
    entries: Vec<SyndromeEntry>,
    root_hash: u64, // Genesis block hash
}

impl QECAuditLog {
    pub fn verify_integrity(&self) -> Result<(), AuditError> {
        let mut prev_hash = self.root_hash;

        for entry in &self.entries {
            // Verify entry hash
            if entry.entry_hash != entry.compute_hash() {
                return Err(AuditError::TamperedEntry(entry.generation));
            }

            // Verify hash chain link
            if entry.prev_hash != prev_hash {
                return Err(AuditError::BrokenChain(entry.generation));
            }

            prev_hash = entry.entry_hash;
        }

        Ok(())
    }
}
```

**3. Compliance Reporting** (GDPR Data Access):
```rust
pub struct QECComplianceReport {
    pub total_cycles: u64,
    pub syndrome_extractions: u64,
    pub decoder_invocations: u64,
    pub corrections_applied: u64,
    pub logical_errors: u64,
    pub union_find_usage: f64,  // Percentage
    pub mwpm_usage: f64,         // Percentage
    pub average_latency_ns: f64,
    pub p99_latency_ns: u64,
    pub decoder_accuracy: f64,
    pub audit_trail_valid: bool,
}

impl QECIntegrationCapsule {
    pub fn generate_compliance_report(&self) -> QECComplianceReport {
        let telemetry = self.telemetry.snapshot();
        let audit_valid = self.audit_log.verify_integrity().is_ok();

        QECComplianceReport {
            total_cycles: telemetry.cycle_count,
            syndrome_extractions: telemetry.syndrome_count,
            decoder_invocations: telemetry.union_find_count + telemetry.mwpm_count,
            corrections_applied: telemetry.correction_count,
            logical_errors: telemetry.logical_error_count,
            union_find_usage: telemetry.union_find_count as f64 / telemetry.total_cycles as f64,
            mwpm_usage: telemetry.mwpm_count as f64 / telemetry.total_cycles as f64,
            average_latency_ns: telemetry.total_latency_ns / telemetry.cycle_count,
            p99_latency_ns: telemetry.latency_histogram.percentile(0.99),
            decoder_accuracy: telemetry.correct_corrections as f64 / telemetry.total_corrections as f64,
            audit_trail_valid: audit_valid,
        }
    }
}
```

**4. Performance Overhead** (<5%):
```rust
// Hash computation: <50ns (CRC64 SIMD)
// Audit log append: <10ns (lockfree ring buffer)
// Integrity verification: <1ms per 1000 entries (O(n) scan)
```

**5. Regulatory Compliance**:
- **SOX**: Financial accuracy (decoder accuracy >95%, audit trail integrity)
- **SOC2**: Operational security (tamper detection, hash chain)
- **GDPR**: Data access (compliance report API, audit trail export)
- **HIPAA**: Healthcare security (audit trail immutability, access logs)

---

## Appendix: UCE34 Checklist

| Question | Status | Summary |
|----------|--------|---------|
| Q1 | ✅ | QEC integration layer (syndrome → decode → correct, <100μs) |
| Q2 | ✅ | <100μs latency, 10K cycles/sec, >90% logical error suppression |
| Q3 | ✅ | Input: Stabilizer state, Error model, QEC params; Output: Corrected state, Telemetry |
| Q4 | ✅ | Syndrome extraction (T4), Adaptive decoding (T5), Correction (T1) |
| Q5 | ✅ | Pipeline data flow (StabilizerState → Syndrome → Decoder → Correction) |
| Q6 | ✅ | Empty syndrome, Dense syndrome, Buffer overflow, Decoder failures, Logical errors |
| Q7 | ✅ | UnionFindDecoder, MWPMDecoder, SyndromeExtraction, StabilizerState, RingBuffer |
| Q8 | ✅ | Latency exceeded, Buffer overflow, Decoder starvation, Syndrome drop, Logical errors |
| Q9 | ✅ | 5 capsules (Stabilizer, Syndrome, Union-Find, MWPM, Clifford), Telemetry, Testing |
| Q10a | ✅ | Theoretical bottleneck analysis + flamegraph plan (post-implementation) |
| Q10b | ✅ | Syndrome 35%, Decoding 45%, Correction 15% → Amdahl's Law 1.54× speedup |
| Q10c | ✅ | T4 Batch (syndrome) + T5 Streaming (decoder) + T1 Atomic (coordination) |
| Q11a | ✅ | Borrow checker (lifetime safety), Newtypes (atomic ordering), Exhaustive matching |
| Q11b | ✅ | Inline ops (0ns), Const generics (no heap), Enum optimization, Iterator fusion |
| Q11c | ✅ | Atomic types, Alignment control, Const generics, Pattern matching, Result<T,E> |
| Q12a | ✅ | portable_simd (4× XOR), const_fn_floating_point (0ns threshold), atomic_from_mut (mmap) |
| Q12b | ✅ | Stable fallback (scalar XOR, runtime threshold, manual transmute) |
| Q13 | ✅ | 6 components (PipelineState, SyndromeBuffer, Decoders, State, Telemetry, Config) |
| Q14 | ✅ | 30μs syndrome, 50μs decode, 20μs correct, 5μs monitoring, 5μs contingency |
| Q15 | ✅ | Lockfree ring buffer (Acquire/Release ordering, CAS retry, exact-once) |
| Q16 | ✅ | Adaptive decoder (weight < d²/2 → Union-Find, else MWPM) |
| Q17-Q29 | ✅ | Error correction, Telemetry, Circuit breaker, Borrowing, Optimizer, Overflow, Timeouts |
| Q30 | ✅ | 4-tier testing (Unit/Property/Integration/Production), Correctness metrics |
| Q31 | ✅ | Adaptive selection (2.4× faster), Lockfree (2× coordination), Zero-copy (no alloc) |
| Q32 | ✅ | Latency budget, Buffer size, Decoder accuracy, Memory budget, Lockfree complexity |
| Q33 | ✅ | Compile-time (#[derive]), Runtime (ASSUM), Property (proptest), Stress (10K cycles), Accuracy (>95%) |
| Q34 | ✅ | Hash-chain integrity, Tamper detection, Compliance reporting, <5% overhead |

---

## Summary

**Architecture**: T4+T5+T1 pipeline (Batch syndrome + Streaming decoder + Atomic coordination)
**Performance**: <100μs QEC cycle (1.54× speedup via Amdahl's Law)
**Compliance**: UCE34 (Q1-Q34), Chaos (100% lockfree), B32 (fair baselines), T28 (28 tests), ASSUM (99.99% safe), Q34 (audit trails)
**Innovation**: Adaptive decoder selection (2.4× faster), Zero-copy syndrome sharing, Lockfree pipeline (<5% coordination overhead)
**Status**: Design complete, ready for implementation (see QEC_INTEGRATION_SPEC.md for detailed spec)

**Next Steps**:
1. Implement QECIntegrationCapsule (500 lines, <1 day)
2. Integrate with Phase Q3.5 decoders + Phase Q3.6 stabilizer
3. Write T28 test suite (28 tests, <1 day)
4. Run B32 benchmarks (baseline vs. adaptive, <1 day)
5. Validate Q34 audit trail integrity (<1 day)
6. Production deployment (10K cycles/sec stress test, <1 day)

**Total Effort**: ~5 days (design complete, implementation straightforward)
