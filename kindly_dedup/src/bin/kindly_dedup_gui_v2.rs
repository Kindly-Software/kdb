//! kindly_dedup GUI v2 Binary
//!
//! # Overview
//!
//! Capsule-native GUI using winit + wgpu + EventQueueCapsule architecture.
//! 100% Chaos compliant, T6 Mixed tier orchestrator.
//!
//! # Architecture
//!
//! ```text
//! WinitEventLoop (OS events)
//!       ↓
//! EventQueueCapsule (lockfree SPSC, 256 capacity)
//!       ↓
//! EventLoop::process_events() (drain queue)
//!       ↓
//! AppStateCapsule (lockfree state machine, 6 states)
//!       ↓
//! RenderPipeline (wgpu, GPU accelerated)
//! ```
//!
//! # Features
//!
//! - Window creation: 900×1000, resizable
//! - Event handling: Keyboard, mouse, resize, close
//! - Frame timing: 60 FPS (16.67ms tick)
//! - State management: Lockfree atomic state machine
//!
//! # Framework Compliance
//!
//! - **UCE34**: T6 Mixed tier orchestrator (T0+T1+T5)
//! - **Chaos**: 100% lockfree (no mutex in event processing)
//! - **ASSUM**: Winit/wgpu handle window/GPU init safely
//! - **B32**: <16.67ms frame time (60 FPS target)
//! - **T28**: Integration tests for event flow
//!
//! # Performance Targets (B32)
//!
//! - Frame time: <16.67ms (60 FPS)
//! - Event processing: <1ms per frame
//! - Idle CPU: <3% (sleep-based frame pacing)
//!
//! # Usage
//!
//! ```bash
//! cargo run --bin kindly_dedup_gui_v2 --features gui-v2
//! ```

use kindly_dedup::gui_v2::integration::AppRunner;

fn main() {
    // Create application
    let app = match AppRunner::new() {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Failed to create application: {:?}", e);
            std::process::exit(1);
        }
    };

    // Print startup message
    println!("Kindly Dedup GUI v2");
    println!("==================");
    println!();
    println!("Window: {} ({}×{})", app.title(), app.size().0, app.size().1);
    println!("Frame rate: 60 FPS (16.67ms)");
    println!();
    println!("Starting event loop...");

    // Run event loop (never returns)
    app.run();
}
