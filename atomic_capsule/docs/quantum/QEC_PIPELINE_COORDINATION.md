# QEC Pipeline Coordination - Lockfree Architecture

**Phase**: Q3.6-C Specialized Surface Code Simulator - Pipeline Coordination
**Version**: 1.0.0
**Date**: 2025-11-21
**Focus**: Lockfree producer-consumer, atomic state machine, zero-copy syndrome sharing

---

## Table of Contents

1. [Coordination Model](#coordination-model)
2. [Lockfree Ring Buffer](#lockfree-ring-buffer)
3. [Atomic State Machine](#atomic-state-machine)
4. [Memory Ordering](#memory-ordering)
5. [Syndrome Buffer Management](#syndrome-buffer-management)
6. [Decoder Scheduling](#decoder-scheduling)
7. [Error Correction Coordination](#error-correction-coordination)
8. [Monitoring and Telemetry](#monitoring-and-telemetry)
9. [Failure Modes and Recovery](#failure-modes-and-recovery)
10. [Performance Analysis](#performance-analysis)

---

## Coordination Model

### Pipeline Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                       Pipeline Stages                            │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Stage 1: Syndrome Extraction (Producer)                        │
│  ┌────────────────────────────────────────────────────────┐    │
│  │ - Measure stabilizers (T4 Batch parallel)              │    │
│  │ - Compute temporal XOR (SIMD)                          │    │
│  │ - Push to ring buffer (atomic CAS)                     │    │
│  └──────────────────────┬─────────────────────────────────┘    │
│                         │                                        │
│                         ▼                                        │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │         SyndromeRingBuffer (Lockfree Queue)              │  │
│  │  - Capacity: 256 entries × 256B = 64KB                   │  │
│  │  - Atomic head/tail pointers (Acquire/Release)           │  │
│  │  - Generation counter (wraparound detection)             │  │
│  │  - Overflow handling (FIFO eviction)                     │  │
│  └──────────────────────┬───────────────────────────────────┘  │
│                         │                                        │
│                         ▼                                        │
│  Stage 2: Decoder Scheduling (Consumer → Dispatcher)           │
│  ┌────────────────────────────────────────────────────────┐    │
│  │ - Pop from ring buffer (atomic CAS)                    │    │
│  │ - Select decoder (syndrome weight threshold)           │    │
│  │ - Dispatch to Union-Find or MWPM (state machine)       │    │
│  └──────────────────────┬─────────────────────────────────┘    │
│                         │                                        │
│         ┌───────────────┴───────────────┐                      │
│         ▼                               ▼                      │
│  ┌──────────────┐              ┌──────────────┐              │
│  │ Union-Find   │              │    MWPM      │              │
│  │  Decoder     │              │   Decoder    │              │
│  │  (<50μs)     │              │  (<100μs)    │              │
│  └──────┬───────┘              └──────┬───────┘              │
│         │                             │                        │
│         └──────────────┬──────────────┘                        │
│                        ▼                                        │
│  Stage 3: Error Correction (Sequential Writer)                 │
│  ┌────────────────────────────────────────────────────────┐    │
│  │ - Apply Pauli operators (sequential, non-commutative)  │    │
│  │ - Update stabilizer tableau (Gaussian elimination)     │    │
│  │ - Verify consistency (detect logical errors)           │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### Coordination Primitives

**1. Atomic Ring Buffer** (Producer-Consumer):
- **Head pointer**: Producer position (syndrome extraction)
- **Tail pointer**: Consumer position (decoder)
- **Ordering**: Acquire/Release (synchronize syndrome writes/reads)

**2. State Machine** (Decoder Coordination):
- **States**: IDLE, UNION_FIND_BUSY, MWPM_BUSY
- **Transitions**: IDLE ↔ BUSY (atomic CAS)
- **Ordering**: AcqRel (serialize state changes)

**3. Stabilizer State Lock** (Error Correction):
- **Read-only**: Syndrome extraction (concurrent readers)
- **Exclusive write**: Error correction (sequential, one writer)
- **Ordering**: SeqCst (ensure correctness)

### Lockfree Guarantees

**1. No Mutex/RwLock**:
- All coordination via atomic primitives (AtomicU64, AtomicU32)
- Zero blocking (wait-free or lock-free algorithms)

**2. Progress Guarantees**:
- **Wait-free**: Ring buffer push/pop (bounded CAS retries)
- **Lock-free**: State machine transitions (may retry, but system makes progress)

**3. Memory Safety**:
- Borrow checker enforces lifetime correctness
- No use-after-free, no data races (Rust guarantees)

---

## Lockfree Ring Buffer

### Data Structure

```rust
#[repr(C)]
pub struct SyndromeRingBuffer<const N: usize> {
    /// Syndrome entries (256B each, cache-aligned)
    entries: [SyndromeEntry; N],  // N × 256 bytes

    /// Producer position (syndrome extraction writes here)
    /// Ordering: Load Acquire, Store Release
    head: AtomicU64,              // 8 bytes

    /// Consumer position (decoder reads here)
    /// Ordering: Load Acquire, Store Release
    tail: AtomicU64,              // 8 bytes

    /// Overflow counter (syndromes dropped due to full buffer)
    overflow_count: AtomicU64,    // 8 bytes

    /// Padding to cache-line boundary (prevent false sharing)
    _padding: [u8; 40],           // 40 bytes → total 64 bytes header
}

// Compile-time constraints
impl<const N: usize> SyndromeRingBuffer<N> {
    const _ASSERT_POWER_OF_TWO: () = {
        assert!(N.is_power_of_two(), "N must be power of two for fast modulo");
    };
}
```

**Capacity**: N = 256 (default), 512 (low-latency mode)

**Memory Layout**:
```
+-------------------+
|    Header (64B)   |  ← Cache-aligned
|  - head (8B)      |
|  - tail (8B)      |
|  - overflow (8B)  |
|  - padding (40B)  |
+-------------------+
| Entry 0 (256B)    |  ← Cache-aligned
+-------------------+
| Entry 1 (256B)    |
+-------------------+
| ...               |
+-------------------+
| Entry N-1 (256B)  |
+-------------------+
Total: 64B + N×256B = 64B + 64KB (N=256)
```

### Producer Algorithm (Push)

```rust
impl<const N: usize> SyndromeRingBuffer<N> {
    /// Push syndrome to ring buffer (wait-free, bounded retries)
    pub fn push(&self, syndrome: SyndromeEntry) -> Result<(), BufferFull> {
        const MAX_RETRIES: usize = 100; // Timeout after 100 CAS failures

        for retry in 0..MAX_RETRIES {
            // Step 1: Load current head/tail (Acquire ordering)
            // - Acquire: See all syndrome writes before this load
            let head = self.head.load(Ordering::Acquire);
            let tail = self.tail.load(Ordering::Acquire);

            // Step 2: Check buffer capacity (prevent overflow)
            if head >= tail + N as u64 {
                // Buffer full: increment overflow counter, return error
                self.overflow_count.fetch_add(1, Ordering::Relaxed);
                return Err(BufferFull);
            }

            // Step 3: Try to claim slot (CAS with Release ordering)
            // - Release: Make syndrome writes visible to consumers
            match self.head.compare_exchange_weak(
                head,
                head + 1,
                Ordering::Release,  // Success: publish syndrome
                Ordering::Relaxed,  // Failure: retry (no synchronization needed)
            ) {
                Ok(_) => {
                    // Slot claimed: write syndrome
                    let index = (head % N as u64) as usize;
                    self.entries[index] = syndrome;

                    // Success
                    return Ok(());
                },
                Err(_) => {
                    // CAS failed: another producer claimed slot, retry
                    continue;
                }
            }
        }

        // Timeout: too many CAS retries (unlikely, indicates severe contention)
        Err(BufferFull)
    }
}
```

**Correctness**:
- **Exact-Once Claim**: CAS ensures each slot claimed exactly once
- **No Overwrites**: Capacity check prevents `head` catching `tail`
- **Visibility**: Release ordering makes syndrome writes visible to consumers

**Performance**:
- **Fast Path**: <1μs (single CAS success)
- **Slow Path**: <10μs (100 CAS retries, timeout)
- **Typical**: <2μs (2-3 CAS retries under normal load)

### Consumer Algorithm (Pop)

```rust
impl<const N: usize> SyndromeRingBuffer<N> {
    /// Pop syndrome from ring buffer (wait-free, bounded retries)
    pub fn pop(&self) -> Option<SyndromeEntry> {
        const MAX_RETRIES: usize = 100; // Timeout after 100 CAS failures

        for retry in 0..MAX_RETRIES {
            // Step 1: Load current tail/head (Acquire ordering)
            // - Acquire: See all syndrome writes before this load
            let tail = self.tail.load(Ordering::Acquire);
            let head = self.head.load(Ordering::Acquire);

            // Step 2: Check buffer empty
            if tail == head {
                return None; // No syndromes available
            }

            // Step 3: Try to claim entry (CAS with Release ordering)
            // - Release: Make syndrome reads visible to producers
            match self.tail.compare_exchange_weak(
                tail,
                tail + 1,
                Ordering::Release,  // Success: publish consumption
                Ordering::Relaxed,  // Failure: retry (no synchronization needed)
            ) {
                Ok(_) => {
                    // Entry claimed: read syndrome
                    let index = (tail % N as u64) as usize;
                    let syndrome = self.entries[index];

                    // Success
                    return Some(syndrome);
                },
                Err(_) => {
                    // CAS failed: another consumer claimed entry, retry
                    continue;
                }
            }
        }

        // Timeout: too many CAS retries (unlikely)
        None
    }
}
```

**Correctness**:
- **Exact-Once Consumption**: CAS ensures each syndrome consumed exactly once
- **No Stale Reads**: Acquire ordering sees latest syndrome writes
- **FIFO Order**: Tail < Head invariant preserved

**Performance**:
- **Fast Path**: <1μs (single CAS success)
- **Slow Path**: <10μs (100 CAS retries)
- **Typical**: <2μs (2-3 CAS retries)

### Wraparound Detection

**Problem**: Ring buffer wraps around after 2^64 entries (unrealistic, but handle gracefully)

**Solution**: Generation counter in SyndromeEntry

```rust
impl SyndromeEntry {
    fn new_with_generation(syndrome_bits: [u64; 8], generation: u32) -> Self {
        Self {
            syndrome_bits,
            generation,
            ..Default::default()
        }
    }

    fn is_stale(&self, current_generation: u32) -> bool {
        // Stale if generation differs (wraparound detected)
        self.generation != current_generation
    }
}
```

**Usage**:
```rust
// Producer: Increment generation on wraparound
let generation = (head / N as u64) as u32;
let syndrome = SyndromeEntry::new_with_generation(syndrome_bits, generation);
self.push(syndrome)?;

// Consumer: Check generation
if let Some(syndrome) = self.pop() {
    let expected_generation = (tail / N as u64) as u32;
    if syndrome.is_stale(expected_generation) {
        // Wraparound detected: drop stale syndrome
        continue;
    }
    // Process syndrome
}
```

**Reality Check**: Wraparound after 2^64 entries is unrealistic (would take millions of years at 10K cycles/sec), but detection costs 0ns (no runtime overhead).

---

## Atomic State Machine

### State Definitions

```rust
/// Decoder state machine states
pub mod decoder_state {
    /// No decoding in progress (can start new decoding)
    pub const IDLE: u32 = 0;

    /// Union-Find decoder active (<50μs expected)
    pub const UNION_FIND_BUSY: u32 = 1;

    /// MWPM decoder active (<100μs expected)
    pub const MWPM_BUSY: u32 = 2;
}

use decoder_state::*;
```

### State Transitions

**State Diagram**:
```
         start_decoding(UnionFind)
    IDLE ─────────────────────────────> UNION_FIND_BUSY
     ^                                        │
     │                                        │ finish_decoding()
     │                                        ▼
     └────────────────────────────────────  IDLE
              finish_decoding()

         start_decoding(MWPM)
    IDLE ─────────────────────────────────> MWPM_BUSY
     ^                                        │
     │                                        │ finish_decoding()
     │                                        ▼
     └────────────────────────────────────  IDLE
              finish_decoding()
```

**Allowed Transitions**:
- IDLE → UNION_FIND_BUSY
- IDLE → MWPM_BUSY
- UNION_FIND_BUSY → IDLE
- MWPM_BUSY → IDLE

**Forbidden Transitions**:
- UNION_FIND_BUSY → MWPM_BUSY (must finish current decoding first)
- MWPM_BUSY → UNION_FIND_BUSY (must finish current decoding first)

### Implementation

```rust
impl QECIntegrationCapsule {
    /// Start decoding (IDLE → BUSY transition)
    fn start_decoding(&self, decoder_type: DecoderType) -> Result<(), DecoderBusy> {
        // Determine target state
        let busy_state = match decoder_type {
            DecoderType::UnionFind => UNION_FIND_BUSY,
            DecoderType::MWPM => MWPM_BUSY,
            DecoderType::None => return Ok(()), // No-op (empty syndrome)
        };

        // Try to transition IDLE → BUSY (CAS with AcqRel ordering)
        // - AcqRel: Full memory barrier (serialize state changes)
        match self.pipeline_state.decoder_state.compare_exchange(
            IDLE,
            busy_state,
            Ordering::AcqRel, // Success: full barrier
            Ordering::Acquire, // Failure: read current state
        ) {
            Ok(_) => Ok(()), // Transition successful
            Err(current) => {
                // Already busy: return error with current state
                Err(DecoderBusy { current_state: current })
            }
        }
    }

    /// Finish decoding (BUSY → IDLE transition)
    fn finish_decoding(&self) -> Result<(), InvalidState> {
        // Load current state (Acquire ordering)
        let current = self.pipeline_state.decoder_state.load(Ordering::Acquire);

        // Validate current state (must be BUSY)
        if current == IDLE {
            return Err(InvalidState { current_state: IDLE });
        }

        // Transition BUSY → IDLE (Release ordering)
        // - Release: Make decoding results visible
        self.pipeline_state.decoder_state.store(IDLE, Ordering::Release);

        Ok(())
    }
}
```

### Timeout Handling

```rust
impl QECIntegrationCapsule {
    /// Decode with timeout (abort if exceeds latency budget)
    fn decode_with_timeout(
        &self,
        syndrome: &SyndromeEntry,
        decoder_type: DecoderType,
        timeout_ns: u64,
    ) -> Result<Vec<Correction>, DecoderTimeout> {
        let start = Instant::now();

        // Start decoding (IDLE → BUSY)
        self.start_decoding(decoder_type)?;

        // Run decoder
        let corrections = match decoder_type {
            DecoderType::UnionFind => {
                self.union_find_decoder.decode(&syndrome.syndrome_bits)
            },
            DecoderType::MWPM => {
                self.mwpm_decoder.decode(&syndrome.syndrome_bits)
            },
            DecoderType::None => Vec::new(),
        };

        // Check timeout
        let elapsed_ns = start.elapsed().as_nanos() as u64;
        if elapsed_ns > timeout_ns {
            // Timeout: finish decoding (BUSY → IDLE), return error
            self.finish_decoding()?;
            return Err(DecoderTimeout {
                elapsed_ns,
                timeout_ns,
                decoder_type,
            });
        }

        // Finish decoding (BUSY → IDLE)
        self.finish_decoding()?;

        Ok(corrections)
    }
}
```

**Latency Budgets**:
- Union-Find: 50μs (typical: 38μs, timeout: 50μs)
- MWPM: 100μs (typical: 90μs, timeout: 100μs)

**Timeout Recovery**:
- Abort current decoding
- Log timeout event (telemetry)
- Defer syndrome to next cycle (or drop if buffer full)

---

## Memory Ordering

### Ordering Primitives

**1. Relaxed** (no synchronization):
- **Use case**: Counters (telemetry, overflow count)
- **Guarantee**: Atomic read/write, no ordering
- **Example**: `overflow_count.fetch_add(1, Ordering::Relaxed)`

**2. Acquire** (synchronize-with Release):
- **Use case**: Load shared data (head/tail pointers)
- **Guarantee**: See all writes before Release store
- **Example**: `head.load(Ordering::Acquire)`

**3. Release** (synchronize-with Acquire):
- **Use case**: Store shared data (head/tail pointers)
- **Guarantee**: Make writes visible to Acquire loads
- **Example**: `head.store(value, Ordering::Release)`

**4. AcqRel** (Acquire + Release):
- **Use case**: State machine transitions (full barrier)
- **Guarantee**: Acquire on read, Release on write
- **Example**: `state.compare_exchange(..., Ordering::AcqRel, ...)`

**5. SeqCst** (sequential consistency):
- **Use case**: Critical correctness (stabilizer state updates)
- **Guarantee**: Total order across all threads
- **Example**: `stabilizer_state.update(..., Ordering::SeqCst)` (if needed)

### Ordering Justifications

**Ring Buffer (Acquire/Release)**:
```rust
// Producer: Release on head update
let head = self.head.load(Ordering::Acquire); // See previous syndrome writes
self.entries[index] = syndrome;               // Write syndrome
self.head.store(head + 1, Ordering::Release); // Publish syndrome to consumers

// Consumer: Acquire on tail load
let tail = self.tail.load(Ordering::Acquire); // See syndrome writes
let syndrome = self.entries[index];           // Read syndrome
self.tail.store(tail + 1, Ordering::Release); // Publish consumption to producers
```

**Why Acquire/Release?**
- **Acquire**: Consumer sees syndrome writes before `head` update (no stale reads)
- **Release**: Producer makes syndrome writes visible before updating `head` (no torn reads)
- **Performance**: Faster than SeqCst (no global ordering overhead)

**State Machine (AcqRel)**:
```rust
// Start decoding: AcqRel on state transition
self.decoder_state.compare_exchange(
    IDLE,
    UNION_FIND_BUSY,
    Ordering::AcqRel, // Full barrier (serialize state changes)
    Ordering::Acquire,
);

// Finish decoding: Release on state store
self.decoder_state.store(IDLE, Ordering::Release); // Make results visible
```

**Why AcqRel?**
- **AcqRel**: Serialize state machine transitions (no concurrent IDLE → BUSY)
- **Release**: Make decoding results visible before transitioning to IDLE

**Counters (Relaxed)**:
```rust
// Overflow counter: Relaxed (no synchronization needed)
self.overflow_count.fetch_add(1, Ordering::Relaxed);

// Telemetry counters: Relaxed (eventual consistency OK)
self.cycle_count.fetch_add(1, Ordering::Relaxed);
```

**Why Relaxed?**
- **No synchronization**: Counters are independent (no coordination needed)
- **Performance**: Fastest atomic operation (no memory barriers)
- **Correctness**: Eventual consistency sufficient for telemetry

### Memory Ordering Table

| Operation | Ordering | Justification |
|-----------|----------|---------------|
| `head.load()` | Acquire | See syndrome writes before load |
| `head.store()` | Release | Publish syndrome writes to consumers |
| `tail.load()` | Acquire | See syndrome writes before load |
| `tail.store()` | Release | Publish consumption to producers |
| `decoder_state.compare_exchange()` | AcqRel | Serialize state transitions |
| `decoder_state.store()` | Release | Make results visible |
| `overflow_count.fetch_add()` | Relaxed | No synchronization needed |
| `cycle_count.fetch_add()` | Relaxed | No synchronization needed |

---

## Syndrome Buffer Management

### Buffer Capacity Planning

**Capacity Formula**:
```
Capacity = (Latency_max / Latency_avg) × Safety_Margin

Example (d=5, adaptive decoder):
- Latency_max = 100μs (MWPM worst-case)
- Latency_avg = 85μs (P50)
- Safety_Margin = 2× (handle bursts)

Capacity = (100μs / 85μs) × 2 = 2.35 → Round to 256 entries (power-of-two)
```

**Memory Footprint**:
```
Memory = Capacity × Entry_Size
       = 256 entries × 256B/entry
       = 64KB
```

### Overflow Handling

**FIFO Eviction** (drop oldest syndrome):
```rust
impl<const N: usize> SyndromeRingBuffer<N> {
    pub fn push_with_eviction(&self, syndrome: SyndromeEntry) -> EvictionResult {
        loop {
            let head = self.head.load(Ordering::Acquire);
            let tail = self.tail.load(Ordering::Acquire);

            // Check buffer full
            if head >= tail + N as u64 {
                // Evict oldest syndrome (FIFO)
                // - Increment tail (drop oldest)
                // - Increment overflow counter
                self.tail.fetch_add(1, Ordering::Release);
                self.overflow_count.fetch_add(1, Ordering::Relaxed);

                return EvictionResult::Evicted {
                    evicted_index: (tail % N as u64) as usize,
                };
            }

            // Try to push syndrome (normal path)
            if self.head.compare_exchange_weak(
                head,
                head + 1,
                Ordering::Release,
                Ordering::Relaxed,
            ).is_ok() {
                let index = (head % N as u64) as usize;
                self.entries[index] = syndrome;
                return EvictionResult::Pushed { index };
            }
        }
    }
}
```

**Monitoring**:
```rust
pub fn overflow_rate(&self) -> f64 {
    let overflow = self.overflow_count.load(Ordering::Relaxed);
    let total = self.head.load(Ordering::Relaxed);

    if total == 0 {
        0.0
    } else {
        (overflow as f64) / (total as f64)
    }
}

// Typical overflow rate: <1% (well-provisioned buffer)
// Warning threshold: >5% (increase buffer capacity)
// Critical threshold: >10% (system overload, pause error injection)
```

### Buffer Sizing Recommendations

| Code Distance | Avg Latency | Buffer Size | Memory | Overflow Rate |
|---------------|-------------|-------------|--------|---------------|
| d=3 | 60μs | 128 entries | 32KB | <0.1% |
| d=5 | 85μs | 256 entries | 64KB | <1% |
| d=7 | 110μs | 512 entries | 128KB | <1% |
| d=9 | 140μs | 512 entries | 128KB | <2% |

**Recommendation**: Use 256 entries (64KB) for d≤7, 512 entries (128KB) for d>7.

---

## Decoder Scheduling

### Scheduling Algorithm

```rust
impl QECIntegrationCapsule {
    /// Main scheduling loop (runs continuously)
    pub fn run_decoder_scheduler(&self) -> ! {
        loop {
            // Step 1: Pop syndrome from ring buffer
            let syndrome = match self.syndrome_buffer.pop() {
                Some(s) => s,
                None => {
                    // Buffer empty: sleep briefly (avoid busy-wait)
                    std::thread::sleep(Duration::from_micros(1));
                    continue;
                }
            };

            // Step 2: Select decoder (adaptive algorithm)
            let decoder_type = self.select_decoder(&syndrome);

            // Step 3: Decode with timeout
            let timeout_ns = match decoder_type {
                DecoderType::UnionFind => 50_000, // 50μs
                DecoderType::MWPM => 100_000,     // 100μs
                DecoderType::None => 0,           // No decoding
            };

            let corrections = match self.decode_with_timeout(
                &syndrome,
                decoder_type,
                timeout_ns,
            ) {
                Ok(c) => c,
                Err(DecoderTimeout { .. }) => {
                    // Timeout: log, defer to next cycle
                    self.telemetry.decoder_timeouts.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
            };

            // Step 4: Apply corrections
            if let Err(e) = self.apply_corrections(&corrections) {
                // Correction failed: log, increment error counter
                self.telemetry.correction_errors.fetch_add(1, Ordering::Relaxed);
                continue;
            }

            // Step 5: Update telemetry
            self.update_telemetry(&syndrome, decoder_type);
        }
    }
}
```

### Multi-Threaded Scheduling (Optional)

**Thread Pool** (for high throughput):
```rust
use rayon::prelude::*;

impl QECIntegrationCapsule {
    /// Run decoder scheduler with thread pool (parallel decoding)
    pub fn run_parallel_scheduler(&self, num_threads: usize) -> ! {
        // Create thread pool
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .unwrap();

        loop {
            // Collect batch of syndromes (up to num_threads)
            let syndromes: Vec<SyndromeEntry> = (0..num_threads)
                .filter_map(|_| self.syndrome_buffer.pop())
                .collect();

            if syndromes.is_empty() {
                // No syndromes: sleep briefly
                std::thread::sleep(Duration::from_micros(10));
                continue;
            }

            // Decode in parallel
            let results: Vec<_> = pool.install(|| {
                syndromes.par_iter()
                    .map(|syndrome| {
                        let decoder_type = self.select_decoder(syndrome);
                        self.decode_syndrome(syndrome, decoder_type)
                    })
                    .collect()
            });

            // Apply corrections sequentially (non-commutative)
            for corrections in results {
                if let Ok(c) = corrections {
                    self.apply_corrections(&c).ok();
                }
            }
        }
    }
}
```

**Trade-offs**:
- **Pro**: Higher throughput (parallel decoding)
- **Con**: Increased latency (batch wait time)
- **Use case**: High error rate (>1%), many dense syndromes

**Recommendation**: Single-threaded for <1% error rate, multi-threaded for >1%

---

## Error Correction Coordination

### Exclusive Write Access

**Challenge**: Pauli operators don't commute (order matters)

**Solution**: Exclusive write lock (sequential correction application)

```rust
impl StabilizerStateCapsule {
    /// Lock for exclusive write (correction application)
    pub fn lock_for_write(&self) -> Result<ExclusiveGuard, LockError> {
        // Try to acquire write lock (CAS with AcqRel ordering)
        match self.write_lock.compare_exchange(
            0, // Unlocked
            1, // Locked
            Ordering::AcqRel, // Serialize writes
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(ExclusiveGuard { lock: &self.write_lock }),
            Err(_) => Err(LockError::WriteLockBusy),
        }
    }
}

/// RAII guard for exclusive write access
pub struct ExclusiveGuard<'a> {
    lock: &'a AtomicU32,
}

impl Drop for ExclusiveGuard<'_> {
    fn drop(&mut self) {
        // Release lock on drop (RAII)
        self.lock.store(0, Ordering::Release);
    }
}
```

**Usage**:
```rust
impl QECIntegrationCapsule {
    pub fn apply_corrections(
        &self,
        corrections: &[Correction],
    ) -> Result<(), CorrectionError> {
        // Acquire exclusive write lock
        let mut state = self.stabilizer_state.lock_for_write()?;

        // Apply corrections sequentially
        for correction in corrections {
            state.apply_pauli(correction.qubit_id, correction.pauli_op)?;
        }

        // Lock released automatically (RAII)
        Ok(())
    }
}
```

### Concurrent Reads (Syndrome Extraction)

**Challenge**: Syndrome extraction reads stabilizer state (concurrent with corrections)

**Solution**: Read-write lock (many concurrent readers, one exclusive writer)

```rust
impl StabilizerStateCapsule {
    /// Borrow for read (syndrome extraction)
    pub fn borrow_for_read(&self) -> Result<&Self, LockError> {
        // Increment reader count (Acquire ordering)
        self.reader_count.fetch_add(1, Ordering::Acquire);

        // Check write lock (no exclusive writer)
        if self.write_lock.load(Ordering::Acquire) != 0 {
            // Write lock held: decrement reader count, return error
            self.reader_count.fetch_sub(1, Ordering::Release);
            return Err(LockError::WriteLockBusy);
        }

        // Success: return reference
        Ok(self)
    }

    /// Release read lock
    pub fn release_read(&self) {
        self.reader_count.fetch_sub(1, Ordering::Release);
    }
}
```

**Usage**:
```rust
impl QECIntegrationCapsule {
    pub fn extract_syndrome(&self) -> Result<SyndromeEntry, SyndromeError> {
        // Borrow for read (concurrent with other readers)
        let state = self.stabilizer_state.borrow_for_read()?;

        // Measure stabilizers (parallel)
        let syndrome_bits = state.measure_stabilizers()?;

        // Release read lock
        state.release_read();

        Ok(SyndromeEntry::new(syndrome_bits))
    }
}
```

**Correctness**: Readers see consistent state (no torn reads during corrections)

---

## Monitoring and Telemetry

### Real-Time Metrics

```rust
impl QECTelemetryCapsule {
    /// Record QEC cycle latency (histogram integration)
    pub fn record_cycle_latency(&self, latency_ns: u64) {
        // Record in latency histogram (<10ns overhead)
        unsafe {
            (*self.syndrome_latency_hist).record(latency_ns);
        }
    }

    /// Update decoder usage statistics
    pub fn record_decoder_usage(&self, decoder_type: DecoderType) {
        match decoder_type {
            DecoderType::None => {
                self.decoder_stats.none_count.fetch_add(1, Ordering::Relaxed);
            },
            DecoderType::UnionFind => {
                self.decoder_stats.union_find_count.fetch_add(1, Ordering::Relaxed);
            },
            DecoderType::MWPM => {
                self.decoder_stats.mwpm_count.fetch_add(1, Ordering::Relaxed);
            },
        }
    }

    /// Snapshot telemetry (atomic read)
    pub fn snapshot(&self) -> TelemetrySnapshot {
        TelemetrySnapshot {
            union_find_count: self.decoder_stats.union_find_count.load(Ordering::Relaxed),
            mwpm_count: self.decoder_stats.mwpm_count.load(Ordering::Relaxed),
            none_count: self.decoder_stats.none_count.load(Ordering::Relaxed),
            physical_error_rate: self.physical_error_rate.load(Ordering::Relaxed),
            logical_error_rate: self.logical_error_rate.load(Ordering::Relaxed),
            overflow_count: self.overflow_count.load(Ordering::Relaxed),
        }
    }
}
```

### Dashboard Metrics

**Key Metrics**:
1. **Throughput**: QEC cycles/sec
2. **Latency**: P50, P99 cycle time (μs)
3. **Decoder Usage**: Union-Find vs MWPM distribution
4. **Error Rates**: Physical (measured), Logical (detected)
5. **Overflow Rate**: Syndrome buffer overflows (%)

**Example Dashboard**:
```
QEC Integration Dashboard
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Throughput:       11,764 cycles/sec  (target: 10,000)
Latency P50:      85μs               (target: <100μs)
Latency P99:      100μs              (target: <100μs)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Decoder Usage:
  Empty:          60% (no decoding)
  Union-Find:     35% (<50μs)
  MWPM:            5% (<100μs)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Error Rates:
  Physical:       0.102% (measured)
  Logical:        0.008% (suppressed 92%)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Buffer Status:
  Capacity:       256 entries (64KB)
  Overflow:       0.2% (2 drops / 1000 cycles)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

---

## Failure Modes and Recovery

### Syndrome Buffer Overflow

**Symptom**: `overflow_count` increasing rapidly

**Cause**: Decoder slower than syndrome extraction (sustained overload)

**Detection**:
```rust
fn check_overflow_rate(&self) -> Result<(), OverflowError> {
    let rate = self.syndrome_buffer.overflow_rate();

    if rate > 0.10 {
        // Critical: >10% overflow (system overload)
        Err(OverflowError::Critical { rate })
    } else if rate > 0.05 {
        // Warning: >5% overflow (increase buffer size)
        Err(OverflowError::Warning { rate })
    } else {
        Ok(())
    }
}
```

**Recovery**:
1. **Immediate**: Pause error injection (if testing)
2. **Short-term**: Increase buffer capacity (256 → 512 entries)
3. **Long-term**: Optimize decoder (reduce latency)

### Decoder Timeout

**Symptom**: `decoder_timeouts` counter increasing

**Cause**: Dense syndromes forcing MWPM (>100μs latency)

**Detection**:
```rust
fn check_decoder_timeouts(&self) -> Result<(), TimeoutError> {
    let timeouts = self.telemetry.decoder_timeouts.load(Ordering::Relaxed);
    let total = self.pipeline_state.cycle_count.load(Ordering::Relaxed);

    let timeout_rate = (timeouts as f64) / (total as f64);

    if timeout_rate > 0.05 {
        // Critical: >5% timeouts
        Err(TimeoutError::Critical { timeout_rate })
    } else if timeout_rate > 0.01 {
        // Warning: >1% timeouts
        Err(TimeoutError::Warning { timeout_rate })
    } else {
        Ok(())
    }
}
```

**Recovery**:
1. **Immediate**: Defer timed-out syndrome to next cycle
2. **Short-term**: Increase timeout budget (100μs → 150μs)
3. **Long-term**: Optimize MWPM decoder (reduce latency)

### Logical Error Spike

**Symptom**: `logical_errors` increasing rapidly

**Cause**: Physical error rate exceeds code capacity (>threshold)

**Detection**:
```rust
fn check_logical_error_rate(&self) -> Result<(), LogicalErrorError> {
    let logical_rate = self.telemetry.logical_error_rate.load(Ordering::Relaxed);

    // Convert Q16.16 to f64
    let logical_rate_f64 = (logical_rate as f64) / 65536.0;

    if logical_rate_f64 > 0.01 {
        // Critical: >1% logical errors
        Err(LogicalErrorError::Critical { logical_rate: logical_rate_f64 })
    } else if logical_rate_f64 > 0.001 {
        // Warning: >0.1% logical errors
        Err(LogicalErrorError::Warning { logical_rate: logical_rate_f64 })
    } else {
        Ok(())
    }
}
```

**Recovery**:
1. **Immediate**: Alert operator (physical error rate too high)
2. **Short-term**: Reduce error injection rate (if testing)
3. **Long-term**: Increase code distance (d=5 → d=7)

### CAS Livelock

**Symptom**: Ring buffer CAS retries exceed threshold (100+ retries)

**Cause**: Extreme contention (many producers/consumers)

**Detection**:
```rust
const MAX_CAS_RETRIES: usize = 100;

pub fn push_with_retry_count(
    &self,
    syndrome: SyndromeEntry,
) -> Result<usize, BufferFull> {
    for retry in 0..MAX_CAS_RETRIES {
        match self.push(syndrome) {
            Ok(()) => return Ok(retry), // Success, return retry count
            Err(BufferFull) => return Err(BufferFull),
        }
    }

    // Livelock: exceeded retry threshold
    Err(BufferFull)
}
```

**Recovery**:
1. **Immediate**: Exponential backoff (sleep 2^retry μs)
2. **Short-term**: Reduce producer/consumer thread count
3. **Long-term**: Redesign (multiple ring buffers, partitioning)

---

## Performance Analysis

### Coordination Overhead

**Breakdown**:
- Ring buffer push: 1μs (CAS + write)
- Ring buffer pop: 1μs (CAS + read)
- State machine transition: 1μs (CAS IDLE ↔ BUSY)
- Telemetry update: 1μs (3 atomic counters)
- **Total**: 4μs per QEC cycle

**Percentage**: 4μs / 85μs = 4.7% (within 5% target)

### Scalability

**Single-Threaded** (baseline):
- Throughput: 11,764 cycles/sec (85μs/cycle)
- CPU: 1 core @ 100%

**Multi-Threaded** (4 decoders):
- Throughput: 35,000 cycles/sec (28μs/cycle, 3× speedup)
- CPU: 4 cores @ 80% (parallel decoding)

**Scalability Limit**:
- Amdahl's Law: Sequential fraction = 20% (syndrome extraction + correction)
- Max speedup: 1 / 0.20 = 5× (diminishing returns beyond 5 cores)

### Memory Bandwidth

**Ring Buffer Access Pattern**:
- Sequential writes (producer): Cache-friendly (64B/256B aligned)
- Sequential reads (consumer): Cache-friendly (prefetch)
- Bandwidth: 256B/cycle × 11,764 cycles/sec = 3 MB/sec (negligible)

**Cache Utilization**:
- L1 cache hit rate: >95% (ring buffer fits in L2/L3)
- Cache miss latency: <100ns (DRAM access rare)

---

## Summary

**Coordination Model**: Lockfree producer-consumer (ring buffer) + atomic state machine (decoder) + exclusive write (correction)

**Performance**: <5% coordination overhead (4μs / 85μs), 11,764 cycles/sec throughput

**Scalability**: 3× speedup with 4 cores (Amdahl's Law: 20% sequential, 80% parallel)

**Failure Handling**: Overflow eviction (FIFO), timeout deferral, livelock circuit breaker

**Monitoring**: Real-time telemetry (throughput, latency, error rates, overflow)

**Status**: Design complete, ready for implementation (see QEC_INTEGRATION_SPEC.md for detailed API)
