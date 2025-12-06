# KDB Quick Wins - Immediate High-Impact Features
**Goal**: Implement 3 breakthrough features in 1-2 weeks to make kdb the default AI debugger
**Priority**: Highest ROI features that can be built quickly with existing capsules
**Date**: 2025-11-15

---

## Quick Win #1: Automatic Root Cause Analysis (3-5 days)

### Why This First?
- **Highest AI agent impact**: Eliminates 80% of manual analysis work
- **Uses existing capsules**: StackUnwinderCapsule, SymbolResolverCapsule, RegisterReaderCapsule
- **Simple ML model**: Decision tree with 20-30 patterns (no deep learning needed)
- **Immediate differentiation**: GDB has NOTHING like this

### Implementation

#### Step 1: Pattern Database (Day 1)
```rust
// src/ptrace/root_cause.rs
pub struct CrashPattern {
    pattern_type: CrashType,
    signature: u64,           // Hash of stack frames
    indicators: Vec<Indicator>,
}

pub enum CrashType {
    NullPointerDereference,
    UseAfterFree,
    DoubleFree,
    BufferOverflow,
    StackOverflow,
    DivideByZero,
    UnwrapNone,
    PanicUnreachable,
    SegmentationFault,
    Assertion,
}

pub enum Indicator {
    StackFramePattern(&'static str),  // Function name regex
    RegisterValue { reg: &'static str, value: u64 },
    MemoryState { addr: u64, expected: u8, actual: u8 },
    SymbolName(&'static str),
}
```

#### Step 2: Pattern Matching (Day 2)
```rust
impl RootCauseAnalyzerCapsule {
    pub fn analyze(&self, snapshot: &Snapshot) -> AnalysisResult {
        // Get stack trace (existing SIMD unwinder)
        let stack = self.stack_unwinder.unwind(snapshot.rsp, 32)?;

        // Get register state
        let regs = snapshot.registers;

        // Match against known patterns
        let mut scores = Vec::new();
        for pattern in &self.patterns {
            let score = self.score_pattern(pattern, &stack, &regs);
            scores.push((pattern, score));
        }

        // Return best match
        scores.sort_by_key(|(_, s)| -s);
        let (best_pattern, confidence) = scores[0];

        AnalysisResult {
            crash_type: best_pattern.pattern_type,
            confidence: confidence as f32 / 100.0,
            location: self.find_crash_location(&stack),
            explanation: self.generate_explanation(best_pattern, &stack),
            fix_suggestion: self.suggest_fix(best_pattern),
        }
    }
}
```

#### Step 3: Bootstrap Patterns (Day 3)
```rust
// Start with 10 high-confidence patterns (later expand to 100+)
const PATTERNS: &[CrashPattern] = &[
    // Pattern 1: Null pointer dereference
    CrashPattern {
        pattern_type: CrashType::NullPointerDereference,
        indicators: vec![
            Indicator::RegisterValue { reg: "rax", value: 0 },  // Accessing 0x0
            Indicator::StackFramePattern(".*deref.*|.*unwrap.*"),
        ],
    },

    // Pattern 2: Rust panic (unwrap on None)
    CrashPattern {
        pattern_type: CrashType::UnwrapNone,
        indicators: vec![
            Indicator::SymbolName("rust_panic"),
            Indicator::StackFramePattern(".*unwrap.*|.*expect.*"),
        ],
    },

    // Pattern 3: Buffer overflow
    CrashPattern {
        pattern_type: CrashType::BufferOverflow,
        indicators: vec![
            Indicator::RegisterValue { reg: "rip", value: 0x4141414141414141 },  // 'AAAA...'
            Indicator::StackFramePattern(".*strcpy.*|.*memcpy.*|.*sprintf.*"),
        ],
    },

    // Add 7 more patterns...
];
```

#### Step 4: MCP Tool Integration (Day 4)
```rust
// src/cli/commands.rs
pub fn handle_analyze_crash(snapshot_id: usize) -> Result<String> {
    let analyzer = RootCauseAnalyzerCapsule::new()?;
    let snapshot = REPLAY_ENGINE.get_snapshot(snapshot_id)?;
    let result = analyzer.analyze(&snapshot)?;

    // Return JSON for MCP
    Ok(serde_json::to_string(&json!({
        "root_cause": format!("{:?}", result.crash_type),
        "confidence": result.confidence,
        "location": result.location,
        "explanation": result.explanation,
        "fix_suggestion": result.fix_suggestion,
    }))?)
}
```

#### Step 5: Testing (Day 5)
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_null_pointer_detection() {
        let crash = create_null_pointer_crash();
        let result = analyze(&crash);
        assert_eq!(result.crash_type, CrashType::NullPointerDereference);
        assert!(result.confidence > 0.9);
    }

    #[test]
    fn test_unwrap_none_detection() {
        let crash = create_unwrap_none_crash();
        let result = analyze(&crash);
        assert_eq!(result.crash_type, CrashType::UnwrapNone);
        assert!(result.confidence > 0.85);
    }

    // Add 20+ test cases covering all patterns
}
```

### Expected Results
- **95%+ accuracy** on 10 common crash types (Rust panics, null derefs, buffer overflows)
- **<100μs analysis** (SIMD stack unwinding is already fast)
- **10× faster** than AI agent manually inspecting stack/registers

### MCP Usage
```json
// Before (AI agent does this manually):
1. debugger.get_stack_trace() → 8 frames
2. debugger.read_registers() → rax=0, rip=0x1234
3. AI agent infers: "Probably null pointer because rax=0"
4. Total latency: ~500ms (AI reasoning time)

// After (automatic):
1. debugger.analyze_crash(snapshot_id=142)
2. Response: { "root_cause": "NullPointerDereference", "confidence": 0.94, ... }
3. Total latency: <100μs
```

---

## Quick Win #2: Natural Language Queries (3-5 days)

### Why This Second?
- **Massive UX improvement**: AI agents ask questions directly, no shell parsing
- **Leverages time-travel**: Replay engine already captures all state
- **Simple NLP**: Regex + keyword matching (no ML needed initially)
- **20 common queries cover 80% of use cases**

### Implementation

#### Step 1: Query Parser (Day 1)
```rust
// src/ptrace/query_engine.rs
pub enum Query {
    WhereCorrupted { var_name: String },
    ShowAllocations { filter: AllocationFilter },
    FindDivergence { run_a: usize, run_b: usize },
    TraceVariable { var_name: String, from: usize, to: usize },
    WhoModified { address: u64 },
}

pub struct QueryParser;

impl QueryParser {
    pub fn parse(query: &str) -> Result<Query> {
        // Simple regex matching (expand later with ML)
        if let Some(caps) = Regex::new(r"(?i)where.*'(\w+)'.*corrupt").unwrap().captures(query) {
            return Ok(Query::WhereCorrupted {
                var_name: caps[1].to_string(),
            });
        }

        if query.contains("allocations") && query.contains("freed") {
            return Ok(Query::ShowAllocations {
                filter: AllocationFilter::NotFreed,
            });
        }

        // Add 18 more patterns...

        Err(Error::UnknownQuery)
    }
}
```

#### Step 2: Query Executor (Day 2-3)
```rust
impl QueryExecutor {
    pub fn execute(&self, query: Query) -> QueryResult {
        match query {
            Query::WhereCorrupted { var_name } => {
                // 1. Find variable address via symbol resolver
                let addr = self.symbol_resolver.resolve_variable(&var_name)?;

                // 2. Replay all snapshots, check memory at addr
                let mut modifications = Vec::new();
                for snapshot_id in 0..self.replay_engine.total_snapshots() {
                    let snapshot = self.replay_engine.get_snapshot(snapshot_id)?;
                    let value = self.memory_reader.read_u64(snapshot, addr)?;

                    // Detect modifications (value changed)
                    if snapshot_id > 0 {
                        let prev_snapshot = self.replay_engine.get_snapshot(snapshot_id - 1)?;
                        let prev_value = self.memory_reader.read_u64(prev_snapshot, addr)?;

                        if value != prev_value {
                            modifications.push(Modification {
                                snapshot_id,
                                location: self.get_current_location(&snapshot),
                                old_value: prev_value,
                                new_value: value,
                            });
                        }
                    }
                }

                QueryResult::Modifications(modifications)
            },

            Query::ShowAllocations { filter } => {
                // Track allocations via malloc/free calls
                // (Need to hook malloc/free or parse DWARF allocator symbols)
                self.find_memory_leaks(filter)
            },

            // Add 18 more query executors...
        }
    }
}
```

#### Step 3: MCP Tool (Day 4)
```rust
pub fn handle_query(query_text: &str) -> Result<String> {
    let query = QueryParser::parse(query_text)?;
    let executor = QueryExecutor::new()?;
    let result = executor.execute(query)?;

    Ok(serde_json::to_string(&json!({
        "query": query_text,
        "answer": result.to_human_readable(),
        "evidence": result.evidence(),
        "visualization": result.export_svg(),
    }))?)
}
```

#### Step 4: Testing (Day 5)
```rust
#[test]
fn test_where_corrupted_query() {
    let query = "Where does variable 'config' get corrupted?";
    let result = execute_query(query);
    assert_eq!(result.modifications.len(), 3);
    assert_eq!(result.modifications[0].location, "src/parser.rs:127");
}

#[test]
fn test_show_allocations_query() {
    let query = "Show me all heap allocations that aren't freed";
    let result = execute_query(query);
    assert!(result.leaks.len() > 0);
}
```

### Supported Queries (Phase 1)
1. "Where does variable 'X' get corrupted?"
2. "Show me all heap allocations that aren't freed"
3. "Find the first divergence point between runs A and B"
4. "Trace variable 'X' from snapshot A to B"
5. "Who modified address 0xXXXX?"
6. "Show me all function calls between snapshots A and B"
7. "Find all panics/asserts in this run"
8. "What's the value of variable 'X' at snapshot Y?"
9. "Show me the call tree for function 'foo'"
10. "Find all memory accesses to range 0xXXXX-0xYYYY"

### Expected Results
- **10/20 queries** supported in Phase 1
- **<1ms latency** for most queries (time-travel replay is fast)
- **5-10× faster** than AI agent parsing GDB output

---

## Quick Win #3: Smart Snapshot Selection (2-3 days)

### Why This Third?
- **100× better scalability**: Debug longer sessions with same memory
- **Simple heuristics**: No ML needed initially
- **Builds on existing code**: ReplayEngineCapsule already handles snapshots
- **Immediate benefit**: Users can debug 100× longer sessions

### Implementation

#### Step 1: Event Detection (Day 1)
```rust
// src/ptrace/event_detector.rs
pub enum Event {
    FunctionCall { symbol: String },
    FunctionReturn,
    Branch { taken: bool },
    MemoryAllocation { size: usize },
    MemoryFree { addr: u64 },
    Syscall { number: u64 },
    StateChange { old: u64, new: u64 },
}

impl EventDetector {
    pub fn detect(&self, snapshot: &Snapshot, prev_snapshot: &Snapshot) -> Vec<Event> {
        let mut events = Vec::new();

        // Detect function call (rip in new function)
        if self.is_function_entry(snapshot.rip) {
            events.push(Event::FunctionCall {
                symbol: self.symbol_resolver.resolve(snapshot.rip),
            });
        }

        // Detect branch (rflags changed)
        if snapshot.rflags != prev_snapshot.rflags {
            events.push(Event::Branch {
                taken: (snapshot.rflags & 0x40) != 0,  // Zero flag
            });
        }

        // Detect syscall (rax = syscall number)
        if self.is_syscall_instruction(snapshot.rip) {
            events.push(Event::Syscall {
                number: snapshot.rax,
            });
        }

        events
    }
}
```

#### Step 2: Adaptive Sampling (Day 2)
```rust
impl AdaptiveReplayEngine {
    pub fn should_snapshot(&self, events: &[Event]) -> bool {
        // Snapshot if ANY interesting event occurred
        for event in events {
            match event {
                Event::FunctionCall { .. } => return true,  // Always snapshot calls
                Event::Branch { .. } => return true,        // Always snapshot branches
                Event::MemoryAllocation { .. } => return true,
                Event::Syscall { .. } => return true,
                _ => {},
            }
        }

        // Otherwise, snapshot every Nth step (sparse sampling)
        self.step_count % 100 == 0
    }

    pub fn step_with_adaptive_snapshot(&mut self) -> Result<()> {
        let prev_snapshot = self.get_current_snapshot()?;

        // Execute single step (existing ptrace SINGLESTEP)
        self.ptrace_wrapper.single_step()?;

        // Capture new state
        let new_snapshot = self.capture_state()?;

        // Detect events
        let events = self.event_detector.detect(&new_snapshot, &prev_snapshot)?;

        // Snapshot if interesting
        if self.should_snapshot(&events) {
            self.replay_engine.take_snapshot(new_snapshot)?;
        }

        Ok(())
    }
}
```

#### Step 3: Testing (Day 3)
```rust
#[test]
fn test_adaptive_sampling_compression() {
    // Run 10K steps with adaptive sampling
    let mut engine = AdaptiveReplayEngine::new();
    for _ in 0..10_000 {
        engine.step_with_adaptive_snapshot()?;
    }

    // Should snapshot ~100-200 times (1-2%), not 10K times
    assert!(engine.total_snapshots() < 300);
    assert!(engine.total_snapshots() > 50);
}

#[test]
fn test_critical_events_captured() {
    let mut engine = AdaptiveReplayEngine::new();

    // Execute code with function calls, branches, allocations
    engine.run_until_address(0x1234)?;

    // Verify all critical events were snapshotted
    let snapshots = engine.get_all_snapshots();
    assert!(snapshots.iter().any(|s| is_function_call(s)));
    assert!(snapshots.iter().any(|s| is_branch(s)));
    assert!(snapshots.iter().any(|s| is_allocation(s)));
}
```

### Expected Results
- **100:1 compression ratio** (snapshot 1-2% of steps instead of 100%)
- **99%+ coverage** (all critical events captured)
- **200,000+ effective snapshots** (vs current 2,047)

---

## Integration Plan

### Week 1
- **Days 1-5**: Implement Root Cause Analysis
- **Day 5**: Ship kdb 0.1.1 with `debugger.analyze_crash` MCP tool

### Week 2
- **Days 1-5**: Implement Natural Language Queries
- **Day 3-5**: Implement Smart Snapshot Selection (parallel)
- **Day 5**: Ship kdb 0.2.0 with 3 breakthrough features

### Success Criteria
- **AI agent adoption**: Claude Code uses kdb for 50%+ of debugging sessions (up from 0%)
- **User feedback**: 8/10 satisfaction rating
- **Performance**: 100× faster debugging sessions (root cause in <100μs vs 10s manual)

---

## MCP Tool Summary (After Quick Wins)

### Before (10 tools, basic primitives)
1. debugger.attach
2. debugger.set_breakpoint
3. debugger.continue
4. debugger.capture_snapshot
5. debugger.step_backward
6. debugger.step_forward
7. debugger.get_stack_trace
8. debugger.read_memory
9. debugger.read_registers
10. debugger.verify_audit_trail

### After (13 tools, intelligent automation)
1-10. (existing tools)
11. **debugger.analyze_crash** ← NEW (automatic root cause)
12. **debugger.query** ← NEW (natural language)
13. **debugger.enable_adaptive_sampling** ← NEW (100× longer history)

---

## Expected Impact

| Metric | Before | After Quick Wins | Improvement |
|--------|--------|------------------|-------------|
| **Root cause time** | 10-30s (manual) | <100μs (auto) | 100,000× |
| **Query latency** | 500ms (shell) | <1ms (direct) | 500× |
| **Snapshot capacity** | 2,047 | 200,000+ | 100× |
| **AI agent adoption** | 0% | 50%+ | ∞ |
| **User satisfaction** | N/A | 8/10 | NEW |

**Result**: kdb becomes **competitive with GDB** for AI agents in just 2 weeks, with clear path to **dominance** in 3-6 months.
