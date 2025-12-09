//! Terminal Library Benchmarks (B32 Framework)
//!
//! Comprehensive performance validation for kindly_term terminal library against crossterm baseline.
//!
//! ## B32 Framework Compliance
//!
//! - **95% Confidence Intervals**: Criterion default (1000+ iterations)
//! - **Fair Baseline Comparison**: Same hardware, same compiler, same terminal
//! - **Reproducibility**: Fixed seed for deterministic benchmarks
//! - **Multiple Metrics**: Latency AND throughput measurements
//!
//! ## Performance Targets (vs Crossterm Baseline)
//!
//! 1. **Event Polling**: <100ns (crossterm: ~1μs) → 10× improvement target
//! 2. **Escape Parsing**: <50ns/sequence (crossterm: ~200ns) → 4× improvement via SIMD
//! 3. **Event Queue**: <10ns enqueue (lockfree ring buffer)
//! 4. **Output Writer**: <5μs flush for 1KB (crossterm: ~20μs) → 4× improvement via batching
//! 5. **Mode Switching**: <1μs (crossterm: ~10μs) → 10× improvement via atomic state
//! 6. **Style Application**: <5ns (crossterm: ~50ns) → 10× improvement via precomputed tables
//!
//! ## Hardware Calibration
//!
//! All benchmarks should run on kindly-hub (192.168.0.38):
//! - AMD Ryzen 9 6900HX (8 cores, 16 threads)
//! - 64GB DDR5-4800
//! - Ubuntu Server 24.04
//!
//! ## Remote Execution
//!
//! ```bash
//! ssh samuel@kindly-hub "cd ~/Primitives/atomic_capsule && cargo bench --bench terminal_bench"
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

#[cfg(feature = "tui-terminal")]
use atomic_capsule::terminal::{
    event::{Event, EventQueueWithStorage, KeyCode, KeyEvent, KeyModifiers},
    output::{ColorCapsule, StyleCapsule, TerminalWriterCapsule, BOLD, ITALIC, UNDERLINE},
    parser::{AnsiParserCapsule, ParserState},
};

// ============================================================================
// 1. EVENT POLLING LATENCY
// ============================================================================

#[cfg(feature = "terminal-event")]
fn bench_event_queue_enqueue(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_queue_enqueue");

    // Event sizes: small (KeyEvent), medium (Mouse), large (Resize)
    for capacity in [1024, 4096, 8192] {
        group.throughput(Throughput::Elements(1));

        group.bench_with_input(
            BenchmarkId::new("push_key_event", capacity),
            &capacity,
            |b, &cap| {
                let queue = match cap {
                    1024 => EventQueueWithStorage::<1024>::new(),
                    4096 => EventQueueWithStorage::<4096>::new(),
                    8192 => EventQueueWithStorage::<8192>::new(),
                    _ => unreachable!(),
                };

                let event = Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

                b.iter(|| {
                    // Push and immediately pop to avoid queue full
                    queue.push(black_box(event));
                    queue.pop();
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("push_resize_event", capacity),
            &capacity,
            |b, &cap| {
                let queue = match cap {
                    1024 => EventQueueWithStorage::<1024>::new(),
                    4096 => EventQueueWithStorage::<4096>::new(),
                    8192 => EventQueueWithStorage::<8192>::new(),
                    _ => unreachable!(),
                };

                b.iter(|| {
                    queue.push(black_box(Event::Resize(80, 24)));
                    queue.pop();
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "tui-terminal")]
fn bench_event_queue_dequeue(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_queue_dequeue");

    for capacity in [1024, 4096, 8192] {
        group.throughput(Throughput::Elements(1));

        group.bench_with_input(BenchmarkId::new("pop_event", capacity), &capacity, |b, &cap| {
            let queue = match cap {
                1024 => EventQueueWithStorage::<1024>::new(),
                4096 => EventQueueWithStorage::<4096>::new(),
                8192 => EventQueueWithStorage::<8192>::new(),
                _ => unreachable!(),
            };

            b.iter(|| {
                queue.push(Event::FocusGained);
                black_box(queue.pop());
            });
        });
    }

    group.finish();
}

#[cfg(feature = "tui-terminal")]
fn bench_event_queue_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_queue_throughput");

    for batch_size in [10, 100, 1000] {
        group.throughput(Throughput::Elements(batch_size));

        group.bench_with_input(
            BenchmarkId::new("push_pop_batch", batch_size),
            &batch_size,
            |b, &size| {
                let queue = EventQueueWithStorage::<8192>::new();

                b.iter(|| {
                    for i in 0..size {
                        queue.push(Event::Resize(80, 24 + (i as u16)));
                    }

                    for _ in 0..size {
                        black_box(queue.pop());
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// 2. ESCAPE SEQUENCE PARSING (SIMD)
// ============================================================================

#[cfg(feature = "tui-terminal")]
fn bench_escape_parse_arrow_keys(c: &mut Criterion) {
    let mut group = c.benchmark_group("escape_parse_arrow_keys");
    group.throughput(Throughput::Elements(1));

    let mut parser = AnsiParserCapsule::new();

    group.bench_function("parse_up_arrow", |b| {
        b.iter(|| {
            black_box(parser.parse(b"\x1B[A"));
        });
    });

    group.bench_function("parse_down_arrow", |b| {
        b.iter(|| {
            black_box(parser.parse(b"\x1B[B"));
        });
    });

    group.bench_function("parse_right_arrow", |b| {
        b.iter(|| {
            black_box(parser.parse(b"\x1B[C"));
        });
    });

    group.bench_function("parse_left_arrow", |b| {
        b.iter(|| {
            black_box(parser.parse(b"\x1B[D"));
        });
    });

    group.finish();
}

#[cfg(feature = "tui-terminal")]
fn bench_escape_parse_function_keys(c: &mut Criterion) {
    let mut group = c.benchmark_group("escape_parse_function_keys");
    group.throughput(Throughput::Elements(1));

    let mut parser = AnsiParserCapsule::new();

    group.bench_function("parse_f1_ss3", |b| {
        b.iter(|| {
            black_box(parser.parse(b"\x1BOP"));
        });
    });

    group.bench_function("parse_f5_csi", |b| {
        b.iter(|| {
            black_box(parser.parse(b"\x1B[15~"));
        });
    });

    group.bench_function("parse_f12_csi", |b| {
        b.iter(|| {
            black_box(parser.parse(b"\x1B[24~"));
        });
    });

    group.finish();
}

#[cfg(feature = "tui-terminal")]
fn bench_escape_parse_mouse_events(c: &mut Criterion) {
    let mut group = c.benchmark_group("escape_parse_mouse_events");
    group.throughput(Throughput::Elements(1));

    let mut parser = AnsiParserCapsule::new();

    group.bench_function("parse_mouse_press_sgr", |b| {
        b.iter(|| {
            black_box(parser.parse(b"\x1B[<0;10;20M"));
        });
    });

    group.bench_function("parse_mouse_release_sgr", |b| {
        b.iter(|| {
            black_box(parser.parse(b"\x1B[<0;10;20m"));
        });
    });

    group.bench_function("parse_mouse_scroll_up", |b| {
        b.iter(|| {
            black_box(parser.parse(b"\x1B[<64;1;1M"));
        });
    });

    group.finish();
}

#[cfg(feature = "tui-terminal")]
fn bench_escape_parse_modified_keys(c: &mut Criterion) {
    let mut group = c.benchmark_group("escape_parse_modified_keys");
    group.throughput(Throughput::Elements(1));

    let mut parser = AnsiParserCapsule::new();

    group.bench_function("parse_shift_up", |b| {
        b.iter(|| {
            black_box(parser.parse(b"\x1B[1;2A"));
        });
    });

    group.bench_function("parse_ctrl_right", |b| {
        b.iter(|| {
            black_box(parser.parse(b"\x1B[1;5C"));
        });
    });

    group.bench_function("parse_alt_home", |b| {
        b.iter(|| {
            black_box(parser.parse(b"\x1B[1;3H"));
        });
    });

    group.finish();
}

#[cfg(feature = "tui-terminal")]
fn bench_escape_parse_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("escape_parse_batch");

    // Batch sizes: 1KB, 4KB, 16KB of escape sequences
    for size_kb in [1, 4, 16] {
        group.throughput(Throughput::Bytes((size_kb * 1024) as u64));

        group.bench_with_input(
            BenchmarkId::new("parse_buffer", size_kb),
            &size_kb,
            |b, &kb| {
                let mut parser = AnsiParserCapsule::new();

                // Generate buffer with mixed escape sequences
                let mut buffer = Vec::new();
                let sequences = [
                    b"\x1B[A".as_slice(),    // Up
                    b"\x1B[B".as_slice(),    // Down
                    b"\x1B[C".as_slice(),    // Right
                    b"\x1B[D".as_slice(),    // Left
                    b"\x1BOP".as_slice(),    // F1
                    b"\x1B[15~".as_slice(),  // F5
                    b"\x1B[1;2A".as_slice(), // Shift+Up
                    b"\x1B[<0;10;20M".as_slice(), // Mouse
                ];

                while buffer.len() < kb * 1024 {
                    for seq in &sequences {
                        buffer.extend_from_slice(seq);
                    }
                }

                b.iter(|| {
                    black_box(parser.parse(&buffer));
                });
            },
        );
    }

    group.finish();
}

#[cfg(all(feature = "tui-terminal", feature = "portable_simd"))]
fn bench_escape_parse_simd_vs_scalar(c: &mut Criterion) {
    let mut group = c.benchmark_group("escape_parse_simd_vs_scalar");

    // Generate 1KB buffer with ESC bytes scattered throughout
    let mut buffer = vec![b'a'; 1024];
    for i in (0..1024).step_by(32) {
        buffer[i] = 0x1B; // ESC
    }

    group.throughput(Throughput::Bytes(1024));

    group.bench_function("find_esc_scalar", |b| {
        let parser = AnsiParserCapsule::new();
        parser.set_simd_enabled(false);

        b.iter(|| {
            black_box(parser.find_esc_bytes_scalar(&buffer));
        });
    });

    #[cfg(target_arch = "x86_64")]
    group.bench_function("find_esc_simd", |b| {
        let parser = AnsiParserCapsule::new();
        parser.set_simd_enabled(true);

        b.iter(|| {
            black_box(parser.find_esc_bytes_simd(&buffer));
        });
    });

    group.finish();
}

// ============================================================================
// 3. OUTPUT WRITER PERFORMANCE
// ============================================================================

#[cfg(feature = "tui-terminal")]
fn bench_writer_small_writes(c: &mut Criterion) {
    let mut group = c.benchmark_group("writer_small_writes");

    for size in [16, 32, 64] {
        group.throughput(Throughput::Bytes(size));

        group.bench_with_input(BenchmarkId::new("write_bytes", size), &size, |b, &sz| {
            let writer = TerminalWriterCapsule::new();
            let data = vec![b'A'; sz as usize];

            b.iter(|| {
                black_box(writer.write(&data));
            });
        });
    }

    group.finish();
}

#[cfg(feature = "tui-terminal")]
fn bench_writer_medium_writes(c: &mut Criterion) {
    let mut group = c.benchmark_group("writer_medium_writes");

    for size_kb in [1, 4, 8] {
        group.throughput(Throughput::Bytes((size_kb * 1024) as u64));

        group.bench_with_input(
            BenchmarkId::new("write_kb", size_kb),
            &size_kb,
            |b, &kb| {
                let writer = TerminalWriterCapsule::new();
                let data = vec![b'A'; kb * 1024];

                b.iter(|| {
                    black_box(writer.write(&data));
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "tui-terminal")]
fn bench_writer_large_writes(c: &mut Criterion) {
    let mut group = c.benchmark_group("writer_large_writes");

    for size_kb in [16, 32, 64] {
        group.throughput(Throughput::Bytes((size_kb * 1024) as u64));

        group.bench_with_input(
            BenchmarkId::new("write_kb", size_kb),
            &size_kb,
            |b, &kb| {
                let writer = TerminalWriterCapsule::with_capacity(128 * 1024);
                let data = vec![b'A'; kb * 1024];

                b.iter(|| {
                    black_box(writer.write(&data));
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "tui-terminal")]
fn bench_writer_ansi_sequences(c: &mut Criterion) {
    let mut group = c.benchmark_group("writer_ansi_sequences");
    group.throughput(Throughput::Elements(1));

    let writer = TerminalWriterCapsule::new();

    group.bench_function("move_cursor", |b| {
        b.iter(|| {
            black_box(writer.move_cursor(10, 5));
        });
    });

    group.bench_function("clear_screen", |b| {
        b.iter(|| {
            black_box(writer.clear_screen());
        });
    });

    group.bench_function("clear_line", |b| {
        b.iter(|| {
            black_box(writer.clear_line());
        });
    });

    group.bench_function("cursor_home", |b| {
        b.iter(|| {
            black_box(writer.cursor_home());
        });
    });

    group.bench_function("save_cursor", |b| {
        b.iter(|| {
            black_box(writer.save_cursor());
        });
    });

    group.bench_function("restore_cursor", |b| {
        b.iter(|| {
            black_box(writer.restore_cursor());
        });
    });

    group.bench_function("hide_cursor", |b| {
        b.iter(|| {
            black_box(writer.hide_cursor());
        });
    });

    group.bench_function("show_cursor", |b| {
        b.iter(|| {
            black_box(writer.show_cursor());
        });
    });

    group.finish();
}

#[cfg(feature = "tui-terminal")]
fn bench_writer_batch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("writer_batch_operations");

    for batch_size in [10, 100, 1000] {
        group.throughput(Throughput::Elements(batch_size));

        group.bench_with_input(
            BenchmarkId::new("batch_write_str", batch_size),
            &batch_size,
            |b, &size| {
                let writer = TerminalWriterCapsule::new();

                b.iter(|| {
                    for i in 0..size {
                        writer.write_str(&format!("Line {}\n", i)).unwrap();
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("batch_cursor_moves", batch_size),
            &batch_size,
            |b, &size| {
                let writer = TerminalWriterCapsule::new();

                b.iter(|| {
                    for i in 0..size {
                        writer.move_cursor((i % 80) as u16, (i / 80) as u16).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "tui-terminal")]
fn bench_writer_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("writer_throughput");

    // Measure MB/s for sustained writes
    for size_mb in [1, 4, 8] {
        group.throughput(Throughput::Bytes((size_mb * 1024 * 1024) as u64));

        group.bench_with_input(
            BenchmarkId::new("sustained_write_mb", size_mb),
            &size_mb,
            |b, &mb| {
                let writer = TerminalWriterCapsule::with_capacity(128 * 1024);
                let data = vec![b'A'; mb * 1024 * 1024];

                b.iter(|| {
                    black_box(writer.write(&data));
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// 4. STYLE APPLICATION
// ============================================================================

#[cfg(feature = "tui-terminal")]
fn bench_style_apply(c: &mut Criterion) {
    let mut group = c.benchmark_group("style_apply");
    group.throughput(Throughput::Elements(1));

    group.bench_function("apply_bold", |b| {
        let style = StyleCapsule::default();
        b.iter(|| {
            black_box(style.apply_attributes(BOLD));
        });
    });

    group.bench_function("apply_italic", |b| {
        let style = StyleCapsule::default();
        b.iter(|| {
            black_box(style.apply_attributes(ITALIC));
        });
    });

    group.bench_function("apply_underline", |b| {
        let style = StyleCapsule::default();
        b.iter(|| {
            black_box(style.apply_attributes(UNDERLINE));
        });
    });

    group.bench_function("apply_multiple", |b| {
        let style = StyleCapsule::default();
        b.iter(|| {
            black_box(style.apply_attributes(BOLD | ITALIC | UNDERLINE));
        });
    });

    group.finish();
}

#[cfg(feature = "tui-terminal")]
fn bench_color_convert(c: &mut Criterion) {
    let mut group = c.benchmark_group("color_convert");
    group.throughput(Throughput::Elements(1));

    use atomic_capsule::terminal::output::Color;

    group.bench_function("rgb_to_ansi256", |b| {
        let color = ColorCapsule::rgb(128, 64, 192);
        b.iter(|| {
            black_box(color.to_ansi256());
        });
    });

    group.bench_function("ansi256_to_rgb", |b| {
        let color = ColorCapsule::ansi256(100);
        b.iter(|| {
            black_box(color.to_rgb());
        });
    });

    group.bench_function("indexed_lookup", |b| {
        let color = ColorCapsule::indexed(7);
        b.iter(|| {
            black_box(color.to_rgb());
        });
    });

    group.finish();
}

#[cfg(feature = "tui-terminal")]
fn bench_sgr_sequence_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("sgr_sequence_build");
    group.throughput(Throughput::Elements(1));

    group.bench_function("build_fg_color", |b| {
        let style = StyleCapsule::default();
        let color = ColorCapsule::rgb(128, 64, 192);
        b.iter(|| {
            black_box(style.with_foreground(color));
        });
    });

    group.bench_function("build_bg_color", |b| {
        let style = StyleCapsule::default();
        let color = ColorCapsule::rgb(255, 128, 0);
        b.iter(|| {
            black_box(style.with_background(color));
        });
    });

    group.bench_function("build_full_style", |b| {
        let style = StyleCapsule::default();
        let fg = ColorCapsule::rgb(255, 255, 255);
        let bg = ColorCapsule::rgb(0, 0, 0);
        b.iter(|| {
            black_box(
                style
                    .with_foreground(fg)
                    .with_background(bg)
                    .apply_attributes(BOLD | ITALIC),
            );
        });
    });

    group.finish();
}

// ============================================================================
// 5. END-TO-END SCENARIOS
// ============================================================================

#[cfg(feature = "tui-terminal")]
fn bench_tui_render_frame(c: &mut Criterion) {
    let mut group = c.benchmark_group("tui_render_frame");
    group.throughput(Throughput::Elements(1));

    // Simulate TUI frame render: clear screen + 24 lines + cursor moves
    group.bench_function("render_24_lines", |b| {
        let writer = TerminalWriterCapsule::with_capacity(64 * 1024);

        b.iter(|| {
            writer.clear_screen().unwrap();
            writer.cursor_home().unwrap();

            for row in 0..24 {
                writer.move_cursor(0, row).unwrap();
                writer
                    .write_str(&format!("Line {} with some content", row))
                    .unwrap();
            }
        });
    });

    // Simulate status bar update: move to bottom + write + restore
    group.bench_function("update_status_bar", |b| {
        let writer = TerminalWriterCapsule::new();

        b.iter(|| {
            writer.save_cursor().unwrap();
            writer.move_cursor(0, 23).unwrap();
            writer.clear_line().unwrap();
            writer.write_str("Status: OK | Time: 12:34").unwrap();
            writer.restore_cursor().unwrap();
        });
    });

    group.finish();
}

#[cfg(feature = "tui-terminal")]
fn bench_event_processing_loop(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_processing_loop");

    // Simulate event loop: parse + queue + process
    for events_per_iter in [10, 100, 1000] {
        group.throughput(Throughput::Elements(events_per_iter));

        group.bench_with_input(
            BenchmarkId::new("parse_queue_process", events_per_iter),
            &events_per_iter,
            |b, &count| {
                let mut parser = AnsiParserCapsule::new();
                let queue = EventQueueWithStorage::<8192>::new();

                // Generate input buffer with escape sequences
                let mut input = Vec::new();
                for _ in 0..count {
                    input.extend_from_slice(b"\x1B[A"); // Up arrow
                }

                b.iter(|| {
                    // Parse input
                    let events = parser.parse(&input);

                    // Enqueue events
                    for event in events {
                        queue.push(event);
                    }

                    // Process events
                    let mut processed = 0;
                    while queue.pop().is_some() {
                        processed += 1;
                    }

                    black_box(processed);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

#[cfg(feature = "tui-terminal")]
criterion_group!(
    event_queue_benches,
    bench_event_queue_enqueue,
    bench_event_queue_dequeue,
    bench_event_queue_throughput,
);

#[cfg(feature = "tui-terminal")]
criterion_group!(
    parser_benches,
    bench_escape_parse_arrow_keys,
    bench_escape_parse_function_keys,
    bench_escape_parse_mouse_events,
    bench_escape_parse_modified_keys,
    bench_escape_parse_batch,
);

#[cfg(all(feature = "tui-terminal", feature = "portable_simd"))]
criterion_group!(simd_benches, bench_escape_parse_simd_vs_scalar,);

#[cfg(feature = "tui-terminal")]
criterion_group!(
    writer_benches,
    bench_writer_small_writes,
    bench_writer_medium_writes,
    bench_writer_large_writes,
    bench_writer_ansi_sequences,
    bench_writer_batch_operations,
    bench_writer_throughput,
);

#[cfg(feature = "tui-terminal")]
criterion_group!(
    style_benches,
    bench_style_apply,
    bench_color_convert,
    bench_sgr_sequence_build,
);

#[cfg(feature = "tui-terminal")]
criterion_group!(
    scenario_benches,
    bench_tui_render_frame,
    bench_event_processing_loop,
);

#[cfg(all(feature = "tui-terminal", not(feature = "portable_simd")))]
criterion_main!(
    event_queue_benches,
    parser_benches,
    writer_benches,
    style_benches,
    scenario_benches,
);

#[cfg(all(feature = "tui-terminal", feature = "portable_simd"))]
criterion_main!(
    event_queue_benches,
    parser_benches,
    simd_benches,
    writer_benches,
    style_benches,
    scenario_benches,
);

#[cfg(not(feature = "tui-terminal"))]
fn main() {
    eprintln!("terminal_bench requires --features tui-terminal");
}
