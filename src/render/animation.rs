//! A small controlled frame loop — not a game engine.
//!
//! Each frame is the *same* seeded scene sampled at a later time, so animation
//! adds motion without adding a second source of truth.

use std::io::{Write, stdout};
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::{cursor, event, execute, terminal};

use crate::error::{AppError, Result};
use crate::render::ascii::{Layout, panel};
use crate::render::color;
use crate::scene::generator::{RenderOptions, generate};
use crate::weather::WeatherData;

const FRAME_MS: u64 = 130;

/// Restores the terminal no matter how we leave — including on panic.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> std::io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(stdout(), terminal::EnterAlternateScreen, cursor::Hide)?;
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(stdout(), cursor::Show, terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

/// Loops until any key (or Ctrl+C / q / Esc) is pressed.
pub fn play(weather: &WeatherData, layout: &Layout, colored: bool, seed: u64) -> Result<()> {
    let guard = TerminalGuard::enter()
        .map_err(|e| AppError::Config(format!("cannot take over the terminal: {e}")))?;
    let night = weather.is_night();
    let mut frame: u64 = 0;

    loop {
        let scene = generate(
            weather,
            &RenderOptions {
                width: layout.inner_width(),
                height: None,
                seed,
                frame,
                ascii: layout.plain,
            },
        );
        let body = color::to_string(&panel(weather, &scene, layout), colored, night);
        let mut out = stdout();
        execute!(
            out,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0)
        )
        .and_then(|_| write!(out, "{}", body.replace('\n', "\r\n")))
        .and_then(|_| out.flush())
        .map_err(|e| AppError::Config(format!("cannot draw: {e}")))?;

        if event::poll(Duration::from_millis(FRAME_MS)).unwrap_or(false)
            && let Ok(Event::Key(KeyEvent {
                code, modifiers, ..
            })) = event::read()
        {
            let quit = matches!(code, KeyCode::Char('q') | KeyCode::Esc)
                || (modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c'));
            if quit || matches!(code, KeyCode::Char(_)) {
                break;
            }
        }
        frame = frame.wrapping_add(1);
    }

    drop(guard);
    Ok(())
}
