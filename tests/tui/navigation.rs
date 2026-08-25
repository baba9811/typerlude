use super::support::*;

#[test]
fn modified_enter_cannot_change_or_persist_navigation_screen_state() {
    let (_root, mut app) = fixture_app();
    let before = app.settings.clone();
    app.open(Screen::Settings);

    for modifiers in [KeyModifiers::OTHER, KeyModifiers::CONTROL] {
        app.handle_event(
            key_with(Key::Enter, modifiers, KeyKind::Press),
            Instant::now(),
        )
        .unwrap();
    }

    assert_eq!(app.settings, before);
    assert!(!app.paths.config.exists());
}

#[test]
fn screen_all_is_exact_unique_and_app_starts_at_home() {
    assert_eq!(
        Screen::ALL,
        [
            Screen::Home,
            Screen::ModeOptions,
            Screen::Practice,
            Screen::Result,
            Screen::Games,
            Screen::GameOptions,
            Screen::Game,
            Screen::GameResult,
            Screen::Stats,
            Screen::History,
            Screen::WeakKeys,
            Screen::Goals,
            Screen::Content,
            Screen::ContentDetail,
            Screen::Settings,
            Screen::Themes,
            Screen::Help,
        ]
    );
    assert_eq!(Screen::ALL.into_iter().collect::<HashSet<_>>().len(), 17);

    let (_root, app) = fixture_app();
    assert_eq!(app.screen(), Screen::Home);
    assert_eq!(app.parent(), Screen::Home);
    assert_eq!(app.focus(), 0);
    assert!(!app.should_quit());
    assert!(app.practice.is_none());
    assert!(app.result.is_none());
    assert_eq!(app.warnings, ["review warning"]);
}

#[test]
fn games_follow_the_screen_hierarchy() {
    let (_root, mut app) = fixture_app();
    let now = Instant::now();

    press(&mut app, Key::Tab, 6, now);
    app.handle_event(key(Key::Enter), now).unwrap();
    assert_eq!((app.screen(), app.parent()), (Screen::Games, Screen::Home));

    app.handle_event(key(Key::Enter), now).unwrap();
    assert_eq!(
        (app.screen(), app.parent()),
        (Screen::GameOptions, Screen::Games)
    );

    press(&mut app, Key::Tab, 2, now);
    app.handle_event(key(Key::Enter), now).unwrap();
    assert_eq!((app.screen(), app.parent()), (Screen::Game, Screen::Games));

    app.handle_event(key(Key::Esc), now).unwrap();
    app.handle_event(key(Key::Char('q')), now).unwrap();
    app.handle_event(key(Key::Char('q')), now).unwrap();
    assert_eq!((app.screen(), app.parent()), (Screen::Games, Screen::Home));

    app.handle_event(key(Key::Esc), now).unwrap();
    assert_eq!(app.screen(), Screen::Home);
    assert_eq!(app.focus(), 6);
}

#[test]
fn escape_returns_to_the_parent_and_nested_help_returns_to_its_opener() {
    let (_root, mut app) = fixture_app();
    let now = Instant::now();

    app.open(Screen::Settings);
    assert_eq!(app.parent(), Screen::Home);
    app.open(Screen::Help);
    assert_eq!(app.parent(), Screen::Settings);
    app.open(Screen::Help);
    assert_eq!(
        app.parent(),
        Screen::Settings,
        "Help must not parent itself"
    );

    app.handle_event(key(Key::Esc), now).unwrap();
    assert_eq!(app.screen(), Screen::Settings);
    assert_eq!(app.parent(), Screen::Home);
    app.handle_event(key(Key::Esc), now).unwrap();
    assert_eq!(app.screen(), Screen::Home);
    app.handle_event(key(Key::Esc), now).unwrap();
    assert!(app.should_quit());

    let (_root, mut nested) = fixture_app();
    nested.open(Screen::Settings);
    nested.open(Screen::Stats);
    assert_eq!(nested.parent(), Screen::Settings);
    nested.open(Screen::Stats);
    assert_eq!(
        nested.parent(),
        Screen::Settings,
        "a screen must not parent itself"
    );
    nested.open(Screen::Help);
    nested.handle_event(key(Key::Esc), now).unwrap();
    assert_eq!(nested.screen(), Screen::Stats);
    assert_eq!(nested.parent(), Screen::Settings);
    nested.handle_event(key(Key::Esc), now).unwrap();
    assert_eq!(nested.screen(), Screen::Settings);
}

#[test]
fn q_commands_follow_the_escape_hierarchy_in_both_keyboard_layouts() {
    for command in ['q', 'ㅂ'] {
        let (_root, mut app) = fixture_app();
        let now = Instant::now();
        app.open(Screen::Settings);
        app.open(Screen::Stats);

        app.handle_event(key(Key::Char(command)), now).unwrap();
        assert_eq!(app.screen(), Screen::Settings, "{command}");
        assert!(!app.should_quit(), "{command}");

        app.handle_event(key(Key::Char(command)), now).unwrap();
        assert_eq!(app.screen(), Screen::Home, "{command}");
        assert!(!app.should_quit(), "{command}");

        app.handle_event(key(Key::Char(command)), now).unwrap();
        assert!(app.should_quit(), "{command}");
    }
}

#[test]
fn escape_restores_the_departure_focus_at_every_nested_level() {
    let (_root, mut app) = fixture_app();
    let now = Instant::now();

    press(&mut app, Key::Tab, 7, now);
    app.handle_event(key(Key::Enter), now).unwrap();
    press(&mut app, Key::Tab, 3, now);
    app.handle_event(key(Key::Enter), now).unwrap();
    app.handle_event(key(Key::Esc), now).unwrap();
    assert_eq!((app.screen(), app.focus()), (Screen::Stats, 3));
    app.handle_event(key(Key::Esc), now).unwrap();
    assert_eq!((app.screen(), app.focus()), (Screen::Home, 7));

    app.open(Screen::Settings);
    press(&mut app, Key::Tab, 2, now);
    app.handle_event(key(Key::Enter), now).unwrap();
    app.open(Screen::Help);
    app.handle_event(key(Key::Esc), now).unwrap();
    assert_eq!((app.screen(), app.focus()), (Screen::Themes, 0));
    app.handle_event(key(Key::Esc), now).unwrap();
    assert_eq!((app.screen(), app.focus()), (Screen::Settings, 2));
}

#[test]
fn result_escape_always_returns_home() {
    let (_root, mut app) = fixture_app();
    app.open(Screen::Settings);
    app.open(Screen::Result);

    app.handle_event(key(Key::Esc), Instant::now()).unwrap();

    assert_eq!(app.screen(), Screen::Home);
    assert_eq!(app.parent(), Screen::Home);
}

#[test]
fn global_and_printable_shortcuts_obey_screen_and_key_kind() {
    for screen in Screen::ALL {
        let (_root, mut app) = fixture_app();
        app.open(screen);
        app.handle_event(
            key_with(Key::Char('c'), KeyModifiers::CONTROL, KeyKind::Press),
            Instant::now(),
        )
        .unwrap();
        assert!(app.should_quit(), "{screen:?}");
    }

    let (_root, mut released) = fixture_app();
    released
        .handle_event(InputEvent::Ignored, Instant::now())
        .unwrap();
    assert!(!released.should_quit());

    let (_root, mut outside) = fixture_app();
    outside
        .handle_event(key(Key::Char('q')), Instant::now())
        .unwrap();
    assert!(outside.should_quit());

    let (_root, mut repeat) = fixture_app();
    repeat
        .handle_event(
            key_with(Key::Char('q'), KeyModifiers::NONE, KeyKind::Repeat),
            Instant::now(),
        )
        .unwrap();
    assert!(repeat.should_quit());

    let (_root, mut modified) = fixture_app();
    modified
        .handle_event(
            key_with(Key::Char('q'), KeyModifiers::OTHER, KeyKind::Press),
            Instant::now(),
        )
        .unwrap();
    assert!(!modified.should_quit(), "modified q must not navigate");

    let (_root, mut help) = fixture_app();
    help.open(Screen::Stats);
    help.handle_event(
        key_with(Key::Char('?'), KeyModifiers::SHIFT, KeyKind::Press),
        Instant::now(),
    )
    .unwrap();
    assert_eq!(help.screen(), Screen::Help);
    help.handle_event(key(Key::Esc), Instant::now()).unwrap();
    assert_eq!(help.screen(), Screen::Stats);

    let (_root, mut practice) = fixture_app();
    practice
        .start_mode(
            request(
                PracticeKind::Words,
                Language::En,
                "q?jkz",
                StopRule::TargetEnd,
            ),
            Instant::now(),
        )
        .unwrap();
    for (index, printable) in ['q', '?', 'j', 'k'].into_iter().enumerate() {
        practice
            .handle_event(key(Key::Char(printable)), Instant::now())
            .unwrap();
        assert_eq!(practice.screen(), Screen::Practice);
        assert_eq!(practice.focus(), 0);
        assert!(!practice.should_quit());
        assert_eq!(
            practice
                .active_practice()
                .unwrap()
                .engine
                .metrics(Instant::now())
                .attempted_units,
            (index + 1) as u64
        );
    }
}

#[test]
fn focus_keys_wrap_and_home_enter_opens_the_exact_static_action() {
    let (_root, mut app) = fixture_app();
    let now = Instant::now();

    app.handle_event(key(Key::BackTab), now).unwrap();
    assert_eq!(app.focus(), 10);
    app.handle_event(key(Key::Enter), now).unwrap();
    assert_eq!(app.screen(), Screen::Settings);
    assert_eq!(app.focus(), 0);

    for backward in [Key::Up, Key::Char('k')] {
        app.open(Screen::Home);
        app.handle_event(key(backward), now).unwrap();
        assert_eq!(app.focus(), 10);
    }

    for forward in [Key::Tab, Key::Down, Key::Char('j')] {
        app.open(Screen::Home);
        for _ in 0..11 {
            app.handle_event(key(forward), now).unwrap();
        }
        assert_eq!(app.focus(), 0);
    }

    app.handle_event(InputEvent::Ignored, now).unwrap();
    assert_eq!(app.focus(), 0);
}

#[test]
fn non_key_events_do_not_change_domain_state() {
    let (_root, mut app) = fixture_app();
    let now = Instant::now();
    app.start_mode(
        request(PracticeKind::Words, Language::En, "ab", StopRule::Items(2)),
        now,
    )
    .unwrap();
    let before_request = app.retry_request().unwrap().clone();
    let before_metrics = app.active_practice().unwrap().engine.metrics(now);

    app.handle_event(InputEvent::Ignored, now).unwrap();

    assert_eq!(app.screen(), Screen::Practice);
    assert_eq!(app.parent(), Screen::Home);
    assert_eq!(app.focus(), 0);
    assert!(!app.should_quit());
    assert_eq!(app.retry_request(), Some(&before_request));
    let active = app.active_practice().unwrap();
    assert_eq!(active.engine.metrics(now), before_metrics);
    assert_eq!(active.status, None);
    assert!(app.result.is_none());
}
