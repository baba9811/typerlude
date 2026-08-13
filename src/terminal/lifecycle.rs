use anyhow::{Context, Result, bail};
use crossterm::{
    cursor::Show,
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::{
    io::{self, Write},
    panic::{self, PanicHookInfo},
    sync::Arc,
};

type PanicHook = dyn for<'a> Fn(&PanicHookInfo<'a>) + Send + Sync + 'static;

pub(super) struct TerminalGuard {
    restored: bool,
    previous_hook: Option<Arc<PanicHook>>,
}

impl TerminalGuard {
    pub(super) fn enter() -> Result<Self> {
        if !super::is_interactive_terminal() {
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

    pub(super) fn restore(&mut self) -> io::Result<()> {
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

pub fn write_restore_sequence(writer: &mut impl Write) -> io::Result<()> {
    execute!(writer, DisableBracketedPaste, LeaveAlternateScreen, Show)
}

fn restore_terminal() -> io::Result<()> {
    let raw = disable_raw_mode();
    let mut stdout = io::stdout();
    let screen = write_restore_sequence(&mut stdout);
    raw.and(screen)
}
