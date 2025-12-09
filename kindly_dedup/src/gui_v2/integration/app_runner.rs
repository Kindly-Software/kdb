//! AppRunner - Main Application Entry Point
//!
//! # Overview
//!
//! Initializes and runs the GUI application. Owns:
//! - Window (winit, 900×1000, centered)
//! - GPU context (wgpu)
//! - Event queue (EventQueueCapsule)
//! - App capsule (KindlyDedupAppCapsule)
//! - Event loop (EventLoop)
//! - Render pipeline (RenderPipeline)
//!
//! # Architecture
//!
//! ```text
//! AppRunner::new()
//!   ├── Create window (900×1000, centered)
//!   ├── Initialize GPU (wgpu)
//!   ├── Create event queue (EventQueueCapsule)
//!   ├── Create app capsule (KindlyDedupAppCapsule)
//!   ├── Create event loop (EventLoop)
//!   └── Create render pipeline (RenderPipeline)
//!
//! AppRunner::run() [never returns]
//!   Loop:
//!     1. Process OS events → event queue
//!     2. EventLoop::process_events() → app.handle_event()
//!     3. RenderPipeline::render() → GPU
//!     4. Sleep until next frame (16.67ms = 60 FPS)
//! ```
//!
//! # Performance Targets (B32)
//!
//! - Frame time: <16.67ms (60 FPS target)
//! - Event processing: <1ms per frame (typically 5-10 events)
//! - Render time: <10ms (GPU accelerated)
//! - Idle CPU: <3% (sleep-based frame pacing)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T6 Mixed tier orchestrator
//! - **Chaos**: 100% lockfree coordination
//! - **ASSUM**: Window/GPU init is safe abstractions (verified by winit/wgpu)
//! - **B32**: Frame timing validated with criterion
//! - **T28**: Integration tests for initialization

use crate::gui_v2::state_machine::AppStateCapsule;
use crate::gui_v2::events::{GuiEvent, KeyCode, MouseButton, MouseEventKind};
use super::types::{EventQueueCapsule, GuiError, GuiResult};
use super::event_loop::EventLoop;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(feature = "gui-v2")]
use super::gpu_backend::GpuBackendCapsule;
#[cfg(feature = "gui-v2")]
use super::render::RenderPipeline;

#[cfg(feature = "gui-v2")]
use winit::{
    event::{Event, WindowEvent, ElementState, MouseButton as WinitMouseButton},
    event_loop::{EventLoop as WinitEventLoop, ControlFlow},
    window::Window,
    dpi::LogicalSize,
};

/// Main application runner
///
/// # Example
///
/// ```ignore
/// use kindly_dedup::gui_v2::integration::AppRunner;
///
/// // Create and run application (never returns)
/// let app = AppRunner::new()?;
/// app.run(); // Enters event loop
/// ```
pub struct AppRunner {
    /// Window title
    title: String,

    /// Window dimensions (width × height)
    size: (u32, u32),

    /// Event queue (lockfree SPSC)
    event_queue: Arc<EventQueueCapsule>,

    /// Application state
    app_state: Arc<AppStateCapsule>,

    /// Target frame duration (16.67ms = 60 FPS)
    frame_duration: Duration,

    /// Winit event loop (only with gui-v2 feature)
    #[cfg(feature = "gui-v2")]
    winit_event_loop: Option<WinitEventLoop<()>>,

    /// Winit window (only with gui-v2 feature)
    #[cfg(feature = "gui-v2")]
    window: Option<Window>,
}

impl AppRunner {
    /// Create new application runner
    ///
    /// # Initialization Steps
    ///
    /// 1. Create window (900×1000, centered on screen)
    /// 2. Initialize GPU context (wgpu, Vulkan/Metal/DX12)
    /// 3. Create event queue (EventQueueCapsule, 256 capacity)
    /// 4. Create app state (AppStateCapsule, Idle)
    /// 5. Create event loop (EventLoop)
    /// 6. Create render pipeline (RenderPipeline)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Window creation fails (OS resources exhausted)
    /// - GPU initialization fails (no compatible adapter)
    /// - Event queue allocation fails (out of memory)
    ///
    /// # Performance
    ///
    /// - Initialization: <100ms (GPU device creation is slow)
    /// - Memory: ~50MB (wgpu allocates GPU buffers)
    ///
    /// #ASSUME_WINDOW_CREATION: winit handles window creation safely
    /// #VERIFY: Test on Linux/Windows/macOS
    ///
    /// #ASSUME_GPU_INIT: wgpu finds compatible adapter or returns error
    /// #VERIFY: Test with no GPU (should fallback to software rendering)
    pub fn new() -> GuiResult<Self> {
        // #ASSUME: Event queue creation never fails (stack allocation)
        let event_queue = Arc::new(EventQueueCapsule::new());

        // #ASSUME: App state creation never fails (stack allocation, const new)
        let app_state = Arc::new(AppStateCapsule::new());

        #[cfg(feature = "gui-v2")]
        {
            // Create winit event loop
            let winit_event_loop = WinitEventLoop::new()
                .map_err(|_| GuiError::InitializationFailed)?;

            // Create window attributes
            let window_attributes = Window::default_attributes()
                .with_title("Kindly Dedup")
                .with_inner_size(LogicalSize::new(900, 1000))
                .with_resizable(true);

            // Build window
            let window = winit_event_loop.create_window(window_attributes)
                .map_err(|_| GuiError::InitializationFailed)?;

            Ok(Self {
                title: "Kindly Dedup".to_string(),
                size: (900, 1000),
                event_queue,
                app_state,
                frame_duration: Duration::from_micros(16_667), // 60 FPS
                winit_event_loop: Some(winit_event_loop),
                window: Some(window),
            })
        }

        #[cfg(not(feature = "gui-v2"))]
        Ok(Self {
            title: "kindly_dedup - Deduplication Tool".to_string(),
            size: (900, 1000),
            event_queue,
            app_state,
            frame_duration: Duration::from_micros(16_667), // 60 FPS
        })
    }

    /// Run the application (never returns)
    ///
    /// # Event Loop
    ///
    /// ```text
    /// Loop (60 FPS):
    ///   1. Poll OS events (winit) → event_queue
    ///   2. Process events: event_queue → app.handle_event()
    ///   3. Render frame: app_state → GPU
    ///   4. Frame pacing: sleep until 16.67ms elapsed
    /// ```
    ///
    /// # Performance
    ///
    /// - Frame time: <16.67ms (60 FPS target)
    /// - Event processing: <1ms (5-10 events per frame)
    /// - Render time: <10ms (GPU accelerated)
    /// - Sleep time: ~5-15ms (varies by workload)
    /// - Idle CPU: <3% (sleep-based pacing)
    ///
    /// #ASSUME_EVENT_LOOP_NEVER_EXITS: winit event loop runs until window closes
    /// #VERIFY: Test with Alt+F4, window close button
    ///
    /// #ASSUME_FRAME_PACING_ACCURATE: std::thread::sleep is accurate to ~1ms
    /// #VERIFY: Measure actual frame times with criterion
    #[allow(unreachable_code)]
    pub fn run(mut self) -> ! {
        #[cfg(feature = "gui-v2")]
        {
            let event_queue = self.event_queue.clone();
            let app_state = self.app_state.clone();
            let frame_duration = self.frame_duration;

            let winit_event_loop = self.winit_event_loop.take()
                .expect("Winit event loop not initialized");
            let window = self.window.take()
                .expect("Window not initialized");

            // Wrap window in Arc for GPU backend (required for wgpu surface creation)
            let window = Arc::new(window);

            // Initialize GPU backend (blocking on async)
            println!("Initializing GPU backend...");
            let gpu_backend = match pollster::block_on(GpuBackendCapsule::new(window.clone())) {
                Ok(backend) => {
                    println!("GPU initialized: {:?} backend", backend.backend());
                    Arc::new(backend)
                }
                Err(e) => {
                    eprintln!("GPU initialization failed: {:?}", e);
                    eprintln!("Exiting...");
                    std::process::exit(1);
                }
            };

            // Create render pipeline
            let mut render_pipeline = match RenderPipeline::new(app_state.clone(), gpu_backend.clone()) {
                Ok(pipeline) => {
                    println!("Render pipeline created");
                    pipeline
                }
                Err(e) => {
                    eprintln!("Render pipeline creation failed: {:?}", e);
                    std::process::exit(1);
                }
            };

            let mut event_loop_processor = EventLoop::new(event_queue.clone(), app_state.clone());
            let mut last_frame = Instant::now();

            println!("Starting render loop...");

            // Run winit event loop (never returns)
            winit_event_loop.run(move |event, event_loop_target| {
                event_loop_target.set_control_flow(ControlFlow::Poll);

                match event {
                    Event::WindowEvent { event, .. } => {
                        match event {
                            WindowEvent::CloseRequested => {
                                let _ = event_queue.push_event(GuiEvent::Close);
                                event_loop_target.exit();
                            }
                            WindowEvent::Resized(size) => {
                                // Handle resize for render pipeline
                                if size.width > 0 && size.height > 0 {
                                    if let Err(e) = render_pipeline.resize(size.width, size.height) {
                                        eprintln!("Resize failed: {:?}", e);
                                    }
                                }
                                let _ = event_queue.push_event(GuiEvent::Resize {
                                    width: size.width,
                                    height: size.height,
                                });
                            }
                            WindowEvent::KeyboardInput { event: key_event, .. } => {
                                if let Some(key_code) = Self::map_key_code(key_event.physical_key) {
                                    let pressed = key_event.state == ElementState::Pressed;
                                    let _ = event_queue.push_event(GuiEvent::Key {
                                        code: key_code,
                                        modifiers: 0, // TODO: Map modifiers
                                        pressed,
                                    });
                                }
                            }
                            WindowEvent::CursorMoved { position, .. } => {
                                let _ = event_queue.push_event(GuiEvent::Mouse {
                                    kind: MouseEventKind::Move,
                                    x: position.x as u16,
                                    y: position.y as u16,
                                    button: MouseButton::None,
                                });
                            }
                            WindowEvent::MouseInput { state, button, .. } => {
                                let gui_button = Self::map_mouse_button(button);
                                let kind = match state {
                                    ElementState::Pressed => MouseEventKind::Press,
                                    ElementState::Released => MouseEventKind::Release,
                                };
                                let _ = event_queue.push_event(GuiEvent::Mouse {
                                    kind,
                                    x: 0, // Position filled in by cursor move
                                    y: 0,
                                    button: gui_button,
                                });
                            }
                            WindowEvent::MouseWheel { delta, .. } => {
                                let delta_y = match delta {
                                    winit::event::MouseScrollDelta::LineDelta(_x, y) => (y * 100.0) as i16,
                                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as i16,
                                };
                                let _ = event_queue.push_event(GuiEvent::Mouse {
                                    kind: MouseEventKind::Scroll { delta_y },
                                    x: 0,
                                    y: 0,
                                    button: MouseButton::None,
                                });
                            }
                            WindowEvent::RedrawRequested => {
                                // Render frame using GPU pipeline
                                if let Err(e) = render_pipeline.begin_frame() {
                                    eprintln!("begin_frame failed: {:?}", e);
                                    return;
                                }

                                // Render UI layout
                                if let Err(e) = render_pipeline.render_layout() {
                                    eprintln!("render_layout failed: {:?}", e);
                                }

                                if let Err(e) = render_pipeline.end_frame() {
                                    eprintln!("end_frame failed: {:?}", e);
                                }

                                let _ = event_queue.push_event(GuiEvent::Redraw);
                            }
                            _ => {}
                        }
                    }
                    Event::AboutToWait => {
                        // Process events
                        event_loop_processor.process_events();

                        // Frame pacing
                        let elapsed = last_frame.elapsed();
                        if elapsed >= frame_duration {
                            window.request_redraw();
                            last_frame = Instant::now();
                        } else {
                            // Sleep until next frame
                            std::thread::sleep(frame_duration - elapsed);
                        }
                    }
                    _ => {}
                }
            }).expect("Event loop failed");
        }

        #[cfg(not(feature = "gui-v2"))]
        {
            let _frame_start = Instant::now();
            loop {
                let elapsed = _frame_start.elapsed();
                if elapsed < self.frame_duration {
                    std::thread::sleep(self.frame_duration - elapsed);
                }
                break;
            }
        }

        // Event loop never exits normally
        unreachable!("Event loop terminated unexpectedly")
    }

    /// Get window title
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Get window size
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Get frame duration (60 FPS = 16.67ms)
    pub fn frame_duration(&self) -> Duration {
        self.frame_duration
    }

    /// Map winit key code to GUI key code
    #[cfg(feature = "gui-v2")]
    fn map_key_code(key: winit::keyboard::PhysicalKey) -> Option<KeyCode> {
        use winit::keyboard::{PhysicalKey, KeyCode as WinitKeyCode};

        match key {
            PhysicalKey::Code(code) => match code {
                WinitKeyCode::Escape => Some(KeyCode::Escape),
                WinitKeyCode::Enter => Some(KeyCode::Enter),
                WinitKeyCode::Space => Some(KeyCode::Space),
                WinitKeyCode::Tab => Some(KeyCode::Tab),
                WinitKeyCode::Backspace => Some(KeyCode::Backspace),
                WinitKeyCode::Delete => Some(KeyCode::Delete),
                WinitKeyCode::ArrowLeft => Some(KeyCode::Left),
                WinitKeyCode::ArrowRight => Some(KeyCode::Right),
                WinitKeyCode::ArrowUp => Some(KeyCode::Up),
                WinitKeyCode::ArrowDown => Some(KeyCode::Down),
                _ => None,
            },
            _ => None,
        }
    }

    /// Map winit mouse button to GUI mouse button
    #[cfg(feature = "gui-v2")]
    fn map_mouse_button(button: WinitMouseButton) -> MouseButton {
        match button {
            WinitMouseButton::Left => MouseButton::Left,
            WinitMouseButton::Right => MouseButton::Right,
            WinitMouseButton::Middle => MouseButton::Middle,
            _ => MouseButton::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Requires main thread for winit event loop - run manually"]
    fn test_app_runner_creation() {
        let app = AppRunner::new().expect("Failed to create AppRunner");
        assert_eq!(app.title(), "kindly_dedup - Deduplication Tool");
        assert_eq!(app.size(), (900, 1000));
        assert_eq!(app.frame_duration(), Duration::from_micros(16_667));
    }

    #[test]
    #[ignore = "Requires main thread for winit event loop - run manually"]
    fn test_frame_duration_60fps() {
        let app = AppRunner::new().expect("Failed to create AppRunner");
        let fps_60 = Duration::from_secs(1) / 60;

        // Allow 1µs tolerance for rounding
        let diff = app.frame_duration().as_micros().abs_diff(fps_60.as_micros());
        assert!(diff <= 1, "Frame duration off by {}µs", diff);
    }

    #[test]
    #[ignore = "Requires main thread for winit event loop - run manually"]
    fn test_initial_state_idle() {
        use crate::gui_v2::state_machine::AppState;

        let app = AppRunner::new().expect("Failed to create AppRunner");
        assert_eq!(app.app_state.state(), AppState::Idle);
        assert_eq!(app.app_state.generation(), 0);
    }

    #[test]
    #[ignore = "Requires main thread for winit event loop - run manually"]
    fn test_event_queue_empty() {
        let app = AppRunner::new().expect("Failed to create AppRunner");
        assert!(app.event_queue.is_empty());
        assert_eq!(app.event_queue.len(), 0);
    }
}
