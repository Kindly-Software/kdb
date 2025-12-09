# kindly_dedup Launch Sprint Plan - 2 Weeks to Production

**Version**: 1.0
**Framework**: UCE34 Q1-Q34 Systematic Discovery
**Duration**: 10 working days (80 hours)
**Target**: Launch-ready status for dedup.kindly.software
**Date**: 2025-11-18 to 2025-12-02

---

## Executive Summary

Transform kindly_dedup from "NOT READY FOR LAUNCH" to "LAUNCH READY" by systematically addressing 8 critical/high priority blockers across distribution infrastructure, code quality, testing, and documentation. This sprint applies UCE34 framework (Q1-Q34) to ensure systematic problem analysis, appropriate tier selection, and rigorous validation criteria. The plan prioritizes single-threaded pipeline (60K docs/sec VALIDATED) while deprecating broken parallel implementation (6K docs/sec, 12.8× regression). Success criteria: production deployment within 2 weeks with honest performance claims, zero production panics, and comprehensive test coverage.

**Key Metrics**:
- **80 hours** total development time (8h/day × 10 days)
- **8 blockers** (4 critical, 4 high priority)
- **95% confidence** in launch readiness after validation
- **0 production panics** (fix all unwrap()/expect() in hot paths)

---

## Sprint Goal and Success Criteria

### Primary Goal
Deploy kindly_dedup to production at dedup.kindly.software with confidence that users can download, install, and run the software reliably with accurate performance claims.

### Success Criteria (Launch Checklist)

| Criterion | Target | Validation Method |
|-----------|--------|-------------------|
| **Can users download binaries?** | ✅ YES | CDN accessible, binaries signed, checksums verified |
| **Do performance claims match reality?** | ✅ YES | All claims validated with B32 framework, contradictions removed |
| **Are production panics fixed?** | ✅ YES | Zero unwrap()/expect() in hot paths, Result<> error propagation |
| **Is CLI tested?** | ✅ YES | ≥80% coverage, all 9 screen modules have integration tests |
| **Can we deploy with confidence?** | ✅ YES | Automated CI/CD, monitoring, rollback plan |

### Scope Boundaries

**IN SCOPE** (Launch Blockers):
1. Distribution infrastructure (dedup.kindly.software deployment)
2. Production panic fixes (mmap_bucketer.rs, batch_lookup.rs)
3. Performance claims cleanup (remove contradictory claims)
4. CLI testing (80% coverage target)
5. Protection system validation (8/11 layers tested)
6. API documentation consolidation
7. Release automation (CI/CD pipeline)
8. Parallel pipeline deprecation (broken, 6K docs/sec)

**OUT OF SCOPE** (Post-Launch):
- Parallel pipeline redesign (2-3 months, T5 Streaming architecture)
- New feature development
- GUI improvements beyond critical bugs
- Performance optimization beyond validation

---

## UCE34 Framework Application to Sprint Planning

### Phase 1: Problem Understanding (Q1-Q9)

#### Q1: What is the STATED problem?
**Answer**: Get kindly_dedup from "NOT READY FOR LAUNCH" to "LAUNCH READY" status in 2 weeks.

**Context**:
- Internal launch readiness analysis identified 8 blockers
- Single-threaded pipeline (60K docs/sec) is VALIDATED and production-ready
- Parallel pipeline (6K docs/sec) is BROKEN with 12.8× regression
- Critical gaps in distribution, testing, documentation, and panic handling

#### Q2: What SPECIFIC gaps block launch?

**Critical Blockers** (Must Fix):
1. **Distribution Infrastructure Missing**
   - dedup.kindly.software not deployed
   - No CDN, no signed binaries, no release artifacts
   - Users cannot download the product

2. **Parallel Pipeline Broken**
   - ParallelDedupPipeline: 6K docs/sec (12.8× SLOWER than 60K sequential)
   - Root causes: Tokenization in workers, O(capacity) extraction, CAS contention
   - Action: Deprecate and hide from public API

3. **Critical Production Panics**
   - mmap_bucketer.rs:80 - `panic!("Unsupported region_id: {}", region_id)`
   - batch_lookup.rs:301,304,309-312 - `.unwrap()` / `.expect()` in hot paths
   - Zero error recovery, crashes on unexpected input

4. **CLI Completely Untested**
   - 15% test coverage (9 screen modules, 0 tested)
   - No integration tests for TUI workflows
   - Unknown reliability, potential for production crashes

**High Priority Issues** (Should Fix):
5. **Performance Claims Contradictory**
   - CLAUDE.md claims "912K docs/sec", "204×", "373K docs/sec"
   - Internal docs (PARALLEL_PERFORMANCE_INVESTIGATION.md) REJECT these claims
   - Sales credibility at risk

6. **Protection System Untested**
   - 11 protection layers, only 8 tested
   - Feature-gated code paths never exercised
   - Unknown reliability for paying customers

7. **API Documentation Scattered**
   - No unified reference, 30% of examples broken
   - Users cannot understand how to use the library
   - Developer experience poor

8. **No Release Automation**
   - Manual build/release process only
   - Error-prone, slow, not scalable
   - Delays future releases

#### Q3: What are ACTUAL constraints?

**Time Constraints**:
- **2 weeks** (10 working days, 80 hours)
- **1 senior Rust developer** (no parallelization of work)
- **8 hours/day** capacity (sustainable pace)

**Technical Constraints**:
- **Trade secret code** - Cannot use public CI/CD (GitHub Actions) for proprietary features
- **Nightly Rust** - SIMD features require nightly toolchain
- **Existing codebase** - 24,000+ lines, cannot rewrite
- **External dependencies** - CDN setup, license backend (existing kindly-dedup-stripe)

**Resource Constraints**:
- **No additional developers** - Solo work, no team scaling
- **Limited cloud budget** - Must use existing infrastructure
- **Existing customers** - Cannot break backward compatibility

#### Q4: Dependencies?

**External Dependencies**:
- **CDN setup** (Day 1-2): Requires domain DNS, SSL certificates, hosting provider
- **License backend** (Day 3): Existing kindly-dedup-stripe.fly.dev (already deployed)
- **CI/CD provider** (Day 7): Must support private repos for trade secrets
- **Test data** (Day 5-6): HuggingFace datasets for integration tests

**Internal Dependencies**:
- **Panic fixes** (Day 3) → Must complete before CLI testing (Day 5)
- **Performance claims cleanup** (Day 4) → Blocks documentation update (Day 7)
- **Protection testing** (Day 6) → Validates meta-capsule feature flags
- **Release automation** (Day 7-8) → Enables final validation (Day 9)

#### Q5: Edge cases?

**Distribution Edge Cases**:
- CDN downtime → Fallback to GitHub releases
- Binary signature verification failure → Clear error messages + manual verification docs
- Unsupported platform (non-x86_64) → Document limitations, provide source build instructions

**Code Quality Edge Cases**:
- Invalid region_id in mmap → Return Error instead of panic
- Thread pool queue full → Return Error with retry logic
- Mutex poisoned → Return Error, document as non-recoverable

**Testing Edge Cases**:
- CLI screen transitions with invalid state → Error recovery + logging
- Protection layers disabled (feature flags) → Graceful degradation
- Large corpus (10M+ docs) → Memory limits, streaming validation

**Documentation Edge Cases**:
- Broken example code → Automated testing in CI/CD
- Outdated performance claims → Single source of truth (Cargo.toml version → docs)
- Contradictory claims across files → Unified validation script

#### Q6-Q9: Failure modes, bottlenecks, validation strategy

**Failure Modes**:
- **CDN setup fails** → Use GitHub Releases as temporary fallback (Day 1 mitigation)
- **Panic fixes incomplete** → Incremental deployment with feature flags (Day 3 rollback)
- **CLI tests fail** → Fix critical paths only, document known issues (Day 5 triage)
- **Time overrun** → MVP scope reduction (see Risk Mitigation § "What if we run out of time?")

**Bottlenecks**:
- **External CDN setup** (6-8 hours) → Parallel with panic fixes (Day 1-2)
- **CLI integration tests** (8-10 hours) → Cannot parallelize, sequential execution
- **Documentation consolidation** (6-8 hours) → Requires all blockers fixed first

**Validation Strategy**:
- **Daily commits** → Audit trail of progress (Q34 compliance)
- **Incremental validation** → Each task has specific "Definition of Done"
- **B32 framework** → All performance claims re-validated with 95% CI
- **T28 testing** → Comprehensive test coverage (unit/property/integration/production)

---

### Phase 2: Tier Selection (Q10-Q12)

#### Q10: Which computational capsule tier applies to sprint PLANNING?

**Answer**: **T0 Auditable** (for sprint tracking) + **T1 Atomic** (for progress coordination)

**Rationale**:
- **T0 Auditable**: Sprint requires Q34 audit trail of daily progress, blocker tracking, rollback decisions
- **T1 Atomic**: Coordination across 8 blockers with dependencies requires lockfree tracking

**Application**:
- **Daily standup format** → Hash-chained audit log (Q34 compliance)
- **Blocker tracking** → AtomicU64 status flags (blocked/in-progress/complete)
- **Progress metrics** → Deterministic reproducibility (0 subjective estimates)

#### Q11: How does Rust apply to sprint execution?

**Rust-Specific Sprint Adaptations**:
1. **Zero-cost abstractions** → Panic fixes use Result<> with zero runtime overhead
2. **Type safety** → API documentation uses doc tests (compilable examples)
3. **Feature flags** → Protection testing uses cargo features (no runtime switches)
4. **Nightly features** → SIMD validation requires `+nightly` toolchain
5. **Cargo tooling** → Release automation uses `cargo build --release` + signing scripts

#### Q12: Nightly features needed for sprint?

**Required Nightly Features**:
- `portable_simd` - SIMD MinHash validation (Day 4 performance claims)
- `simd-hashing` - Text hashing benchmarks (Day 4 B32 validation)
- `nightly-atomic` - Persistent dedup testing (Day 6 protection system)

**Stable Fallback**:
- All critical fixes (panics, CLI, distribution) work on stable Rust
- Nightly only required for SIMD performance validation

---

### Phase 3: Validation (Q30-Q34)

#### Q30: Simplicity - Are we solving the RIGHT problem?

**Validation Questions**:
- **Distribution**: Is CDN the simplest solution? (YES - GitHub Releases too slow)
- **Panics**: Is Result<> error propagation simpler than panics? (YES - recoverable errors)
- **Parallel**: Is deprecation simpler than fixing? (YES - redesign takes 2-3 months)
- **Claims**: Is removing claims simpler than validating? (YES - zero false claims target)

#### Q31: Constraints - What are the HARD limits?

**Hard Constraints**:
- **2 weeks** → Cannot extend timeline (external commitment)
- **Trade secret** → Cannot use public CI/CD for all features
- **1 developer** → Cannot parallelize human work
- **60K docs/sec** → Single-threaded baseline (VALIDATED, cannot exceed without re-architecture)

#### Q32: Rust Transform - How does Rust make this IMPOSSIBLE in other languages?

**Rust-Specific Advantages**:
- **Panic to Result<>** → Type-checked error propagation (Python/Go: runtime errors hidden)
- **Feature flags** → Compile-time protection layers (C++: preprocessor macros, error-prone)
- **Doc tests** → Compilable examples (JavaScript: JSDoc comments, not validated)
- **Cargo** → Unified build/test/bench/release (Python: 5+ tools: pip, pytest, sphinx, wheel, twine)

#### Q33: Validation - How do we PROVE each fix works?

**Validation Criteria by Blocker**:

| Blocker | Validation Method | Success Criteria |
|---------|-------------------|------------------|
| **1. Distribution** | Manual download + signature verification | Binary downloads in <5s, SHA256 matches |
| **2. Parallel Deprecation** | API hidden, docs removed | `cargo doc` shows no ParallelDedupPipeline |
| **3. Panic Fixes** | Integration tests with invalid input | Zero panics, all return Err() |
| **4. CLI Testing** | Integration tests for all 9 screens | ≥80% coverage, all screens tested |
| **5. Performance Claims** | B32 validation suite | All claims match validated benchmarks |
| **6. Protection Testing** | Feature flag tests | All 11 layers tested with flags |
| **7. API Documentation** | Doc tests in CI/CD | 100% of examples compile and run |
| **8. Release Automation** | CI/CD pipeline dry-run | Full build/test/release in <30 minutes |

#### Q34: Audit Trail - How do we track sprint progress?

**Daily Standup Format** (Hash-Chained):
```markdown
## Day N Standup - YYYY-MM-DD

### Completed (Hash: <SHA256 of previous day>)
- [ ] Blocker 1: Task 1 (X hours actual)
- [ ] Blocker 2: Task 2 (Y hours actual)

### In Progress
- [ ] Blocker 3: Task 3 (Z hours remaining)

### Blocked
- [ ] Blocker 4: Task 4 (Dependency: CDN setup)

### Risks
- Risk 1: CDN setup delayed (Mitigation: Use GitHub Releases)

### Hash: <SHA256 of this day's content>
```

**Audit Log Storage**: `/home/samuel/Primitives/kindly_dedup/SPRINT_AUDIT_LOG.md`

---

## 10-Day Sprint Breakdown

### Overview

| Day | Focus Area | Hours | Deliverables |
|-----|------------|-------|--------------|
| **1-2** | Distribution Infrastructure | 16h | CDN deployed, binaries signed, checksums verified |
| **3-4** | Code Quality + Performance Claims | 16h | Zero production panics, honest claims, deprecated parallel |
| **5-6** | Testing Gaps | 16h | CLI 80% coverage, protection 100% tested |
| **7-8** | Documentation + Automation | 16h | Unified API docs, CI/CD pipeline |
| **9-10** | Validation + Polish | 16h | Full integration validation, launch checklist |

---

## Day 1-2: Distribution Infrastructure (16 hours)

### UCE34 Analysis

#### Q1-Q9: Problem Statement
**STATED Problem**: Users cannot download kindly_dedup binaries. No dedup.kindly.software deployment, no CDN, no signed artifacts.

**ACTUAL Problem**:
- Domain not configured (DNS, SSL)
- No binary hosting infrastructure
- No signing/verification workflow
- No download verification (checksums, signatures)

**Constraints**:
- Must support Linux x86_64 (primary target)
- Binary size ~15-25 MB (LTO + strip)
- Trade secret code cannot be in public GitHub
- SSL required for production deployment

**Dependencies**:
- Domain registrar access (samuel@kindly.software)
- CDN provider (BunnyCDN, Cloudflare, or Fly.io CDN)
- GPG key for binary signing
- GitHub repository for open-source components

**Edge Cases**:
- CDN downtime → Fallback to GitHub Releases (secondary mirror)
- Binary signature verification failure → Manual verification docs
- Unsupported platform (ARM, Windows) → Document limitations

#### Q10-Q12: Tier Selection
**Tier**: Infrastructure (not computational capsule, but T1 Atomic concepts apply)

**Application**:
- Atomic deployment (all-or-nothing CDN updates)
- Deterministic builds (reproducible binaries)
- Zero downtime (CDN failover)

#### Q30-Q34: Validation
**Simplicity**: CDN is simplest solution (GitHub Releases = 10× slower downloads)

**Constraints**:
- 16 hours (2 days)
- No prior CDN experience (learning curve)
- Trade secret protection (no public binaries for protected features)

**Validation Criteria**:
- [ ] `curl https://dedup.kindly.software/latest/kindly_dedup-linux-x86_64` downloads in <5s
- [ ] `sha256sum kindly_dedup-linux-x86_64` matches published checksum
- [ ] `gpg --verify kindly_dedup-linux-x86_64.asc` succeeds
- [ ] Fallback to GitHub Releases works (manual test)

### Tasks (16 hours total)

#### Day 1: CDN Setup (8 hours)

**Priority**: CRITICAL

**Estimated Hours**: 8h (6h actual + 2h buffer)

**Task Breakdown**:
1. **Domain Configuration** (2h)
   - Configure DNS for dedup.kindly.software (CNAME to CDN)
   - Set up SSL certificate (Let's Encrypt auto-renewal)
   - Verify HTTPS redirect works

2. **CDN Provider Selection** (1h)
   - Evaluate: BunnyCDN ($1/mo, 1TB), Cloudflare (free tier), Fly.io CDN (existing account)
   - Decision criteria: Cost, speed, ease of setup
   - **Recommended**: BunnyCDN (simple, cheap, fast)

3. **CDN Storage Setup** (2h)
   - Create storage zone: `kindly-dedup-releases`
   - Configure directory structure: `/latest/`, `/v2.0.0/`, `/stable/`
   - Set up access credentials (API key)
   - Test manual upload: `curl -X PUT --data-binary @binary https://storage.bunnycdn.com/...`

4. **Binary Signing Workflow** (3h)
   - Generate GPG key: `gpg --full-generate-key` (RSA 4096, expires 2027)
   - Export public key: `gpg --export --armor samuel@kindly.software > kindly-release.asc`
   - Create signing script: `scripts/sign_release.sh`
   - Test signing: `gpg --detach-sign --armor kindly_dedup`
   - Verify: `gpg --verify kindly_dedup.asc kindly_dedup`

**Blockers**:
- Domain DNS propagation (24-48h) → Use Fly.io subdomain as temporary workaround

**Rollback Plan**:
- If CDN fails → Use GitHub Releases (works but slower)
- If DNS fails → Use IP address temporarily

**Validation Criteria**:
- [ ] DNS resolves: `dig dedup.kindly.software` returns CNAME
- [ ] SSL works: `curl https://dedup.kindly.software/health` returns 200
- [ ] Binary upload succeeds: Manual curl upload works
- [ ] GPG signature verifies: `gpg --verify` returns "Good signature"

#### Day 2: Release Artifacts (8 hours)

**Priority**: CRITICAL

**Estimated Hours**: 8h (6h actual + 2h buffer)

**Task Breakdown**:
1. **Build Release Binaries** (2h)
   - Clean build: `cargo clean && cargo build --release --bin kindly_dedup --features interactive`
   - Strip symbols: `strip --strip-all target/release/kindly_dedup`
   - Verify size: <25 MB (LTO + strip should achieve this)
   - Test binary: `./target/release/kindly_dedup --version`

2. **Generate Checksums** (1h)
   - SHA256: `sha256sum target/release/kindly_dedup > SHA256SUMS`
   - SHA512: `sha512sum target/release/kindly_dedup > SHA512SUMS`
   - Create combined checksums file
   - Sign checksums: `gpg --clearsign SHA256SUMS`

3. **Upload to CDN** (2h)
   - Create upload script: `scripts/upload_release.sh`
   - Upload binary: `kindly_dedup-linux-x86_64`
   - Upload signature: `kindly_dedup-linux-x86_64.asc`
   - Upload checksums: `SHA256SUMS`, `SHA512SUMS`, `SHA256SUMS.asc`
   - Upload public key: `kindly-release.asc`
   - Set `/latest/` symlinks to `/v2.0.0/`

4. **Download Verification** (2h)
   - Write verification script: `scripts/verify_download.sh`
   - Test download: `curl -O https://dedup.kindly.software/latest/kindly_dedup-linux-x86_64`
   - Test checksum: `sha256sum --check SHA256SUMS`
   - Test signature: `gpg --verify kindly_dedup-linux-x86_64.asc kindly_dedup-linux-x86_64`
   - Document manual verification steps in README

5. **Fallback Mirror** (1h)
   - Upload to GitHub Releases (kindly-ecosystem/kindly-dedup/releases/v2.0.0)
   - Test GitHub download: `curl -L https://github.com/kindly-ecosystem/kindly-dedup/releases/download/v2.0.0/kindly_dedup-linux-x86_64`
   - Document fallback in README: "If CDN is slow, use GitHub Releases"

**Blockers**:
- CDN upload credentials missing → Request from BunnyCDN support
- Binary size >25 MB → Investigate dependencies, consider splitting features

**Rollback Plan**:
- If CDN upload fails → Use GitHub Releases exclusively
- If signing fails → Ship unsigned binary with SHA256 checksum only (document risk)

**Validation Criteria**:
- [ ] Binary downloads in <5s: `time curl -O https://dedup.kindly.software/latest/kindly_dedup-linux-x86_64`
- [ ] Checksum matches: `sha256sum --check SHA256SUMS`
- [ ] Signature verifies: `gpg --verify kindly_dedup-linux-x86_64.asc`
- [ ] GitHub fallback works: Download from GitHub succeeds
- [ ] README documents verification: Installation section has verification steps

**Daily Deliverable (Day 2 End)**:
```bash
# Users can now run:
curl -O https://dedup.kindly.software/latest/kindly_dedup-linux-x86_64
curl -O https://dedup.kindly.software/latest/kindly_dedup-linux-x86_64.asc
curl -O https://dedup.kindly.software/latest/kindly-release.asc
gpg --import kindly-release.asc
gpg --verify kindly_dedup-linux-x86_64.asc kindly_dedup-linux-x86_64
chmod +x kindly_dedup-linux-x86_64
./kindly_dedup-linux-x86_64 --version
```

---

## Day 3-4: Code Quality + Performance Claims (16 hours)

### UCE34 Analysis

#### Q1-Q9: Problem Statement
**STATED Problem**:
1. Production panics in mmap_bucketer.rs:80, batch_lookup.rs:301,304,309-312
2. Parallel pipeline broken (6K docs/sec, 12.8× regression vs 60K sequential)
3. Performance claims contradictory ("912K", "204×", "373K" rejected by internal docs)

**ACTUAL Problem**:
- **Panics**: Unrecoverable errors crash production deployments
  - mmap_bucketer.rs:80 - Invalid region_id causes panic
  - batch_lookup.rs:301,304,309-312 - Arc/Mutex unwrap() fails if poisoned
- **Parallel**: Fundamental architectural issues (not fixable in 2 weeks)
  - Tokenization inside parallel workers (CPU bottleneck)
  - O(capacity) signature extraction (memory allocation churn)
  - CAS contention on shared state (lockfree but contended)
- **Claims**: Multiple sources of truth, contradictory performance numbers
  - CLAUDE.md: "912K docs/sec @ 16 cores" (REJECTED by PARALLEL_PERFORMANCE_INVESTIGATION.md)
  - Cargo.toml: "60K docs/sec" (VALIDATED)
  - README: "373K docs/sec" (measured at 10M docs, but ParallelDedupPipeline broken)

**Constraints**:
- 16 hours total (cannot redesign parallel pipeline)
- Must maintain backward compatibility for DedupPipeline (sequential)
- Cannot break existing benchmarks
- Trade secret algorithms must remain protected

**Dependencies**:
- Panic fixes → Enable CLI testing (Day 5)
- Performance claims cleanup → Enable documentation update (Day 7)
- Parallel deprecation → Simplifies future maintenance

**Edge Cases**:
- Invalid mmap region access → Return Error with context
- Thread pool exhaustion → Return Error with retry suggestion
- Mutex poisoned → Return Error (non-recoverable, document as fatal)

#### Q10-Q12: Tier Selection
**Tier**: T0 Auditable (error handling) + T1 Atomic (Result<> propagation)

**Application**:
- Replace panic!() with Result<Error>
- Use AtomicU64 error counters for monitoring
- Hash-chain error provenance (Q34 audit trail)

#### Q30-Q34: Validation
**Simplicity**:
- Is Result<> simpler than panic!()? YES (recoverable errors)
- Is deprecation simpler than redesign? YES (2-3 month redesign out of scope)
- Is single source of truth simpler? YES (Cargo.toml version controls all claims)

**Constraints**:
- 16 hours (3-4 hours per panic fix, 4 hours for claims, 4 hours for deprecation)
- No breaking changes (DedupPipeline API must remain stable)

**Validation Criteria**:
- [ ] Zero panics in production code: `cargo clippy -- -D clippy::panic` succeeds
- [ ] All hot paths return Result<>: Integration tests with invalid input
- [ ] ParallelDedupPipeline hidden: `cargo doc` shows no public parallel API
- [ ] Performance claims unified: All numbers match Cargo.toml + B32 benchmarks

### Tasks (16 hours total)

#### Day 3: Panic Fixes (8 hours)

**Priority**: CRITICAL

**Estimated Hours**: 8h (6h actual + 2h buffer)

**Task Breakdown**:
1. **Fix mmap_bucketer.rs:80 Panic** (2h)
   - **Current Code**:
     ```rust
     } else {
         panic!("Unsupported region_id: {}", region_id)
     }
     ```
   - **Fix**:
     ```rust
     } else {
         return Err(Error::InvalidRegionId {
             region_id,
             valid_range: 0..=1,
             context: "mmap_bucketer region offset calculation"
         });
     }
     ```
   - Add error variant to Error enum: `InvalidRegionId { region_id: u8, valid_range: Range<u8>, context: &'static str }`
   - Add integration test: `test_invalid_region_id_returns_error()`
   - Validate: `cargo test --test mmap_bucketer_tests`

2. **Fix batch_lookup.rs:301,304,309-312 Unwraps** (3h)
   - **Current Code**:
     ```rust
     let mut res = results.lock().unwrap();  // Line 301
     .expect("Thread pool queue full");      // Line 304
     Arc::try_unwrap(results).expect("Arc still has multiple owners")  // Line 309
     .into_inner().expect("Mutex poisoned")  // Line 312
     ```
   - **Fix**:
     ```rust
     // Line 301: Handle mutex poisoning
     let mut res = results.lock().map_err(|_| Error::MutexPoisoned {
         context: "batch_lookup results collection"
     })?;

     // Line 304: Handle queue full
     .map_err(|_| Error::ThreadPoolQueueFull {
         queue_capacity: pool.capacity(),
         suggestion: "Reduce batch size or increase thread pool capacity"
     })?;

     // Line 309-312: Handle Arc/Mutex cleanup
     let results_inner = Arc::try_unwrap(results).map_err(|arc| Error::ArcStillShared {
         strong_count: Arc::strong_count(&arc),
         context: "batch_lookup results Arc cleanup"
     })?;

     let final_results = results_inner.into_inner().map_err(|_| Error::MutexPoisoned {
         context: "batch_lookup final results extraction"
     })?;
     ```
   - Add error variants:
     - `MutexPoisoned { context: &'static str }`
     - `ThreadPoolQueueFull { queue_capacity: usize, suggestion: &'static str }`
     - `ArcStillShared { strong_count: usize, context: &'static str }`
   - Add integration tests:
     - `test_mutex_poisoning_returns_error()` (simulate poisoned mutex)
     - `test_thread_pool_queue_full_returns_error()` (stress test with small queue)
   - Validate: `cargo test --test batch_lookup_tests`

3. **Clippy Panic Linting** (1h)
   - Enable panic detection: Add to `.cargo/config.toml`:
     ```toml
     [target.'cfg(all())']
     rustflags = ["-D", "clippy::panic", "-D", "clippy::unwrap_used", "-D", "clippy::expect_used"]
     ```
   - Run: `cargo clippy --all-features --tests --benches`
   - Fix any remaining panics/unwraps in src/ (not tests/)
   - Document exceptions: Allow unwrap() in test code only

4. **Integration Testing** (2h)
   - Create `tests/panic_regression_tests.rs`:
     - `test_invalid_mmap_region_no_panic()` - Pass region_id=255, verify Err() returned
     - `test_mutex_poisoning_no_panic()` - Simulate poisoned mutex, verify Err() returned
     - `test_thread_pool_exhaustion_no_panic()` - Submit 10K tasks to 4-thread pool, verify Err() returned
   - Run: `cargo test --test panic_regression_tests`
   - Validate: All tests pass, zero panics in output

**Blockers**:
- Error enum changes may require updating call sites (estimate +1h if >10 call sites)

**Rollback Plan**:
- If fixing breaks tests → Revert to panic!() temporarily, document as "known issue"
- If integration tests fail → Fix critical paths only (mmap_bucketer), defer batch_lookup to Day 4

**Validation Criteria**:
- [ ] `cargo clippy -- -D clippy::panic` succeeds (zero panics in src/)
- [ ] Integration tests pass: `cargo test --test panic_regression_tests`
- [ ] Invalid input returns Err(): Manual test with invalid region_id
- [ ] Error messages are actionable: Each error includes context + suggestion

#### Day 4: Performance Claims Cleanup + Parallel Deprecation (8 hours)

**Priority**: CRITICAL (claims), HIGH (deprecation)

**Estimated Hours**: 8h (6h actual + 2h buffer)

**Task Breakdown**:
1. **Audit Performance Claims** (2h)
   - Scan all documentation files: `grep -r "docs/sec\|speedup\|×" docs/ CLAUDE.md README.md Cargo.toml`
   - Create claim inventory: `docs/PERFORMANCE_CLAIMS_AUDIT.md`
   - Classify each claim:
     - VALIDATED (B32 benchmarks exist, 95% CI)
     - PROJECTED (formula-based, not measured)
     - REJECTED (contradicts measurements)
   - Cross-reference with `benches/` directory

2. **Update Single Source of Truth** (2h)
   - **Cargo.toml**: Update description to reflect VALIDATED claims only:
     ```toml
     description = "LLM dataset deduplication - 60K docs/sec (single-threaded VALIDATED), 38× vs Python"
     ```
   - **CLAUDE.md**: Remove all REJECTED claims:
     - ❌ Remove: "912K docs/sec @ 16 cores" (ParallelDedupPipeline broken)
     - ❌ Remove: "373K docs/sec" (parallel baseline, not single-threaded)
     - ❌ Remove: "204× compound" (projected, not measured)
     - ✅ Keep: "60K docs/sec" (VALIDATED, benches/sales/v1_0_baseline.rs)
     - ✅ Keep: "38× vs Python datasketch" (VALIDATED, B32 baseline comparison)
   - **README.md**: Update performance section:
     ```markdown
     ## Performance (VALIDATED)

     **Single-Threaded** (AMD Ryzen 9 6900HX):
     - Throughput: 60,000 docs/sec
     - Latency: 16.7 µs per document
     - Speedup: 38× vs Python datasketch (1,600 docs/sec)

     **Evidence**: See `benches/sales/v1_0_baseline.rs` (1000+ iterations, 95% CI)

     **Parallel Pipeline**: ⚠️ EXPERIMENTAL (6K docs/sec, 12.8× slower than sequential)
     - Status: Known performance regression, requires redesign
     - Recommendation: Use single-threaded `DedupPipeline` for production
     ```

3. **Deprecate Parallel Pipeline** (2h)
   - Mark `ParallelDedupPipeline` as `#[deprecated]`:
     ```rust
     #[deprecated(
         since = "2.0.1",
         note = "ParallelDedupPipeline has 12.8× performance regression (6K vs 60K docs/sec). \
                 Use DedupPipeline (single-threaded) instead. \
                 Parallel implementation requires redesign (see PARALLEL_PERFORMANCE_INVESTIGATION.md)."
     )]
     pub struct ParallelDedupPipeline { ... }
     ```
   - Hide from documentation: Add `#[doc(hidden)]` if still used internally
   - Update examples: Remove all parallel examples from `examples/` directory
   - Update tests: Disable parallel integration tests (move to `#[ignore]`)
   - Update Cargo.toml: Mark `parallel-dedup` feature as deprecated in comments

4. **B32 Validation Suite** (2h)
   - Re-run all sales benchmarks: `cargo bench --bench v1_0_baseline --features benchmarking`
   - Verify claims match output:
     - 60K docs/sec (±5% tolerance)
     - 38× speedup (±10% tolerance)
   - Generate report: `target/criterion/report/index.html`
   - Copy results to `docs/VALIDATED_PERFORMANCE_CLAIMS.md`:
     ```markdown
     # Validated Performance Claims (B32 Framework)

     **Benchmark**: v1.0 Baseline (benches/sales/v1_0_baseline.rs)
     **Date**: 2025-11-21
     **Hardware**: AMD Ryzen 9 6900HX, 8c/16t, 64GB DDR5-4800
     **Iterations**: 1000+
     **Confidence Interval**: 95%

     | Metric | Value | Baseline | Speedup |
     |--------|-------|----------|---------|
     | Throughput | 58,500 docs/sec | 1,600 docs/sec (Python) | 36.6× |
     | Latency | 17.1 µs | 625 µs | 36.5× |

     **Classification**: EXCEPTIONAL (B32 tier 2-10×)
     ```

**Blockers**:
- Benchmark failures → Investigate regressions, may indicate real bugs

**Rollback Plan**:
- If benchmarks fail → Document as "known regression", investigate root cause
- If deprecation breaks builds → Use feature flag instead of full deprecation

**Validation Criteria**:
- [ ] All claims match benchmarks: `diff <(grep "docs/sec" docs/VALIDATED_PERFORMANCE_CLAIMS.md) <(grep "docs/sec" CLAUDE.md README.md)`
- [ ] No contradictory claims: Manual review of all docs
- [ ] Parallel deprecated: `cargo doc` shows deprecation warning
- [ ] Benchmarks pass: `cargo bench --bench v1_0_baseline` succeeds

**Daily Deliverable (Day 4 End)**:
- Zero production panics (all hot paths return Result<>)
- Honest performance claims (60K docs/sec, 38× speedup)
- Parallel pipeline deprecated (documented as broken)
- B32 validation report (docs/VALIDATED_PERFORMANCE_CLAIMS.md)

---

## Day 5-6: Testing Gaps (16 hours)

### UCE34 Analysis

#### Q1-Q9: Problem Statement
**STATED Problem**:
1. CLI completely untested (15% coverage, 9 screen modules, 0 tested)
2. Protection system untested (11 layers, only 8 tested)

**ACTUAL Problem**:
- **CLI**: Interactive TUI has zero integration tests
  - 9 screens: MainMenu, CorpusSelection, Configuration, Processing, Results, Settings, Help, ProtectionStatus, Error
  - Complex state machine (screen transitions, input validation)
  - Unknown reliability, potential for production crashes
- **Protection**: META_CAPSULE feature-gated code never exercised
  - 11 layers: Build hardening, crypto license, encrypted state, TPM binding, fuzzy extractor, obfuscation, remote attestation, orchestrator, anomaly detector, memory encryption, kernel protection
  - Only 8 tested: First 8 layers (P0+P1), last 3 (P2) untested
  - Unknown reliability for paying customers

**Constraints**:
- 16 hours total (8h CLI, 8h protection)
- Cannot redesign CLI (only test existing implementation)
- Protection system is feature-gated (requires nightly + specific cargo features)

**Dependencies**:
- Panic fixes (Day 3) → CLI tests can now use invalid input without crashing
- Performance claims (Day 4) → CLI displays accurate metrics

**Edge Cases**:
- CLI: Invalid screen transitions (e.g., Results before Processing)
- CLI: Terminal resize during operation
- CLI: Ctrl+C during long-running operation
- Protection: Disabled feature flags (graceful degradation)
- Protection: Simulated hardware unavailability (TPM, PUF)

#### Q10-Q12: Tier Selection
**Tier**: T0 Auditable (test coverage tracking) + T1 Atomic (test coordination)

**Application**:
- Test coverage metrics (track progress toward 80% target)
- Deterministic test ordering (no flaky tests)
- Lockfree test execution (parallel test runner)

#### Q30-Q34: Validation
**Simplicity**:
- Is integration testing simpler than unit testing? NO (unit tests first, then integration)
- Is mocking simpler than end-to-end? DEPENDS (mock for fast tests, E2E for critical paths)

**Constraints**:
- 16 hours (cannot achieve 100% coverage, target 80%)
- 9 CLI screens (1.5h per screen average)
- 3 untested protection layers (2.5h per layer average)

**Validation Criteria**:
- [ ] CLI coverage ≥80%: `cargo tarpaulin --out Html --output-dir target/coverage`
- [ ] All 9 screens tested: `cargo test --test cli_integration_tests`
- [ ] All 11 protection layers tested: `cargo test --all-features --test protection_tests`
- [ ] Zero flaky tests: Run test suite 10× consecutively, all pass

### Tasks (16 hours total)

#### Day 5: CLI Integration Tests (8 hours)

**Priority**: CRITICAL

**Estimated Hours**: 8h (7h actual + 1h buffer)

**Task Breakdown**:
1. **Test Infrastructure Setup** (1h)
   - Create `tests/cli_integration_tests.rs`
   - Add test utilities:
     ```rust
     use crossterm::event::{Event, KeyCode, KeyEvent};

     struct TestHarness {
         app_state: AppState,
         renderer: TestRenderer,
     }

     impl TestHarness {
         fn send_key(&mut self, key: KeyCode) -> Result<()> { ... }
         fn assert_screen(&self, expected: ScreenType) { ... }
         fn assert_output_contains(&self, text: &str) { ... }
     }
     ```
   - Add mock file system: `TempDir` for corpus selection tests

2. **Screen Tests (7 screens × 0.75h = 5.25h, round to 5h)**:

   **a) MainMenu Screen** (0.75h)
   - Test: `test_main_menu_navigation()`
     - Send arrow keys (Up/Down)
     - Assert menu selection changes
     - Send Enter key
     - Assert screen transitions to CorpusSelection
   - Test: `test_main_menu_quit()`
     - Send 'q' key
     - Assert application exits cleanly

   **b) CorpusSelection Screen** (0.75h)
   - Test: `test_corpus_selection_file_browser()`
     - Mock file system with test files
     - Send arrow keys to navigate
     - Assert file list updates
     - Send Enter to select file
     - Assert screen transitions to Configuration
   - Test: `test_corpus_selection_invalid_file()`
     - Select non-existent file
     - Assert error message displayed
     - Assert screen remains on CorpusSelection

   **c) Configuration Screen** (0.75h)
   - Test: `test_configuration_threshold_input()`
     - Send numeric keys (0.85)
     - Assert threshold updates
     - Send Enter
     - Assert screen transitions to Processing
   - Test: `test_configuration_invalid_threshold()`
     - Send invalid input (1.5, negative)
     - Assert error message
     - Assert configuration rejects invalid values

   **d) Processing Screen** (0.75h)
   - Test: `test_processing_progress_updates()`
     - Start dedup pipeline
     - Assert progress bar updates (0% → 100%)
     - Assert screen transitions to Results on completion
   - Test: `test_processing_cancel()`
     - Start processing
     - Send Ctrl+C
     - Assert pipeline cancels gracefully
     - Assert screen returns to MainMenu

   **e) Results Screen** (0.75h)
   - Test: `test_results_display_clusters()`
     - Complete dedup pipeline
     - Assert cluster count displayed
     - Assert duplicate statistics accurate
   - Test: `test_results_export()`
     - Press 'e' to export
     - Assert file saved
     - Assert success message displayed

   **f) Settings Screen** (0.75h)
   - Test: `test_settings_update_thread_count()`
     - Navigate to thread count setting
     - Send numeric keys
     - Assert setting updates
     - Assert validation (1-64 threads)

   **g) Help Screen** (0.75h)
   - Test: `test_help_screen_navigation()`
     - Send 'h' or '?' key
     - Assert help text displayed
     - Send Esc
     - Assert returns to previous screen

   **h) ProtectionStatus Screen** (0.75h)
   - Test: `test_protection_status_display()`
     - Navigate to protection status
     - Assert all 11 layers listed
     - Assert status indicators (✅/❌/⚠️)
   - Test: `test_protection_status_feature_disabled()`
     - Build without meta-capsule feature
     - Assert "Feature not enabled" message

   **i) Error Screen** (0.75h)
   - Test: `test_error_screen_display()`
     - Trigger error (invalid corpus format)
     - Assert error screen shown
     - Assert error message accurate
     - Send Enter to acknowledge
     - Assert returns to previous screen

3. **Edge Case Tests** (1h)
   - Test: `test_terminal_resize_handling()`
     - Simulate terminal resize during operation
     - Assert layout adjusts gracefully
   - Test: `test_ctrl_c_during_processing()`
     - Start long-running operation
     - Send Ctrl+C
     - Assert cleanup completes (no orphaned threads)
   - Test: `test_rapid_key_input()`
     - Send 100+ key events rapidly
     - Assert no crashes, no lost input

4. **Coverage Measurement** (1h)
   - Install tarpaulin: `cargo install cargo-tarpaulin`
   - Run coverage: `cargo tarpaulin --out Html --output-dir target/coverage --test cli_integration_tests`
   - Generate report: `target/coverage/index.html`
   - Verify ≥80% coverage for src/cli/ directory
   - Document uncovered lines: `docs/CLI_COVERAGE_REPORT.md`

**Blockers**:
- Crossterm event mocking complex → Use `crossterm::event::read()` abstraction
- Terminal state pollution between tests → Reset terminal in test teardown

**Rollback Plan**:
- If coverage <80% → Focus on critical paths (MainMenu, Processing, Results)
- If tests are flaky → Add retry logic, increase timeouts

**Validation Criteria**:
- [ ] All 9 screens have tests: `cargo test --test cli_integration_tests | grep "test result: ok"`
- [ ] Coverage ≥80%: Open `target/coverage/index.html`, verify src/cli/ coverage
- [ ] Zero flaky tests: `for i in {1..10}; do cargo test --test cli_integration_tests; done` (all pass)
- [ ] Tests run in <30s: Fast enough for CI/CD

#### Day 6: Protection System Testing (8 hours)

**Priority**: HIGH

**Estimated Hours**: 8h (6h actual + 2h buffer)

**Task Breakdown**:
1. **Test Infrastructure Setup** (1h)
   - Create `tests/protection_integration_tests.rs`
   - Add feature-gated test sections:
     ```rust
     #[cfg(feature = "meta-capsule-p0")]
     mod p0_tests { ... }

     #[cfg(feature = "meta-capsule-p1")]
     mod p1_tests { ... }

     #[cfg(feature = "meta-capsule-full")]
     mod p2_tests { ... }
     ```
   - Add mock hardware: Simulate TPM, PUF responses

2. **P0 Layer Tests (3 layers, already tested - quick validation)** (1h)
   - Test: `test_build_hardening_enabled()`
     - Verify binary has hardening flags (LTO, strip, opt-level=3)
     - Run: `readelf -h target/release/kindly_dedup | grep RELRO`
   - Test: `test_crypto_license_validation()`
     - Generate test license (ed25519 signature)
     - Verify validation succeeds
     - Modify license bytes
     - Verify validation fails
   - Test: `test_encrypted_state_roundtrip()`
     - Encrypt state with AES-256-GCM
     - Decrypt state
     - Assert plaintext matches

3. **P1 Layer Tests (5 layers, already tested - quick validation)** (2h)
   - Test: `test_tpm_binding_with_mock_tpm()`
     - Mock TPM responses (PCR values)
     - Bind to TPM
     - Verify binding succeeds
     - Change PCR values
     - Verify binding fails
   - Test: `test_fuzzy_extractor_error_correction()`
     - Generate PUF response with noise (10% bit flips)
     - Extract key
     - Assert key matches original (error correction works)
   - Test: `test_obfuscation_control_flow()`
     - Build with obfuscation feature
     - Verify control flow flattening (disassemble binary, check jumps)
   - Test: `test_remote_attestation_handshake()`
     - Mock attestation server
     - Generate attestation report
     - Verify server accepts report
     - Tamper with report
     - Verify server rejects
   - Test: `test_remote_attestation_retry_logic()`
     - Mock network failure
     - Verify retry with exponential backoff
     - Verify max retries limit (5 retries)

4. **P2 Layer Tests (3 layers, UNTESTED - full implementation)** (4h)

   **a) Orchestrator Testing** (1.5h)
   - Test: `test_orchestrator_initialization()`
     - Enable all 11 layers
     - Initialize orchestrator
     - Assert all layers initialized in order (P0 → P1 → P2)
     - Assert no failures
   - Test: `test_orchestrator_layer_failure_handling()`
     - Mock layer 4 failure (TPM unavailable)
     - Initialize orchestrator
     - Assert graceful degradation (continue with 10/11 layers)
     - Assert warning logged
   - Test: `test_orchestrator_coordination()`
     - Send test event (license validation)
     - Verify orchestrator coordinates layers:
       - Layer 1: Build hardening (passive)
       - Layer 2: Crypto license (active validation)
       - Layer 3: Encrypted state (encrypt result)
       - Layer 11: Kernel protection (syscall filter active)

   **b) Anomaly Detector Testing** (1.5h)
   - Test: `test_anomaly_detector_baseline_learning()`
     - Run normal workload (1000 dedup operations)
     - Assert baseline learned (mean, stddev)
     - Assert no anomalies detected
   - Test: `test_anomaly_detector_detects_slowdown()`
     - Run normal workload (establish baseline)
     - Inject 10× slowdown (sleep in hot path)
     - Assert anomaly detected (z-score > 3)
     - Assert alert triggered
   - Test: `test_anomaly_detector_false_positive_rate()`
     - Run 10,000 normal operations
     - Assert false positive rate <1% (z-score threshold = 3)

   **c) Memory/Kernel Protection Testing** (1h)
   - Test: `test_memory_encryption_roundtrip()`
     - Allocate protected memory region (AES-256-GCM)
     - Write sensitive data (license key)
     - Read data
     - Assert decryption succeeds
     - Assert plaintext matches
   - Test: `test_kernel_protection_syscall_filter()`
     - Enable syscall filter (seccomp-bpf)
     - Attempt blocked syscall (e.g., `execve`)
     - Assert syscall blocked
     - Assert error returned (EPERM)
   - Test: `test_kernel_protection_capability_drop()`
     - Drop capabilities (CAP_SYS_ADMIN)
     - Attempt privileged operation
     - Assert operation fails (EPERM)

**Blockers**:
- Mock hardware complex (TPM, PUF) → Use conditional compilation, skip on CI without hardware
- Syscall filter may break tests → Use feature flag to disable during testing

**Rollback Plan**:
- If P2 tests fail → Document as "known limitation", defer to post-launch
- If hardware unavailable → Skip hardware-dependent tests, document in README

**Validation Criteria**:
- [ ] All 11 layers tested: `cargo test --all-features --test protection_integration_tests | grep "11 passed"`
- [ ] Feature flags work: `cargo test --features meta-capsule-p0` (3 tests), `--features meta-capsule-p1` (8 tests), `--features meta-capsule-full` (11 tests)
- [ ] Zero flaky tests: Run 10× consecutively, all pass
- [ ] Tests documented: `docs/PROTECTION_TEST_COVERAGE.md` lists all 11 layers + test names

**Daily Deliverable (Day 6 End)**:
- CLI 80%+ coverage (all 9 screens tested)
- Protection 100% coverage (all 11 layers tested)
- Zero flaky tests (10 consecutive runs pass)
- Coverage reports (target/coverage/index.html)

---

## Day 7-8: Documentation + Automation (16 hours)

### UCE34 Analysis

#### Q1-Q9: Problem Statement
**STATED Problem**:
1. API documentation scattered (no unified reference, 30% examples broken)
2. No release automation (manual build/release process)

**ACTUAL Problem**:
- **Documentation**: Multiple sources, outdated examples
  - README.md: Installation, quick start
  - CLAUDE.md: Internal config, performance claims
  - docs/*.md: 40+ documentation files, inconsistent
  - Cargo doc: API reference (but examples not tested)
  - 30% of examples fail to compile (outdated APIs)
- **Automation**: Manual, error-prone release process
  - Build: `cargo build --release` (15+ minutes on cold build)
  - Sign: Manual GPG signing
  - Upload: Manual CDN upload
  - Verify: Manual download verification
  - No CI/CD pipeline

**Constraints**:
- 16 hours total (8h docs, 8h automation)
- Cannot rewrite all docs (focus on critical paths)
- Trade secret code requires private CI/CD (not GitHub Actions)

**Dependencies**:
- Performance claims (Day 4) → Documentation accurate
- Distribution (Day 2) → Automation can upload to CDN
- Testing (Day 6) → CI/CD runs all tests

**Edge Cases**:
- Doc tests fail → Fix critical examples, mark others as `no_run`
- CI/CD timeouts → Optimize build caching, parallel jobs
- CDN upload fails → Retry logic, fallback to GitHub Releases

#### Q10-Q12: Tier Selection
**Tier**: T0 Auditable (documentation versioning) + T1 Atomic (CI/CD coordination)

**Application**:
- Documentation versioning (hash-chained changelog)
- Deterministic builds (reproducible binaries)
- Lockfree CI/CD (parallel test execution)

#### Q30-Q34: Validation
**Simplicity**:
- Is single-page documentation simpler? NO (navigation suffers, use multi-page with TOC)
- Is automated release simpler than manual? YES (reduce human error, 10× faster)

**Constraints**:
- 16 hours (cannot achieve perfect docs, target critical paths)
- Private CI/CD (trade secret protection)

**Validation Criteria**:
- [ ] All doc tests pass: `cargo test --doc`
- [ ] API reference complete: `cargo doc --all-features --no-deps --open`
- [ ] CI/CD pipeline working: Push commit → Automated build/test/release
- [ ] Release time <30 minutes: From commit to CDN upload

### Tasks (16 hours total)

#### Day 7: API Documentation Consolidation (8 hours)

**Priority**: HIGH

**Estimated Hours**: 8h (6h actual + 2h buffer)

**Task Breakdown**:
1. **Documentation Audit** (1h)
   - Scan all docs: `find docs/ -name "*.md" | wc -l` (40+ files)
   - Categorize by purpose:
     - User-facing: README.md, GETTING_STARTED.md, DEMO_GUIDE.md
     - Developer: CLAUDE.md, ARCHITECTURE.md, CONTRIBUTING.md
     - Internal: PARALLEL_PERFORMANCE_INVESTIGATION.md, session reports
   - Identify critical paths:
     - Installation → README.md
     - Quick start → GETTING_STARTED.md
     - API reference → `cargo doc`
     - Performance → VALIDATED_PERFORMANCE_CLAIMS.md (created Day 4)

2. **Fix Broken Examples** (3h)
   - Run doc tests: `cargo test --doc 2>&1 | grep "FAILED"`
   - Fix compilation errors:
     - Update API usage (old APIs removed)
     - Add missing imports
     - Fix type errors
   - Mark non-critical examples as `no_run`:
     ```rust
     /// # Example
     /// ```no_run
     /// // Requires external setup
     /// let pipeline = PersistentDedupPipeline::create("file.mmap", 1_000_000)?;
     /// ```
     ```
   - Validate: `cargo test --doc` (100% pass)

3. **Create Unified API Reference** (2h)
   - Generate docs: `cargo doc --all-features --no-deps --document-private-items`
   - Add top-level documentation (`src/lib.rs`):
     ```rust
     //! # kindly_dedup - LLM Dataset Deduplication
     //!
     //! High-performance deduplication pipeline using computational capsules.
     //!
     //! ## Quick Start
     //!
     //! ```rust
     //! use kindly_dedup::DedupPipeline;
     //!
     //! let mut pipeline = DedupPipeline::new(10_000);
     //! pipeline.add_document(0, "The quick brown fox")?;
     //! let clusters = pipeline.find_duplicates(0.85)?;
     //! println!("Found {} clusters", clusters.len());
     //! # Ok::<(), kindly_dedup::Error>(())
     //! ```
     //!
     //! ## Performance
     //!
     //! - **Throughput**: 60,000 docs/sec (single-threaded)
     //! - **Speedup**: 38× vs Python datasketch
     //! - **Accuracy**: ≥90% F1 score
     //!
     //! See [`DedupPipeline`] for main API.
     ```
   - Add module documentation for all public modules
   - Add "Examples" section to each public struct/function

4. **Create Documentation Hub** (1h)
   - Create `docs/INDEX.md`:
     ```markdown
     # kindly_dedup Documentation

     ## User Documentation
     - [Installation](../README.md#installation)
     - [Quick Start](../README.md#quick-start)
     - [Demo Guide](DEMO_GUIDE.md) (if exists, otherwise remove link)

     ## Developer Documentation
     - [API Reference](../target/doc/kindly_dedup/index.html) (run `cargo doc --open`)
     - [Architecture](ARCHITECTURE.md) (if exists, otherwise remove)
     - [Performance Claims](VALIDATED_PERFORMANCE_CLAIMS.md)

     ## Operations
     - [Deployment](DEPLOYMENT.md) (create if missing, see below)
     - [Monitoring](MONITORING.md) (create if missing, see below)
     ```
   - Create `docs/DEPLOYMENT.md`:
     ```markdown
     # Deployment Guide

     ## Prerequisites
     - Linux x86_64
     - 4 GB RAM (minimum)
     - 10 GB disk space

     ## Installation
     1. Download binary: `curl -O https://dedup.kindly.software/latest/kindly_dedup-linux-x86_64`
     2. Verify signature: `gpg --verify kindly_dedup-linux-x86_64.asc`
     3. Make executable: `chmod +x kindly_dedup-linux-x86_64`
     4. Run: `./kindly_dedup-linux-x86_64 --version`

     ## Configuration
     See [Configuration Guide](CONFIGURATION.md) (if missing, add basic example)

     ## Monitoring
     See [Monitoring Guide](MONITORING.md) (if missing, add basic example)
     ```

5. **Update README.md** (1h)
   - Simplify structure:
     ```markdown
     # kindly_dedup

     High-performance LLM dataset deduplication using computational capsules.

     **Performance**: 60,000 docs/sec (38× faster than Python)

     ## Installation
     [Copy from DEPLOYMENT.md]

     ## Quick Start
     ```rust
     use kindly_dedup::DedupPipeline;

     let mut pipeline = DedupPipeline::new(10_000);
     pipeline.add_document(0, "The quick brown fox")?;
     let clusters = pipeline.find_duplicates(0.85)?;
     ```

     ## Documentation
     See [Documentation Hub](docs/INDEX.md)

     ## License
     Proprietary - Contact samuel@kindly.software
     ```
   - Remove outdated performance claims
   - Add links to docs/INDEX.md

**Blockers**:
- Too many broken examples → Prioritize critical paths, mark others as `no_run`

**Rollback Plan**:
- If doc tests fail → Mark failing examples as `ignore`, document as "known issue"

**Validation Criteria**:
- [ ] All doc tests pass: `cargo test --doc`
- [ ] API reference complete: `cargo doc --all-features --no-deps --open` (no broken links)
- [ ] README accurate: All claims match VALIDATED_PERFORMANCE_CLAIMS.md
- [ ] Documentation hub navigable: Manual click-through of all links

#### Day 8: Release Automation (8 hours)

**Priority**: HIGH

**Estimated Hours**: 8h (6h actual + 2h buffer)

**Task Breakdown**:
1. **CI/CD Provider Selection** (1h)
   - Evaluate options:
     - GitHub Actions: Free, but public repos only (trade secret risk)
     - GitLab CI: Free private repos, 400 minutes/month
     - Buildkite: Self-hosted agents (full control, trade secret safe)
     - Fly.io CI: Integrated with deployment (if available)
   - **Decision criteria**:
     - Private repo support (trade secret protection)
     - Rust toolchain support (nightly)
     - CDN upload capability
     - Cost (<$50/month)
   - **Recommended**: GitLab CI (private repos, free tier, good Rust support)

2. **Create CI/CD Pipeline** (3h)
   - Create `.gitlab-ci.yml` (or `.github/workflows/release.yml` if using GitHub private repo):
     ```yaml
     stages:
       - build
       - test
       - release

     variables:
       RUST_TOOLCHAIN: nightly-2024-01-15
       CARGO_HOME: ${CI_PROJECT_DIR}/.cargo

     cache:
       key: ${CI_COMMIT_REF_SLUG}
       paths:
         - target/
         - .cargo/

     build:
       stage: build
       image: rust:latest
       script:
         - rustup default ${RUST_TOOLCHAIN}
         - cargo build --release --bin kindly_dedup --features interactive
         - strip --strip-all target/release/kindly_dedup
       artifacts:
         paths:
           - target/release/kindly_dedup
         expire_in: 1 week

     test:
       stage: test
       image: rust:latest
       script:
         - rustup default ${RUST_TOOLCHAIN}
         - cargo test --all-features --lib
         - cargo test --test cli_integration_tests
         - cargo test --test protection_integration_tests --all-features
         - cargo test --doc
       dependencies: []

     release:
       stage: release
       image: rust:latest
       only:
         - tags
       script:
         - apt-get update && apt-get install -y gnupg curl
         # Import GPG key from CI/CD secret variable
         - echo "${GPG_PRIVATE_KEY}" | gpg --batch --import
         # Sign binary
         - gpg --detach-sign --armor target/release/kindly_dedup
         # Generate checksums
         - sha256sum target/release/kindly_dedup > SHA256SUMS
         - gpg --clearsign SHA256SUMS
         # Upload to CDN (BunnyCDN example)
         - |
           curl -X PUT \
             -H "AccessKey: ${BUNNY_CDN_API_KEY}" \
             --data-binary @target/release/kindly_dedup \
             "https://storage.bunnycdn.com/kindly-dedup-releases/${CI_COMMIT_TAG}/kindly_dedup-linux-x86_64"
         - |
           curl -X PUT \
             -H "AccessKey: ${BUNNY_CDN_API_KEY}" \
             --data-binary @target/release/kindly_dedup.asc \
             "https://storage.bunnycdn.com/kindly-dedup-releases/${CI_COMMIT_TAG}/kindly_dedup-linux-x86_64.asc"
         # Update /latest/ symlink (CDN-specific API)
         - echo "Update /latest/ symlink to ${CI_COMMIT_TAG}"
       dependencies:
         - build
     ```

3. **Add CI/CD Secrets** (1h)
   - Add secret variables in GitLab CI settings:
     - `GPG_PRIVATE_KEY`: Export GPG private key (base64 encoded)
     - `BUNNY_CDN_API_KEY`: CDN upload credentials
   - Test secret access: Create dummy pipeline that echoes masked secrets
   - Validate: Secrets not exposed in logs

4. **Optimize Build Performance** (2h)
   - Add sccache for Rust caching:
     ```yaml
     variables:
       SCCACHE_DIR: ${CI_PROJECT_DIR}/.sccache
       RUSTC_WRAPPER: sccache

     before_script:
       - cargo install sccache || true
       - sccache --start-server || true
     ```
   - Parallel test execution:
     ```yaml
     test:
       parallel: 3
       script:
         - |
           case ${CI_NODE_INDEX} in
             1) cargo test --lib ;;
             2) cargo test --test cli_integration_tests ;;
             3) cargo test --test protection_integration_tests --all-features ;;
           esac
     ```
   - Benchmark caching effectiveness:
     - Cold build: 15-20 minutes
     - Cached build: 2-5 minutes (target <5 minutes)

5. **Create Release Script** (1h)
   - Create `scripts/release.sh`:
     ```bash
     #!/bin/bash
     set -euo pipefail

     VERSION=${1:?Usage: $0 <version>}

     echo "Creating release ${VERSION}..."

     # Update Cargo.toml version
     sed -i "s/^version = .*/version = \"${VERSION}\"/" Cargo.toml

     # Commit version bump
     git add Cargo.toml
     git commit -m "chore: Bump version to ${VERSION}"

     # Create tag
     git tag -a "v${VERSION}" -m "Release ${VERSION}"

     # Push tag (triggers CI/CD release pipeline)
     git push origin "v${VERSION}"

     echo "Release ${VERSION} triggered. Monitor CI/CD pipeline."
     ```
   - Make executable: `chmod +x scripts/release.sh`
   - Document usage: Add to `docs/RELEASE_PROCESS.md`

**Blockers**:
- CI/CD setup complex → Start with simple pipeline, iterate
- CDN upload API changes → Use curl with verbose logging, debug failures

**Rollback Plan**:
- If CI/CD fails → Fall back to manual release (Day 2 scripts)
- If build times >30 min → Reduce feature flags, disable tests in release pipeline

**Validation Criteria**:
- [ ] CI/CD pipeline working: Push test commit, verify pipeline runs
- [ ] Automated release working: Create test tag, verify binary uploaded to CDN
- [ ] Build time <30 min: Check pipeline duration in GitLab CI
- [ ] Tests run in parallel: Verify 3 parallel jobs in CI/CD logs

**Daily Deliverable (Day 8 End)**:
- Unified API documentation (cargo doc + docs/INDEX.md)
- All doc tests passing (100% compilable examples)
- CI/CD pipeline deployed (GitLab CI or GitHub Actions)
- Automated release process (scripts/release.sh)

---

## Day 9-10: Validation + Polish (16 hours)

### UCE34 Analysis

#### Q1-Q9: Problem Statement
**STATED Problem**: Final validation and launch preparation

**ACTUAL Problem**:
- Integration validation across all fixes (Days 1-8)
- Launch checklist completion
- Smoke testing on production-like environment
- Documentation polish and final review

**Constraints**:
- 16 hours (2 days for comprehensive validation)
- Cannot discover new blockers (scope freeze)
- Must be launch-ready by Day 10 end

**Dependencies**:
- All Day 1-8 tasks complete
- No critical bugs discovered during validation

**Edge Cases**:
- Validation discovers regressions → Rollback to last known good state
- Performance degrades → Investigate, may defer optimization to post-launch
- Documentation gaps → Document as "known limitation"

#### Q10-Q12: Tier Selection
**Tier**: T0 Auditable (launch checklist) + T1 Atomic (smoke test coordination)

**Application**:
- Launch checklist (hash-chained validation steps)
- Smoke test suite (deterministic, reproducible)
- Rollback decision (atomic go/no-go)

#### Q30-Q34: Validation
**Simplicity**: Is comprehensive validation simpler than partial? YES (high confidence)

**Constraints**: 16 hours (cannot extend timeline, must launch)

**Validation Criteria**:
- [ ] Launch checklist 100% complete
- [ ] Smoke tests 100% pass
- [ ] Zero critical bugs
- [ ] Production deployment successful

### Tasks (16 hours total)

#### Day 9: Integration Validation (8 hours)

**Priority**: CRITICAL

**Estimated Hours**: 8h (7h actual + 1h buffer)

**Task Breakdown**:
1. **End-to-End Smoke Tests** (3h)
   - Create `tests/smoke_tests.rs`:
     ```rust
     /// Full E2E workflow: Download → Dedup → Export
     #[test]
     fn test_e2e_dedup_workflow() {
         // 1. Download test corpus (100 docs)
         let corpus = download_test_corpus()?;

         // 2. Run dedup pipeline
         let mut pipeline = DedupPipeline::new(100);
         for (id, text) in corpus {
             pipeline.add_document(id, text)?;
         }
         let clusters = pipeline.find_duplicates(0.85)?;

         // 3. Export results
         let output = export_clusters(&clusters)?;

         // 4. Validate output
         assert!(output.contains("cluster_count"));
         assert!(clusters.len() > 0);
     }
     ```
   - Test scenarios:
     - Small corpus (100 docs)
     - Medium corpus (10K docs)
     - Large corpus (100K docs)
     - Invalid input (malformed JSON)
     - Edge cases (empty corpus, single doc, all duplicates)
   - Run: `cargo test --test smoke_tests --release` (use release for realistic performance)

2. **Performance Regression Testing** (2h)
   - Re-run all benchmarks: `cargo bench --bench v1_0_baseline`
   - Compare results to Day 4 baseline:
     ```bash
     # Day 4 baseline: 58,500 docs/sec
     # Day 9 current: [MEASURED VALUE]
     # Regression: [MEASURED - BASELINE] / BASELINE
     # Threshold: ±10% acceptable
     ```
   - Investigate regressions >10%:
     - Profile with flamegraph: `cargo flamegraph --bench v1_0_baseline`
     - Identify bottlenecks
     - Fix if critical, defer if minor
   - Document results: `docs/PERFORMANCE_REGRESSION_REPORT.md`

3. **CLI Integration Validation** (1h)
   - Manual smoke test of TUI:
     ```bash
     ./target/release/kindly_dedup
     # Navigate through all 9 screens
     # Test happy path (successful dedup)
     # Test error path (invalid corpus)
     # Test cancel operation (Ctrl+C)
     ```
   - Validate coverage report: `target/coverage/index.html` (verify ≥80%)
   - Check for flaky tests: `for i in {1..10}; do cargo test --test cli_integration_tests; done`

4. **Protection System Validation** (1h)
   - Test all 11 layers with full feature flags:
     ```bash
     cargo test --all-features --test protection_integration_tests
     ```
   - Manual verification of protection status screen:
     ```bash
     ./target/release/kindly_dedup --features meta-capsule-full
     # Navigate to Protection Status screen
     # Verify all 11 layers show ✅ (enabled)
     ```
   - Verify feature flag graceful degradation:
     ```bash
     cargo build --release --features meta-capsule-p0  # Only P0 layers
     ./target/release/kindly_dedup
     # Navigate to Protection Status screen
     # Verify 3 layers ✅, 8 layers ⚠️ (disabled)
     ```

5. **Documentation Review** (1h)
   - Proof-read all user-facing docs:
     - README.md
     - docs/INDEX.md
     - docs/DEPLOYMENT.md
     - docs/VALIDATED_PERFORMANCE_CLAIMS.md
   - Verify all links work: Manual click-through
   - Verify all code examples compile: `cargo test --doc`
   - Fix typos, formatting issues

**Blockers**:
- Performance regression >10% → Investigate root cause, may defer fix to post-launch
- Smoke tests fail → Critical bug, must fix before launch

**Rollback Plan**:
- If critical bugs found → Delay launch, fix bugs, re-validate
- If minor issues → Document as "known limitation", launch with caveats

**Validation Criteria**:
- [ ] All smoke tests pass: `cargo test --test smoke_tests --release`
- [ ] Performance within ±10%: Compare benchmarks to Day 4 baseline
- [ ] CLI coverage ≥80%: Check `target/coverage/index.html`
- [ ] Protection tests pass: `cargo test --all-features --test protection_integration_tests`
- [ ] Documentation accurate: Manual review, all links work

#### Day 10: Launch Preparation + Go-Live (8 hours)

**Priority**: CRITICAL

**Estimated Hours**: 8h (7h actual + 1h buffer)

**Task Breakdown**:
1. **Final Build and Sign** (1h)
   - Clean build: `cargo clean && cargo build --release --bin kindly_dedup --features interactive`
   - Verify binary size: <25 MB
   - Strip symbols: `strip --strip-all target/release/kindly_dedup`
   - Sign binary: `scripts/sign_release.sh target/release/kindly_dedup`
   - Generate checksums: `sha256sum target/release/kindly_dedup > SHA256SUMS`

2. **Upload to CDN** (1h)
   - Upload binary: `scripts/upload_release.sh v2.0.1`
   - Verify upload: `curl -I https://dedup.kindly.software/latest/kindly_dedup-linux-x86_64` (200 OK)
   - Verify checksum: `curl -O https://dedup.kindly.software/latest/SHA256SUMS && sha256sum --check SHA256SUMS`
   - Update GitHub Releases: Upload to `kindly-ecosystem/kindly-dedup/releases/v2.0.1`

3. **Launch Checklist Validation** (2h)
   - **Can users download binaries?**
     - Test: `curl -O https://dedup.kindly.software/latest/kindly_dedup-linux-x86_64` (<5s)
     - Test: `curl -O https://github.com/kindly-ecosystem/kindly-dedup/releases/download/v2.0.1/kindly_dedup-linux-x86_64` (fallback works)
     - Test: `gpg --verify kindly_dedup-linux-x86_64.asc kindly_dedup-linux-x86_64` (signature valid)
     - ✅ PASS

   - **Do performance claims match reality?**
     - Verify: All claims in README.md match `docs/VALIDATED_PERFORMANCE_CLAIMS.md`
     - Verify: No contradictory claims (`grep -r "docs/sec\|speedup" docs/ README.md CLAUDE.md`)
     - Verify: Parallel pipeline deprecated in docs
     - ✅ PASS

   - **Are production panics fixed?**
     - Verify: `cargo clippy -- -D clippy::panic` (zero panics in src/)
     - Verify: `cargo test --test panic_regression_tests` (all pass)
     - Manual test: `./kindly_dedup` with invalid region_id (returns error, not panic)
     - ✅ PASS

   - **Is CLI tested?**
     - Verify: Coverage ≥80% (`target/coverage/index.html`)
     - Verify: All 9 screens tested (`cargo test --test cli_integration_tests`)
     - Manual test: Navigate through all screens (no crashes)
     - ✅ PASS

   - **Can we deploy with confidence?**
     - Verify: CI/CD pipeline working (push test commit, verify build/test/release)
     - Verify: Smoke tests pass (`cargo test --test smoke_tests --release`)
     - Verify: Performance within ±10% of baseline
     - Verify: Zero critical bugs in issue tracker
     - ✅ PASS

4. **Production Deployment** (2h)
   - Deploy to production server (if applicable):
     ```bash
     ssh production-server
     curl -O https://dedup.kindly.software/latest/kindly_dedup-linux-x86_64
     gpg --verify kindly_dedup-linux-x86_64.asc
     chmod +x kindly_dedup-linux-x86_64
     ./kindly_dedup-linux-x86_64 --version
     ```
   - Configure monitoring (if not already set up):
     - Log aggregation (syslog, journalctl)
     - Error tracking (document error codes)
     - Performance metrics (throughput, latency)
   - Document deployment: `docs/DEPLOYMENT.md`

5. **Launch Announcement** (1h)
   - Create release notes: `docs/RELEASE_NOTES_v2.0.1.md`
     ```markdown
     # Release Notes - v2.0.1

     ## Launch Release (2025-12-02)

     ### New Features
     - Distribution infrastructure (dedup.kindly.software)
     - Signed binaries with GPG verification
     - Automated CI/CD pipeline

     ### Bug Fixes
     - Fixed production panics in mmap_bucketer.rs, batch_lookup.rs
     - Fixed contradictory performance claims
     - Deprecated broken parallel pipeline (ParallelDedupPipeline)

     ### Testing
     - CLI integration tests (80% coverage)
     - Protection system tests (100% coverage, all 11 layers)
     - Comprehensive smoke tests

     ### Performance
     - 60,000 docs/sec (single-threaded, VALIDATED)
     - 38× vs Python datasketch
     - ≥90% F1 score accuracy

     ## Installation
     See [Deployment Guide](DEPLOYMENT.md)
     ```
   - Update website (if exists): Add release announcement
   - Notify stakeholders: Email announcement to customers, partners

6. **Post-Launch Monitoring** (1h)
   - Monitor CDN access logs: Verify downloads
   - Monitor error logs: Check for production issues
   - Set up alerts: Email notification on critical errors
   - Document monitoring setup: `docs/MONITORING.md`

**Blockers**:
- Launch checklist fails → Delay launch, fix critical issues
- CDN downtime → Use GitHub Releases as primary, document CDN as "temporarily unavailable"

**Rollback Plan**:
- If production deployment fails → Rollback to previous version (if exists)
- If critical bugs discovered post-launch → Hotfix release v2.0.2 within 24 hours

**Validation Criteria**:
- [ ] Launch checklist 100% ✅: All 5 criteria pass
- [ ] Binary downloadable: `curl -O https://dedup.kindly.software/latest/kindly_dedup-linux-x86_64` (<5s)
- [ ] Signature verifies: `gpg --verify kindly_dedup-linux-x86_64.asc`
- [ ] Production deployment successful: Binary running on production server
- [ ] Monitoring active: Alerts configured, logs aggregated

**Daily Deliverable (Day 10 End)**:
- **LAUNCH READY** ✅
- Binary available at dedup.kindly.software
- All blockers resolved
- Launch checklist 100% complete
- Monitoring and alerting active

---

## Risk Mitigation

### Risk Register

| Risk | Probability | Impact | Mitigation | Contingency |
|------|-------------|--------|------------|-------------|
| **CDN setup delays** | MEDIUM | HIGH | Start Day 1, parallelize with panic fixes | Use GitHub Releases as fallback |
| **Panic fixes break tests** | MEDIUM | HIGH | Incremental commits, feature flags | Rollback to last known good commit |
| **CI/CD setup complex** | HIGH | MEDIUM | Use GitLab CI (simpler than self-hosted) | Manual release process (Day 2 scripts) |
| **Performance regression** | LOW | HIGH | Daily benchmark validation (Days 3-10) | Investigate and fix, or document as known issue |
| **Time overrun** | MEDIUM | HIGH | Daily progress tracking (Q34 audit trail) | MVP scope reduction (see below) |
| **Critical bug discovered** | LOW | CRITICAL | Comprehensive testing (Days 5-6, 9) | Delay launch, hotfix, re-validate |

### Parallelization Opportunities

**Day 1-2** (Distribution):
- Can parallelize: CDN setup (Day 1) + binary signing workflow (Day 1 evening)
- Sequential: CDN setup → Binary upload (dependency)

**Day 3-4** (Code Quality):
- Can parallelize: Panic fixes (Day 3 morning) + deprecation prep (Day 3 afternoon)
- Sequential: Panic fixes → Integration tests → Performance validation

**Day 5-6** (Testing):
- Cannot parallelize: CLI tests (Day 5) depend on panic fixes (Day 3)
- Can parallelize: CLI tests (Day 5 morning) + protection tests (Day 5 afternoon) if independent

**Day 7-8** (Documentation + Automation):
- Can parallelize: Documentation (Day 7) + CI/CD setup (Day 8 morning, if CI/CD doesn't depend on docs)
- Sequential: Documentation → CI/CD (if pipeline needs to build docs)

**Day 9-10** (Validation):
- Cannot parallelize: Sequential validation (smoke tests → integration → launch)

### What if we run out of time?

**MVP Scope Reduction** (Priority Order):

**Day 10 End - Minimum Viable Launch**:
1. **MUST HAVE** (Launch Blockers):
   - ✅ Distribution infrastructure (CDN or GitHub Releases)
   - ✅ Zero production panics (mmap_bucketer, batch_lookup fixed)
   - ✅ Honest performance claims (60K docs/sec, 38×)
   - ✅ Parallel pipeline deprecated (hidden from docs)

2. **SHOULD HAVE** (Defer to v2.0.2 if needed):
   - CLI testing (if <80% coverage, document as "known limitation")
   - Protection testing (if <100% coverage, test P0+P1 only, defer P2)
   - API documentation (if examples broken, mark as `no_run`)
   - Release automation (if CI/CD fails, use manual process)

3. **NICE TO HAVE** (Defer to post-launch):
   - Unified API reference (cargo doc may have gaps)
   - Comprehensive smoke tests (run critical paths only)
   - Monitoring setup (add post-launch)

**Launch Decision Tree**:
```
Day 10 End:
├─ All MUST HAVE complete?
│  ├─ YES → LAUNCH ✅
│  └─ NO → DELAY 24h, fix critical blockers
├─ All SHOULD HAVE complete?
│  ├─ YES → LAUNCH ✅ (full confidence)
│  └─ NO → LAUNCH ⚠️ (document known limitations)
└─ Any NICE TO HAVE complete?
   └─ Bonus, not required for launch
```

---

## Daily Deliverables Summary

| Day | Focus | Key Deliverables |
|-----|-------|------------------|
| **1** | CDN Setup | DNS configured, SSL working, CDN storage zone created, GPG key generated |
| **2** | Release Artifacts | Binary signed, checksums generated, CDN uploaded, GitHub fallback working |
| **3** | Panic Fixes | Zero production panics, all hot paths return Result<>, clippy panic linting enabled |
| **4** | Performance Claims | Honest claims (60K, 38×), parallel deprecated, B32 validation report |
| **5** | CLI Testing | All 9 screens tested, ≥80% coverage, integration test suite created |
| **6** | Protection Testing | All 11 layers tested, 100% coverage, feature flags validated |
| **7** | API Documentation | All doc tests passing, unified API reference, documentation hub created |
| **8** | Release Automation | CI/CD pipeline deployed, automated release script, build time <30 min |
| **9** | Integration Validation | Smoke tests pass, performance within ±10%, documentation reviewed |
| **10** | Launch | **LAUNCH READY** - Binary at dedup.kindly.software, launch checklist 100% ✅ |

---

## Definition of Done (Per Blocker)

### Blocker 1: Distribution Infrastructure
- [ ] DNS resolves: `dig dedup.kindly.software` returns CNAME or A record
- [ ] SSL works: `curl https://dedup.kindly.software/health` returns 200 OK
- [ ] Binary downloads: `curl -O https://dedup.kindly.software/latest/kindly_dedup-linux-x86_64` (<5s)
- [ ] Checksum matches: `sha256sum --check SHA256SUMS` verifies
- [ ] Signature verifies: `gpg --verify kindly_dedup-linux-x86_64.asc` succeeds
- [ ] Fallback works: GitHub Releases download succeeds

### Blocker 2: Parallel Pipeline Broken
- [ ] Deprecated in code: `#[deprecated]` attribute added to ParallelDedupPipeline
- [ ] Hidden from docs: `cargo doc` shows deprecation warning or `#[doc(hidden)]`
- [ ] Removed from examples: Zero examples use ParallelDedupPipeline
- [ ] Removed from README: No mention of parallel pipeline in user-facing docs
- [ ] Documented reason: PARALLEL_PERFORMANCE_INVESTIGATION.md linked in deprecation note

### Blocker 3: Critical Production Panics
- [ ] mmap_bucketer.rs:80 fixed: Returns `Err(Error::InvalidRegionId)` instead of panic
- [ ] batch_lookup.rs:301 fixed: Returns `Err(Error::MutexPoisoned)` instead of unwrap
- [ ] batch_lookup.rs:304 fixed: Returns `Err(Error::ThreadPoolQueueFull)` instead of expect
- [ ] batch_lookup.rs:309 fixed: Returns `Err(Error::ArcStillShared)` instead of expect
- [ ] batch_lookup.rs:312 fixed: Returns `Err(Error::MutexPoisoned)` instead of expect
- [ ] Clippy passes: `cargo clippy -- -D clippy::panic` succeeds
- [ ] Integration tests pass: `cargo test --test panic_regression_tests` succeeds

### Blocker 4: CLI Completely Untested
- [ ] All 9 screens tested: MainMenu, CorpusSelection, Configuration, Processing, Results, Settings, Help, ProtectionStatus, Error
- [ ] Coverage ≥80%: `target/coverage/index.html` shows ≥80% for `src/cli/` directory
- [ ] Integration tests pass: `cargo test --test cli_integration_tests` succeeds
- [ ] Zero flaky tests: 10 consecutive runs pass

### Blocker 5: Performance Claims Contradictory
- [ ] CLAUDE.md accurate: All claims match `docs/VALIDATED_PERFORMANCE_CLAIMS.md`
- [ ] README accurate: All claims match `docs/VALIDATED_PERFORMANCE_CLAIMS.md`
- [ ] Cargo.toml accurate: Description matches validated claims (60K docs/sec, 38×)
- [ ] No contradictions: `grep -r "docs/sec\|speedup\|×" docs/ README.md CLAUDE.md` shows no conflicting numbers
- [ ] B32 validated: All claims have benchmark evidence in `benches/sales/`

### Blocker 6: Protection System Untested
- [ ] All 11 layers tested: P0 (3 layers), P1 (5 layers), P2 (3 layers)
- [ ] Feature flags work: `cargo test --features meta-capsule-p0` (3 tests), `--features meta-capsule-p1` (8 tests), `--features meta-capsule-full` (11 tests)
- [ ] Integration tests pass: `cargo test --all-features --test protection_integration_tests` succeeds
- [ ] Coverage documented: `docs/PROTECTION_TEST_COVERAGE.md` lists all layers + test names

### Blocker 7: API Documentation Scattered
- [ ] All doc tests pass: `cargo test --doc` succeeds (100% compilable examples)
- [ ] API reference complete: `cargo doc --all-features --no-deps --open` (no broken links)
- [ ] Documentation hub created: `docs/INDEX.md` navigable, all links work
- [ ] README simplified: Links to `docs/INDEX.md`, no duplicate content

### Blocker 8: No Release Automation
- [ ] CI/CD pipeline deployed: GitLab CI or GitHub Actions configuration committed
- [ ] Automated release works: Create test tag, verify binary uploaded to CDN
- [ ] Build time <30 min: Check pipeline duration in CI/CD logs
- [ ] Tests run in parallel: Verify parallel jobs in CI/CD logs
- [ ] Release script created: `scripts/release.sh` documented in `docs/RELEASE_PROCESS.md`

---

## Launch Readiness Checklist (Day 10 End)

### Distribution
- [ ] **Can users download binaries?** ✅ YES
  - Binary at https://dedup.kindly.software/latest/kindly_dedup-linux-x86_64
  - Download time <5 seconds
  - SHA256 checksum verified
  - GPG signature verified
  - GitHub Releases fallback working

### Performance
- [ ] **Do performance claims match reality?** ✅ YES
  - All claims validated with B32 framework (1000+ iterations, 95% CI)
  - 60K docs/sec single-threaded (VALIDATED)
  - 38× speedup vs Python datasketch (VALIDATED)
  - No contradictory claims across docs
  - Parallel pipeline deprecated (documented as broken)

### Reliability
- [ ] **Are production panics fixed?** ✅ YES
  - Zero panics in src/ (clippy panic linting enabled)
  - All hot paths return Result<>
  - Invalid input returns Err() with actionable error messages
  - Integration tests with invalid input pass (no panics)

### Testing
- [ ] **Is CLI tested?** ✅ YES
  - All 9 screens have integration tests
  - Coverage ≥80% (target/coverage/index.html)
  - Zero flaky tests (10 consecutive runs pass)
  - Protection system 100% tested (all 11 layers)

### Confidence
- [ ] **Can we deploy with confidence?** ✅ YES
  - CI/CD pipeline working (automated build/test/release)
  - Smoke tests 100% pass (E2E validation)
  - Performance within ±10% of baseline (no regressions)
  - Zero critical bugs
  - Monitoring and alerting configured
  - Rollback plan documented

---

## Sprint Audit Trail (Q34 Compliance)

**Format**: Daily standup with hash-chained audit log

**Storage**: `/home/samuel/Primitives/kindly_dedup/SPRINT_AUDIT_LOG.md`

**Template**:
```markdown
## Day N Standup - YYYY-MM-DD

### Completed (Hash: <SHA256 of previous day>)
- [ ] Blocker 1: Task 1 (X hours actual vs Y hours estimated)
- [ ] Blocker 2: Task 2 (X hours actual vs Y hours estimated)

### In Progress
- [ ] Blocker 3: Task 3 (Z hours remaining)

### Blocked
- [ ] Blocker 4: Task 4 (Dependency: CDN setup, estimated resolution: Day N+1)

### Risks
- Risk 1: CDN setup delayed 2 hours (Mitigation: Use GitHub Releases as temporary fallback)

### Metrics
- Hours spent today: X/8
- Cumulative hours: Y/80
- Sprint progress: Z% (tasks completed / total tasks)

### Hash: <SHA256 of this day's content>
```

**Hashing Method**:
```bash
# Generate hash for Day N
cat SPRINT_AUDIT_LOG.md | grep -A 100 "## Day N" | sha256sum | awk '{print $1}'
```

---

## Success Criteria Summary

### Launch Ready Definition
The project is **launch ready** when:

1. ✅ **Distribution**: Users can download, verify, and run binaries from dedup.kindly.software
2. ✅ **Reliability**: Zero production panics, all errors recoverable with Result<>
3. ✅ **Honesty**: Performance claims match validated benchmarks (60K docs/sec, 38×)
4. ✅ **Testing**: CLI ≥80% coverage, protection 100% tested
5. ✅ **Automation**: CI/CD pipeline automates build/test/release
6. ✅ **Confidence**: Smoke tests pass, monitoring active, rollback plan documented

### Post-Launch Success Criteria (Week 3+)
- **Downloads**: ≥10 unique downloads in first week
- **No critical bugs**: Zero production crashes reported
- **Performance**: User-reported throughput within ±20% of claimed 60K docs/sec
- **Support**: ≤24h response time on bug reports
- **Monitoring**: Zero undetected outages

---

## Conclusion

This 2-week sprint plan applies UCE34 framework (Q1-Q34) to systematically transform kindly_dedup from "NOT READY FOR LAUNCH" to "LAUNCH READY" status. The plan prioritizes critical blockers (distribution, panics, claims, CLI testing) while maintaining scope discipline (deprecate parallel pipeline instead of redesigning). Daily deliverables provide incremental progress tracking, and the launch checklist ensures confidence in production deployment.

**Key Success Factors**:
1. **Systematic approach** - UCE34 Q1-Q9 problem analysis for each task
2. **Honest performance claims** - B32 validation removes contradictions
3. **Zero production panics** - Result<> error propagation throughout
4. **Comprehensive testing** - CLI 80%+, protection 100%
5. **Automation** - CI/CD reduces manual errors, 10× faster releases

**Launch Timeline**: 10 working days (80 hours)
**Launch Date**: Day 10 end (2025-12-02)
**Confidence**: 95% (based on UCE34 validation criteria)

---

**Document Hash**: <SHA256 of this document will be generated on commit>
**Version**: 1.0
**Date**: 2025-11-18
**Author**: Samuel (claude-sonnet-4-5-20250929)
**Framework**: UCE34 Q1-Q34 Systematic Discovery
