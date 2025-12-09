//! File input widget with drag-drop support
//!
//! # Features
//! - File path display with size formatting
//! - Browse button with hover state
//! - Drag-drop zone with visual feedback
//! - Chaos-compliant AtomicU64 state (T1 Atomic tier)

use std::sync::atomic::{AtomicU64, Ordering};
use crate::gui_v2::layout::Rect;

/// File input widget state
///
/// # State Encoding (AtomicU64)
/// - Bits 0-7: Hover state (0=none, 1=button, 2=drop_zone)
/// - Bits 8-15: Reserved
/// - Bits 16-47: File size in bytes (32-bit, max 4GB)
/// - Bits 48-63: Reserved
#[repr(C, align(64))]
pub struct FileInputWidget {
    /// Packed state (hover + file size)
    state: AtomicU64,
    /// File path (heap-allocated, not in hot path)
    path: std::sync::RwLock<Option<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverState {
    None = 0,
    Button = 1,
    DropZone = 2,
}

impl FileInputWidget {
    /// Create new file input widget
    pub fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            path: std::sync::RwLock::new(None),
        }
    }

    /// Set file path and size
    pub fn set_file(&self, path: String, size_bytes: u64) {
        // Clamp size to 32-bit max (4GB)
        let size = size_bytes.min(u32::MAX as u64) as u32;

        // Pack size into bits 16-47
        let packed = (size as u64) << 16;

        // Update state (preserves hover state in bits 0-7)
        let old = self.state.load(Ordering::Acquire);
        let new = (old & 0xFFFF) | packed;
        self.state.store(new, Ordering::Release);

        // Update path
        *self.path.write().unwrap() = Some(path);
    }

    /// Clear file selection
    pub fn clear(&self) {
        self.state.store(0, Ordering::Release);
        *self.path.write().unwrap() = None;
    }

    /// Get current file path
    pub fn get_path(&self) -> Option<String> {
        self.path.read().unwrap().clone()
    }

    /// Get file size in bytes
    pub fn get_size(&self) -> u64 {
        let state = self.state.load(Ordering::Acquire);
        ((state >> 16) & 0xFFFFFFFF) as u64
    }

    /// Set hover state
    pub fn set_hover(&self, hover: HoverState) {
        let old = self.state.load(Ordering::Acquire);
        let new = (old & !0xFF) | (hover as u64);
        self.state.store(new, Ordering::Release);
    }

    /// Get hover state
    pub fn get_hover(&self) -> HoverState {
        let state = self.state.load(Ordering::Acquire);
        let hover_bits = state & 0xFF;
        match hover_bits {
            1 => HoverState::Button,
            2 => HoverState::DropZone,
            _ => HoverState::None,
        }
    }

    /// Check if button is hovered
    pub fn is_button_hovered(&self) -> bool {
        self.get_hover() == HoverState::Button
    }

    /// Check if drop zone is hovered
    pub fn is_drop_zone_hovered(&self) -> bool {
        self.get_hover() == HoverState::DropZone
    }

    /// Format file size for display
    pub fn format_size(&self) -> String {
        let size = self.get_size();
        if size == 0 {
            return String::from("No file selected");
        }

        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;

        if size >= GB {
            format!("{:.2} GB", size as f64 / GB as f64)
        } else if size >= MB {
            format!("{:.2} MB", size as f64 / MB as f64)
        } else if size >= KB {
            format!("{:.2} KB", size as f64 / KB as f64)
        } else {
            format!("{} bytes", size)
        }
    }

    /// Get browse button bounds (for hit testing)
    pub fn get_button_bounds(&self) -> Rect {
        // Button is 200px wide, 40px tall
        // Positioned at (20, 100) in layout
        Rect {
            x: 20,
            y: 100,
            width: 200,
            height: 40,
        }
    }

    /// Get drop zone bounds (for hit testing)
    pub fn get_drop_zone_bounds(&self) -> Rect {
        // Drop zone is full width minus padding, 150px tall
        // Positioned at (20, 160) in layout
        Rect {
            x: 20,
            y: 160,
            width: 760, // 800 - 2*20 padding
            height: 150,
        }
    }

    /// Render widget text (for testing)
    pub fn render_text(&self) -> String {
        if let Some(path) = self.get_path() {
            format!("{} ({})", path, self.format_size())
        } else {
            String::from("Drag file here or click Browse")
        }
    }
}

impl Default for FileInputWidget {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_widget() {
        let widget = FileInputWidget::new();
        assert_eq!(widget.get_hover(), HoverState::None);
        assert_eq!(widget.get_size(), 0);
        assert!(widget.get_path().is_none());
    }

    #[test]
    fn test_set_file() {
        let widget = FileInputWidget::new();
        widget.set_file(String::from("/path/to/file.txt"), 1024);

        assert_eq!(widget.get_path(), Some(String::from("/path/to/file.txt")));
        assert_eq!(widget.get_size(), 1024);
    }

    #[test]
    fn test_clear() {
        let widget = FileInputWidget::new();
        widget.set_file(String::from("/path/to/file.txt"), 1024);
        widget.clear();

        assert!(widget.get_path().is_none());
        assert_eq!(widget.get_size(), 0);
    }

    #[test]
    fn test_hover_state() {
        let widget = FileInputWidget::new();

        widget.set_hover(HoverState::Button);
        assert_eq!(widget.get_hover(), HoverState::Button);
        assert!(widget.is_button_hovered());
        assert!(!widget.is_drop_zone_hovered());

        widget.set_hover(HoverState::DropZone);
        assert_eq!(widget.get_hover(), HoverState::DropZone);
        assert!(!widget.is_button_hovered());
        assert!(widget.is_drop_zone_hovered());

        widget.set_hover(HoverState::None);
        assert_eq!(widget.get_hover(), HoverState::None);
    }

    #[test]
    fn test_hover_preserves_file_state() {
        let widget = FileInputWidget::new();
        widget.set_file(String::from("/path/to/file.txt"), 2048);

        widget.set_hover(HoverState::Button);
        assert_eq!(widget.get_size(), 2048);
        assert_eq!(widget.get_path(), Some(String::from("/path/to/file.txt")));
    }

    #[test]
    fn test_format_size_bytes() {
        let widget = FileInputWidget::new();
        widget.set_file(String::from("test"), 512);
        assert_eq!(widget.format_size(), "512 bytes");
    }

    #[test]
    fn test_format_size_kb() {
        let widget = FileInputWidget::new();
        widget.set_file(String::from("test"), 2048);
        assert_eq!(widget.format_size(), "2.00 KB");
    }

    #[test]
    fn test_format_size_mb() {
        let widget = FileInputWidget::new();
        widget.set_file(String::from("test"), 5_242_880); // 5 MB
        assert_eq!(widget.format_size(), "5.00 MB");
    }

    #[test]
    fn test_format_size_gb() {
        let widget = FileInputWidget::new();
        widget.set_file(String::from("test"), 2_147_483_648); // 2 GB
        assert_eq!(widget.format_size(), "2.00 GB");
    }

    #[test]
    fn test_format_size_empty() {
        let widget = FileInputWidget::new();
        assert_eq!(widget.format_size(), "No file selected");
    }

    #[test]
    fn test_max_file_size() {
        let widget = FileInputWidget::new();
        // Test max u32 size (4GB)
        widget.set_file(String::from("huge.txt"), u32::MAX as u64);
        assert_eq!(widget.get_size(), u32::MAX as u64);
    }

    #[test]
    fn test_oversized_file_clamped() {
        let widget = FileInputWidget::new();
        // Test size > 4GB gets clamped
        widget.set_file(String::from("huge.txt"), 10_000_000_000); // 10 GB
        assert_eq!(widget.get_size(), u32::MAX as u64);
    }

    #[test]
    fn test_button_bounds() {
        let widget = FileInputWidget::new();
        let bounds = widget.get_button_bounds();
        assert_eq!(bounds.x, 20);
        assert_eq!(bounds.y, 100);
        assert_eq!(bounds.width, 200);
        assert_eq!(bounds.height, 40);
    }

    #[test]
    fn test_drop_zone_bounds() {
        let widget = FileInputWidget::new();
        let bounds = widget.get_drop_zone_bounds();
        assert_eq!(bounds.x, 20);
        assert_eq!(bounds.y, 160);
        assert_eq!(bounds.width, 760);
        assert_eq!(bounds.height, 150);
    }

    #[test]
    fn test_render_text_empty() {
        let widget = FileInputWidget::new();
        assert_eq!(widget.render_text(), "Drag file here or click Browse");
    }

    #[test]
    fn test_render_text_with_file() {
        let widget = FileInputWidget::new();
        widget.set_file(String::from("/data/corpus.jsonl"), 1024);
        assert_eq!(widget.render_text(), "/data/corpus.jsonl (1.00 KB)");
    }

    #[test]
    fn test_atomic_alignment() {
        // Verify 64-byte cache line alignment
        let widget = FileInputWidget::new();
        let ptr = &widget as *const FileInputWidget as usize;
        assert_eq!(ptr % 64, 0, "FileInputWidget not 64-byte aligned");
    }

    #[test]
    fn test_concurrent_hover_updates() {
        use std::sync::Arc;
        use std::thread;

        let widget = Arc::new(FileInputWidget::new());
        let widget1 = Arc::clone(&widget);
        let widget2 = Arc::clone(&widget);

        let h1 = thread::spawn(move || {
            for _ in 0..1000 {
                widget1.set_hover(HoverState::Button);
            }
        });

        let h2 = thread::spawn(move || {
            for _ in 0..1000 {
                widget2.set_hover(HoverState::DropZone);
            }
        });

        h1.join().unwrap();
        h2.join().unwrap();

        // Final state should be one of the two (no corruption)
        let final_state = widget.get_hover();
        assert!(
            final_state == HoverState::Button || final_state == HoverState::DropZone,
            "Concurrent updates corrupted state"
        );
    }
}
