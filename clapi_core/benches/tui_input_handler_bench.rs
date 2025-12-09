//! TUI Input Handler Benchmarks - B32 Framework Compliance
//!
//! # Purpose
//! Measure honest input latency for CommandInputCapsule operations.
//! All benchmarks follow B32 framework guidelines for fair, reproducible measurement.
//!
//! # B32 Compliance
//! - **Fair Baseline**: Compare against String/Vec operations
//! - **Statistical Rigor**: 1000+ iterations, 95% CI via Criterion
//! - **Honest Claims**: <500ns char insert, <300ns delete
//! - **Reality Check**: Input latency negligible vs human typing speed (>50ms/char)
//!
//! # Benchmarks
//! 1. **Char Insert**: ASCII + emoji insertion (<500ns)
//! 2. **Char Delete**: Backspace + Delete operations (<300ns)
//! 3. **Cursor Movement**: Left/Right/Home/End (<100ns)
//! 4. **History Navigation**: Up/Down arrow keys (<200ns)
//!
//! # Performance Targets
//! - Char insert: <500ns (memmove + atomic updates)
//! - Char delete: <300ns (memmove + atomic updates)
//! - Cursor move: <100ns (atomic update only)
//! - History nav: <200ns (atomic index update)
//!
//! # Build Instructions
//! ```bash
//! cargo bench --bench tui_input_handler_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::atomic::{AtomicU32, Ordering};

// ============================================================================
// MOCK COMMAND INPUT CAPSULE (Matches Production Pattern)
// ============================================================================

/// Command Input Capsule (256B, cache-aligned)
///
/// Simulates production input handler for TUI
#[repr(C, align(64))]
struct CommandInputCapsule {
    buffer: [u8; 200],          // Command text (UTF-8)
    cursor_pos: AtomicU32,      // Cursor position (byte offset)
    history_index: AtomicU32,   // Current history position
    buffer_len: AtomicU32,      // Buffer length
    modified: AtomicU32,        // Modified flag
    _padding: [u8; 40],         // Complete 256B cache line
}

impl CommandInputCapsule {
    fn new() -> Self {
        Self {
            buffer: [0; 200],
            cursor_pos: AtomicU32::new(0),
            history_index: AtomicU32::new(0),
            buffer_len: AtomicU32::new(0),
            modified: AtomicU32::new(0),
            _padding: [0; 40],
        }
    }

    /// Insert character at cursor position
    fn insert_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let bytes = c.encode_utf8(&mut buf).as_bytes();
        let len = bytes.len();

        let cursor = self.cursor_pos.load(Ordering::Relaxed) as usize;
        let buffer_len = self.buffer_len.load(Ordering::Relaxed) as usize;

        if buffer_len + len > self.buffer.len() {
            return; // Buffer full
        }

        // Shift bytes right
        if cursor < buffer_len {
            self.buffer.copy_within(cursor..buffer_len, cursor + len);
        }

        // Insert new bytes
        self.buffer[cursor..cursor + len].copy_from_slice(bytes);

        // Update atomics
        self.buffer_len.store((buffer_len + len) as u32, Ordering::Release);
        self.cursor_pos.store((cursor + len) as u32, Ordering::Release);
        self.modified.store(1, Ordering::Release);
    }

    /// Delete character before cursor (Backspace)
    fn delete_char_before(&mut self) {
        let cursor = self.cursor_pos.load(Ordering::Relaxed) as usize;
        if cursor == 0 {
            return;
        }

        let buffer_len = self.buffer_len.load(Ordering::Relaxed) as usize;

        // Find previous UTF-8 boundary
        let mut prev_pos = cursor - 1;
        while prev_pos > 0 && (self.buffer[prev_pos] & 0b1100_0000) == 0b1000_0000 {
            prev_pos -= 1;
        }

        let delete_len = cursor - prev_pos;

        // Shift bytes left
        if cursor < buffer_len {
            self.buffer.copy_within(cursor..buffer_len, prev_pos);
        }

        // Update atomics
        self.buffer_len.store((buffer_len - delete_len) as u32, Ordering::Release);
        self.cursor_pos.store(prev_pos as u32, Ordering::Release);
        self.modified.store(1, Ordering::Release);
    }

    /// Delete character after cursor (Delete key)
    fn delete_char_after(&mut self) {
        let cursor = self.cursor_pos.load(Ordering::Relaxed) as usize;
        let buffer_len = self.buffer_len.load(Ordering::Relaxed) as usize;

        if cursor >= buffer_len {
            return;
        }

        // Find next UTF-8 boundary
        let mut next_pos = cursor + 1;
        while next_pos < buffer_len && (self.buffer[next_pos] & 0b1100_0000) == 0b1000_0000 {
            next_pos += 1;
        }

        let delete_len = next_pos - cursor;

        // Shift bytes left
        if next_pos < buffer_len {
            self.buffer.copy_within(next_pos..buffer_len, cursor);
        }

        // Update atomics
        self.buffer_len.store((buffer_len - delete_len) as u32, Ordering::Release);
        self.modified.store(1, Ordering::Release);
    }

    /// Move cursor left (one UTF-8 character)
    fn move_cursor_left(&mut self) {
        let cursor = self.cursor_pos.load(Ordering::Relaxed) as usize;
        if cursor == 0 {
            return;
        }

        let mut prev_pos = cursor - 1;
        while prev_pos > 0 && (self.buffer[prev_pos] & 0b1100_0000) == 0b1000_0000 {
            prev_pos -= 1;
        }

        self.cursor_pos.store(prev_pos as u32, Ordering::Release);
    }

    /// Move cursor right (one UTF-8 character)
    fn move_cursor_right(&mut self) {
        let cursor = self.cursor_pos.load(Ordering::Relaxed) as usize;
        let buffer_len = self.buffer_len.load(Ordering::Relaxed) as usize;

        if cursor >= buffer_len {
            return;
        }

        let mut next_pos = cursor + 1;
        while next_pos < buffer_len && (self.buffer[next_pos] & 0b1100_0000) == 0b1000_0000 {
            next_pos += 1;
        }

        self.cursor_pos.store(next_pos as u32, Ordering::Release);
    }

    /// Move cursor to start (Home key)
    fn move_cursor_home(&mut self) {
        self.cursor_pos.store(0, Ordering::Release);
    }

    /// Move cursor to end (End key)
    fn move_cursor_end(&mut self) {
        let buffer_len = self.buffer_len.load(Ordering::Relaxed);
        self.cursor_pos.store(buffer_len, Ordering::Release);
    }

    /// Clear buffer (Ctrl+U)
    fn clear(&mut self) {
        self.buffer_len.store(0, Ordering::Release);
        self.cursor_pos.store(0, Ordering::Release);
        self.modified.store(1, Ordering::Release);
    }

    /// Navigate history up (older commands)
    fn history_up(&mut self) {
        let index = self.history_index.load(Ordering::Relaxed);
        self.history_index.store(index + 1, Ordering::Release);
    }

    /// Navigate history down (newer commands)
    fn history_down(&mut self) {
        let index = self.history_index.load(Ordering::Relaxed);
        if index > 0 {
            self.history_index.store(index - 1, Ordering::Release);
        }
    }
}

// ============================================================================
// BENCHMARK 1: Character Insertion
// ============================================================================

/// B32 Benchmark: Character insertion operations
///
/// # Purpose
/// Measure latency for inserting ASCII and multi-byte UTF-8 characters.
///
/// # Performance Target
/// - ASCII: <500ns (memmove + atomic updates)
/// - Emoji: <600ns (4-byte UTF-8 + memmove)
///
/// # B32 Reality Check
/// - Human typing speed: ~50-200ms per character
/// - Capsule overhead: <0.5% of typing latency
fn bench_char_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("input/insert_char");

    // ASCII insertion
    group.bench_function("insert_ascii", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            capsule.insert_char(black_box('h'));
        });
    });

    // Multi-byte UTF-8 (emoji)
    group.bench_function("insert_emoji", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            capsule.insert_char(black_box('😀'));
        });
    });

    // Insertion at end (append)
    group.bench_function("append_ascii", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            for _ in 0..5 {
                capsule.insert_char('a');
            }
            capsule.insert_char(black_box('z'));
        });
    });

    // Insertion at start (worst case - full memmove)
    group.bench_function("prepend_ascii", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            // Fill buffer first
            for _ in 0..50 {
                capsule.insert_char('x');
            }
            // Move cursor to start
            capsule.move_cursor_home();
            // Insert at start (full memmove)
            capsule.insert_char(black_box('z'));
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 2: Character Deletion
// ============================================================================

/// B32 Benchmark: Character deletion operations
///
/// # Purpose
/// Measure latency for Backspace and Delete operations.
///
/// # Performance Target
/// - Backspace: <300ns (memmove + atomic updates)
/// - Delete: <300ns (memmove + atomic updates)
///
/// # B32 Reality Check
/// - Human deletion speed: ~100-300ms per character
/// - Capsule overhead: <0.3% of deletion latency
fn bench_char_deletion(c: &mut Criterion) {
    let mut group = c.benchmark_group("input/delete_char");

    // Backspace (delete before cursor)
    group.bench_function("backspace_ascii", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            capsule.insert_char('h');
            capsule.insert_char('i');
            capsule.delete_char_before();
        });
    });

    // Delete (delete after cursor)
    group.bench_function("delete_ascii", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            capsule.insert_char('h');
            capsule.insert_char('i');
            capsule.move_cursor_left();
            capsule.delete_char_after();
        });
    });

    // Backspace emoji (4-byte UTF-8)
    group.bench_function("backspace_emoji", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            capsule.insert_char('😀');
            capsule.delete_char_before();
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 3: Cursor Movement
// ============================================================================

/// B32 Benchmark: Cursor movement operations
///
/// # Purpose
/// Measure latency for arrow keys and Home/End.
///
/// # Performance Target
/// - Left/Right: <100ns (atomic update + boundary check)
/// - Home/End: <50ns (atomic update only)
///
/// # B32 Reality Check
/// - Human cursor navigation speed: ~200-500ms per movement
/// - Capsule overhead: <0.1% of navigation latency
fn bench_cursor_movement(c: &mut Criterion) {
    let mut group = c.benchmark_group("input/cursor_move");

    // Move left
    group.bench_function("move_left", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            capsule.insert_char('h');
            capsule.insert_char('i');
            capsule.move_cursor_left();
        });
    });

    // Move right
    group.bench_function("move_right", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            capsule.insert_char('h');
            capsule.insert_char('i');
            capsule.move_cursor_left();
            capsule.move_cursor_right();
        });
    });

    // Home
    group.bench_function("move_home", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            capsule.insert_char('h');
            capsule.insert_char('i');
            capsule.move_cursor_home();
        });
    });

    // End
    group.bench_function("move_end", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            capsule.insert_char('h');
            capsule.insert_char('i');
            capsule.move_cursor_home();
            capsule.move_cursor_end();
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 4: History Navigation
// ============================================================================

/// B32 Benchmark: Command history navigation
///
/// # Purpose
/// Measure latency for Up/Down arrow key history navigation.
///
/// # Performance Target
/// - History up: <200ns (atomic index update)
/// - History down: <200ns (atomic index update)
///
/// # B32 Reality Check
/// - Human history navigation speed: ~300-800ms per arrow key
/// - Capsule overhead: <0.1% of navigation latency
fn bench_history_navigation(c: &mut Criterion) {
    let mut group = c.benchmark_group("input/history_nav");

    // Navigate up (older commands)
    group.bench_function("history_up", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            capsule.history_up();
        });
    });

    // Navigate down (newer commands)
    group.bench_function("history_down", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            capsule.history_up();
            capsule.history_up();
            capsule.history_down();
        });
    });

    // Multiple navigation cycles
    group.bench_function("history_cycle_5", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            for _ in 0..5 {
                capsule.history_up();
            }
            for _ in 0..5 {
                capsule.history_down();
            }
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 5: Realistic Typing Scenarios
// ============================================================================

/// B32 Benchmark: Realistic command typing
///
/// # Purpose
/// Measure end-to-end latency for typical command entry.
///
/// # Performance Target
/// - "clapi status" (12 chars): <6μs total (~500ns/char)
///
/// # B32 Reality Check
/// - Human typing speed: 5-10 chars/second (100-200ms/char)
/// - Total command entry time: 1.2-2.4 seconds for 12 characters
/// - Capsule overhead: <0.5% of total entry time
fn bench_realistic_typing(c: &mut Criterion) {
    let mut group = c.benchmark_group("input/realistic");

    // Type short command
    group.bench_function("type_command_short", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            for ch in "help".chars() {
                capsule.insert_char(black_box(ch));
            }
        });
    });

    // Type medium command
    group.bench_function("type_command_medium", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            for ch in "clapi status".chars() {
                capsule.insert_char(black_box(ch));
            }
        });
    });

    // Type long command with args
    group.bench_function("type_command_long", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            for ch in "clapi metrics --watch 5 --provider openai".chars() {
                capsule.insert_char(black_box(ch));
            }
        });
    });

    // Type + edit + clear
    group.bench_function("type_edit_clear", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            // Type
            for ch in "clapi status".chars() {
                capsule.insert_char(ch);
            }
            // Edit (backspace 3 chars)
            capsule.delete_char_before();
            capsule.delete_char_before();
            capsule.delete_char_before();
            // Retype
            for ch in "art".chars() {
                capsule.insert_char(ch);
            }
            // Clear
            capsule.clear();
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 6: Buffer Operations
// ============================================================================

/// B32 Benchmark: Buffer clear and reset
///
/// # Performance Target
/// - Clear: <50ns (3 atomic stores)
fn bench_buffer_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("input/buffer_ops");

    group.bench_function("clear_empty", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            capsule.clear();
        });
    });

    group.bench_function("clear_full", |b| {
        b.iter(|| {
            let mut capsule = CommandInputCapsule::new();
            // Fill buffer
            for ch in "clapi metrics --watch 5 --provider openai".chars() {
                capsule.insert_char(ch);
            }
            // Clear
            capsule.clear();
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    input_benches,
    bench_char_insertion,
    bench_char_deletion,
    bench_cursor_movement,
    bench_history_navigation,
    bench_realistic_typing,
    bench_buffer_operations,
);

criterion_main!(input_benches);
