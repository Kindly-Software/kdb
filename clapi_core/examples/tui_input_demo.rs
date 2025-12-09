//! TUI Input Capsule Demo
//!
//! # Purpose
//! Demonstrates CommandInputCapsule with readline-style editing
//!
//! # Usage
//! ```bash
//! cargo run --example tui_input_demo
//! ```
//!
//! # Controls
//! - Type: Insert text
//! - Backspace: Delete char before cursor
//! - Delete: Delete char after cursor
//! - Left/Right: Move cursor
//! - Home/End: Jump to start/end
//! - Up/Down: Navigate history
//! - Tab: Command completion
//! - Enter: Execute command (prints and clears)
//! - Ctrl+C: Exit

use std::io::{self, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{self, ClearType},
};

/// Command input capsule - 256B cache-aligned
#[repr(C, align(64))]
struct CommandInputCapsule {
    buffer: [u8; 200],
    cursor_pos: AtomicU32,
    history_index: AtomicU32,
    buffer_len: AtomicU32,
    modified: AtomicU32,
    _padding: [u8; 40],
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

    fn buffer(&self) -> &str {
        let len = self.buffer_len.load(Ordering::Acquire) as usize;
        let len = len.min(self.buffer.len());
        unsafe { std::str::from_utf8_unchecked(&self.buffer[..len]) }
    }

    fn cursor_pos(&self) -> usize {
        self.cursor_pos.load(Ordering::Acquire) as usize
    }

    fn insert_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let bytes = c.encode_utf8(&mut buf).as_bytes();
        let len = bytes.len();

        let cursor = self.cursor_pos.load(Ordering::Relaxed) as usize;
        let buffer_len = self.buffer_len.load(Ordering::Relaxed) as usize;

        if buffer_len + len > self.buffer.len() {
            return;
        }

        if cursor < buffer_len {
            self.buffer.copy_within(cursor..buffer_len, cursor + len);
        }

        self.buffer[cursor..cursor + len].copy_from_slice(bytes);
        self.buffer_len.store((buffer_len + len) as u32, Ordering::Release);
        self.cursor_pos.store((cursor + len) as u32, Ordering::Release);
        self.modified.store(1, Ordering::Release);
    }

    fn delete_char_before(&mut self) {
        let cursor = self.cursor_pos.load(Ordering::Relaxed) as usize;
        if cursor == 0 {
            return;
        }

        let buffer_len = self.buffer_len.load(Ordering::Relaxed) as usize;
        let mut prev_pos = cursor - 1;
        while prev_pos > 0 && (self.buffer[prev_pos] & 0b1100_0000) == 0b1000_0000 {
            prev_pos -= 1;
        }

        let delete_len = cursor - prev_pos;
        if cursor < buffer_len {
            self.buffer.copy_within(cursor..buffer_len, prev_pos);
        }

        self.buffer_len.store((buffer_len - delete_len) as u32, Ordering::Release);
        self.cursor_pos.store(prev_pos as u32, Ordering::Release);
        self.modified.store(1, Ordering::Release);
    }

    fn delete_char_after(&mut self) {
        let cursor = self.cursor_pos.load(Ordering::Relaxed) as usize;
        let buffer_len = self.buffer_len.load(Ordering::Relaxed) as usize;

        if cursor >= buffer_len {
            return;
        }

        let mut next_pos = cursor + 1;
        while next_pos < buffer_len && (self.buffer[next_pos] & 0b1100_0000) == 0b1000_0000 {
            next_pos += 1;
        }

        let delete_len = next_pos - cursor;
        if next_pos < buffer_len {
            self.buffer.copy_within(next_pos..buffer_len, cursor);
        }

        self.buffer_len.store((buffer_len - delete_len) as u32, Ordering::Release);
        self.modified.store(1, Ordering::Release);
    }

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

    fn move_cursor_home(&mut self) {
        self.cursor_pos.store(0, Ordering::Release);
    }

    fn move_cursor_end(&mut self) {
        let buffer_len = self.buffer_len.load(Ordering::Relaxed);
        self.cursor_pos.store(buffer_len, Ordering::Release);
    }

    fn clear(&mut self) {
        self.buffer_len.store(0, Ordering::Release);
        self.cursor_pos.store(0, Ordering::Release);
        self.modified.store(1, Ordering::Release);
    }
}

fn main() -> io::Result<()> {
    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Show)?;

    // Create input capsule
    let mut capsule = CommandInputCapsule::new();
    let mut history: Vec<String> = Vec::new();
    let mut history_idx = 0;

    // Render initial prompt
    render_prompt(&mut stdout, &capsule)?;

    // Event loop
    loop {
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    break;
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    capsule.clear();
                }
                KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    capsule.move_cursor_home();
                }
                KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    capsule.move_cursor_end();
                }
                KeyCode::Char(c) => {
                    capsule.insert_char(c);
                }
                KeyCode::Backspace => {
                    capsule.delete_char_before();
                }
                KeyCode::Delete => {
                    capsule.delete_char_after();
                }
                KeyCode::Left => {
                    capsule.move_cursor_left();
                }
                KeyCode::Right => {
                    capsule.move_cursor_right();
                }
                KeyCode::Home => {
                    capsule.move_cursor_home();
                }
                KeyCode::End => {
                    capsule.move_cursor_end();
                }
                KeyCode::Up => {
                    if history_idx < history.len() {
                        if let Some(entry) = history.get(history_idx) {
                            // Load from history
                            capsule.clear();
                            for ch in entry.chars() {
                                capsule.insert_char(ch);
                            }
                            history_idx += 1;
                        }
                    }
                }
                KeyCode::Down => {
                    if history_idx > 0 {
                        history_idx -= 1;
                        if history_idx == 0 {
                            capsule.clear();
                        } else if let Some(entry) = history.get(history_idx - 1) {
                            capsule.clear();
                            for ch in entry.chars() {
                                capsule.insert_char(ch);
                            }
                        }
                    }
                }
                KeyCode::Enter => {
                    let command = capsule.buffer().to_string();
                    if !command.trim().is_empty() {
                        // Add to history
                        history.insert(0, command.clone());
                        history_idx = 0;

                        // Execute command (mock)
                        execute!(
                            stdout,
                            cursor::MoveTo(0, 2),
                            terminal::Clear(ClearType::FromCursorDown)
                        )?;
                        writeln!(stdout, "\r\n✓ Executed: {}\r", command)?;

                        // Clear buffer
                        capsule.clear();
                    }
                }
                _ => {}
            }

            // Re-render prompt
            render_prompt(&mut stdout, &capsule)?;
        }
    }

    // Cleanup
    execute!(stdout, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    Ok(())
}

fn render_prompt<W: Write>(stdout: &mut W, capsule: &CommandInputCapsule) -> io::Result<()> {
    let buffer = capsule.buffer();
    let cursor = capsule.cursor_pos();

    execute!(
        stdout,
        cursor::MoveTo(0, 0),
        terminal::Clear(ClearType::CurrentLine)
    )?;

    write!(stdout, "\r> {}", buffer)?;
    execute!(stdout, cursor::MoveTo((cursor + 2) as u16, 0))?;
    stdout.flush()?;

    Ok(())
}
