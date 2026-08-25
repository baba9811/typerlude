use crate::{
    app::{App, InputEvent, Key, KeyInput, KeyKind, KeyModifiers as AppKeyModifiers},
    tui,
};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers as CrosstermKeyModifiers};
use ratatui::layout::Size;
use std::time::Instant;

pub(super) fn handle_event_at_size(
    app: &mut App,
    event: Event,
    size: Size,
    now: Instant,
) -> Result<()> {
    let viewport = match &event {
        Event::Resize(width, height) => Size::new(*width, *height),
        _ => size,
    };
    app.set_game_viewport_supported(tui::supports_size(viewport.width, viewport.height), now);
    let resized = matches!(event, Event::Resize(..));
    let event = input_event(event);
    let global_quit = matches!(
        &event,
        InputEvent::Key(key)
            if matches!(key.key, Key::Char('c' | 'C')) && key.modifiers.control
    );
    if tui::supports_size(size.width, size.height) || resized || global_quit {
        return app.handle_event(event, now);
    }
    if matches!(
        event,
        InputEvent::Key(key)
            if key.is_plain_q_command()
    ) {
        app.request_quit();
    }
    Ok(())
}

pub(super) fn tick_at_size(app: &mut App, size: Size, now: Instant) -> Result<()> {
    app.set_game_viewport_supported(tui::supports_size(size.width, size.height), now);
    app.tick(now)
}

fn input_event(event: Event) -> InputEvent {
    let Event::Key(event) = event else {
        return match event {
            Event::Paste(_) => InputEvent::Paste,
            _ => InputEvent::Ignored,
        };
    };
    let kind = match event.kind {
        KeyEventKind::Press => KeyKind::Press,
        KeyEventKind::Repeat => KeyKind::Repeat,
        KeyEventKind::Release => return InputEvent::Ignored,
    };
    let key = match event.code {
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Char(character) => Key::Char(character),
        KeyCode::Down => Key::Down,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Tab => Key::Tab,
        KeyCode::Up => Key::Up,
        _ => Key::Other,
    };
    InputEvent::Key(KeyInput {
        key,
        modifiers: AppKeyModifiers {
            shift: event.modifiers.contains(CrosstermKeyModifiers::SHIFT),
            control: event.modifiers.contains(CrosstermKeyModifiers::CONTROL),
            other: event.modifiers.intersects(
                CrosstermKeyModifiers::ALT
                    | CrosstermKeyModifiers::SUPER
                    | CrosstermKeyModifiers::HYPER
                    | CrosstermKeyModifiers::META,
            ),
        },
        kind,
    })
}

#[cfg(test)]
mod tests {
    use super::{handle_event_at_size, input_event, tick_at_size};
    use crate::{
        app::{App, InputEvent, Key, KeyInput, KeyKind, KeyModifiers as AppKeyModifiers, Screen},
        config::Settings,
        content::ContentCatalog,
        storage::AppPaths,
        theme::ThemeCatalog,
    };
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Size;
    use std::time::{Duration, Instant};

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

    fn app_key(key: Key) -> InputEvent {
        InputEvent::Key(KeyInput {
            key,
            modifiers: AppKeyModifiers::NONE,
            kind: KeyKind::Press,
        })
    }

    fn start_game(app: &mut App, now: Instant) {
        for _ in 0..6 {
            app.handle_event(app_key(Key::Tab), now).unwrap();
        }
        app.handle_event(app_key(Key::Enter), now).unwrap();
        app.handle_event(app_key(Key::Enter), now).unwrap();
        app.handle_event(app_key(Key::Tab), now).unwrap();
        app.handle_event(app_key(Key::Tab), now).unwrap();
        app.handle_event(app_key(Key::Enter), now).unwrap();
        assert_eq!(app.screen(), Screen::Game);
    }

    #[test]
    fn crossterm_events_map_to_app_input_without_paste_payload() {
        assert_eq!(
            input_event(Event::Key(KeyEvent::new_with_kind(
                KeyCode::Char('a'),
                KeyModifiers::NONE,
                crossterm::event::KeyEventKind::Press,
            ))),
            InputEvent::Key(KeyInput {
                key: Key::Char('a'),
                modifiers: AppKeyModifiers {
                    shift: false,
                    control: false,
                    other: false,
                },
                kind: KeyKind::Press,
            })
        );
        assert_eq!(
            input_event(Event::Key(KeyEvent::new_with_kind(
                KeyCode::Enter,
                KeyModifiers::SHIFT | KeyModifiers::ALT,
                crossterm::event::KeyEventKind::Repeat,
            ))),
            InputEvent::Key(KeyInput {
                key: Key::Enter,
                modifiers: AppKeyModifiers {
                    shift: true,
                    control: false,
                    other: true,
                },
                kind: KeyKind::Repeat,
            })
        );
        assert_eq!(
            input_event(Event::Key(KeyEvent::new_with_kind(
                KeyCode::Char('q'),
                KeyModifiers::NONE,
                crossterm::event::KeyEventKind::Release,
            ))),
            InputEvent::Ignored
        );
        assert_eq!(
            input_event(Event::Paste("private text".into())),
            InputEvent::Paste
        );
        assert_eq!(input_event(Event::FocusGained), InputEvent::Ignored);
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

        for command in ['q', 'ㅂ'] {
            let mut quit = fixture_app();
            handle_event_at_size(
                &mut quit,
                key(KeyCode::Char(command), KeyModifiers::NONE),
                tiny,
                now,
            )
            .unwrap();
            assert!(quit.should_quit(), "{command}");
        }

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

    #[test]
    fn tiny_terminal_transitions_suspend_and_resynchronize_word_rain_time() {
        let mut app = fixture_app();
        let now = Instant::now();
        start_game(&mut app, now);
        let supported = Size::new(80, 24);
        let tiny = Size::new(79, 23);
        let initial = app
            .active_word_rain()
            .unwrap()
            .game
            .active_words()
            .next()
            .unwrap()
            .progress();

        tick_at_size(&mut app, supported, now + Duration::from_millis(250)).unwrap();
        let advanced = app
            .active_word_rain()
            .unwrap()
            .game
            .active_words()
            .next()
            .unwrap()
            .progress();
        assert!(advanced > initial);

        tick_at_size(&mut app, tiny, now + Duration::from_millis(500)).unwrap();
        tick_at_size(&mut app, tiny, now + Duration::from_secs(100)).unwrap();
        assert_eq!(
            app.active_word_rain()
                .unwrap()
                .game
                .active_words()
                .next()
                .unwrap()
                .progress(),
            advanced
        );

        tick_at_size(&mut app, supported, now + Duration::from_secs(100)).unwrap();
        assert_eq!(
            app.active_word_rain()
                .unwrap()
                .game
                .active_words()
                .next()
                .unwrap()
                .progress(),
            advanced
        );
        tick_at_size(&mut app, supported, now + Duration::from_millis(100_250)).unwrap();
        let game = &app.active_word_rain().unwrap().game;
        assert!(game.active_words().next().unwrap().progress() > advanced);

        app.handle_event(app_key(Key::Esc), now + Duration::from_secs(101))
            .unwrap();
        tick_at_size(&mut app, tiny, now + Duration::from_secs(102)).unwrap();
        tick_at_size(&mut app, supported, now + Duration::from_secs(103)).unwrap();
        assert!(app.active_word_rain().unwrap().game.is_paused());
    }

    #[test]
    fn resize_events_use_the_reported_dimensions_for_immediate_suspension() {
        let mut app = fixture_app();
        let now = Instant::now();
        start_game(&mut app, now);
        tick_at_size(
            &mut app,
            Size::new(80, 24),
            now + Duration::from_millis(250),
        )
        .unwrap();
        let before_resize = app
            .active_word_rain()
            .unwrap()
            .game
            .active_words()
            .next()
            .unwrap()
            .progress();

        handle_event_at_size(
            &mut app,
            Event::Resize(79, 23),
            Size::new(80, 24),
            now + Duration::from_millis(500),
        )
        .unwrap();

        assert_eq!(
            app.active_word_rain()
                .unwrap()
                .game
                .active_words()
                .next()
                .unwrap()
                .progress(),
            before_resize
        );
    }
}
