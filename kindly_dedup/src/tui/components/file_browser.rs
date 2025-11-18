//! File Browser Component - Tree Navigation with Multi-Select
//!
//! # UCE34 Framework
//! - Q1-Q9: File system browser with tree navigation, multi-select, glob filtering
//! - Q10: Tier 1 (Atomic) - Lockfree state management for selection indices
//! - Q11: Rust AtomicU64 for packed indices (selected_index:32 + scroll_offset:32)
//! - Q12: Nightly N/A (stable atomics sufficient)
//! - Q13-Q21: Ratatui rendering, keyboard navigation, file metadata display
//! - Q31: Simplicity - Hide atomic details behind clean API
//! - Q33: Validation - #[derive(cache-optimized data structure)] compile-time verification
//! - Q34: Auditability N/A (read-only UI component, no state modification)
//!
//! # Architecture
//! ```text
//! FileBrowserCapsule (128B, cache-aligned)
//! ├─ state_packed: AtomicU64       // selected:32 + scroll:32
//! ├─ filter_active: AtomicBool     // Glob pattern filtering enabled
//! ├─ multi_select_mode: AtomicBool // Multi-select with Space key
//! └─ _padding: [u8; N]             // Complete 128B cache line
//! ```
//!
//! # Controls
//! - Up/Down: Navigate entries
//! - Enter: Select file or enter directory
//! - Space: Toggle multi-select (adds to selection)
//! - u: Go up to parent directory
//! - /: Enter glob pattern filter
//! - Esc: Clear filter or exit
//! - q: Quit browser

// cache-optimized data structure
use atomic_capsule_derive::ComputationalCapsule;
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

/// File browser state capsule (128B aligned)
///
/// # Memory Layout
/// - 8 bytes: state_packed (selected_index:32 + scroll_offset:32)
/// - 1 byte: filter_active
/// - 1 byte: multi_select_mode
/// - 118 bytes: padding (complete 128B cache line)
#[derive(Debug, ComputationalCapsule)]
#[capsule(alignment = 64, size = 128, tier = "Atomic")]
#[repr(C, align(64))]
pub struct FileBrowserCapsule {
    /// Packed state: selected_index (upper 32) + scroll_offset (lower 32)
    /// #ASSUME: u32 sufficient for file indices (4B entries max)
    /// #VERIFY: Atomic operations maintain consistency
    state_packed: AtomicU64,

    /// Glob filter active flag
    filter_active: AtomicBool,

    /// Multi-select mode enabled
    multi_select_mode: AtomicBool,

    /// Padding to 128B
    _padding: [u8; 118],
}

impl FileBrowserCapsule {
    /// Create new file browser capsule
    pub fn new() -> Self {
        Self {
            state_packed: AtomicU64::new(0),
            filter_active: AtomicBool::new(false),
            multi_select_mode: AtomicBool::new(false),
            _padding: [0u8; 118],
        }
    }

    /// Get selected index
    #[inline(always)]
    pub fn selected_index(&self) -> u32 {
        let packed = self.state_packed.load(Ordering::Acquire);
        (packed >> 32) as u32
    }

    /// Get scroll offset
    #[inline(always)]
    pub fn scroll_offset(&self) -> u32 {
        let packed = self.state_packed.load(Ordering::Acquire);
        packed as u32
    }

    /// Set selected index and scroll offset
    #[inline(always)]
    pub fn set_state(&self, selected: u32, scroll: u32) {
        let packed = ((selected as u64) << 32) | (scroll as u64);
        self.state_packed.store(packed, Ordering::Release);
    }

    /// Move selection up
    pub fn move_up(&self) {
        let selected = self.selected_index();
        if selected > 0 {
            let new_selected = selected - 1;
            let scroll = self.scroll_offset();
            self.set_state(new_selected, scroll.min(new_selected));
        }
    }

    /// Move selection down
    pub fn move_down(&self, max: u32) {
        let selected = self.selected_index();
        if selected + 1 < max {
            let new_selected = selected + 1;
            let scroll = self.scroll_offset();
            self.set_state(new_selected, scroll);
        }
    }

    /// Is filter active?
    pub fn is_filter_active(&self) -> bool {
        self.filter_active.load(Ordering::Acquire)
    }

    /// Set filter active
    pub fn set_filter_active(&self, active: bool) {
        self.filter_active.store(active, Ordering::Release);
    }

    /// Is multi-select mode enabled?
    pub fn is_multi_select(&self) -> bool {
        self.multi_select_mode.load(Ordering::Acquire)
    }

    /// Toggle multi-select mode
    pub fn toggle_multi_select(&self) {
        let current = self.is_multi_select();
        self.multi_select_mode.store(!current, Ordering::Release);
    }
}

impl Default for FileBrowserCapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// File entry metadata
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub modified: Option<SystemTime>,
    pub estimated_docs: Option<usize>,
}

impl FileEntry {
    /// Create file entry from path
    pub fn from_path(path: PathBuf) -> std::io::Result<Self> {
        let metadata = fs::metadata(&path)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());

        let size_bytes = metadata.len();
        let modified = metadata.modified().ok();
        let is_dir = metadata.is_dir();

        // Estimate document count for known formats
        let estimated_docs = if !is_dir {
            estimate_doc_count(&path, size_bytes)
        } else {
            None
        };

        Ok(Self {
            path,
            name,
            is_dir,
            size_bytes,
            modified,
            estimated_docs,
        })
    }

    /// Format size for display (KB/MB/GB)
    pub fn format_size(&self) -> String {
        if self.is_dir {
            return "DIR".to_string();
        }

        let bytes = self.size_bytes as f64;
        if bytes < 1024.0 {
            format!("{} B", bytes)
        } else if bytes < 1024.0 * 1024.0 {
            format!("{:.1} KB", bytes / 1024.0)
        } else if bytes < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.1} MB", bytes / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", bytes / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

/// Estimate document count from file size (heuristic)
fn estimate_doc_count(path: &Path, size_bytes: u64) -> Option<usize> {
    let ext = path.extension()?.to_str()?;

    // Average bytes per document (rough estimates)
    let bytes_per_doc = match ext {
        "jsonl" | "ndjson" => 500, // ~500 bytes per JSON doc
        "txt" => 300,              // ~300 bytes per text doc
        "csv" => 200,              // ~200 bytes per CSV row
        "parquet" => 100,          // Compressed, ~100 bytes per row
        _ => return None,
    };

    Some((size_bytes / bytes_per_doc as u64) as usize)
}

/// File browser component
pub struct FileBrowser {
    /// Atomic state capsule
    capsule: FileBrowserCapsule,

    /// Current directory
    current_dir: PathBuf,

    /// File entries (filtered)
    entries: Vec<FileEntry>,

    /// Selected files (multi-select)
    selected_files: Vec<PathBuf>,

    /// Glob pattern filter
    filter_pattern: String,

    /// Recent directories
    recent_dirs: Vec<PathBuf>,
}

impl FileBrowser {
    /// Create new file browser
    pub fn new(start_dir: PathBuf) -> std::io::Result<Self> {
        let mut browser = Self {
            capsule: FileBrowserCapsule::new(),
            current_dir: start_dir.clone(),
            entries: Vec::new(),
            selected_files: Vec::new(),
            filter_pattern: String::new(),
            recent_dirs: vec![start_dir],
        };

        browser.refresh_entries()?;
        Ok(browser)
    }

    /// Refresh file entries from current directory
    pub fn refresh_entries(&mut self) -> std::io::Result<()> {
        self.entries.clear();

        // Read directory entries
        let mut entries: Vec<FileEntry> = fs::read_dir(&self.current_dir)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                FileEntry::from_path(entry.path()).ok()
            })
            .collect();

        // Apply glob filter if active
        if self.capsule.is_filter_active() && !self.filter_pattern.is_empty() {
            entries.retain(|e| {
                // Simple glob matching (supports * and ?)
                glob_match(&e.name, &self.filter_pattern)
            });
        }

        // Sort: directories first, then by name
        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        self.entries = entries;

        // Reset selection if out of bounds
        let selected = self.capsule.selected_index();
        if selected >= self.entries.len() as u32 {
            self.capsule.set_state(0, 0);
        }

        Ok(())
    }

    /// Move up to parent directory
    pub fn go_parent(&mut self) -> std::io::Result<()> {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.refresh_entries()?;
            self.capsule.set_state(0, 0);
        }
        Ok(())
    }

    /// Enter selected directory or select file
    pub fn enter_selected(&mut self) -> std::io::Result<Option<PathBuf>> {
        let idx = self.capsule.selected_index() as usize;
        if idx >= self.entries.len() {
            return Ok(None);
        }

        let entry = &self.entries[idx];
        if entry.is_dir {
            // Enter directory
            self.current_dir = entry.path.clone();
            self.recent_dirs.push(self.current_dir.clone());
            if self.recent_dirs.len() > 10 {
                self.recent_dirs.remove(0);
            }
            self.refresh_entries()?;
            self.capsule.set_state(0, 0);
            Ok(None)
        } else {
            // Return selected file
            Ok(Some(entry.path.clone()))
        }
    }

    /// Toggle multi-select for current file
    pub fn toggle_select(&mut self) {
        let idx = self.capsule.selected_index() as usize;
        if idx >= self.entries.len() {
            return;
        }

        let entry = &self.entries[idx];
        if !entry.is_dir {
            let path = entry.path.clone();
            if let Some(pos) = self.selected_files.iter().position(|p| p == &path) {
                self.selected_files.remove(pos);
            } else {
                self.selected_files.push(path);
            }
        }
    }

    /// Set glob filter pattern
    pub fn set_filter(&mut self, pattern: String) -> std::io::Result<()> {
        self.filter_pattern = pattern;
        self.capsule.set_filter_active(!self.filter_pattern.is_empty());
        self.refresh_entries()?;
        Ok(())
    }

    /// Clear filter
    pub fn clear_filter(&mut self) -> std::io::Result<()> {
        self.filter_pattern.clear();
        self.capsule.set_filter_active(false);
        self.refresh_entries()?;
        Ok(())
    }

    /// Get selected files (multi-select result)
    pub fn get_selected_files(&self) -> Vec<PathBuf> {
        self.selected_files.clone()
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, key: crossterm::event::KeyCode) -> std::io::Result<FileBrowserAction> {
        use crossterm::event::KeyCode;

        match key {
            KeyCode::Up => {
                self.capsule.move_up();
                Ok(FileBrowserAction::Continue)
            }
            KeyCode::Down => {
                let max = self.entries.len() as u32;
                self.capsule.move_down(max);
                Ok(FileBrowserAction::Continue)
            }
            KeyCode::Enter => {
                if let Some(file) = self.enter_selected()? {
                    Ok(FileBrowserAction::FileSelected(file))
                } else {
                    Ok(FileBrowserAction::Continue)
                }
            }
            KeyCode::Char(' ') => {
                self.toggle_select();
                Ok(FileBrowserAction::Continue)
            }
            KeyCode::Char('u') => {
                self.go_parent()?;
                Ok(FileBrowserAction::Continue)
            }
            KeyCode::Char('/') => Ok(FileBrowserAction::EnterFilter),
            KeyCode::Esc => {
                if self.capsule.is_filter_active() {
                    self.clear_filter()?;
                    Ok(FileBrowserAction::Continue)
                } else {
                    Ok(FileBrowserAction::Exit)
                }
            }
            KeyCode::Char('q') => Ok(FileBrowserAction::Exit),
            _ => Ok(FileBrowserAction::Continue),
        }
    }

    /// Render file browser to frame
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Create layout: header + file list + footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(5),    // File list
                Constraint::Length(3), // Footer
            ])
            .split(area);

        // Render header
        let header_text = vec![
            Line::from(vec![
                Span::styled("Dir: ", Style::default().fg(Color::Cyan)),
                Span::raw(self.current_dir.display().to_string()),
            ]),
            Line::from(vec![
                Span::styled("Filter: ", Style::default().fg(Color::Yellow)),
                Span::raw(if self.capsule.is_filter_active() {
                    &self.filter_pattern
                } else {
                    "(none)"
                }),
            ]),
        ];
        let header = Paragraph::new(header_text).block(Block::default().borders(Borders::ALL).title("File Browser"));
        frame.render_widget(header, chunks[0]);

        // Render file list
        let selected_idx = self.capsule.selected_index() as usize;
        let items: Vec<ListItem> = self
            .entries
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                let is_selected_multi = self.selected_files.iter().any(|p| p == &entry.path);
                let symbol = if entry.is_dir {
                    "📁 "
                } else if is_selected_multi {
                    "✓ "
                } else {
                    "  "
                };

                let size_str = entry.format_size();
                let docs_str = entry
                    .estimated_docs
                    .map(|n| format!(" (~{} docs)", n))
                    .unwrap_or_default();

                let content = format!("{}{:<40} {:>10}{}", symbol, entry.name, size_str, docs_str);

                let style = if idx == selected_idx {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else if entry.is_dir {
                    Style::default().fg(Color::Cyan)
                } else if is_selected_multi {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };

                ListItem::new(content).style(style)
            })
            .collect();

        let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Files"));
        frame.render_widget(list, chunks[1]);

        // Render footer (controls)
        let footer_text = vec![Line::from(vec![
            Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
            Span::raw(": Navigate | "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": Select | "),
            Span::styled("Space", Style::default().fg(Color::Yellow)),
            Span::raw(": Multi-select | "),
            Span::styled("u", Style::default().fg(Color::Yellow)),
            Span::raw(": Parent | "),
            Span::styled("/", Style::default().fg(Color::Yellow)),
            Span::raw(": Filter | "),
            Span::styled("q", Style::default().fg(Color::Yellow)),
            Span::raw(": Quit"),
        ])];
        let footer = Paragraph::new(footer_text).block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, chunks[2]);
    }
}

/// File browser action result
#[derive(Debug, Clone)]
pub enum FileBrowserAction {
    Continue,
    FileSelected(PathBuf),
    EnterFilter,
    Exit,
}

/// Simple glob pattern matching (supports * and ?)
fn glob_match(name: &str, pattern: &str) -> bool {
    // Simple implementation: * matches anything, ? matches one char
    let mut name_chars = name.chars().peekable();
    let mut pattern_chars = pattern.chars().peekable();

    loop {
        match (pattern_chars.peek(), name_chars.peek()) {
            (None, None) => return true,
            (None, Some(_)) => return false,
            (Some(&'*'), _) => {
                pattern_chars.next();
                // Try matching rest of pattern at each position
                let pattern_rest: String = pattern_chars.clone().collect();
                if pattern_rest.is_empty() {
                    return true;
                }
                loop {
                    let name_rest: String = name_chars.clone().collect();
                    if glob_match(&name_rest, &pattern_rest) {
                        return true;
                    }
                    if name_chars.next().is_none() {
                        return false;
                    }
                }
            }
            (Some(&'?'), Some(_)) => {
                pattern_chars.next();
                name_chars.next();
            }
            (Some(&p), Some(&n)) if p == n => {
                pattern_chars.next();
                name_chars.next();
            }
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match() {
        assert!(glob_match("file.txt", "*.txt"));
        assert!(glob_match("test.json", "test.*"));
        assert!(glob_match("abc", "a?c"));
        assert!(!glob_match("file.txt", "*.json"));
        assert!(glob_match("anything", "*"));
    }

    #[test]
    fn test_capsule_state() {
        let capsule = FileBrowserCapsule::new();
        assert_eq!(capsule.selected_index(), 0);
        assert_eq!(capsule.scroll_offset(), 0);

        capsule.set_state(10, 5);
        assert_eq!(capsule.selected_index(), 10);
        assert_eq!(capsule.scroll_offset(), 5);
    }

    #[test]
    fn test_file_entry_size_format() {
        let entry = FileEntry {
            path: PathBuf::from("test.txt"),
            name: "test.txt".to_string(),
            is_dir: false,
            size_bytes: 1024 * 1024 * 2, // 2 MB
            modified: None,
            estimated_docs: None,
        };

        assert_eq!(entry.format_size(), "2.0 MB");
    }
}
