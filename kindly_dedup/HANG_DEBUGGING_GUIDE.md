# Hang Debugging Guide - C4 Full Corpus Validation

**Status**: Process hangs during Phase 1 baseline after ~7 minutes
**Location**: `src/universal/pipeline.rs::process_corpus()` line 528
**Hanging At**: First call to `self.reader.next_chunk_iter()`
**Timeout Evidence**: Exit code 124 (timeout) after 60 seconds

---

## Quick Diagnosis (5 Minutes)

### Step 1: Test with 10K Document Subset
```bash
cd /home/samuel/Primitives/kindly_dedup

# This should complete in < 1 second
timeout 10 ./target/release/c4_parallel_real_benchmark 2>&1 | tee /tmp/debug_10k.log &
# Edit c4_parallel_real_benchmark.rs line 200:
# OLD: let corpus_path = "test_data/c4_1b_FIXED.jsonl";
# NEW: let corpus_path = "test_data/c4_10k.jsonl";
```

**Expected**: Should print "Before first chunk" and continue processing
**If hangs**: Issue is in parsing logic, not corpus size
**If succeeds**: Issue is memory/contention related to large corpus

### Step 2: Add Logging to Pinpoint Hang Location
Edit `src/universal/pipeline.rs` around line 528:

```rust
println!("  Phase 1: Read (Zero-copy JSONL parsing)");
eprintln!("[TRACE] About to start chunk iteration");

let mut chunk_count = 0;
let mut doc_count_total = 0;

while let Some(doc_iter) = self.reader.next_chunk_iter(mmap_data, CHUNK_SIZE)
    .map_err(|e| UniversalPipelineError::from(e))?
{
    chunk_count += 1;
    eprintln!("[TRACE] Processing chunk {} (position: {}/{} bytes)",
              chunk_count,
              self.reader.current_position(),
              self.reader.total_size());

    let mut chunk_docs = 0;
    for doc_result in doc_iter {
        match doc_result {
            Ok(doc) => {
                chunk_docs += 1;
                if chunk_docs % 1000 == 0 {
                    eprintln!("[TRACE] Chunk {}: {} docs processed", chunk_count, chunk_docs);
                }

                let signature = self.signature.compute_signature_simd(doc.text);
                self.signature.write_signature(doc.id, signature)?;
            }
            Err(e) => {
                eprintln!("[ERROR] Parse error in chunk {}, doc {}: {:?}",
                          chunk_count, chunk_docs, e);
                return Err(UniversalPipelineError::CapsuleError(format!("{:?}", e)));
            }
        }
    }

    doc_count_total += chunk_docs;
    eprintln!("[TRACE] Chunk {} complete: {} docs (total: {})",
              chunk_count, chunk_docs, doc_count_total);
}

eprintln!("[TRACE] Document streaming complete: {} total docs", doc_count_total);
```

Then rebuild and run:
```bash
cargo build --release --bin c4_parallel_real_benchmark 2>&1 | grep -E "error|warning" || echo "Build OK"
timeout 60 ./target/release/c4_parallel_real_benchmark 2>&1 | tee /tmp/debug_hang.log
```

**Read output carefully for**:
- How many chunks are processed?
- At which chunk number does it hang?
- How many docs per chunk before hang?

### Step 3: Identify Hang Pattern
```bash
# Analyze debug output
tail -100 /tmp/debug_hang.log | grep -E "\[TRACE\]|\[ERROR\]" | tail -20
```

**Look for**:
- Last successful chunk/doc count before hang
- Any error messages
- Whether it's making progress or stuck in infinite loop

---

## Deep Diagnostics (30 Minutes)

### Diagnosis Path A: Is It JSONL Parsing?

Test the parser in isolation:
```bash
# Create test file: extract first 100 lines
head -100 test_data/c4_1b_FIXED.jsonl > test_data/c4_parse_test.jsonl

# Edit parse_jsonl_line to add timeout detection:
# File: src/universal/corpus_reader.rs line 914
const MAX_PARSE_ITERATIONS: usize = 10_000;

let mut iterations = 0;
while i < bytes.len() {
    iterations += 1;
    if iterations > MAX_PARSE_ITERATIONS {
        return Err(CorpusReaderError::MalformedJson {
            line: line_num,
            reason: format!("Parser exceeded {} iterations (likely infinite loop)", MAX_PARSE_ITERATIONS),
        });
    }
    // ...rest of loop...
}
```

Rebuild and test:
```bash
cargo build --release --bin test_jsonl_parse 2>&1 | grep -v "^Compiling"
timeout 10 ./target/release/test_jsonl_parse 2>&1
```

**Expected**: If infinite loop in parser, will hit MAX_PARSE_ITERATIONS limit

### Diagnosis Path B: Is It LSH Bucketing?

Comment out signature writing temporarily:
```rust
// File: src/universal/pipeline.rs line 536-542
// Disable actual signature writing
// let signature = self.signature.compute_signature_simd(doc.text);
// self.signature.write_signature(doc.id, signature)?;

// Replace with:
let _signature = self.signature.compute_signature_simd(doc.text);
// Don't write - just parse
```

If this makes it work: **Issue is in signature writing (LSH bucketing)**
If this still hangs: **Issue is in JSON parsing**

### Diagnosis Path C: Is It Atomic Contention?

Add measurement around atomic operations:
```rust
use std::time::Instant;

let atomic_start = Instant::now();
self.signature.write_signature(doc.id, signature)?;
let atomic_time = atomic_start.elapsed();

if atomic_time.as_micros() > 100 {
    eprintln!("[PERF] Slow signature write: {} μs", atomic_time.as_micros());
}
```

If you see 100μs+ writes: **Atomic contention on counter**

### Diagnosis Path D: Memory-Mapped File Issue

Check if issue is mmap-specific:
```rust
// Replace mmap with regular file I/O temporarily
use std::io::Read;

let mut file = File::open(&self.corpus_path)?;
let mut contents = Vec::new();
file.read_to_end(&mut contents)?;
let mmap_data = &contents[..];

// Rest of code unchanged
```

If this works: **Issue is in memmap2 interaction with large files**

---

## Hypothesis Testing (Order by Likelihood)

### Hypothesis 1: Unbounded Loop in parse_jsonl_line (60% likely)
**Test**: Add iteration counter and max limit (see Diagnosis Path A above)
**Fix if true**: Limit max iterations or identify problematic JSON

### Hypothesis 2: Infinite Loop in next_chunk_iter newline detection (20% likely)
**Test**: Add logging to rposition() result
```rust
// File: src/universal/corpus_reader.rs line 476-478
let search_slice = &mmap[start_usize..tentative_end_usize];
eprintln!("[TRACE] Searching for newline in {} byte chunk", search_slice.len());

let last_newline_offset = search_slice
    .iter()
    .rposition(|&b| b == b'\n');

eprintln!("[TRACE] Newline found at: {:?}", last_newline_offset);
```

**Fix if true**: Add safeguard on newline search

### Hypothesis 3: Signature Atomic Contention (15% likely)
**Test**: Compare single-threaded baseline vs parallel (see Diagnosis Path C above)
**Fix if true**: Reduce atomic operation frequency or use sharded counter

### Hypothesis 4: memmap2 Page Fault Loop (5% likely)
**Test**: Use standard I/O instead (see Diagnosis Path D above)
**Fix if true**: Increase page cache or use pread64 with larger buffer

---

## Automatic Hang Detection

Add this to catch infinite loops early:
```rust
use std::time::{Instant, Duration};

let hang_detector_start = Instant::now();
let hang_timeout = Duration::from_secs(30);

while let Some(doc_iter) = self.reader.next_chunk_iter(mmap_data, CHUNK_SIZE)? {
    for doc_result in doc_iter {
        if hang_detector_start.elapsed() > hang_timeout {
            eprintln!("[ERROR] Hang detected: no documents processed in {} seconds",
                      hang_timeout.as_secs());
            eprintln!("[ERROR] Current position: {}/{} bytes",
                      self.reader.current_position(),
                      self.reader.total_size());
            return Err(UniversalPipelineError::ConfigError(
                "Document processing hang detected".to_string()
            ));
        }

        // ...rest of processing...
    }
}
```

---

## Tools & Commands

### Memory Profiling
```bash
# Monitor memory in real-time while running
watch -n 1 "ps aux | grep c4_parallel_real_benchmark | grep -v grep | awk '{print \"Memory: \" \$6/1024 \" MB, CPU: \" \$3 \"%\"}'"
```

### CPU Profiling (perf)
```bash
# If process hangs, attach with perf (requires root or capabilities)
sudo perf record -g -p $(pgrep c4_parallel_real_benchmark) --max-stack=16
# Let it run for 5-10 seconds, then Ctrl+C
sudo perf report --sort=comm,dso,sym
```

### Strace (System Call Tracing)
```bash
# See what system calls it's making
strace -e trace=none -c ./target/release/c4_parallel_real_benchmark 2>&1 &
sleep 10
pkill -TERM c4_parallel_real_benchmark
```

### GDB (Interactive Debugging)
```bash
# If you have gdb configured
gdb --args ./target/release/c4_parallel_real_benchmark
(gdb) run
# [Process runs, wait for hang]
# [In another terminal: Ctrl+C to break into GDB]
(gdb) bt
(gdb) frame 0
```

---

## Success Criteria

**Hang is FIXED when**:
- [ ] `timeout 10 ./target/release/c4_parallel_real_benchmark` completes on 100K dataset
- [ ] Phase 1 baseline completes on 1M dataset in <30 seconds
- [ ] Phase 1 baseline completes on 21M dataset in <400 seconds (60K docs/sec)
- [ ] Memory stays under 15 GB throughout
- [ ] Phase 2 parallel starts and shows speedup

---

## Prevention

Once hang is fixed, add regression tests:
```rust
#[test]
fn test_phase1_baseline_100k() {
    // Should complete in < 2 seconds
    let start = Instant::now();
    let result = run_baseline("test_data/c4_100k.jsonl", 100_000)?;
    assert!(start.elapsed().as_secs() < 2);
    assert!(result.throughput > 50_000.0);
}

#[test]
fn test_phase1_baseline_1m() {
    // Should complete in < 30 seconds
    let start = Instant::now();
    let result = run_baseline("test_data/c4_1m.jsonl", 1_000_000)?;
    assert!(start.elapsed().as_secs() < 30);
    assert!(result.throughput > 30_000.0);  // May be slower due to I/O
}
```

---

## Next Steps

1. **Execute Quick Diagnosis** (Step 1-3) → 5 minutes to identify hang location
2. **Run appropriate Deep Diagnostic** (Diagnosis Path A-D) → 15 minutes to confirm root cause
3. **Implement targeted fix** → 30 minutes based on diagnosis
4. **Re-run full validation** → should complete successfully

**Total estimated time to fix**: 1-2 hours

Report findings to user with:
- Exact hang location (file:line)
- Root cause (hypothesis confirmed)
- Fix applied
- Validation results on full corpus
