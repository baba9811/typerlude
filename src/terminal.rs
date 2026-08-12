use crate::{app::App, ui};
use anyhow::{Context, Result, bail};
use crossterm::{
    cursor::Show,
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind,
        KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Size};
use std::{
    io::{self, IsTerminal, Write},
    panic::{self, PanicHookInfo},
    sync::Arc,
    time::{Duration, Instant},
};

type PanicHook = dyn for<'a> Fn(&PanicHookInfo<'a>) + Send + Sync + 'static;

pub struct TerminalGuard {
    restored: bool,
    previous_hook: Option<Arc<PanicHook>>,
}

impl TerminalGuard {
    pub fn enter() -> Result<Self> {
        if !is_interactive_terminal() {
            bail!("interactive terminal required");
        }
        enable_raw_mode().context("failed to enable terminal raw mode")?;
        let mut guard = Self {
            restored: false,
            previous_hook: None,
        };
        guard.install_panic_hook();
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableBracketedPaste, Show) {
            let _ = guard.restore();
            return Err(error).context("failed to enter the terminal screen");
        }
        Ok(guard)
    }

    pub fn restore(&mut self) -> io::Result<()> {
        if self.restored {
            return Ok(());
        }
        let restored = restore_terminal();
        if !std::thread::panicking()
            && let Some(previous) = self.previous_hook.take()
        {
            drop(panic::take_hook());
            panic::set_hook(Box::new(move |info| previous(info)));
        }
        self.restored = restored.is_ok();
        restored
    }

    fn install_panic_hook(&mut self) {
        let previous = Arc::<PanicHook>::from(panic::take_hook());
        let prior = Arc::clone(&previous);
        panic::set_hook(Box::new(move |info| {
            let _ = restore_terminal();
            prior(info);
        }));
        self.previous_hook = Some(previous);
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

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

pub fn write_restore_sequence(writer: &mut impl Write) -> io::Result<()> {
    execute!(writer, DisableBracketedPaste, LeaveAlternateScreen, Show)
}

fn restore_terminal() -> io::Result<()> {
    let raw = disable_raw_mode();
    let mut stdout = io::stdout();
    let screen = write_restore_sequence(&mut stdout);
    raw.and(screen)
}

pub fn run(mut app: App) -> Result<()> {
    let mut guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).context("failed to initialize terminal drawing")?;
    let result = (|| -> Result<()> {
        terminal
            .draw(|frame| ui::render(frame, &app))
            .context("failed to draw terminal UI")?;
        while !app.should_quit() {
            if event::poll(Duration::from_millis(50)).context("failed to poll terminal input")? {
                let event = event::read().context("failed to read terminal input")?;
                let size = terminal.size().context("failed to read terminal size")?;
                handle_event_at_size(&mut app, event, size, Instant::now())?;
            } else {
                app.tick(Instant::now())?;
            }
            terminal
                .draw(|frame| ui::render(frame, &app))
                .context("failed to draw terminal UI")?;
        }
        Ok(())
    })();
    drop(terminal);
    let restored = guard.restore().context("failed to restore terminal state");
    result.and(restored)
}

fn handle_event_at_size(app: &mut App, event: Event, size: Size, now: Instant) -> Result<()> {
    let global_quit = matches!(
        &event,
        Event::Key(key)
            if key.kind != KeyEventKind::Release
                && matches!(key.code, KeyCode::Char('c' | 'C'))
                && key.modifiers.contains(KeyModifiers::CONTROL)
    );
    if ui::supports_size(size.width, size.height)
        || matches!(event, Event::Resize(..))
        || global_quit
    {
        return app.handle_event(event, now);
    }
    if matches!(
        event,
        Event::Key(key)
            if key.kind != KeyEventKind::Release
                && key.code == KeyCode::Char('q')
                && key.modifiers == KeyModifiers::NONE
    ) {
        app.request_quit();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::handle_event_at_size;
    use crate::{
        app::{App, Screen},
        config::Settings,
        content::ContentCatalog,
        storage::AppPaths,
        theme::ThemeCatalog,
    };
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Size;
    use std::time::Instant;

    fn fixture_app() -> App {
        App::new(
            Settings::default(),
            AppPaths::from_override(std::env::temp_dir().join("typerlude-tiny-terminal-unused")),
            ContentCatalog::load_builtins().unwrap(),
            ThemeCatalog::load_builtins().unwrap(),
            Vec::new(),
            Vec::new(),
        )
    }

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    #[test]
    fn tiny_terminals_accept_only_resize_and_quit_input() {
        let mut app = fixture_app();
        let tiny = Size::new(79, 23);
        let now = Instant::now();

        handle_event_at_size(&mut app, key(KeyCode::Enter, KeyModifiers::NONE), tiny, now).unwrap();
        handle_event_at_size(&mut app, key(KeyCode::Enter, KeyModifiers::NONE), tiny, now).unwrap();
        assert_eq!(app.screen(), Screen::Home);

        handle_event_at_size(&mut app, Event::Resize(80, 24), tiny, now).unwrap();
        assert_eq!(app.screen(), Screen::Home);

        handle_event_at_size(
            &mut app,
            key(KeyCode::Char('q'), KeyModifiers::NONE),
            tiny,
            now,
        )
        .unwrap();
        assert!(app.should_quit());

        let mut supported = fixture_app();
        let size = Size::new(80, 24);
        handle_event_at_size(
            &mut supported,
            key(KeyCode::Enter, KeyModifiers::NONE),
            size,
            now,
        )
        .unwrap();
        assert_eq!(supported.screen(), Screen::ModeOptions);
    }
}
