use super::support::*;

#[test]
fn typing_tests_refuse_both_pause_keys() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_mode(
        request(
            PracticeKind::Test,
            Language::En,
            "abc",
            StopRule::ActiveTime(Duration::from_secs(60)),
        ),
        start,
    )
    .unwrap();
    let footer = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(!footer.contains("Pause: Esc / Ctrl+P"), "{footer}");
    assert!(footer.contains("Esc: Leave"), "{footer}");

    for pause in [
        key(Key::Esc),
        key_with(Key::Char('p'), KeyModifiers::CONTROL, KeyKind::Press),
    ] {
        app.handle_event(pause, start).unwrap();
        assert_eq!(app.screen(), Screen::Practice);
        assert!(!app.active_practice().unwrap().engine.is_paused());
    }
}

#[test]
fn typing_test_escape_then_q_persists_one_attempted_session() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_mode(
        request(
            PracticeKind::Test,
            Language::En,
            "abc",
            StopRule::ActiveTime(Duration::from_secs(60)),
        ),
        start,
    )
    .unwrap();
    app.handle_event(key(Key::Char('a')), start).unwrap();
    app.handle_event(key(Key::Esc), start).unwrap();

    let confirmation = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(confirmation.contains("Q: Confirm"), "{confirmation}");
    assert!(confirmation.contains("Esc: Cancel"), "{confirmation}");
    assert!(!confirmation.contains("Pause"), "{confirmation}");
    assert!(!app.active_practice().unwrap().engine.is_paused());

    let before_confirmation_input = app.active_practice().unwrap().engine.metrics(start);
    app.handle_event(key(Key::Char('b')), start).unwrap();
    app.handle_event(key(Key::Backspace), start).unwrap();
    app.handle_event(InputEvent::Paste, start).unwrap();
    let active = app.active_practice().unwrap();
    assert_eq!(active.engine.metrics(start), before_confirmation_input);
    assert!(app.practice_status().is_none());

    app.handle_event(key(Key::Char('q')), start).unwrap();

    assert_eq!(app.screen(), Screen::Result);
    assert!(app.result.is_some());
    assert_eq!(app.sessions.len(), 1);
    let session_files = fs::read_dir(&app.paths.sessions)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    assert_eq!(session_files.len(), 1);

    app.handle_event(key(Key::Char('q')), start).unwrap();
    assert_eq!(app.sessions.len(), 1);
    assert_eq!(
        fs::read_dir(&app.paths.sessions)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
            })
            .count(),
        1
    );
}

#[test]
fn empty_typing_test_escape_then_q_returns_home_without_a_session() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_mode(
        request(
            PracticeKind::Test,
            Language::En,
            "abc",
            StopRule::ActiveTime(Duration::from_secs(60)),
        ),
        start,
    )
    .unwrap();

    app.handle_event(key(Key::Esc), start).unwrap();
    app.handle_event(key(Key::Char('q')), start).unwrap();

    assert_eq!(app.screen(), Screen::Home);
    assert!(app.result.is_none());
    assert!(app.sessions.is_empty());
    assert!(!app.paths.sessions.exists());
}

#[test]
fn typing_test_second_escape_cancels_leave_confirmation() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_mode(
        request(
            PracticeKind::Test,
            Language::En,
            "abc",
            StopRule::ActiveTime(Duration::from_secs(60)),
        ),
        start,
    )
    .unwrap();

    app.handle_event(key(Key::Esc), start).unwrap();
    assert!(app.active_practice().unwrap().leave_confirmation());
    app.handle_event(key(Key::Esc), start).unwrap();
    assert!(!app.active_practice().unwrap().leave_confirmation());
    app.handle_event(
        key_with(Key::Char('p'), KeyModifiers::CONTROL, KeyKind::Press),
        start,
    )
    .unwrap();
    app.handle_event(key(Key::Char('a')), start).unwrap();

    let active = app.active_practice().unwrap();
    assert!(!active.engine.is_paused());
    assert_eq!(active.engine.attempted_units(), 1);
}

#[test]
fn a_deadline_crossing_key_is_consumed_instead_of_becoming_a_result_command() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_mode(
        request(
            PracticeKind::Test,
            Language::En,
            "abc",
            StopRule::ActiveTime(Duration::from_secs(1)),
        ),
        start,
    )
    .unwrap();
    app.handle_event(key(Key::Char('a')), start).unwrap();

    app.handle_event(key(Key::Char('r')), start + Duration::from_secs(1))
        .unwrap();

    assert_eq!(app.screen(), Screen::Result);
    assert!(app.result.is_some());
    assert!(app.active_practice().is_none());
    assert_eq!(app.sessions.len(), 1);

    let (_root, mut quit) = fixture_app();
    quit.start_mode(
        request(
            PracticeKind::Test,
            Language::En,
            "abc",
            StopRule::ActiveTime(Duration::from_secs(1)),
        ),
        start,
    )
    .unwrap();
    quit.handle_event(key(Key::Char('a')), start).unwrap();
    quit.handle_event(
        key_with(Key::Char('c'), KeyModifiers::CONTROL, KeyKind::Press),
        start + Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(quit.screen(), Screen::Result);
    assert!(quit.should_quit());
    assert_eq!(quit.sessions.len(), 1);
}

#[test]
fn typing_test_uses_long_texts_and_exposes_random_or_selected_content() {
    let start = Instant::now();
    let (_root, mut options) = fixture_app();
    let selected = options.long_items(Language::En, None)[0];
    let selected_id = selected.id.clone();
    let selected_title = selected.title.clone().unwrap();
    open_mode_options(&mut options, 5, start);
    let random_options = buffer_text(&draw(&options, 80, 24).buffer);
    assert!(random_options.contains("Text: Random"), "{random_options}");
    press(&mut options, Key::Tab, 2, start);
    press(&mut options, Key::Right, 1, start);
    let selected_options = buffer_text(&draw(&options, 80, 24).buffer);
    assert!(
        selected_options.contains(&selected_title),
        "{selected_options}"
    );
    press(&mut options, Key::Tab, 1, start);
    options.handle_event(key(Key::Enter), start).unwrap();
    let active = options.active_practice().unwrap();
    assert_eq!(active.content_ids, [selected_id]);
    assert_eq!(
        active.stop,
        StopRule::TargetOrActiveTime(Duration::from_secs(300))
    );

    let (_root, mut random) = fixture_app();
    random
        .start_test(Language::En, Some(60), None, 7, start)
        .unwrap();
    let active = random.active_practice().unwrap();
    assert_eq!(active.content_ids.len(), 1);
    assert!(random.content.items().any(|item| {
        item.id == active.content_ids[0]
            && item.language == Language::En
            && item.kind == ContentKind::Text
    }));
}

#[test]
fn selected_test_finishes_when_its_text_ends_before_the_timer() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    let item = app.long_items(Language::En, None)[0];
    let id = item.id.clone();
    let target = item.text.clone();
    app.start_test(Language::En, Some(300), Some(&id), 7, start)
        .unwrap();

    type_text(&mut app, &target, start);

    assert_eq!(app.screen(), Screen::Result);
    assert_eq!(app.result.as_ref().unwrap().session.content_id, id);
    assert!(app.result.as_ref().unwrap().session.duration_ms < 300_000);
}

#[test]
fn random_test_continues_with_a_different_long_text_until_time_expires() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_test(Language::En, Some(60), None, 11, start)
        .unwrap();
    let first_id = app.active_practice().unwrap().content_ids[0].clone();
    let first_end = app.active_practice().unwrap().item_ends[0];
    let first = app
        .active_practice()
        .unwrap()
        .engine
        .target_cells()
        .take(first_end)
        .map(|(target, _)| target)
        .collect::<String>();

    type_text(&mut app, &first, start);

    let active = app.active_practice().unwrap();
    assert_eq!(app.screen(), Screen::Practice);
    assert!(active.content_ids.len() >= 2);
    assert_ne!(active.content_ids[1], first_id);
    app.tick(start + Duration::from_secs(60)).unwrap();
    assert_eq!(app.screen(), Screen::Result);
}

#[test]
fn typing_test_uses_allowed_durations_long_text_extension_and_relative_grade() {
    let start = Instant::now();
    for seconds in [60, 180, 300, 600] {
        let (_root, mut app) = fixture_app();
        app.start_test(Language::En, Some(seconds), None, 7, start)
            .unwrap();
        assert_eq!(
            app.active_practice().unwrap().stop,
            StopRule::ActiveTime(Duration::from_secs(seconds))
        );
    }
    let (_invalid_root, mut invalid) = fixture_app();
    assert!(
        invalid
            .start_test(Language::En, Some(120), None, 7, start)
            .is_err()
    );
    assert_eq!(invalid.screen(), Screen::Home);

    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    app.start_test(Language::En, None, None, 11, start).unwrap();
    let active = app.active_practice().unwrap();
    assert_eq!(active.stop, StopRule::ActiveTime(Duration::from_secs(300)));
    assert_eq!(active.content_ids.len(), 1);
    assert!(
        active
            .content_ids
            .iter()
            .all(|id| app.content.items().any(|item| {
                item.id == *id && item.language == Language::En && item.kind == ContentKind::Text
            }))
    );
    assert!(!buffer_text(&draw(&app, 80, 24).buffer).contains("Pause:"));
    assert!(buffer_text(&draw(&app, 80, 24).buffer).contains("Remaining: 300s"));
    assert!(
        !app.active_practice_mut()
            .unwrap()
            .engine
            .toggle_pause(start)
    );

    let initial_len = app.active_practice().unwrap().engine.target_len();
    let first_end = app.active_practice().unwrap().item_ends[0];
    let first = app
        .active_practice()
        .unwrap()
        .engine
        .target_cells()
        .take(first_end)
        .map(|(grapheme, _)| grapheme)
        .collect::<String>();
    type_text(&mut app, &first, start);
    assert!(app.active_practice().unwrap().engine.target_len() > initial_len);
    assert!(app.active_practice().unwrap().content_ids.len() > 1);

    app.tick(start + Duration::from_secs(299)).unwrap();
    assert_eq!(app.screen(), Screen::Practice);
    app.tick(start + Duration::from_secs(300)).unwrap();
    assert_eq!(app.screen(), Screen::Result);
    let result = app.result.as_ref().unwrap();
    assert_eq!(result.session.mode, PracticeKind::Test);
    assert_eq!(
        result.grade,
        Some(grade(
            result.session.wpm,
            f64::from(app.settings.target_wpm),
            result.session.accuracy,
            app.settings.target_accuracy,
        ))
    );
    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("Typerlude relative grade"), "{output}");
}
