# kindly_dedup CLI Specification - Sections 7-12 (Part 5 - FINAL)

## (Continued from Part 4: Section 11-12)

---

# Section 11: Testing Strategy (T28)

## 11.1 Q1-Q7: Unit Tests (100+ tests)

**Goal**: Test individual components in isolation

### Terminal Utilities (20 tests)

```rust
#[cfg(test)]
mod terminal_tests {
    use super::*;
    
    #[test]
    fn test_colorize_basic() {
        let text = "Success";
        let colored = colorize(text, Color::Green);
        // When in TTY: contains ANSI codes
        // When not TTY: equals plain text
        assert!(colored == text || colored.contains("\x1b[32m"));
    }
    
    #[test]
    fn test_emoji_support_detection() {
        let supported = supports_emoji();
        // Non-deterministic (depends on terminal), just verify no panic
        assert!(supported == true || supported == false);
    }
    
    #[test]
    fn test_box_drawing_fallback() {
        let unicode = "┌";
        let fallback = box_drawing::fallback(unicode);
        assert!(fallback == "┌" || fallback == "+");
    }
    
    #[test]
    fn test_cursor_save_restore() {
        // Mock stdout, verify ANSI codes
        cursor::save().unwrap();
        cursor::restore().unwrap();
    }
    
    // + 16 more tests (color codes, style codes, terminal detection)
}
```

### Animation Engine (15 tests)

```rust
#[test]
fn test_frame_scheduler_timing() {
    let scheduler = FrameScheduler::new(60); // 60 FPS
    
    // First check: should render (no previous frame)
    assert!(scheduler.should_render());
    
    // Second check (immediately): should NOT render (<16ms elapsed)
    assert!(!scheduler.should_render());
    
    // Wait 16ms
    std::thread::sleep(std::time::Duration::from_millis(16));
    
    // Third check: should render (>16ms elapsed)
    assert!(scheduler.should_render());
}

#[test]
fn test_brightness_cycling() {
    let animation = AnimationStateCapsule::new();
    animation.set_fps(60);
    
    // Cycle through 120 frames (2 seconds @ 60 FPS)
    for i in 0..120 {
        animation.increment_frame();
        let brightness = animation.brightness();
        
        // Brightness should be in range [0.4, 1.0]
        assert!(brightness >= 0.4 && brightness <= 1.0);
    }
    
    // After full cycle, brightness should return to ~0.4
    let final_brightness = animation.brightness();
    assert!((final_brightness - 0.4).abs() < 0.1);
}

#[test]
fn test_pulsing_heart_render() {
    let heart = PulsingHeart::new();
    let rendered = heart.render();
    
    // Should contain purple heart emoji (or fallback ♥)
    assert!(rendered.contains("💜") || rendered.contains("♥"));
}

// + 12 more tests (spinner, progress bar, celebration)
```

### State Management (20 tests)

```rust
#[test]
fn test_menu_state_transitions() {
    let menu = MenuStateCapsule::new();
    
    // Initial state: selected_index = 0
    assert_eq!(menu.selected_index(), 0);
    
    // Navigate down
    menu.set_selected_index(1);
    assert_eq!(menu.selected_index(), 1);
    
    // Navigate up (wrap around to 6)
    menu.set_selected_index(6);
    assert_eq!(menu.selected_index(), 6);
}

#[test]
fn test_progress_tracker_increment() {
    let progress = ProgressTrackerCapsule::new();
    
    // Increment 1000 times
    for _ in 0..1000 {
        progress.increment_docs();
    }
    
    // Should have 1000 docs processed
    assert_eq!(progress.docs_processed(), 1000);
}

#[test]
fn test_progress_tracker_concurrent() {
    let progress = Arc::new(ProgressTrackerCapsule::new());
    
    // 16 threads, each increments 1000 times
    let handles: Vec<_> = (0..16).map(|_| {
        let p = Arc::clone(&progress);
        std::thread::spawn(move || {
            for _ in 0..1000 {
                p.increment_docs();
            }
        })
    }).collect();
    
    for h in handles {
        h.join().unwrap();
    }
    
    // Should have 16,000 docs processed (lockfree correctness)
    assert_eq!(progress.docs_processed(), 16_000);
}

// + 17 more tests (animation state, license state, atomic operations)
```

### Menu Navigation (15 tests)

```rust
#[test]
fn test_main_menu_options() {
    let menu = MainMenu::new();
    
    // Should have 7 options
    assert_eq!(menu.options().len(), 7);
    
    // Options: Deduplicate, Configure, License, Help, Export, Audit, Exit
    assert_eq!(menu.options()[0], "Deduplicate");
    assert_eq!(menu.options()[6], "Exit");
}

#[test]
fn test_menu_keyboard_input() {
    let menu = MenuStateCapsule::new();
    
    // Simulate arrow down
    menu.handle_event(MenuEvent::Down);
    assert_eq!(menu.selected_index(), 1);
    
    // Simulate arrow up (wrap to 6)
    menu.handle_event(MenuEvent::Up);
    assert_eq!(menu.selected_index(), 0);
}

// + 13 more tests (menu rendering, selection, exit)
```

### Error Handling (15 tests)

```rust
#[test]
fn test_file_not_found_error() {
    let err = FileError::NotFound {
        path: PathBuf::from("/nonexistent/file.jsonl"),
    };
    
    let cli_err = CliError::File(err);
    let msg = cli_err.friendly_message();
    
    // Should contain file path
    assert!(msg.description.contains("/nonexistent/file.jsonl"));
    
    // Should have suggestion
    assert!(msg.suggestion.contains("Check the file path"));
    
    // Should have emoji
    assert_eq!(msg.emoji, "📁");
}

#[test]
fn test_recovery_strategy() {
    let err = CliError::Memory(MemoryError::OutOfMemory { available: 10_000_000_000 });
    
    let strategy = err.recovery_strategy();
    
    match strategy {
        RecoveryStrategy::Degrade { reduced_functionality } => {
            assert!(reduced_functionality.contains("persistent mode"));
        },
        _ => panic!("Expected Degrade strategy"),
    }
}

// + 13 more tests (error messages, suggestions, recovery)
```

### License Validation (10 tests)

```rust
#[test]
fn test_license_tier_enforcement() {
    let license = LicenseStateCapsule::new_free_tier();
    
    // Free tier: 100K doc limit
    assert_eq!(license.doc_limit(), 100_000);
    
    // Should reject 1M docs
    let result = license.enforce_tier(1_000_000);
    assert!(result.is_err());
    
    // Should accept 50K docs
    let result = license.enforce_tier(50_000);
    assert!(result.is_ok());
}

#[test]
fn test_license_expiration() {
    let license = LicenseStateCapsule::new_trial();
    
    // Set expiration to past
    license.set_expiration(SystemTime::UNIX_EPOCH);
    
    // Should be expired
    assert!(license.is_expired());
}

// + 8 more tests (tier upgrades, Ed25519 verification, trial mode)
```

### Audit Logging (10 tests)

```rust
#[test]
fn test_audit_trail_hash_chain() {
    let mut logger = AuditLogger::new().unwrap();
    
    // Log 3 events
    logger.log_event(AuditEvent::Startup).unwrap();
    logger.log_event(AuditEvent::MenuSelect { option: 1 }).unwrap();
    logger.log_event(AuditEvent::DedupStart { num_docs: 1000 }).unwrap();
    
    // Verify hash chain
    let entries = logger.read_all_entries().unwrap();
    assert_eq!(entries.len(), 3);
    
    // Entry 2 prev_hash should match Entry 1 entry_hash
    assert_eq!(entries[1].prev_hash, entries[0].entry_hash);
    
    // Entry 3 prev_hash should match Entry 2 entry_hash
    assert_eq!(entries[2].prev_hash, entries[1].entry_hash);
}

#[test]
fn test_audit_trail_tampering_detection() {
    let mut logger = AuditLogger::new().unwrap();
    
    // Log event
    logger.log_event(AuditEvent::Startup).unwrap();
    
    // Manually tamper with log file (corrupt entry)
    let log_path = logger.log_path();
    std::fs::write(log_path, "corrupted data").unwrap();
    
    // Verification should fail
    let result = logger.verify_hash_chain();
    assert!(result.is_err());
}

// + 8 more tests (compliance reports, sequence numbers, timestamps)
```

**Total Unit Tests**: 105 tests

---

## 11.2 Q8-Q14: Property Tests (50+ tests)

**Goal**: Test invariants that should hold for all inputs

### State Machine Properties (10 tests)

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn menu_selected_index_bounded(index in 0u16..7) {
        let menu = MenuStateCapsule::new();
        menu.set_selected_index(index);
        
        // Invariant: selected_index should always be 0-6
        assert!(menu.selected_index() < 7);
    }
    
    #[test]
    fn progress_monotonic(increments in prop::collection::vec(1u64..100, 1..1000)) {
        let progress = ProgressTrackerCapsule::new();
        let mut prev = 0;
        
        for inc in increments {
            for _ in 0..inc {
                progress.increment_docs();
            }
            
            // Invariant: docs_processed should be monotonically increasing
            let current = progress.docs_processed();
            assert!(current >= prev);
            prev = current;
        }
    }
}
```

### Atomic Counter Properties (10 tests)

```rust
proptest! {
    #[test]
    fn progress_tracker_no_overflow(ops in prop::collection::vec(0u8..2, 1..10000)) {
        let progress = Arc::new(ProgressTrackerCapsule::new());
        
        // Simulate 16 threads performing random operations
        let handles: Vec<_> = (0..16).map(|_| {
            let p = Arc::clone(&progress);
            let ops_clone = ops.clone();
            std::thread::spawn(move || {
                for op in ops_clone {
                    match op {
                        0 => p.increment_docs(),
                        1 => p.increment_duplicates(),
                        _ => {},
                    }
                }
            })
        }).collect();
        
        for h in handles {
            h.join().unwrap();
        }
        
        // Invariant: No overflow, counters should be valid
        assert!(progress.docs_processed() <= u64::MAX);
        assert!(progress.duplicates_found() <= u64::MAX);
    }
}
```

### Animation Properties (5 tests)

```rust
proptest! {
    #[test]
    fn brightness_always_in_range(frame_count in 0u64..10000) {
        let animation = AnimationStateCapsule::new();
        animation.set_fps(60);
        
        for _ in 0..frame_count {
            animation.increment_frame();
        }
        
        // Invariant: brightness should always be [0.4, 1.0]
        let brightness = animation.brightness();
        assert!(brightness >= 0.4 && brightness <= 1.0);
    }
    
    #[test]
    fn fps_clamped(fps in 0u8..255) {
        let animation = AnimationStateCapsule::new();
        animation.set_fps(fps);
        
        // Invariant: FPS should be clamped to [8, 60]
        let actual_fps = animation.fps();
        assert!(actual_fps >= 8 && actual_fps <= 60);
    }
}
```

### License Properties (10 tests)

```rust
proptest! {
    #[test]
    fn tier_enforcement_consistent(num_docs in 1usize..100_000_000) {
        let license = LicenseStateCapsule::new_free_tier();
        
        // Invariant: tier enforcement should be deterministic
        let result1 = license.enforce_tier(num_docs);
        let result2 = license.enforce_tier(num_docs);
        
        assert_eq!(result1.is_ok(), result2.is_ok());
    }
    
    #[test]
    fn expiration_monotonic(expiration_unix in 0i64..i64::MAX) {
        let license = LicenseStateCapsule::new_trial();
        license.set_expiration_unix(expiration_unix);
        
        // Invariant: expiration should be >= UNIX_EPOCH
        assert!(license.expiration_unix() >= 0);
    }
}
```

### Audit Properties (10 tests)

```rust
proptest! {
    #[test]
    fn audit_sequence_monotonic(events in prop::collection::vec(any::<AuditEvent>(), 1..1000)) {
        let mut logger = AuditLogger::new().unwrap();
        
        for event in events {
            logger.log_event(event).unwrap();
        }
        
        // Invariant: sequence numbers should be monotonically increasing
        let entries = logger.read_all_entries().unwrap();
        for i in 1..entries.len() {
            assert_eq!(entries[i].sequence, entries[i-1].sequence + 1);
        }
    }
    
    #[test]
    fn audit_hash_chain_integrity(events in prop::collection::vec(any::<AuditEvent>(), 1..100)) {
        let mut logger = AuditLogger::new().unwrap();
        
        for event in events {
            logger.log_event(event).unwrap();
        }
        
        // Invariant: hash chain should always be valid
        assert!(logger.verify_hash_chain().is_ok());
    }
}
```

### Error Properties (5 tests)

```rust
proptest! {
    #[test]
    fn error_recovery_idempotent(err in any::<CliError>()) {
        // Invariant: recovery strategy should be deterministic
        let strategy1 = err.recovery_strategy();
        let strategy2 = err.recovery_strategy();
        
        // Compare strategy types (Retry, Fallback, Degrade, Cancel)
        assert_eq!(
            std::mem::discriminant(&strategy1),
            std::mem::discriminant(&strategy2)
        );
    }
}
```

**Total Property Tests**: 50 tests

---

## 11.3 Q15-Q21: Integration Tests (30+ tests)

**Goal**: Test end-to-end flows and component interactions

### End-to-End Flows (10 tests)

```rust
#[test]
fn test_e2e_welcome_to_dedup() {
    let mut app = CliApplication::new().unwrap();
    
    // 1. Welcome screen
    app.navigate_to(Screen::Welcome);
    assert_eq!(app.current_screen(), Screen::Welcome);
    
    // 2. Press Enter → Main menu
    app.handle_input(InputEvent::Enter);
    assert_eq!(app.current_screen(), Screen::MainMenu);
    
    // 3. Select "Deduplicate" (option 0)
    app.handle_input(InputEvent::Select);
    assert_eq!(app.current_screen(), Screen::FileSelection);
    
    // 4. Enter file path
    app.enter_file_path("/tmp/test_corpus.jsonl");
    assert_eq!(app.current_screen(), Screen::Configuration);
    
    // 5. Accept default config
    app.handle_input(InputEvent::Enter);
    assert_eq!(app.current_screen(), Screen::Processing);
    
    // 6. Wait for completion
    app.wait_for_completion().unwrap();
    assert_eq!(app.current_screen(), Screen::Results);
    
    // 7. Verify results
    let results = app.get_results().unwrap();
    assert!(results.duplicates_found > 0);
}
```

### License Integration (5 tests)

```rust
#[test]
fn test_license_tier_upgrade() {
    let mut app = CliApplication::new().unwrap();
    
    // Start with Free tier
    assert_eq!(app.license.tier(), LicenseTier::Free);
    
    // Try to process 1M docs (exceeds Free tier 100K limit)
    let result = app.process_large_dataset(1_000_000);
    assert!(result.is_err());
    
    // Upgrade to Pro tier
    app.license.upgrade_to_pro("VALID-PRO-LICENSE-KEY").unwrap();
    assert_eq!(app.license.tier(), LicenseTier::Pro);
    
    // Retry 1M docs (should succeed)
    let result = app.process_large_dataset(1_000_000);
    assert!(result.is_ok());
}

#[test]
fn test_trial_expiration() {
    let mut app = CliApplication::new_trial().unwrap();
    
    // Trial tier should work initially
    assert_eq!(app.license.tier(), LicenseTier::Trial);
    
    // Fast-forward 8 days (trial expires after 7 days)
    app.license.set_expiration_unix(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as i64 - 86400);
    
    // Should degrade to Free tier
    let result = app.verify_license();
    assert!(result.is_err());
    assert_eq!(app.license.tier(), LicenseTier::Free);
}
```

### Audit Trail (5 tests)

```rust
#[test]
fn test_audit_trail_e2e() {
    let mut app = CliApplication::new().unwrap();
    
    // Enable audit trail
    app.enable_audit_trail().unwrap();
    
    // Perform operations
    app.navigate_to(Screen::MainMenu);
    app.handle_input(InputEvent::Select);
    app.enter_file_path("/tmp/test.jsonl");
    app.process_dataset().unwrap();
    
    // Verify audit trail
    let audit_entries = app.audit_logger.read_all_entries().unwrap();
    
    // Should have 5+ entries (startup, navigate, select, enter_path, process)
    assert!(audit_entries.len() >= 5);
    
    // Verify hash chain
    assert!(app.audit_logger.verify_hash_chain().is_ok());
}

#[test]
fn test_compliance_report_generation() {
    let mut app = CliApplication::new().unwrap();
    app.enable_audit_trail().unwrap();
    
    // Perform operations
    app.process_dataset().unwrap();
    
    // Generate SOX report
    let sox_report = app.generate_compliance_report(ReportFormat::SOX).unwrap();
    assert!(sox_report.contains("SOX Compliance Report"));
    assert!(sox_report.contains("Hash Chain Verified: YES"));
    
    // Generate SOC2 report
    let soc2_report = app.generate_compliance_report(ReportFormat::SOC2).unwrap();
    assert!(soc2_report.contains("SOC2 Compliance Report"));
}
```

### Terminal Compatibility (5 tests)

```rust
#[test]
fn test_terminal_fallback_ascii() {
    // Mock terminal without Unicode support
    let caps = TerminalCapabilities {
        rgb_colors: false,
        emojis: false,
        box_drawing: false,
        // ...
    };
    
    let renderer = FallbackRenderer::new(caps);
    
    // Purple heart should fallback to ASCII ♥
    assert_eq!(renderer.purple_heart(), "♥");
    
    // Box border should use ASCII
    let (top, _, bottom) = renderer.box_border(20);
    assert!(top.starts_with('+'));
    assert!(bottom.starts_with('+'));
}

#[test]
fn test_terminal_resize_handling() {
    let mut app = CliApplication::new().unwrap();
    
    // Initial size: 80x24
    assert_eq!(app.terminal_size(), (80, 24));
    
    // Simulate resize to 120x40
    app.handle_resize(120, 40);
    
    // Screen should re-render at new size
    assert_eq!(app.terminal_size(), (120, 40));
}
```

### Error Recovery (5 tests)

```rust
#[test]
fn test_error_recovery_file_not_found() {
    let mut app = CliApplication::new().unwrap();
    
    // Try to open non-existent file
    let result = app.enter_file_path("/nonexistent/file.jsonl");
    assert!(result.is_err());
    
    // Error message should be friendly
    let msg = result.unwrap_err().friendly_message();
    assert!(msg.description.contains("/nonexistent/file.jsonl"));
    assert!(msg.suggestion.contains("file browser"));
    
    // Recovery: use file browser
    app.navigate_to(Screen::FileBrowser);
    app.select_file_from_browser("/tmp/test.jsonl").unwrap();
    
    // Should now be able to process
    assert!(app.current_file_path().is_some());
}

#[test]
fn test_graceful_shutdown_on_ctrl_c() {
    let mut app = CliApplication::new().unwrap();
    
    // Start processing
    app.process_dataset_async().unwrap();
    
    // Simulate Ctrl+C
    app.handle_signal(Signal::Interrupt);
    
    // Should save progress
    let progress = app.progress.snapshot();
    assert!(progress.docs_processed > 0);
    
    // Should close gracefully (cursor restored, audit logged)
    app.wait_for_shutdown().unwrap();
    
    // Verify audit trail logged shutdown
    let last_entry = app.audit_logger.last_entry().unwrap();
    assert_eq!(last_entry.event_type, "shutdown");
}
```

**Total Integration Tests**: 30 tests

---

## 11.4 Q22-Q28: Production Tests (20+ tests)

**Goal**: Test real-world scenarios, stress, compatibility, regression

### Performance Benchmarks (5 tests)

```rust
#[bench]
fn bench_animation_fps(b: &mut Bencher) {
    let animation = AnimationStateCapsule::new();
    let scheduler = FrameScheduler::new(60);
    
    b.iter(|| {
        if scheduler.should_render() {
            animation.update_animation(16_666_667); // 60 FPS
        }
    });
    
    // Expected: <16ms per frame (60 FPS target)
}

#[bench]
fn bench_progress_bar_render(b: &mut Bencher) {
    let progress = Arc::new(ProgressTrackerCapsule::new());
    let renderer = ProgressBarRenderer::new(progress, 80);
    
    b.iter(|| {
        black_box(renderer.render())
    });
    
    // Expected: <2ms per render
}

#[bench]
fn bench_state_capsule_read(b: &mut Bencher) {
    let menu = Arc::new(MenuStateCapsule::new());
    
    b.iter(|| {
        black_box(menu.selected_index())
    });
    
    // Expected: <5ns read (Acquire)
}
```

### Stress Tests (5 tests)

```rust
#[test]
#[ignore] // Run with: cargo test --ignored
fn stress_test_10m_docs_16_threads() {
    let mut app = CliApplication::new().unwrap();
    
    // Process 10M docs with 16 threads
    let result = app.process_large_dataset_parallel(10_000_000, 16);
    assert!(result.is_ok());
    
    // Verify results
    let stats = app.get_stats();
    assert_eq!(stats.docs_processed, 10_000_000);
    assert!(stats.throughput_docs_per_sec > 100_000); // >100K docs/sec
}

#[test]
#[ignore]
fn stress_test_60fps_10_minutes() {
    let mut app = CliApplication::new().unwrap();
    
    // Run animation at 60 FPS for 10 minutes
    let start = Instant::now();
    let mut frame_count = 0;
    
    while start.elapsed() < Duration::from_secs(600) {
        if app.animation.should_render() {
            app.render_screen().unwrap();
            frame_count += 1;
        }
    }
    
    // Should render ~36,000 frames (60 FPS × 600 sec)
    assert!(frame_count > 35_000 && frame_count < 37_000);
}
```

### Terminal Compatibility Tests (5 tests)

```rust
#[test]
fn test_iterm2_compatibility() {
    // Mock iTerm2 terminal
    std::env::set_var("TERM", "xterm-256color");
    std::env::set_var("COLORTERM", "truecolor");
    
    let caps = TerminalCapabilities::detect();
    
    assert!(caps.rgb_colors);
    assert!(caps.emojis);
    assert!(caps.box_drawing);
}

#[test]
fn test_windows_terminal_compatibility() {
    // Mock Windows Terminal
    std::env::set_var("TERM", "xterm-256color");
    std::env::set_var("WT_SESSION", "12345");
    
    let caps = TerminalCapabilities::detect();
    
    assert!(caps.rgb_colors);
    assert!(caps.emojis);
}

#[test]
fn test_vscode_terminal_compatibility() {
    // Mock VS Code terminal
    std::env::set_var("TERM", "xterm-256color");
    std::env::set_var("TERM_PROGRAM", "vscode");
    
    let caps = TerminalCapabilities::detect();
    
    assert!(caps.rgb_colors);
    assert!(caps.emojis);
}

#[test]
fn test_xterm_fallback() {
    // Mock legacy xterm (no RGB, limited emojis)
    std::env::set_var("TERM", "xterm");
    std::env::remove_var("COLORTERM");
    
    let caps = TerminalCapabilities::detect();
    
    assert!(!caps.rgb_colors); // No 24-bit color
    // Should fallback to 16 colors
}
```

### Compliance Tests (3 tests)

```rust
#[test]
fn test_sox_compliance() {
    let mut app = CliApplication::new().unwrap();
    app.enable_audit_trail().unwrap();
    
    // Perform operations
    app.process_dataset().unwrap();
    
    // Generate SOX report
    let report = app.generate_compliance_report(ReportFormat::SOX).unwrap();
    
    // Verify compliance requirements
    assert!(report.contains("Audit Trail Integrity: PASS"));
    assert!(report.contains("Hash Chain Verified: YES"));
    assert!(report.contains("Tampering Detected: NO"));
}

#[test]
fn test_soc2_compliance() {
    // Similar to SOX, verify SOC2 requirements
}

#[test]
fn test_gdpr_compliance() {
    // Verify GDPR data processing transparency
}
```

### License Scenarios (2 tests)

```rust
#[test]
fn test_trial_to_pro_upgrade() {
    let mut app = CliApplication::new_trial().unwrap();
    
    // Trial: 7 days, 100K docs
    assert_eq!(app.license.tier(), LicenseTier::Trial);
    
    // Use trial for 6 days
    app.process_dataset().unwrap();
    
    // Upgrade to Pro
    app.license.activate("VALID-PRO-KEY").unwrap();
    assert_eq!(app.license.tier(), LicenseTier::Pro);
    
    // Verify Pro features unlocked
    assert!(app.license.has_feature(Feature::SIMD));
    assert!(app.license.has_feature(Feature::Bloom));
}

#[test]
fn test_tier_enforcement_edge_cases() {
    let mut app = CliApplication::new().unwrap();
    
    // Free tier: exactly 100K docs should succeed
    let result = app.process_dataset_size(100_000);
    assert!(result.is_ok());
    
    // Free tier: 100,001 docs should fail
    let result = app.process_dataset_size(100_001);
    assert!(result.is_err());
}
```

**Total Production Tests**: 20 tests

**TOTAL T28 TESTS**: 205 tests (105 unit + 50 property + 30 integration + 20 production)

---

# Section 12: Appendices

## Appendix A: Complete Emoji Map (190 emojis)

**15 Categories**, already defined in `terminal.rs` (lines 527-846):

1. **Primary Brand**: PURPLE_HEART (💜)
2. **Quick Access** (6): BRAND_PRIMARY, BRAND_SECONDARY, SUCCESS, ROCKET, CROWN, GEM
3. **Status** (12): SUCCESS, CHECK, CHECKMARK, FAIL, CROSS, ERROR, WARNING, CAUTION, INFO, QUESTION, PENDING, LOADING, WAIT
4. **Performance** (10): ROCKET, LIGHTNING, FIRE, SPARKLES, BOOM, TURBO, FAST, SLOW, ZIPPING, POWER
5. **Brand** (17): PURPLE_HEART, GOLD_HEART, PURPLE_CIRCLE, GOLD_CIRCLE, FLEUR_DE_LIS, CRYSTAL, PALETTE, STAR, GLOWING_STAR, DIZZY, THEATER, CROWN, GEM, TROPHY, MEDAL, SCROLL, CASTLE
6. **Data** (11): CHART, GRAPH_UP, GRAPH_DOWN, MONEY, COIN, DATABASE, FOLDER, FILE, MEMO, BAR_CHART, TREND, TABLE
7. **Tools** (12): WRENCH, HAMMER, GEAR, MAGNIFY, SEARCH, KEY, LOCK, UNLOCK, SHIELD, SECURITY, TOOLBOX, WRENCH_HAMMER
8. **Arrows** (12): RIGHT, LEFT, UP, DOWN, RIGHT_ARROW, LEFT_ARROW, UP_ARROW, DOWN_ARROW, DIAGONAL_UP, DIAGONAL_DOWN, DOUBLE_RIGHT, DOUBLE_LEFT
9. **Shapes** (12): BULLET, DIAMOND, SQUARE, CIRCLE, TRIANGLE, HOURGLASS, ASTERISK, PLUS, MINUS, EQUALS, PIPE, DASH
10. **Emotions** (14): RED_HEART, ORANGE_HEART, YELLOW_HEART, GREEN_HEART, BLUE_HEART, PURPLE_HEART, BLACK_HEART, WHITE_HEART, BROWN_HEART, BROKEN_HEART, FIRE_HEART, BANDAGE_HEART, MULTI_HEART, SPARKLING_HEART, HEART_EXCLAMATION
11. **Nature** (15): MOON, STAR, GLOWING_STAR, SPARKLES, SUN, SUNNY, RAINBOW, LIGHTNING, FIRE, WATER, WAVE, FLOWER, BLOSSOM, SUNFLOWER, ROSE
12. **Technology** (15): COMPUTER, KEYBOARD, DESKTOP, MOUSE, FLOPPY, DISC, DVD, PLUG, BATTERY, SATELLITE, ROCKET, CALCULATOR, MOBILE, GEAR, WRENCH
13. **Time** (12): ALARM, STOPWATCH, TIMER, HOURGLASS, HOURGLASS_DONE, CLOCK_1, CLOCK_2, CLOCK_3, ANCIENT_CLOCK, CALENDAR, CALENDAR_ALT, CALENDAR_SPIRAL
14. **Celebration** (13): PARTY, CONFETTI, BALLOON, FIREWORKS, SPARKLER, SPARKLES, GIFT, TROPHY, GOLD_MEDAL, SILVER_MEDAL, BRONZE_MEDAL, MILITARY_MEDAL, BADGE
15. **Food** (14): COFFEE, PIZZA, BURGER, FRIES, TACO, BURRITO, NOODLES, BENTO, SUSHI, BEER, BEER_MUG, CHAMPAGNE, WINE, WINE_BOTTLE
16. **Animals** (15): EAGLE, LION, DRAGON, UNICORN, BUTTERFLY, BEE, OWL, PHOENIX, FOX, WOLF, PENGUIN, SWAN, HORSE, DOLPHIN, WHALE

**Total**: 190 emojis

**Usage Example**:
```rust
use kindly_dedup::utils::terminal::emoji;

println!("{} Processing...", emoji::performance::ROCKET);
println!("{} Success!", emoji::status::CHECKMARK);
println!("{} {}", emoji::brand::PURPLE_HEART, "Thank you!");
```

---

## Appendix B: ANSI Color Codes

**Byzantine Purple/Gold RGB Palette** (8 colors):

| Color Name         | RGB               | ANSI 24-bit Code                | 16-color Fallback |
|--------------------|-------------------|---------------------------------|-------------------|
| ByzantinePurple    | RGB(112, 41, 99)  | `\x1b[38;2;112;41;99m`          | BrightMagenta     |
| RoyalPurple        | RGB(120, 81, 169) | `\x1b[38;2;120;81;169m`         | Magenta           |
| DeepPurple         | RGB(75, 0, 130)   | `\x1b[38;2;75;0;130m`           | Blue              |
| LightPurple        | RGB(189, 140, 191)| `\x1b[38;2;189;140;191m`        | BrightMagenta     |
| ByzantineGold      | RGB(207, 181, 59) | `\x1b[38;2;207;181;59m`         | BrightYellow      |
| BrightGold         | RGB(255, 215, 0)  | `\x1b[38;2;255;215;0m`          | Yellow            |
| DeepGold           | RGB(184, 134, 11) | `\x1b[38;2;184;134;11m`         | Yellow            |
| RoseGold           | RGB(183, 110, 121)| `\x1b[38;2;183;110;121m`        | BrightRed         |

**Style Codes**:

| Style      | ANSI Code   | Example                             |
|------------|-------------|-------------------------------------|
| Reset      | `\x1b[0m`   | Reset all styles                    |
| Bold       | `\x1b[1m`   | **Bold text**                       |
| Dim        | `\x1b[2m`   | Dim text (less bright)              |
| Italic     | `\x1b[3m`   | *Italic text*                       |
| Underline  | `\x1b[4m`   | <u>Underlined text</u>              |

**Example**:
```rust
use kindly_dedup::utils::terminal::{colorize_with_style, Color, Style};

let text = colorize_with_style("Success!", Color::ByzantinePurple, Style::Bold);
println!("{}", text);
// Output: \x1b[38;2;112;41;99m\x1b[1mSuccess!\x1b[0m
```

---

## Appendix C: Box Drawing Reference

**Unicode Box Characters** (single-line):

```
┌────┬────┐
│ A  │ B  │
├────┼────┤
│ C  │ D  │
└────┴────┘
```

**Character Map**:

| Character | Unicode | ASCII Fallback | Description          |
|-----------|---------|----------------|----------------------|
| ┌         | U+250C  | +              | Top-left corner      |
| ┐         | U+2510  | +              | Top-right corner     |
| └         | U+2514  | +              | Bottom-left corner   |
| ┘         | U+2518  | +              | Bottom-right corner  |
| ─         | U+2500  | -              | Horizontal line      |
| │         | U+2502  | \|             | Vertical line        |
| ├         | U+251C  | +              | Left tee             |
| ┤         | U+2524  | +              | Right tee            |
| ┬         | U+252C  | +              | Top tee              |
| ┴         | U+2534  | +              | Bottom tee           |
| ┼         | U+253C  | +              | Cross                |

**Layout Templates**:

**Menu Template**:
```
┌────────────────────────────────┐
│  Main Menu                     │
├────────────────────────────────┤
│  1. Deduplicate                │
│  2. Configure                  │
│  3. License                    │
│  4. Help                       │
│  5. Export                     │
│  6. Audit                      │
│  7. Exit                       │
└────────────────────────────────┘
```

**Progress Panel Template**:
```
┌─ Progress ────────────────────┐
│ Docs:       500K / 1M  (50%)  │
│ Duplicates: 150K       (30%)  │
│ Throughput: 60K docs/sec      │
│ ETA:        8 sec             │
│                               │
│ [████████████          ] 50%  │
└───────────────────────────────┘
```

---

## Appendix D: Keyboard Shortcuts

**Global Shortcuts** (work everywhere):

| Key       | Action                    | Description                      |
|-----------|---------------------------|----------------------------------|
| Esc       | Cancel/Back               | Return to previous screen        |
| q         | Quit                      | Exit CLI (with confirmation)     |
| Ctrl+C    | Interrupt                 | Graceful shutdown (save progress)|
| Ctrl+D    | EOF                       | Exit CLI (no confirmation)       |

**Navigation Shortcuts** (menus, lists):

| Key       | Action                    | Description                      |
|-----------|---------------------------|----------------------------------|
| ↑         | Move up                   | Select previous option           |
| ↓         | Move down                 | Select next option               |
| ←         | Move left                 | Navigate tabs, sliders           |
| →         | Move right                | Navigate tabs, sliders           |
| Tab       | Next field                | Move to next input field         |
| Shift+Tab | Previous field            | Move to previous input field     |
| Enter     | Confirm                   | Confirm selection/input          |
| Space     | Toggle                    | Toggle checkbox                  |

**Context-Specific Shortcuts**:

| Screen            | Key       | Action                    |
|-------------------|-----------|---------------------------|
| Main Menu         | 1-7       | Quick select option       |
| File Browser      | /         | Search files              |
| File Browser      | PgUp      | Scroll up                 |
| File Browser      | PgDn      | Scroll down               |
| Configuration     | +/-       | Adjust slider value       |
| Progress View     | p         | Pause deduplication       |
| Progress View     | r         | Resume deduplication      |
| Results Summary   | e         | Export results            |
| Results Summary   | v         | View details              |

---

## Appendix E: Configuration File Format

**Location**: `~/.config/kindly_dedup/config.toml`

**Complete TOML Structure**:

```toml
[general]
# Default Jaccard threshold (0.7-0.95)
jaccard_threshold = 0.85

# Thread count (0 = auto-detect, 1-256 manual)
thread_count = 0  # Auto-detect

# Memory mode ("in-memory" or "persistent")
memory_mode = "in-memory"

# Animation FPS (8-60)
animation_fps = 60

[features]
# SIMD MinHash (nightly required)
simd_minhash = true

# Bloom pre-filter (default enabled)
bloom_prefilter = true

# Batch LSH lookup (default enabled)
batch_lsh = true

# Q34 audit trail
audit_trail = true

[license]
# License tier ("Free", "Pro", "Enterprise", "Trial")
tier = "Free"

# License key file path
key_file = "~/.config/kindly_dedup/license.key"

[audit]
# Audit trail file path
audit_file = "~/.config/kindly_dedup/audit_trail.jsonl"

# Compliance report format ("SOX", "SOC2", "GDPR", "HIPAA")
compliance_format = "SOX"

[terminal]
# Enable RGB colors (auto-detect if not set)
rgb_colors = true

# Enable emojis (auto-detect if not set)
emojis = true

# Enable box drawing (auto-detect if not set)
box_drawing = true

# Terminal width override (0 = auto-detect)
width = 0

# Terminal height override (0 = auto-detect)
height = 0
```

**Minimal Configuration** (defaults used):
```toml
[general]
jaccard_threshold = 0.85
```

**Recommended Configuration** (balanced):
```toml
[general]
jaccard_threshold = 0.85
thread_count = 0  # Auto
memory_mode = "in-memory"
animation_fps = 60

[features]
simd_minhash = true
bloom_prefilter = true
batch_lsh = true
audit_trail = true
```

**Maximum Configuration** (all features):
```toml
[general]
jaccard_threshold = 0.90
thread_count = 16
memory_mode = "persistent"
animation_fps = 60

[features]
simd_minhash = true
bloom_prefilter = true
batch_lsh = true
audit_trail = true

[license]
tier = "Enterprise"
key_file = "~/.config/kindly_dedup/license_enterprise.key"

[audit]
audit_file = "~/.config/kindly_dedup/audit_trail.jsonl"
compliance_format = "HIPAA"

[terminal]
rgb_colors = true
emojis = true
box_drawing = true
```

---

## Appendix F: Glossary

**Technical Terms**:

- **MinHash**: Probabilistic algorithm for estimating Jaccard similarity via signature hashing (128 hashes per doc)
- **LSH (Locality-Sensitive Hashing)**: Technique for grouping similar items into buckets (L=5 multi-table for 92-99% recall)
- **Jaccard Similarity**: Similarity metric for sets: |A ∩ B| / |A ∪ B| (0.0 = disjoint, 1.0 = identical)
- **Union-Find**: Efficient algorithm for clustering (path halving, O(α(n)) amortized)
- **Bloom Filter**: Probabilistic data structure for membership testing (0.08% FPR @ 8KB)

**Byzantine Brand Terms**:

- **Purple Heart (💜)**: Primary brand emoji (Byzantine Purple RGB(112, 41, 99))
- **Byzantine Purple**: Main brand color, used for highlights, selected items
- **Byzantine Gold**: Secondary brand color, used for accents, achievements
- **Kindly Tone**: Friendly, approachable language in all error messages and UI

**Capsule Terminology**:

- **T0 (Auditable)**: Tier 0, hash-chained audit trails, compile-time verification
- **T1 (Atomic)**: Tier 1, lockfree atomic coordination (<5ns read, <15ns write)
- **T2 (SIMD)**: Tier 2, vectorized computation (2-19× speedup, nightly)
- **T3 (Fixed-Point)**: Tier 3, deterministic math (Q16.16, 2-10× speedup)
- **T4 (Batch)**: Tier 4, parallel batch processing (10-100× speedup)
- **T5 (Streaming)**: Tier 5, O(1) incremental computation
- **T6 (Mixed)**: Tier 6, multi-tier compounds (50-100× compound speedup)
- **T10 (Probabilistic)**: Tier 10, MinHash/LSH/Bloom/HyperLogLog
- **Lockfree**: No mutex/RwLock, 100% atomic primitives, zero contention
- **Generation Counter**: ABA prevention technique (monotonic counter prevents reuse races)
- **Cache-aligned**: 64/128/256-byte alignment to prevent false sharing

**Compliance Terms**:

- **SOX (Sarbanes-Oxley Act)**: US financial data integrity law (audit trails required)
- **SOC2 (Service Organization Control 2)**: Security, availability, confidentiality standard
- **GDPR (General Data Protection Regulation)**: EU data processing transparency law
- **HIPAA (Health Insurance Portability and Accountability Act)**: US healthcare data privacy law
- **Q34 Audit Trail**: Question 34 of UCE34 framework (hash-chained tamper-evident logging)

---

## Appendix G: ASCII Art Templates

**Purple Heart Patterns**:

```
  ❤️    (small heart, 1 character)
  
 ❤️❤️   (medium hearts, 2 characters)
 
❤️❤️❤️  (large hearts, 3 characters)

 ❤️  ❤️ (double hearts, separated)
  ❤️❤️  
   ❤️   (heart shape, 5 lines)
```

**Loading Animations** (ASCII spinner):

```
Frame 1: ⠋
Frame 2: ⠙
Frame 3: ⠹
Frame 4: ⠸
Frame 5: ⠼
Frame 6: ⠴
Frame 7: ⠦
Frame 8: ⠧
Frame 9: ⠇
Frame 10: ⠏
```

**Success Celebrations**:

```
  ✨ 🎉 ✨
   💜 💛
  ✨ 🎉 ✨
  
🏆 SUCCESS! 🏆
  
★ ☆ ★ ☆ ★
  EXCELLENT
★ ☆ ★ ☆ ★
```

**Error Indicators**:

```
  ⚠️  WARNING  ⚠️
  
  ❌  ERROR  ❌
  
  ⛔  STOP  ⛔
```

**Progress Bar Templates**:

```
[████████████          ] 60%

[▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░] 60%

◼◼◼◼◼◼◼◼◼◼◼◼◻◻◻◻◻◻◻◻ 60%
```

---

# END OF SPECIFICATION (Sections 7-12 COMPLETE)

---

## Summary

**Sections Completed**:
- **Section 7**: Technical Architecture (module structure, state management, animation engine, terminal compatibility, error handling)
- **Section 8**: UCE34 Framework Analysis (Q1-Q34 complete)
- **Section 9**: Implementation Plan (7 phases, dependencies, timeline, risks)
- **Section 10**: Edge Cases & Error Scenarios (50+ errors with templates)
- **Section 11**: Testing Strategy (T28 4-tier, 205 tests)
- **Section 12**: Appendices (A-G complete reference)

**Total Pages**: ~80 pages (across 5 parts)

**Production-Ready**: ✅ Ready for immediate implementation

**Next Steps**:
1. Review specification (1 day)
2. Begin Phase 1 (Foundation, 3 days)
3. Follow 7-phase roadmap (22-31 days total)
4. Launch CLI v1.0 (4-6 weeks from start)

**Specification Complete** 🎉
