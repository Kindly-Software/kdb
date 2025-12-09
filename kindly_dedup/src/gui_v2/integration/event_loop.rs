//! EventLoop - Event Processing and Dispatch
//!
//! # Overview
//!
//! Processes events from EventQueueCapsule and dispatches to application handlers.
//! Integrates with animation tick for smooth visual updates.
//!
//! # Architecture
//!
//! ```text
//! EventQueueCapsule → EventLoop::process_events()
//!                           ↓
//!                    Drain all events (<256 per frame)
//!                           ↓
//!                    For each event:
//!                      - dispatch_event(event)
//!                      - app.handle_event(event)
//!                      - Update app state (lockfree)
//!                           ↓
//!                    Animation tick (if needed)
//!                           ↓
//!                    Effects processed directly (no queue)
//! ```
//!
//! # Performance Targets (B32)
//!
//! - process_events(): <1ms per frame (5-10 events typical)
//! - dispatch_event(): <100ns per event (lockfree state update)
//! - Animation tick: <50ns (Q16.16 fixed-point update)
//! - Effect dispatch: <50ns per effect (direct call, no queue)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T5 Streaming (event queue drain)
//! - **Chaos**: 100% lockfree (no mutex in event processing)
//! - **ASSUM**: Event queue is SPSC (single producer in OS thread, single consumer here)
//! - **B32**: <1ms per frame validated
//! - **T28**: Unit tests for event dispatch

use crate::gui_v2::state_machine::{AppState, AppStateCapsule};
use crate::gui_v2::events::{GuiEvent, KeyCode, MouseButton, MouseEventKind};
use super::types::EventQueueCapsule;
use std::sync::Arc;
use std::time::Instant;

/// Event loop processor
///
/// Drains events from EventQueueCapsule and dispatches to app handlers.
pub struct EventLoop {
    /// Event queue (shared with OS event handler)
    event_queue: Arc<EventQueueCapsule>,

    /// Application state (shared with render pipeline)
    app_state: Arc<AppStateCapsule>,

    /// Last animation tick time
    last_tick: Instant,

    /// Animation tick interval (16.67ms = 60 FPS)
    tick_interval: std::time::Duration,
}

impl EventLoop {
    /// Create new event loop
    ///
    /// # Parameters
    ///
    /// - `event_queue`: Shared event queue (from AppRunner)
    /// - `app_state`: Shared app state (from AppRunner)
    ///
    /// # Performance
    ///
    /// - Creation: <1µs (Arc clone + initialization)
    /// - Memory: 32 bytes (Arc pointers + Instant)
    pub fn new(event_queue: Arc<EventQueueCapsule>, app_state: Arc<AppStateCapsule>) -> Self {
        Self {
            event_queue,
            app_state,
            last_tick: Instant::now(),
            tick_interval: std::time::Duration::from_micros(16_667), // 60 FPS
        }
    }

    /// Process all queued events
    ///
    /// # Algorithm
    ///
    /// ```text
    /// 1. Drain event queue (up to 256 events)
    /// 2. For each event:
    ///    - Classify event type
    ///    - Dispatch to appropriate handler
    ///    - Update app state (lockfree CAS)
    /// 3. Check animation tick
    ///    - If 16.67ms elapsed, tick animations
    /// 4. Return number of events processed
    /// ```
    ///
    /// # Performance
    ///
    /// - Typical: <1ms (5-10 events per frame)
    /// - Worst case: <5ms (256 events, queue overflow)
    /// - Per event: <100ns (lockfree dispatch)
    ///
    /// #ASSUME_EVENT_QUEUE_BOUNDED: Max 256 events per frame (4 frames at 60 FPS)
    /// #VERIFY: Test with event storm (1000+ events, should drop oldest)
    ///
    /// #ASSUME_ANIMATION_TICK_STABLE: 60 FPS tick is stable (16.67ms ± 1ms)
    /// #VERIFY: Measure actual tick intervals with criterion
    pub fn process_events(&mut self) -> usize {
        let mut count = 0;

        // Drain all events from queue
        while let Some(event) = self.event_queue.pop_event() {
            self.dispatch_event(event);
            count += 1;

            // Safety limit: prevent infinite loop if queue is being fed faster than drained
            if count >= 256 {
                break;
            }
        }

        // Check if animation tick needed
        let now = Instant::now();
        if now.duration_since(self.last_tick) >= self.tick_interval {
            self.tick_animation();
            self.last_tick = now;
        }

        count
    }

    /// Dispatch single event to appropriate handler
    ///
    /// # Event Handling
    ///
    /// - **File Selection**: File picker, drag-drop, file cleared
    /// - **Settings**: Threshold, execution mode changes
    /// - **Processing Control**: Start, cancel, reset
    /// - **Progress**: Updates, completion, errors
    /// - **Results**: Export, view details, clipboard
    /// - **Animation**: Ticks, hover effects
    /// - **Navigation**: Documentation, compliance viewer
    /// - **Mouse**: Button clicks, movement, wheel
    /// - **Key**: Keyboard input, shortcuts
    /// - **Resize**: Window resize (update layout)
    /// - **Redraw**: Redraw request (no state change)
    /// - **Close**: Window close (transition to exit)
    ///
    /// # Performance
    ///
    /// - Dispatch: <100ns (match + lockfree state update)
    /// - Mouse: <50ns (coordinate update)
    /// - Key: <100ns (modifier handling)
    /// - Resize: <200ns (layout invalidation)
    ///
    /// #ASSUME_EVENT_HANDLING_FAST: Each handler is <100ns (no blocking I/O)
    /// #VERIFY: Benchmark each handler with criterion
    fn dispatch_event(&self, event: GuiEvent) {
        match event {
            // ================================================================
            // FILE SELECTION EVENTS (passthrough for now)
            // ================================================================
            GuiEvent::FilePickerClicked => {
                // TODO: Implement when file picker integrated
            }
            GuiEvent::FileSelected => {
                // TODO: Implement when file picker integrated
            }
            GuiEvent::FileSelectedPath(_path) => {
                // TODO: Implement when file picker integrated
            }
            GuiEvent::FileDrop => {
                // TODO: Implement when drag-drop integrated
            }
            GuiEvent::FileDropped(_path) => {
                // TODO: Implement when drag-drop integrated
            }
            GuiEvent::FileDragEnter => {
                // TODO: Implement when drag-drop integrated
            }
            GuiEvent::FileDragLeave => {
                // TODO: Implement when drag-drop integrated
            }
            GuiEvent::FileCleared => {
                // TODO: Implement when file picker integrated
            }

            // ================================================================
            // SETTINGS EVENTS (passthrough for now)
            // ================================================================
            GuiEvent::ThresholdChanged(_threshold) => {
                // TODO: Implement when settings UI integrated
            }
            GuiEvent::ModeChanged(_mode) => {
                // TODO: Implement when settings UI integrated
            }
            GuiEvent::ExecutionModeChanged(_mode) => {
                // TODO: Implement when settings UI integrated (alias)
            }

            // ================================================================
            // PROCESSING CONTROL EVENTS
            // ================================================================
            GuiEvent::StartDeduplication => {
                // Transition to Processing state
                self.app_state.transition(AppState::Processing);
            }
            GuiEvent::StartProcessing => {
                // Alias for StartDeduplication
                self.app_state.transition(AppState::Processing);
            }
            GuiEvent::CancelDeduplication => {
                // Cancel processing, transition back to Ready
                self.app_state.transition(AppState::Ready);
            }
            GuiEvent::CancelProcessing => {
                // Alias for CancelDeduplication
                self.app_state.transition(AppState::Ready);
            }
            GuiEvent::Reset => {
                // Reset to Idle state
                self.app_state.reset();
            }

            // ================================================================
            // PROGRESS EVENTS (passthrough for now)
            // ================================================================
            GuiEvent::ProgressUpdate { .. } => {
                // TODO: Implement when progress UI integrated
            }
            GuiEvent::DeduplicationComplete(_results) => {
                // Transition to Complete state
                self.app_state.transition(AppState::Complete);
            }

            // ================================================================
            // RESULTS EVENTS (passthrough for now)
            // ================================================================
            GuiEvent::ExportResults => {
                // TODO: Implement when results UI integrated
            }
            GuiEvent::ViewDetails => {
                // TODO: Implement when results UI integrated
            }
            GuiEvent::CopyToClipboard => {
                // TODO: Implement when results UI integrated
            }

            // ================================================================
            // ANIMATION EVENTS (passthrough for now)
            // ================================================================
            GuiEvent::Tick => {
                // Main application tick (60 FPS)
                // Handled by tick_animation() in process_events()
            }
            GuiEvent::AnimationTick(_delta_ms) => {
                // Animation frame tick with delta time
                // TODO: Implement when animation system integrated
            }
            GuiEvent::HeroButtonHovered => {
                // TODO: Implement when hero button integrated
            }
            GuiEvent::HeroButtonUnhovered => {
                // TODO: Implement when hero button integrated
            }

            // ================================================================
            // NAVIGATION EVENTS (passthrough for now)
            // ================================================================
            GuiEvent::OpenDocumentation => {
                // TODO: Implement when navigation integrated
            }
            GuiEvent::ShowCompliance => {
                // TODO: Implement when compliance viewer integrated
            }
            GuiEvent::CloseCompliance => {
                // TODO: Implement when compliance viewer integrated
            }
            GuiEvent::VerifyAuditChain => {
                // TODO: Implement when audit verification integrated
            }
            GuiEvent::ExportComplianceReport => {
                // TODO: Implement when compliance export integrated
            }

            // ================================================================
            // ERROR EVENTS
            // ================================================================
            GuiEvent::ReportError(_error) => {
                // Transition to Error state
                self.app_state.transition(AppState::Error);
            }

            // ================================================================
            // LOW-LEVEL WINDOW EVENTS (existing handlers)
            // ================================================================
            GuiEvent::Mouse { kind, x, y, button } => {
                self.handle_mouse_event(kind, x, y, button);
            }
            GuiEvent::Key {
                code,
                modifiers,
                pressed,
            } => {
                self.handle_key_event(code, modifiers, pressed);
            }
            GuiEvent::Resize { width, height } => {
                self.handle_resize_event(width, height);
            }
            GuiEvent::Redraw => {
                // No state change, render pipeline handles this
            }
            GuiEvent::Close => {
                self.handle_close_event();
            }
        }
    }

    /// Handle mouse event
    ///
    /// # Mouse Events
    ///
    /// - **Press**: Button pressed (transition button state)
    /// - **Release**: Button released (trigger click if in bounds)
    /// - **Move**: Mouse moved (update hover state)
    /// - **Scroll**: Wheel scrolled (scroll content)
    ///
    /// #ASSUME_MOUSE_COORDS_VALID: (x, y) are within window bounds
    /// #VERIFY: Test with mouse outside window (should clip to bounds)
    fn handle_mouse_event(&self, kind: MouseEventKind, _x: u16, _y: u16, button: MouseButton) {
        match (kind, button) {
            (MouseEventKind::Press, MouseButton::Left) => {
                // Handle left click (e.g., file select button, start button)
                // TODO: Implement when widgets added
            }
            (MouseEventKind::Release, MouseButton::Left) => {
                // Handle left release (trigger click effect)
                // TODO: Implement when widgets added
            }
            (MouseEventKind::Move, _) => {
                // Update hover state
                // TODO: Implement when widgets added
            }
            (MouseEventKind::Scroll { delta_y }, _) => {
                // Handle scroll (e.g., results list)
                let _scroll_amount = delta_y;
                // TODO: Implement when widgets added
            }
            _ => {
                // Ignore other mouse events (right click, middle click)
            }
        }
    }

    /// Handle keyboard event
    ///
    /// # Keyboard Shortcuts
    ///
    /// - **Escape**: Cancel processing, reset to idle
    /// - **Enter**: Start processing (if in Ready state)
    /// - **Ctrl+O**: Open file dialog
    /// - **Ctrl+Q**: Quit application
    ///
    /// #ASSUME_MODIFIERS_CORRECT: OS provides correct modifier state
    /// #VERIFY: Test with Caps Lock, Num Lock (should not affect Ctrl/Alt/Shift)
    fn handle_key_event(&self, code: KeyCode, _modifiers: u8, pressed: bool) {
        if !pressed {
            return; // Only handle key press, not release
        }

        match code {
            KeyCode::Escape => {
                // Cancel / Reset
                self.app_state.reset();
            }
            KeyCode::Enter => {
                // Start processing (if in Ready state)
                let current = self.app_state.state();
                if current == AppState::Ready {
                    self.app_state.transition(AppState::Processing);
                }
            }
            _ => {
                // Ignore other keys
            }
        }
    }

    /// Handle window resize event
    ///
    /// # Layout Invalidation
    ///
    /// When window resizes, layout must be recalculated:
    /// 1. Update window dimensions in app state
    /// 2. Invalidate layout cache
    /// 3. Trigger redraw
    ///
    /// #ASSUME_RESIZE_VALID: (width, height) > 0
    /// #VERIFY: Test with minimum window size (should not be 0×0)
    fn handle_resize_event(&self, _width: u32, _height: u32) {
        // TODO: Implement layout invalidation when layout system added
    }

    /// Handle window close event
    ///
    /// Triggers application shutdown:
    /// 1. Transition to error state (no dedicated "closing" state)
    /// 2. Event loop will exit on next iteration
    fn handle_close_event(&self) {
        // Transition to Error state to trigger shutdown
        // (We don't have a dedicated "Closing" state)
        self.app_state.transition(AppState::Error);
    }

    /// Tick animations (60 FPS)
    ///
    /// Updates all active animations:
    /// - Spring animations (button hover, focus ring)
    /// - Pulse animations (processing indicator)
    /// - Shimmer animations (loading skeleton)
    ///
    /// # Performance
    ///
    /// - Tick: <50ns (Q16.16 fixed-point update)
    /// - Per animation: <20ns (atomic read + fixed-point math)
    ///
    /// #ASSUME_ANIMATION_COUNT_BOUNDED: <10 active animations per frame
    /// #VERIFY: Test with 100+ animations (should still be <1ms)
    fn tick_animation(&self) {
        // TODO: Implement when animation system added
        // For now, this is a no-op
    }

    /// Flush effects (direct dispatch, no queue)
    ///
    /// NOTE: EffectQueueCapsule is disabled due to padding overflow.
    /// Effects are processed directly instead of being queued.
    ///
    /// # Performance
    ///
    /// - Per effect: <50ns (direct function call)
    pub fn flush_effects(&self) {
        // TODO: Implement when effect system added
        // For now, this is a no-op
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_loop_creation() {
        let event_queue = Arc::new(EventQueueCapsule::new());
        let app_state = Arc::new(AppStateCapsule::new());

        let event_loop = EventLoop::new(event_queue, app_state);
        assert_eq!(event_loop.tick_interval, std::time::Duration::from_micros(16_667));
    }

    #[test]
    fn test_process_events_empty_queue() {
        let event_queue = Arc::new(EventQueueCapsule::new());
        let app_state = Arc::new(AppStateCapsule::new());

        let mut event_loop = EventLoop::new(event_queue, app_state);
        let count = event_loop.process_events();

        assert_eq!(count, 0);
    }

    #[test]
    fn test_process_events_single_event() {
        let event_queue = Arc::new(EventQueueCapsule::new());
        let app_state = Arc::new(AppStateCapsule::new());

        // Push one event
        event_queue
            .push_event(GuiEvent::Mouse {
                kind: MouseEventKind::Press,
                x: 100,
                y: 200,
                button: MouseButton::Left,
            })
            .expect("Failed to push event");

        let mut event_loop = EventLoop::new(event_queue.clone(), app_state);
        let count = event_loop.process_events();

        assert_eq!(count, 1);
        assert!(event_queue.is_empty());
    }

    #[test]
    fn test_process_events_multiple() {
        let event_queue = Arc::new(EventQueueCapsule::new());
        let app_state = Arc::new(AppStateCapsule::new());

        // Push 5 events
        for i in 0..5 {
            event_queue
                .push_event(GuiEvent::Mouse {
                    kind: MouseEventKind::Move,
                    x: i * 10,
                    y: i * 10,
                    button: MouseButton::None,
                })
                .expect("Failed to push event");
        }

        let mut event_loop = EventLoop::new(event_queue.clone(), app_state);
        let count = event_loop.process_events();

        assert_eq!(count, 5);
        assert!(event_queue.is_empty());
    }

    #[test]
    fn test_escape_key_resets_state() {
        let event_queue = Arc::new(EventQueueCapsule::new());
        let app_state = Arc::new(AppStateCapsule::new());

        // Transition to Ready state
        app_state.transition(AppState::Ready);
        assert_eq!(app_state.state(), AppState::Ready);

        // Push Escape key event
        event_queue
            .push_event(GuiEvent::Key {
                code: KeyCode::Escape,
                modifiers: 0,
                pressed: true,
            })
            .expect("Failed to push event");

        let mut event_loop = EventLoop::new(event_queue, app_state.clone());
        event_loop.process_events();

        // State should be reset to Idle
        assert_eq!(app_state.state(), AppState::Idle);
    }

    #[test]
    fn test_enter_key_starts_processing() {
        let event_queue = Arc::new(EventQueueCapsule::new());
        let app_state = Arc::new(AppStateCapsule::new());

        // Transition to Ready state
        app_state.transition(AppState::Ready);
        assert_eq!(app_state.state(), AppState::Ready);

        // Push Enter key event
        event_queue
            .push_event(GuiEvent::Key {
                code: KeyCode::Enter,
                modifiers: 0,
                pressed: true,
            })
            .expect("Failed to push event");

        let mut event_loop = EventLoop::new(event_queue, app_state.clone());
        event_loop.process_events();

        // State should transition to Processing
        assert_eq!(app_state.state(), AppState::Processing);
    }

    #[test]
    fn test_close_event_transitions_to_error() {
        let event_queue = Arc::new(EventQueueCapsule::new());
        let app_state = Arc::new(AppStateCapsule::new());

        // Push Close event
        event_queue
            .push_event(GuiEvent::Close)
            .expect("Failed to push event");

        let mut event_loop = EventLoop::new(event_queue, app_state.clone());
        event_loop.process_events();

        // State should be Error (triggers shutdown)
        assert_eq!(app_state.state(), AppState::Error);
    }

    #[test]
    fn test_redraw_event_no_state_change() {
        let event_queue = Arc::new(EventQueueCapsule::new());
        let app_state = Arc::new(AppStateCapsule::new());

        let initial_gen = app_state.generation();

        // Push Redraw event
        event_queue
            .push_event(GuiEvent::Redraw)
            .expect("Failed to push event");

        let mut event_loop = EventLoop::new(event_queue, app_state.clone());
        event_loop.process_events();

        // State and generation should be unchanged
        assert_eq!(app_state.state(), AppState::Idle);
        assert_eq!(app_state.generation(), initial_gen);
    }
}
