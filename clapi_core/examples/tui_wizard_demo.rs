//! TUI Wizard Demo
//!
//! Demonstrates split-screen layout with animated logo and wizard form.
//!
//! # Features
//! - Byzantine Purple ↔ Gold logo animation (ping-pong)
//! - 4-step wizard form with navigation
//! - Lockfree capsule reads (<30ns total)
//!
//! # Usage
//! ```bash
//! cargo run --example tui_wizard_demo
//! ```

use clapi_core::cli::tui::{
    render_split_screen, LogoAnimationCapsule, WizardStateCapsule,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::{Duration, Instant};

fn main() -> Result<(), io::Error> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create capsules
    let logo_anim = LogoAnimationCapsule::new();
    let wizard_state = WizardStateCapsule::new();

    // Main loop
    let tick_rate = Duration::from_millis(16); // 60 FPS
    let mut last_tick = Instant::now();
    let mut frame_count = 0u64;

    loop {
        // Render frame
        terminal.draw(|f| {
            render_split_screen(f, Some(&logo_anim), Some(&wizard_state));
        })?;

        // Update animation every frame
        logo_anim.update_frame();
        frame_count += 1;

        // Handle input (non-blocking)
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_millis(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Right | KeyCode::Enter => {
                        let _ = wizard_state.next_step();
                    }
                    KeyCode::Left => {
                        let _ = wizard_state.prev_step();
                    }
                    _ => {}
                }
            }
        }

        // Update tick timer
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    println!("Total frames rendered: {}", frame_count);
    Ok(())
}
