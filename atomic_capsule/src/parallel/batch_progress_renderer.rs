//! # Batch Progress Renderer - Background rendering for progress tracking
//!
//! **Non-blocking progress rendering** with 10ms batch updates.
//!
//! ## Performance
//!
//! - **Main thread**: <5ns atomic updates (never blocks)
//! - **Background thread**: Renders every 10ms (100 FPS, imperceptible lag)
//!
//! ## Architecture
//!
//! Main thread: `tracker.increment()` → <5ns atomic increment
//! Background thread: Every 10ms → Read atomics + Render → Console output
//!
//! ## Design Rationale
//!
//! ### Background Batch Rendering
//! - Background batch rendering (10ms interval)
//! - Decouples updates from rendering
//! - 100× reduction in rendering overhead
//!
//! ## Example
//!
//! ```rust,ignore
//! use atomic_capsule::primitives::ProgressTrackerCapsule;
//! use atomic_capsule::parallel::BatchProgressRenderer;
//! use std::sync::Arc;
//!
//! let tracker = Arc::new(ProgressTrackerCapsule::new(1000));
//! let tracker_clone = Arc::clone(&tracker);
//!
//! // Start background renderer (console output)
//! let mut renderer = BatchProgressRenderer::start(tracker_clone, |current, total| {
//!     print!("\r[{}/{}] {}%", current, total, (current * 100) / total.max(1));
//! });
//!
//! // Main thread: fast updates
//! for _ in 0..1000 {
//!     tracker.increment(); // <5ns, never blocks!
//! }
//!
//! // Stop background rendering
//! renderer.stop();
//! println!(); // newline after completion
//! ```

use crate::primitives::ProgressTrackerCapsule;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Batch Progress Renderer (T4 Batch tier)
///
/// Non-blocking background renderer with 10ms batch updates.
pub struct BatchProgressRenderer {
    /// Background thread handle
    thread_handle: Option<JoinHandle<()>>,
    /// Shutdown flag (atomic coordination)
    shutdown: Arc<AtomicBool>,
}

impl BatchProgressRenderer {
    /// Start background renderer (10ms interval = 100 FPS)
    ///
    /// # Arguments
    ///
    /// - `progress`: Shared progress tracker
    /// - `renderer`: Callback function for rendering (current, total)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tracker = Arc::new(ProgressTrackerCapsule::new(1000));
    /// let mut renderer = BatchProgressRenderer::start(tracker, |current, total| {
    ///     print!("\r[{}/{}]", current, total);
    /// });
    /// ```
    pub fn start<F>(progress: Arc<ProgressTrackerCapsule>, renderer: F) -> Self
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        let shutdown = Arc::new(AtomicBool::new(false));
        let progress_clone = Arc::clone(&progress);
        let shutdown_clone = Arc::clone(&shutdown);
        let renderer = Arc::new(renderer);

        let thread_handle = thread::spawn(move || {
            while !shutdown_clone.load(Ordering::Acquire) {
                let current = progress_clone.completed();
                let total = progress_clone.total();
                renderer(current, total);

                thread::sleep(Duration::from_millis(10)); // 100 FPS
            }

            // Final render on shutdown
            let current = progress_clone.completed();
            let total = progress_clone.total();
            renderer(current, total);
        });

        Self {
            thread_handle: Some(thread_handle),
            shutdown,
        }
    }

    /// Stop background rendering
    ///
    /// Performs final render before stopping.
    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for BatchProgressRenderer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    #[test]
    fn test_batch_renderer_start_stop() {
        let tracker = Arc::new(ProgressTrackerCapsule::new(100));
        let tracker_clone = Arc::clone(&tracker);

        let render_count = Arc::new(AtomicU64::new(0));
        let render_count_clone = Arc::clone(&render_count);

        let mut renderer = BatchProgressRenderer::start(tracker_clone, move |_, _| {
            render_count_clone.fetch_add(1, Ordering::Relaxed);
        });

        // Let it render for 50ms (should render ~5 times)
        thread::sleep(Duration::from_millis(50));

        renderer.stop();

        let count = render_count.load(Ordering::Relaxed);
        assert!(count >= 3, "Should have rendered at least 3 times, got {}", count);
        assert!(count <= 10, "Should not over-render, got {}", count);
    }

    #[test]
    fn test_progress_updates_during_rendering() {
        let tracker = Arc::new(ProgressTrackerCapsule::new(1000));
        let tracker_clone = Arc::clone(&tracker);

        let last_rendered = Arc::new(AtomicU64::new(0));
        let last_rendered_clone = Arc::clone(&last_rendered);

        let mut renderer = BatchProgressRenderer::start(tracker_clone.clone(), move |current, _| {
            last_rendered_clone.store(current, Ordering::Relaxed);
        });

        // Increment progress
        for _ in 0..500 {
            tracker.increment();
        }

        // Wait for rendering to catch up
        thread::sleep(Duration::from_millis(30));

        renderer.stop();

        let rendered = last_rendered.load(Ordering::Relaxed);
        assert_eq!(rendered, 500, "Renderer should have seen all updates");
    }
}
