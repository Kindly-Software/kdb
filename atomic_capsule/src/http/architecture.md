# HTTP Parser Capsule Architecture
## UCE34-Compliant Design (T6 Mixed: T1 + T2 + T5)

**Version**: 1.0
**Date**: 2025-10-26
**Framework**: UCE34 (Q1-Q34 systematic discovery)
**Tier**: T6 Mixed (Atomic + SIMD + Streaming compound)
**Target**: <2μs parse time, 7× SIMD speedup (proven in table scans)

---

## UCE34 Q1-Q34 Analysis (MANDATORY)

### PART 0: Meta-Cognitive Analysis (Q1-Q9)

**Q1 (Scope)**: HTTP/1.1 parser for request/response messages
- Request line (method, path, version)
- Headers (field-name: field-value pairs)
- Body (chunked or content-length)
- **Constraint**: no_std + alloc (embedded-friendly)

**Q2 (Assumptions)**:
- Headers fit in 4KB (typical web server limit)
- SIMD available (AVX2 minimum, portable_simd for cross-platform)
- Single-threaded parsing (no concurrent access to same buffer)

**Q3 (Constraints)**:
- no_std + alloc only
- Zero external dependencies (only atomic_capsule foundation)
- 100% lockfree (atomic state machine)
- 4KB max headers (prevent DoS)

**Q4 (Context)**: Embedded HTTP servers, edge proxies, microservices
**Q5 (Success)**: <2μs parse, 7× SIMD speedup, 100% HTTP/1.1 compliant
**Q6 (Failure)**: Buffer overflow, invalid UTF-8, incomplete messages
**Q7 (Patterns)**: State machine, zero-copy parsing, SIMD string scanning
**Q8 (Alternatives)**: httparse (scalar), nom (combinator), manual loops
**Q9 (Trade-offs)**: Optimizing for speed (SIMD) over simplicity (scalar)

---

### PART 1: FOUNDATION (Q10-Q12)

## Q10: Computational Capsule Tier Selection

**DECISION**: **T6 Mixed** (Atomic + SIMD + Streaming compound)

### Tier Breakdown:

**T1 (Atomic)**: State machine coordination
- `ParserState`: Idle → RequestLine → Headers → Body → Complete
- `AtomicU64` packed state: state(4) | headers_count(8) | body_bytes(32) | error_code(8) | generation(12)
- **Speedup**: 3-10× vs mutex-based state (proven in circuit breaker: 9.8ns)

**T2 (SIMD)**: Header field scanning
- SIMD pattern matching for "\r\n" (line endings)
- SIMD case-insensitive comparison for header names
- Vectorized whitespace skipping
- **Speedup**: 7× vs scalar (proven in table scans)
- **Target**: <200ns per header line (vs ~1400ns scalar)

**T5 (Streaming)**: Incremental parsing
- Parse partial HTTP messages as bytes arrive
- Maintain parser offset without re-scanning
- Resume parsing from last known position
- **Benefit**: O(1) incremental overhead, not O(n) re-parse

**T6 (Mixed)**: Compound speedup
- **Formula**: 3× (atomic state) × 7× (SIMD scan) = **21× theoretical**
- **Reality**: 10-15× practical (B32 validation required)
- **Justification**: HTTP parsing is coordination (T1) + string scanning (T2) + incremental (T5)

---

## Q11: Rust Transformation

**Implementation Strategy**:

```rust
// T1: Atomic state machine
#[repr(C, align(128))]
pub struct HttpParserCapsule {
    state: AtomicU64,           // Packed state (8 bytes)
    offset: AtomicUsize,        // Current parse position (8 bytes)
    _padding1: [u8; 48],        // Cache line 1 (64B)

    // T2: SIMD scan buffers (cache line 2)
    header_buf: [u8; 64],       // Aligned for SIMD (64 bytes)
}

// T5: Streaming parser state
pub struct HttpStreamParser {
    capsule: HttpParserCapsule,
    buffer: Vec<u8>,            // Incoming bytes (alloc)
    headers: Vec<HttpHeader>,   // Parsed headers (alloc)
}
```

**Rust Primitives**:
- `AtomicU64` for state machine (T1)
- `portable_simd` for header scanning (T2, nightly)
- `Vec<u8>` for streaming buffer (T5, alloc)
- `#[derive(ComputationalCapsule)]` for verification

---

## Q12: Nightly Enhancement

**Mandatory Features**:

1. **portable_simd** (T2 - CRITICAL):
   ```rust
   #![feature(portable_simd)]
   use std::simd::{u8x16, u8x32, Simd};
   ```
   - 7× proven speedup in table scans
   - AVX2 (32-byte) for header scanning
   - SSE2 fallback (16-byte) if AVX2 unavailable

2. **const_fn_trait_impl** (Optional):
   ```rust
   const fn parse_method(buf: &[u8]) -> Method { /* ... */ }
   ```
   - Compile-time method parsing
   - Zero runtime cost for static routes

**Features NOT Used**:
- `atomic_from_mut`: Not needed (no external memory)
- `const_fn_floating_point`: No FP arithmetic
- `generic_const_exprs`: Simple const generics sufficient

---

### PART 2: DOMAIN ANALYSIS (Q13-Q21)

## Q13: Resources

**Memory**:
- Capsule: 128B (cache-aligned, 2 cache lines)
- Header buffer: 4KB max (prevent DoS)
- Streaming buffer: 8KB typical (configurable)
- **Total**: ~12KB per parser instance

**CPU**:
- T1 state loads: <10ns
- T2 SIMD scans: <200ns per header (7× vs 1400ns scalar)
- T5 offset tracking: <5ns

---

## Q14: Dependencies

**Rust Version**: Nightly (for portable_simd)
**Fallback**: Stable with scalar parsing (2-4× slower)
**External Crates**: ZERO (only atomic_capsule foundation)
**System Deps**: None

---

## Q15: Scale

**Single-threaded**: 1 parser per connection
**Expected throughput**: 5,000-10,000 requests/sec per thread
**Scaling**: Linear with thread count (lockfree)

---

## Q16: Security

**Threats**:
- Buffer overflow: Mitigated by 4KB header limit
- Invalid UTF-8: Validated during parsing
- Slowloris (incomplete requests): Timeout handled externally

**Constant-time**: Not critical (public HTTP parsing, no secrets)

---

## Q17: Interfaces

**Public API**:
```rust
pub trait HttpParser {
    fn parse_request(&mut self, buf: &[u8]) -> Result<HttpRequest, ParseError>;
    fn parse_response(&mut self, buf: &[u8]) -> Result<HttpResponse, ParseError>;
}

pub struct HttpRequest {
    pub method: Method,
    pub path: String,
    pub version: Version,
    pub headers: Vec<HttpHeader>,
    pub body: Vec<u8>,
}
```

**Error Handling**:
```rust
pub enum ParseError {
    Incomplete,
    InvalidMethod,
    InvalidHeader,
    TooLarge,
}
```

---

## Q18-Q21: Testing, Monitoring, Error Handling, Lifecycle

**Testing (T28)**:
- Unit: State machine transitions, SIMD vs scalar equality
- Property: Fuzz testing with arbitrary HTTP inputs
- Integration: httparse compatibility (same results)
- Production: 10,000 req/s load test

**Monitoring**: Atomic counters for parse time, error rates
**Error Handling**: `Result<T, ParseError>` for all operations
**Lifecycle**: `const fn new()` for zero-cost initialization

---

### PART 3: IMPLEMENTATION (Q22-Q30)

## Q22: State Management

**Packed State (64 bits)**:
```
Bits 63-60: ParserState (4 bits)
  0000 = Idle
  0001 = RequestLine
  0010 = Headers
  0011 = Body
  0100 = Complete
  1111 = Error

Bits 59-52: Headers count (8 bits, max 255 headers)
Bits 51-20: Body bytes read (32 bits, max 4GB)
Bits 19-12: Error code (8 bits)
Bits 11-0:  Generation counter (12 bits, 4096 versions)
```

---

## Q23: Concurrency

**Single-threaded parsing**: No contention
**Atomic state**: Enables safe state inspection from other threads
**Memory ordering**: Acquire/Release for state transitions

---

## Q24: Memory Layout

```
Offset 0-7:   state (AtomicU64)
Offset 8-15:  offset (AtomicUsize)
Offset 16-63: padding (48 bytes)
Offset 64-127: header_buf (64 bytes, SIMD-aligned)
```

**Verification**:
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct HttpParserCapsule { /* ... */ }
```

---

## Q25: Verification

**Compile-time**:
- `#[derive(ComputationalCapsule)]` for alignment/size
- `static_assert!` for state bit packing

**Runtime**:
- Bounds checks on header count
- UTF-8 validation on header values

---

## Q26: Optimization

**SIMD Patterns**:
1. **CRLF scanning** (`\r\n`):
   ```rust
   fn find_crlf_simd(buf: &[u8]) -> Option<usize> {
       // AVX2: 32-byte chunks
       let cr = u8x32::splat(b'\r');
       let lf = u8x32::splat(b'\n');

       for (i, chunk) in buf.chunks_exact(32).enumerate() {
           let data = u8x32::from_slice(chunk);
           let cr_mask = data.simd_eq(cr);
           let lf_mask = data.simd_eq(lf);
           // Check for adjacent CR + LF
       }
   }
   ```

2. **Case-insensitive header comparison**:
   ```rust
   fn header_eq_ignore_case_simd(a: &[u8], b: &[u8]) -> bool {
       // SIMD tolower() + compare
   }
   ```

---

## Q27: Composition

**T6 Mixed Composition**:
- T1 (Atomic state) + T2 (SIMD scan) + T5 (Streaming buffer)
- **Alignment**: 128B (max of T1=64B, T2=32B, T5=64B)
- **Expected speedup**: 10-15× compound (conservative vs 21× theoretical)

---

## Q28: Migration

**Not applicable** (new implementation, no legacy code to migrate)

---

## Q29: Documentation

**Invariants**:
- State machine is linear (Idle → Complete, no cycles)
- Offset never decreases
- Headers count ≤ 255

**Performance guarantees**:
- <2μs average parse time
- 7× SIMD speedup for header scanning

---

## Q30: Production Readiness

**Checklist**:
- [x] T28 testing plan defined
- [x] B32 benchmarking strategy
- [x] ASSUM tags for state machine
- [ ] Implementation (next phase)
- [ ] Validation (after implementation)

---

### PART 4: REFINEMENT (Q31-Q34)

## Q31: Simplicity

**Interface Design**:
```rust
// Q31: Hide complexity behind simple trait
pub trait HttpParser {
    fn parse_request(&mut self, buf: &[u8]) -> Result<HttpRequest, ParseError>;
}

// Users don't see capsule internals, SIMD, or state machine
let mut parser = HttpStreamParser::new();
let request = parser.parse_request(b"GET / HTTP/1.1\r\n...")?;
```

---

## Q32: Practical Constraints

**Real-world limits**:
- 4KB headers (nginx default)
- 64B cache lines (x86-64 standard)
- 32-byte SIMD (AVX2 minimum)
- <100ms parse timeout (handled externally)

---

## Q33: Empirical Validation

**Verification Macros** (MANDATORY):
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
verify_capsule_properties!(HttpParserCapsule, 128, 128);
```

**B32 Benchmarking**:
- Baseline: httparse (scalar, proven library)
- Target: 7× SIMD speedup for header scanning
- 95% CI, 1000+ iterations
- Fair comparison: optimized baseline (not strawman)

---

## Q34: Auditability

**Optional for HTTP parser** (no state-modifying operations beyond parsing)
- If logging/metrics added: Use `atomic_capsule::hash` for request IDs
- If caching responses: Hash chain for tamper detection

---

## Implementation Summary

**Tier Selection (Q10)**:
- T6 Mixed = T1 (Atomic state) + T2 (SIMD scan) + T5 (Streaming)

**Rust Transform (Q11)**:
- `AtomicU64` for state machine
- `portable_simd` for header scanning
- `Vec<u8>` for streaming buffer

**Nightly Features (Q12)**:
- `portable_simd` (MANDATORY for 7× speedup)
- SSE2 fallback if AVX2 unavailable

**Expected Performance**:
- <2μs parse time (vs ~14μs scalar)
- 7× SIMD speedup (proven in table scans)
- 10-15× compound speedup (T1 + T2 + T5)

**Memory Layout**:
- 128B capsule (cache-aligned)
- 4KB header buffer max
- 8KB streaming buffer typical

**Feature Flags**:
```toml
[features]
http = ["tier2", "tier5", "alloc"]
http-simd = ["http", "nightly-all"]
```

**Next Steps**:
1. Implement `HttpParserCapsule` (T1 state machine)
2. Implement SIMD header scanning (T2)
3. Implement streaming parser (T5)
4. Benchmark with B32 framework
5. Validate with T28 testing
