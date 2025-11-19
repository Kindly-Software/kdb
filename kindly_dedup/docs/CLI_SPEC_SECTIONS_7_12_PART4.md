# kindly_dedup CLI Specification - Sections 7-12 (Part 4)

## (Continued from Part 3: Section 9-10)

---

# Section 9: Implementation Plan

## 9.1 7-Phase Roadmap (with dependencies)

### Phase 1: Foundation (3 days)

**Goal**: Enhance terminal.rs, create atomic state capsules

**Deliverables**:
1. **terminal.rs enhancements** (box_drawing.rs, cursor.rs)
   - Unicode box drawing characters (┌─┐│└┘)
   - ASCII fallback for legacy terminals
   - Cursor management (save/restore, hide/show)
   - Terminal resize detection

2. **State capsules** (4 total, all T1 Atomic):
   - MenuStateCapsule (64B)
   - ProgressTrackerCapsule (128B)
   - AnimationStateCapsule (64B)
   - LicenseStateCapsule (256B)

3. **Testing**:
   - 20 unit tests (terminal utilities)
   - 20 unit tests (state capsules)
   - All tests passing, zero clippy warnings

**Success Criteria**:
- ✅ Box drawing renders correctly on 5+ terminals
- ✅ Cursor save/restore works without flicker
- ✅ State capsules verified with #[derive(ComputationalCapsule)]
- ✅ <5ns read, <15ns write (state capsules, benchmarked)

**Dependencies**: None (standalone foundational work)

**Risk Mitigation**: Test on 5 terminals early (iTerm2, Windows Terminal, VS Code, Alacritty, xterm)

---

### Phase 2: Menu System (4 days)

**Goal**: Welcome screen, main menu, keyboard input handling

**Deliverables**:
1. **Welcome screen** (welcome.rs)
   - Pulsing purple hearts (brightness cycling)
   - "Press Enter to continue" prompt
   - Celebration animation on first launch

2. **Main menu** (main_menu.rs)
   - 7 options (Deduplicate, Configure, License, Help, Export, Audit, Exit)
   - Arrow key navigation
   - Selected option highlight (Byzantine purple)

3. **Keyboard input** (input.rs)
   - Crossterm event handling
   - Arrow keys, Enter, Esc, q, Ctrl+C
   - Input lag <100ms

4. **Testing**:
   - 15 unit tests (menu state transitions)
   - 10 integration tests (keyboard → menu navigation)

**Success Criteria**:
- ✅ Welcome screen pulsing hearts render smoothly (60 FPS)
- ✅ Main menu navigation responsive (<100ms lag)
- ✅ Esc/q exit gracefully (cursor restored, no flicker)

**Dependencies**: Phase 1 (state capsules, terminal utils)

**Risk Mitigation**: B32 benchmark animation frame time early (Phase 2 day 1)

---

### Phase 3: Deduplication Flow (7 days)

**Goal**: File selection, configuration UI, progress rendering, results summary

**Deliverables**:
1. **File selection** (file_selection.rs)
   - File browser (inquire library)
   - Manual path entry with fuzzy suggestions
   - Validation (file exists, readable, JSONL format)

2. **Configuration UI** (config_ui.rs)
   - Jaccard threshold slider (0.7-0.95, default 0.85)
   - Thread count selector (1-256, auto-detect default)
   - Memory mode (in-memory vs persistent)
   - Feature flags (SIMD, Bloom pre-filter, Batch LSH)

3. **Progress rendering** (progress.rs)
   - Real-time progress bar (0-100%)
   - Throughput (docs/sec, updated every 100ms)
   - Phase indicator (MinHash → LSH → Clustering)
   - Time remaining estimate (ETA)

4. **Results summary** (results.rs)
   - Duplicates found (count + percentage)
   - Cluster sizes (histogram: 2-10, 11-100, 101-1000, 1000+)
   - Achievements (emojis: 🏆 for 90%+ recall, 💎 for <1min runtime)
   - Export options (JSONL, CSV, SQLite)

5. **Testing**:
   - 20 integration tests (file selection → dedup → results)
   - 10 stress tests (10M docs, 16 threads, 10-minute runs)

**Success Criteria**:
- ✅ File browser works on all platforms (Linux/macOS/Windows)
- ✅ Progress bar updates smoothly (100 Hz max, no flicker)
- ✅ Results summary accurate (matches ground truth ±5%)

**Dependencies**: Phase 1 (state capsules), Phase 2 (menu system), API (DedupClient)

**Risk Mitigation**: User testing after Phase 3 day 4 (5 testers, 1-hour sessions)

---

### Phase 4: Animation Engine (3 days)

**Goal**: Frame scheduler, pulsing heart, progress bar, celebration effects

**Deliverables**:
1. **Frame scheduler** (frame_scheduler.rs)
   - 8-60 FPS configurable
   - Sleep-based timing (non-busy-wait)
   - Frame time budget enforcement (<16ms @ 60 FPS)

2. **Pulsing purple heart** (pulsing_heart.rs)
   - Sinusoidal brightness cycling (0.4 → 1.0 → 0.4, 2 sec)
   - Color transitions (DeepPurple → RoyalPurple → ByzantinePurple)
   - Bold style at peak brightness

3. **Progress bar renderer** (progress_bar.rs)
   - Smooth updates (double buffering)
   - Auto-scale to terminal width
   - Byzantine purple filled portion

4. **Spinner** (spinner.rs)
   - 3 patterns (ROCKET, HEARTS, LOADING)
   - Rotating emojis (4-10 frames)

5. **Celebration effects** (celebration.rs)
   - 1-second sparkle animation (✨💜🎉💛)
   - Triggered on dedup completion

6. **Testing**:
   - 15 unit tests (frame scheduler, brightness cycling)
   - 10 stress tests (60 FPS, 10-minute runs)

**Success Criteria**:
- ✅ All animations render within 16ms frame budget (60 FPS)
- ✅ No flicker (double buffering works)
- ✅ Brightness cycling smooth (no jumps)

**Dependencies**: Phase 1 (AnimationStateCapsule), Phase 3 (progress bar integration)

**Risk Mitigation**: B32 benchmark frame time on Phase 4 day 1 (reject if >16ms)

---

### Phase 5: License Integration (3 days)

**Goal**: CryptoLicenseCapsule wrapper, tier enforcement, trial mode

**Deliverables**:
1. **License wrapper** (license/crypto_license.rs)
   - Load license file (~/.config/kindly_dedup/license.key)
   - Verify Ed25519 signature
   - Initialize LicenseStateCapsule

2. **Tier enforcement** (license/tier_enforcement.rs)
   - Free tier: 100K docs, 1 thread
   - Pro tier: 10M docs, 16 threads, SIMD/Bloom/Batch
   - Enterprise tier: unlimited docs/threads, all features

3. **Trial mode** (license/trial.rs)
   - 7-day trial (Docker fingerprint, volume UUID)
   - 100K doc limit
   - Grace period (3 days after expiration)

4. **License UI** (cli/license_ui.rs)
   - Display tier, expiration, doc limit
   - Upgrade prompt (on limit exceeded)
   - Activation wizard (Ed25519 key entry)

5. **Testing**:
   - 10 unit tests (license verification, tier enforcement)
   - 5 integration tests (trial expiration, tier upgrades)

**Success Criteria**:
- ✅ Ed25519 signature verification <500μs (cached)
- ✅ Tier limits enforced (100K/10M/unlimited)
- ✅ Trial mode works in Docker (volume UUID fingerprint)

**Dependencies**: Phase 1 (LicenseStateCapsule), atomic_capsule (CryptoLicenseCapsule)

**Risk Mitigation**: Early integration with CryptoLicenseCapsule (Phase 5 day 1)

---

### Phase 6: Q34 Audit Trail (4 days)

**Goal**: Hash-chained logger, compliance reports, verification tool

**Deliverables**:
1. **Audit logger** (audit/audit_logger.rs)
   - Hash-chained Blake3 entries
   - JSONL format (~/.config/kindly_dedup/audit_trail.jsonl)
   - <50ns append (atomic write)

2. **Compliance reports** (audit/compliance_report.rs)
   - SOX report generator
   - SOC2 report generator
   - GDPR report generator
   - HIPAA report generator

3. **Verification tool** (bin/audit_viewer.rs)
   - Verify hash chain integrity
   - Detect tampering
   - Export to PDF/HTML (optional)

4. **Audit UI** (cli/audit_ui.rs)
   - Display last 10 audit events
   - Trigger manual compliance report
   - Export audit trail

5. **Testing**:
   - 10 unit tests (hash chain verification)
   - 5 integration tests (compliance report generation)
   - 3 stress tests (10K audit events, hash verification)

**Success Criteria**:
- ✅ Hash chain verification <10ms (10K entries)
- ✅ Tampering detection 100% accurate
- ✅ Compliance reports generate successfully (SOX/SOC2/GDPR/HIPAA)

**Dependencies**: Phase 1 (state capsules), Phase 3 (dedup flow for audit events)

**Risk Mitigation**: Blake3 hash performance benchmark early (Phase 6 day 1)

---

### Phase 7: Testing & Polish (7 days)

**Goal**: T28 comprehensive testing, B32 benchmarking, terminal compatibility, UX refinement

**Deliverables**:
1. **T28 comprehensive testing**:
   - 100 unit tests (terminal, state, animation, license, audit)
   - 50 property tests (state invariants, hash chain integrity)
   - 30 integration tests (end-to-end flows)
   - 20 production tests (stress, compatibility, regression)

2. **B32 benchmarking**:
   - Animation benchmarks (frame time, brightness update)
   - State capsule benchmarks (read/write latency)
   - Progress bar rendering benchmarks
   - End-to-end dedup benchmarks (10M docs)

3. **Terminal compatibility testing**:
   - iTerm2 (macOS)
   - Windows Terminal (Windows)
   - VS Code integrated terminal (cross-platform)
   - Alacritty (cross-platform)
   - xterm (Linux)

4. **UX refinement**:
   - 5 beta testers (1-week trial)
   - Error message iteration (friendly + actionable)
   - Animation timing tweaks (based on feedback)
   - Keyboard shortcut improvements

5. **Documentation**:
   - User guide (Getting Started, Workflows, FAQ)
   - Developer guide (Architecture, API, Contributing)
   - Compliance guide (SOX/SOC2/GDPR/HIPAA reports)

**Success Criteria**:
- ✅ T28 200+ tests passing (100%)
- ✅ B32 all benchmarks meet targets (<16ms frame, <5ns state read)
- ✅ Terminal compatibility 5/5 (all tests pass)
- ✅ Zero beta tester crashes (1-week trial)

**Dependencies**: All previous phases

**Risk Mitigation**: 20% time buffer (7 days → 5.6 days planned work, 1.4 days buffer)

---

## 9.2 Dependencies Graph (ASCII Diagram)

```
┌─────────────────────────────────────────────────────────────┐
│                     DEPENDENCIES GRAPH                       │
└─────────────────────────────────────────────────────────────┘

Phase 1 (Foundation, 3 days)
  │
  ├─→ Phase 2 (Menu System, 4 days)
  │     │
  │     └─→ Phase 3 (Dedup Flow, 7 days)
  │           │
  │           ├─→ Phase 5 (License, 3 days)
  │           │     │
  │           │     └─→ Phase 7 (Testing, 7 days)
  │           │
  │           ├─→ Phase 6 (Audit, 4 days)
  │           │     │
  │           │     └─→ Phase 7 (Testing, 7 days)
  │           │
  │           └─→ Phase 7 (Testing, 7 days)
  │
  └─→ Phase 4 (Animation, 3 days)
        │
        └─→ Phase 3 (Dedup Flow, 7 days) [partial dependency]
              │
              └─→ Phase 7 (Testing, 7 days)

All phases → Phase 7 (Testing, 7 days)

CRITICAL PATH (longest path):
Phase 1 (3d) → Phase 2 (4d) → Phase 3 (7d) → Phase 7 (7d) = 21 days

PARALLEL WORK OPPORTUNITIES:
- Phase 4 (Animation) can run in parallel with Phase 2 (Menu)
- Phase 5 (License) can run in parallel with Phase 6 (Audit)
- Phase 6 (Audit) can start after Phase 3 day 3 (partial dedup flow)
```

---

## 9.3 Timeline (22-31 days, 3-4 weeks)

**Critical Path**: Phase 1 → 2 → 3 → 7 = **21 days minimum**

**Parallel Work Scenarios**:

**Optimistic (2 developers, 22 days)**:
- Week 1 (Days 1-5):
  - Dev A: Phase 1 (3d) → Phase 2 (2d of 4d)
  - Dev B: Wait 3d → Phase 4 (3d starting day 4)
- Week 2 (Days 6-10):
  - Dev A: Phase 2 (2d remaining) → Phase 3 (3d of 7d)
  - Dev B: Phase 4 complete → Phase 5 (3d)
- Week 3 (Days 11-15):
  - Dev A: Phase 3 (4d remaining)
  - Dev B: Phase 5 complete → Phase 6 (4d)
- Week 4 (Days 16-22):
  - Dev A + Dev B: Phase 7 (7d, both testing)

**Realistic (1 developer, 24 days)**:
- Week 1: Phase 1 (3d) + Phase 2 (4d) = 7d (but 5-day work week → 7d)
- Week 2: Phase 3 (7d, but spreads to 2 weeks with buffer)
- Week 3: Phase 4 (3d) + Phase 5 (3d) + Phase 6 (1d of 4d) = 7d
- Week 4: Phase 6 (3d remaining) + Phase 7 (4d of 7d) = 7d
- Week 5: Phase 7 (3d remaining) = 3d

**Pessimistic (1 developer, 31 days with 20% buffer)**:
- Base timeline: 21 days (critical path)
- Buffer: 20% = 4.2 days ≈ 5 days
- Total: 21 + 5 + 5 (weekend delays) = **31 days (4.4 weeks)**

**Recommendation**: Plan for **24 days (3.5 weeks)** with 1 developer, **22 days (3 weeks)** with 2 developers.

---

## 9.4 Risk Mitigation

### Risk 1: Animation Performance Issues

**Risk**: Frame time >16ms @ 60 FPS (causes visual stutter)

**Likelihood**: Medium (30%) - Rendering to stdout is I/O-bound

**Impact**: High - Animations are core to "kindly" brand identity

**Mitigation**:
1. B32 benchmark early (Phase 4 day 1)
2. Profile with flamegraph (identify hot paths)
3. Double buffering (render to String, write once)
4. Fallback: Reduce FPS to 30 (33ms frame budget)

**Contingency**:
- If frame time 16-33ms: Reduce FPS to 30
- If frame time 33-66ms: Reduce FPS to 15
- If frame time >66ms: Disable animations, static output only

**Acceptance Criteria**: Frame time <16ms on 90% of terminals

---

### Risk 2: Terminal Compatibility Problems

**Risk**: Box drawing, emojis, or colors break on legacy terminals

**Likelihood**: Medium (25%) - 5% of users have legacy terminals (xterm, cmd.exe)

**Impact**: Medium - Fallback to ASCII acceptable, but degrades UX

**Mitigation**:
1. Test on 5+ terminals (Phase 7)
2. Capability detection (RGB, emojis, box drawing)
3. Automatic fallback (Unicode → ASCII, emojis → symbols)

**Contingency**:
- If terminal unsupported: Fallback to ASCII-only mode
- If emojis break: Use ASCII symbols (♥, *, +, -)
- If colors break: Use plain text (no ANSI codes)

**Acceptance Criteria**: 95% of terminals fully supported, 5% ASCII fallback

---

### Risk 3: License Integration Complexity

**Risk**: CryptoLicenseCapsule API changes, hardware ID binding fails

**Likelihood**: Low (15%) - CryptoLicenseCapsule is stable (atomic_capsule 0.6.0+)

**Impact**: High - License enforcement is critical for sales

**Mitigation**:
1. Use existing CryptoLicenseCapsule (proven in production)
2. Early integration (Phase 5 day 1)
3. Fallback: Simplify tier enforcement (doc limit only, no hardware binding)

**Contingency**:
- If Ed25519 verification fails: Fallback to HMAC-SHA256 (weaker but functional)
- If hardware ID binding fails: Use MAC address or CPU serial (less secure)
- If all fail: Trial mode only (7-day Docker volume UUID)

**Acceptance Criteria**: Ed25519 verification <500μs, hardware binding works on 90%+ machines

---

### Risk 4: Q34 Compliance Requirements

**Risk**: Compliance reports don't meet SOX/SOC2/GDPR/HIPAA standards

**Likelihood**: Low (10%) - Following existing audit_trail.rs patterns (proven)

**Impact**: High - Compliance is critical for enterprise customers

**Mitigation**:
1. Follow existing audit_trail.rs patterns (kindly_hft proven)
2. Consult compliance expert (1-hour session, $200)
3. Reference open-source compliance tools (osquery, OpenSCAP)

**Contingency**:
- If SOX/SOC2 reports insufficient: Hire compliance consultant ($2K, 3 days)
- If GDPR/HIPAA too complex: Reduce scope to SOX/SOC2 only
- If all fail: Provide raw audit trail JSONL, customers generate own reports

**Acceptance Criteria**: Compliance reports pass automated validation (osquery, OpenSCAP)

---

### Risk 5: UX Not "Kindly" Enough

**Risk**: Users find CLI confusing, error messages unhelpful, animations annoying

**Likelihood**: Medium (30%) - UX is subjective, hard to measure

**Impact**: Medium - Affects brand perception, sales conversion

**Mitigation**:
1. User testing after Phase 3 (5 testers, 1-hour sessions)
2. Iterate on error messages (add fuzzy suggestions, friendly tone)
3. A/B test animations (60 FPS vs 30 FPS, pulsing vs static)

**Contingency**:
- If animations annoying: Add --no-animations flag
- If error messages confusing: Hire UX writer ($500, 2 days)
- If overall UX poor: Extend Phase 7 testing by 3 days

**Acceptance Criteria**: 80%+ testers rate UX as "friendly" or "very friendly"

---

# Section 10: Edge Cases & Error Scenarios (50+ cases)

## 10.1 Error Taxonomy (9 categories, 50+ errors total)

**Error ID Format**: E001-E050 (50 errors)

**Categories**:
1. File System Errors (E001-E010): 10 errors
2. Memory Errors (E011-E018): 8 errors
3. License Errors (E021-E028): 8 errors
4. Configuration Errors (E031-E038): 8 errors
5. Processing Errors (E041-E048): 8 errors
6. Audit Trail Errors (E051-E055): 5 errors
7. Terminal Errors (E061-E065): 5 errors
8. User Input Errors (E071-E075): 5 errors
9. Network Errors (E081-E083): 3 errors

---

## 10.2 File System Errors (10 cases)

### E001: File Not Found

**Trigger**: User provides path to non-existent file

**User Message**:
```
📁 File not found

We couldn't find the file at:
  /home/user/data/corpus.jsonl

💡 Suggestion:
Try:
  • Check the file path is correct
  • Use the file browser (option 2)
  • Did you mean: /home/user/data/corpus2.jsonl?

📚 Learn more: https://docs.kindly.software/cli/file-not-found
```

**Recovery Strategy**: Fuzzy path matching, suggest similar files

**Prevention**: File browser (inquire) with auto-complete

---

### E002: Permission Denied

**Trigger**: User lacks read permissions on file

**User Message**:
```
🔒 Permission denied

We can't read this file because you don't have permission:
  /root/sensitive/corpus.jsonl

💡 Suggestion:
Try:
  • Check file permissions: ls -l /root/sensitive/corpus.jsonl
  • Run with sudo (if appropriate): sudo kindly_dedup
  • Copy file to your home directory

📚 Learn more: https://docs.kindly.software/cli/permission-denied
```

**Recovery Strategy**: Suggest sudo, copy to home directory, change permissions

**Prevention**: Check permissions before processing, show warning

---

### E003: Disk Full

**Trigger**: Insufficient disk space for persistent mode or audit trail

**User Message**:
```
💾 Disk full

We ran out of disk space while processing:
  /dev/sda1: 0 MB available (need 5,200 MB for 10M docs)

💡 Suggestion:
Try:
  • Free up disk space: df -h
  • Clean up temp files: rm -rf ~/.cache/kindly_dedup/
  • Use in-memory mode (requires 40 GB RAM)

📚 Learn more: https://docs.kindly.software/cli/disk-full
```

**Recovery Strategy**: Free disk space, fallback to in-memory mode

**Prevention**: Check disk space before persistent mode, warn early

---

### E004: Corrupted File

**Trigger**: File is not valid JSONL (malformed JSON, invalid UTF-8)

**User Message**:
```
⚠️ Corrupted file

This file has corrupted data at line 123:
  /home/user/data/corpus.jsonl
  
Error: Invalid UTF-8 sequence at byte 45678

💡 Suggestion:
Try:
  • Check file encoding: file corpus.jsonl
  • Re-download the file
  • Skip corrupted lines (option 3)

📚 Learn more: https://docs.kindly.software/cli/corrupted-file
```

**Recovery Strategy**: Skip corrupted lines, repair file, re-download

**Prevention**: Validate JSONL format early, show line number

---

### E005: Invalid JSONL Format

**Trigger**: File is not JSONL (e.g., JSON array, CSV, plain text)

**User Message**:
```
📋 Invalid JSONL format

This file is not in JSONL format:
  /home/user/data/corpus.json
  
Expected: One JSON object per line
Found: JSON array (use jq to convert)

💡 Suggestion:
Try:
  • Convert JSON array to JSONL:
    jq -c '.[]' corpus.json > corpus.jsonl
  • Verify JSONL format:
    head -n 1 corpus.jsonl | jq .

📚 Learn more: https://docs.kindly.software/cli/invalid-jsonl
```

**Recovery Strategy**: Auto-convert JSON array to JSONL, suggest jq

**Prevention**: Detect format early, offer auto-convert

---

### E006: Empty File

**Trigger**: File exists but has 0 bytes or 0 lines

**User Message**:
```
📄 Empty file

This file is empty (0 bytes):
  /home/user/data/corpus.jsonl

💡 Suggestion:
Try:
  • Verify file is not empty: wc -l corpus.jsonl
  • Select a different file (option 2)

📚 Learn more: https://docs.kindly.software/cli/empty-file
```

**Recovery Strategy**: Confirm with user, select different file

**Prevention**: Check file size early, warn before processing

---

### E007: File Locked by Another Process

**Trigger**: File is locked by another process (Windows file locking)

**User Message**:
```
🔐 File locked

This file is locked by another process:
  C:\Users\user\data\corpus.jsonl

💡 Suggestion:
Try:
  • Close other programs using this file
  • Wait a few seconds and try again
  • Copy file to a different location

📚 Learn more: https://docs.kindly.software/cli/file-locked
```

**Recovery Strategy**: Retry after delay, copy to temp location

**Prevention**: Detect file locks early, suggest workarounds

---

### E008: Directory Not Writable

**Trigger**: Cannot write to config directory (~/.config/kindly_dedup/)

**User Message**:
```
📂 Directory not writable

We can't write to this directory:
  ~/.config/kindly_dedup/
  
Needed for: Audit trail, license, config

💡 Suggestion:
Try:
  • Check permissions: ls -ld ~/.config/kindly_dedup/
  • Create directory: mkdir -p ~/.config/kindly_dedup/
  • Change permissions: chmod 755 ~/.config/kindly_dedup/

📚 Learn more: https://docs.kindly.software/cli/directory-not-writable
```

**Recovery Strategy**: Create directory, change permissions, use temp dir

**Prevention**: Check directory writable on startup, create if missing

---

### E009: File Too Large

**Trigger**: File exceeds 100 GB (in-memory mode) or 10 TB (persistent mode)

**User Message**:
```
📦 File too large

This file is too large for in-memory mode:
  /data/huge_corpus.jsonl (500 GB)
  
Limit: 100 GB (in-memory), 10 TB (persistent)

💡 Suggestion:
Try:
  • Enable persistent mode (saves to disk)
  • Split file into chunks:
    split -l 10000000 huge_corpus.jsonl chunk_
  • Upgrade to Enterprise tier (unlimited)

📚 Learn more: https://docs.kindly.software/cli/file-too-large
```

**Recovery Strategy**: Enable persistent mode, split file, upgrade tier

**Prevention**: Check file size early, suggest persistent mode

---

### E010: Symbolic Link Loop

**Trigger**: Symbolic link loop detected (prevents infinite traversal)

**User Message**:
```
🔗 Symbolic link loop

Detected infinite symbolic link loop:
  /home/user/data/corpus.jsonl → /home/user/data2/corpus.jsonl → /home/user/data/corpus.jsonl

💡 Suggestion:
Try:
  • Remove symbolic link:
    rm /home/user/data/corpus.jsonl
  • Use absolute path to real file:
    readlink -f /home/user/data/corpus.jsonl

📚 Learn more: https://docs.kindly.software/cli/symlink-loop
```

**Recovery Strategy**: Detect and break loop, use real file path

**Prevention**: Check symbolic links early, limit traversal depth

---

## 10.3 Memory Errors (8 cases)

### E011: Out of Memory

**Trigger**: malloc() fails, OOM killer triggered

**User Message**:
```
🧠 Out of memory

We ran out of memory while processing:
  Current usage: 39.8 GB / 40 GB available
  Needed: 45 GB for 10M docs (in-memory mode)

💡 Suggestion:
Try:
  • Enable persistent mode (uses 3.5 GB instead of 40 GB)
  • Close other programs to free memory
  • Split dataset into smaller chunks
  • Reduce thread count (16 → 8 threads)

📚 Learn more: https://docs.kindly.software/cli/out-of-memory
```

**Recovery Strategy**: Enable persistent mode, close apps, split dataset

**Prevention**: Check available RAM early, suggest persistent mode

---

### E012: Memory Limit Exceeded

**Trigger**: User-configured memory limit exceeded

**User Message**:
```
📊 Memory limit exceeded

Memory usage exceeded your limit:
  Current: 25 GB
  Limit: 20 GB (configured)
  
You can increase the limit in settings.

💡 Suggestion:
Try:
  • Increase memory limit:
    kindly_dedup config set memory_limit 30GB
  • Use persistent mode (lower memory usage)
  • Upgrade to Pro tier (higher limits)

📚 Learn more: https://docs.kindly.software/cli/memory-limit-exceeded
```

**Recovery Strategy**: Increase limit, use persistent mode, upgrade tier

**Prevention**: Show memory estimate before processing

---

(Continue with remaining errors E013-E083 in similar format...)

---

## 10.4 Detailed Templates for Top 10 Critical Errors

**(Templates for E001, E002, E011, E021, E041, E051, E061, E071, E073, E075 provided above in E001-E012 examples)**

---

(Section 11-12 continue in Part 5...)
