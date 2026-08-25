use crate::{app::App, tui};
use anyhow::{Context, Result};
use crossterm::event;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io::{self, IsTerminal},
    time::{Duration, Instant},
};

mod input;
mod lifecycle;

use input::{handle_event_at_size, tick_at_size};
use lifecycle::TerminalGuard;
pub use lifecycle::write_restore_sequence;

pub fn is_interactive_terminal() -> bool {
    if !io::stdout().is_terminal() {
        return false;
    }
    if io::stdin().is_terminal() {
        return true;
    }
    #[cfg(unix)]
    {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .is_ok()
    }
    #[cfg(not(unix))]
    {
        false
    }
}

pub fn run(mut app: App) -> Result<()> {
    let mut guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).context("failed to initialize terminal drawing")?;
    let result = (|| -> Result<()> {
        terminal
            .draw(|frame| tui::render(frame, &app))
            .context("failed to draw terminal UI")?;
        while !app.should_quit() {
            if event::poll(Duration::from_millis(50)).context("failed to poll terminal input")? {
                let event = event::read().context("failed to read terminal input")?;
                let size = terminal.size().context("failed to read terminal size")?;
                handle_event_at_size(&mut app, event, size, Instant::now())?;
            } else {
                let size = terminal.size().context("failed to read terminal size")?;
                tick_at_size(&mut app, size, Instant::now())?;
            }
            terminal
                .draw(|frame| tui::render(frame, &app))
                .context("failed to draw terminal UI")?;
        }
        Ok(())
    })();
    drop(terminal);
    let restored = guard.restore().context("failed to restore terminal state");
    result.and(restored)
}
