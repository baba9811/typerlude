use super::support::*;

#[test]
fn practice_uses_role_styles_and_places_the_unicode_input_cursor() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_mode(
        ModeRequest {
            kind: PracticeKind::Sentence,
            language: Language::Ko,
            target: "한x🙂e\u{301}Z".into(),
            mode: PracticeMode::Sentence {
                completed: 0,
                last_item: None,
            },
            stop: StopRule::TargetEnd,
            item_ends: vec![5],
            content_ids: vec!["unicode".into()],
        },
        start,
    )
    .unwrap();
    app.active_practice_mut()
        .unwrap()
        .engine
        .input("한q", start);

    let drawn = draw(&app, 80, 24);
    let styles = default_styles();
    assert_eq!(drawn.cursor, Some((5, 2)));
    assert_eq!(drawn.buffer[(2, 2)].symbol(), "한");
    assert_role_style(&drawn.buffer[(2, 2)], styles.correct);
    assert_eq!(drawn.buffer[(4, 2)].symbol(), "q");
    assert_role_style(&drawn.buffer[(4, 2)], styles.error);
    assert_eq!(drawn.buffer[(5, 5)].symbol(), "🙂");
    assert_role_style(&drawn.buffer[(5, 5)], styles.cursor);
    assert_eq!(drawn.buffer[(7, 5)].symbol(), "é");
    assert_role_style(&drawn.buffer[(7, 5)], styles.dim);
}

#[test]
fn non_key_practice_separates_actual_input_from_prompt() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_mode(
        ModeRequest {
            kind: PracticeKind::Sentence,
            language: Language::En,
            target: "hello world".into(),
            mode: PracticeMode::Sentence {
                completed: 0,
                last_item: None,
            },
            stop: StopRule::TargetEnd,
            item_ends: vec![11],
            content_ids: vec!["hello-world".into()],
        },
        start,
    )
    .unwrap();
    type_text(&mut app, "hex", start);

    let drawn = draw(&app, 80, 24);
    let output = buffer_text(&drawn.buffer);
    assert!(output.contains("Input"), "{output}");
    assert!(output.contains("Prompt"), "{output}");
    assert!(output.contains("hex"), "{output}");
    assert!(output.contains("hello world"), "{output}");
    let styles = default_styles();
    let actual_error = drawn
        .buffer
        .content
        .iter()
        .find(|cell| cell.symbol() == "x")
        .unwrap();
    assert_role_style(actual_error, styles.error);
    assert_eq!(drawn.cursor, Some((5, 2)));
}

#[test]
fn practice_cursor_handles_wrapping_wrong_full_length_and_bounds() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_mode(
        request(
            PracticeKind::Sentence,
            Language::Ko,
            "한ab🙂c",
            StopRule::TargetEnd,
        ),
        start,
    )
    .unwrap();
    let active = app.active_practice_mut().unwrap();
    active.engine.input("한ab", start);
    assert_eq!(practice_cursor(Rect::new(5, 7, 4, 2), active), Some((5, 8)));

    active.engine.input("x?", start);
    assert_eq!(active.engine.cursor(), active.engine.target_len());
    assert_eq!(practice_cursor(Rect::new(5, 7, 4, 2), active), Some((8, 8)));
    assert_eq!(practice_cursor(Rect::new(5, 7, 4, 1), active), Some((8, 7)));
    assert_eq!(practice_cursor(Rect::new(5, 7, 0, 2), active), None);
    assert_eq!(practice_cursor(Rect::new(5, 7, 4, 0), active), None);
}

#[test]
fn practice_cursor_matches_the_rendered_row_for_an_oversized_grapheme() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_mode(
        request(
            PracticeKind::Sentence,
            Language::En,
            "🙂a",
            StopRule::TargetEnd,
        ),
        start,
    )
    .unwrap();
    let active = app.active_practice_mut().unwrap();
    active.engine.input("🙂", start);

    assert_eq!(practice_cursor(Rect::new(5, 7, 1, 3), active), Some((5, 8)));
}

#[test]
fn practice_prompt_shows_only_nearby_logical_lines() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    let target = (0..30)
        .map(|index| format!("item{index:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    app.start_mode(
        ModeRequest {
            kind: PracticeKind::Sentence,
            language: Language::En,
            target: target.clone(),
            mode: PracticeMode::Sentence {
                completed: 0,
                last_item: None,
            },
            stop: StopRule::ActiveTime(Duration::from_secs(60)),
            item_ends: vec![target.chars().count()],
            content_ids: vec!["long-sentence".into()],
        },
        start,
    )
    .unwrap();
    type_text(&mut app, &target[..target.find("item20").unwrap()], start);

    let drawn = draw(&app, 80, 24);
    let output = buffer_text(&drawn.buffer);
    assert!(output.contains("item19"), "{output}");
    assert!(output.contains("item20"), "{output}");
    assert!(output.contains("item21"), "{output}");
    assert!(!output.contains("item00"), "{output}");
    assert_eq!(drawn.cursor, Some((2, 2)));
}

#[test]
fn every_requested_kind_preserves_mode_stop_metadata_and_engine_language() {
    let cases = [
        (PracticeKind::Quick, Language::En, "a", 1),
        (PracticeKind::Key, Language::Ko, "한", 3),
        (PracticeKind::Words, Language::En, "a", 1),
        (PracticeKind::Sentence, Language::Ko, "한", 3),
        (PracticeKind::Long, Language::En, "a", 1),
        (PracticeKind::Test, Language::Ko, "한", 3),
    ];

    for (index, (kind, language, target, correct_units)) in cases.into_iter().enumerate() {
        let (_root, mut app) = fixture_app();
        let now = Instant::now();
        let stop = if index == 0 {
            StopRule::ActiveTime(Duration::from_secs(30))
        } else {
            StopRule::Items(index + 1)
        };
        let requested = request(kind, language, target, stop.clone());
        let expected_mode = requested.mode.clone();
        let expected_item_ends = requested.item_ends.clone();
        let expected_content_ids = requested.content_ids.clone();

        app.start_mode(requested.clone(), now).unwrap();

        assert_eq!(app.screen(), Screen::Practice);
        assert_eq!(app.parent(), Screen::Home);
        assert_eq!(app.retry_request(), Some(&requested));
        let active = app.active_practice_mut().unwrap();
        assert_eq!(active.kind(), kind);
        assert_eq!(active.mode, expected_mode);
        assert_eq!(active.stop, stop);
        assert_eq!(active.item_ends, expected_item_ends);
        assert_eq!(active.content_ids, expected_content_ids);
        assert_eq!(active.status, None);
        assert_eq!(active.engine.input(target, now), InputOutcome::Finished);
        assert_eq!(active.engine.metrics(now).correct_units, correct_units);
        assert_eq!(active.engine.toggle_pause(now), kind != PracticeKind::Test);
    }
}

#[test]
fn active_time_limit_is_passed_to_the_engine_and_retry_is_exact() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    let requested = request(
        PracticeKind::Quick,
        Language::En,
        "ab",
        StopRule::ActiveTime(Duration::from_secs(5)),
    );
    app.start_mode(requested.clone(), start).unwrap();
    assert_eq!(
        app.active_practice().unwrap().observed_input_language(),
        None
    );
    app.handle_event(key(Key::Char('a')), start).unwrap();
    let active = app.active_practice().unwrap();
    assert_eq!(active.observed_input_language(), Some(Language::En));
    assert!(!active.engine.is_finished(start + Duration::from_secs(4)));
    assert!(active.engine.is_finished(start + Duration::from_secs(5)));

    app.open(Screen::Result);
    app.result = Some(result_view("stale-retry-result"));
    app.handle_event(key(Key::Char('r')), start + Duration::from_secs(6))
        .unwrap();

    assert_eq!(app.screen(), Screen::Practice);
    assert!(app.result.is_none());
    assert_eq!(app.retry_request(), Some(&requested));
    let retried = app.active_practice_mut().unwrap();
    assert_eq!(retried.mode, requested.mode);
    assert_eq!(retried.stop, requested.stop);
    assert_eq!(retried.item_ends, requested.item_ends);
    assert_eq!(retried.content_ids, requested.content_ids);
    assert_eq!(retried.engine.metrics(start).attempted_units, 0);
    assert_eq!(retried.observed_input_language(), None);
    assert_eq!(retried.engine.input("ab", start), InputOutcome::Finished);
}

#[test]
fn practice_shows_observed_input_language_and_preserves_scoring() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_mode(
        request(
            PracticeKind::Words,
            Language::En,
            "abcd",
            StopRule::TargetEnd,
        ),
        start,
    )
    .unwrap();

    assert_eq!(
        app.active_practice().unwrap().observed_input_language(),
        None
    );
    assert!(buffer_text(&draw(&app, 80, 24).buffer).contains("Practice EN · Input —"));

    app.handle_event(key(Key::Char('한')), start).unwrap();
    assert_eq!(
        app.active_practice().unwrap().observed_input_language(),
        Some(Language::Ko)
    );
    let drawn = draw(&app, 80, 24);
    let output = buffer_text(&drawn.buffer);
    assert!(output.contains("Practice EN · Input KO ⚠"), "{output}");
    let styles = default_styles();
    let warning = drawn
        .buffer
        .content
        .iter()
        .find(|cell| cell.symbol() == "⚠")
        .unwrap();
    assert_role_style(warning, styles.error);

    let attempted = app.active_practice().unwrap().engine.attempted_units();
    app.handle_event(key(Key::Char('!')), start).unwrap();
    assert_eq!(
        app.active_practice().unwrap().observed_input_language(),
        Some(Language::Ko)
    );
    assert!(app.active_practice().unwrap().engine.attempted_units() > attempted);
    app.handle_event(key(Key::Char('a')), start).unwrap();
    assert_eq!(
        app.active_practice().unwrap().observed_input_language(),
        Some(Language::En)
    );

    let (_root, mut korean) = fixture_app();
    korean.settings.ui_language = Language::Ko;
    korean
        .start_mode(
            request(
                PracticeKind::Words,
                Language::En,
                "abc",
                StopRule::TargetEnd,
            ),
            start,
        )
        .unwrap();
    korean.handle_event(key(Key::Char('한')), start).unwrap();
    let output = buffer_text(&draw(&korean, 80, 24).buffer);
    assert!(output.contains("연습 EN · 입력 한글 ⚠"), "{output}");
}

#[test]
fn invalid_start_is_transactional_for_all_owned_state() {
    let (_root, mut app) = fixture_app();
    let now = Instant::now();
    let valid = request(PracticeKind::Words, Language::En, "ab", StopRule::Items(2));
    app.start_mode(valid.clone(), now).unwrap();
    assert_eq!(
        app.active_practice_mut().unwrap().engine.input("a", now),
        InputOutcome::Accepted
    );
    app.result = Some(result_view("preserved"));
    app.open(Screen::Settings);
    app.handle_event(key(Key::Tab), now).unwrap();

    let screen = app.screen();
    let parent = app.parent();
    let focus = app.focus();
    let quit = app.should_quit();
    let retry = app.retry_request().unwrap().clone();
    let result = app.result.clone();
    let active = app.active_practice().unwrap();
    let mode = active.mode.clone();
    let stop = active.stop.clone();
    let item_ends = active.item_ends.clone();
    let content_ids = active.content_ids.clone();
    let metrics = active.engine.metrics(now);

    let invalid = request(
        PracticeKind::Words,
        Language::Ko,
        "",
        StopRule::ActiveTime(Duration::from_secs(60)),
    );
    assert!(app.start_mode(invalid, now).is_err());

    assert_eq!(app.screen(), screen);
    assert_eq!(app.parent(), parent);
    assert_eq!(app.focus(), focus);
    assert_eq!(app.should_quit(), quit);
    assert_eq!(app.retry_request(), Some(&retry));
    assert_eq!(app.result, result);
    let active = app.active_practice().unwrap();
    assert_eq!(active.mode, mode);
    assert_eq!(active.stop, stop);
    assert_eq!(active.item_ends, item_ends);
    assert_eq!(active.content_ids, content_ids);
    assert_eq!(active.engine.metrics(now), metrics);

    let mut mismatched = request(
        PracticeKind::Quick,
        Language::En,
        "new target",
        StopRule::TargetEnd,
    );
    mismatched.mode = PracticeMode::Test { grade: None };
    assert!(app.start_mode(mismatched, now).is_err());
    assert_eq!(app.retry_request(), Some(&retry));
    assert_eq!(app.result, result);
    assert_eq!(app.active_practice().unwrap().engine.metrics(now), metrics);
}

#[test]
fn practice_events_route_text_backspace_pause_paste_and_expiry() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let start = Instant::now();
    app.start_mode(
        request(
            PracticeKind::Words,
            Language::En,
            "aBc",
            StopRule::TargetEnd,
        ),
        start,
    )
    .unwrap();

    app.handle_event(InputEvent::Ignored, start).unwrap();
    app.handle_event(
        key_with(Key::Char('z'), KeyModifiers::CONTROL, KeyKind::Press),
        start,
    )
    .unwrap();
    assert_eq!(
        app.active_practice()
            .unwrap()
            .engine
            .metrics(start)
            .attempted_units,
        0
    );

    app.handle_event(key(Key::Char('x')), start).unwrap();
    app.handle_event(
        key_with(Key::Backspace, KeyModifiers::NONE, KeyKind::Repeat),
        start,
    )
    .unwrap();
    app.handle_event(
        key_with(Key::Backspace, KeyModifiers::NONE, KeyKind::Repeat),
        start,
    )
    .unwrap();
    app.handle_event(key(Key::Char('a')), start).unwrap();
    app.handle_event(
        key_with(Key::Char('B'), KeyModifiers::SHIFT, KeyKind::Press),
        start,
    )
    .unwrap();
    let before_pause = app.active_practice().unwrap().engine.metrics(start);
    assert_eq!(before_pause.attempted_units, 3);
    assert_eq!(before_pause.errors, 1);
    assert_eq!(before_pause.backspaces, 2);

    app.handle_event(
        key_with(Key::Char('p'), KeyModifiers::CONTROL, KeyKind::Press),
        start,
    )
    .unwrap();
    assert!(app.active_practice().unwrap().engine.is_paused());
    assert!(buffer_text(&draw(&app, 80, 24).buffer).contains("Resume"));
    app.handle_event(key(Key::Char('c')), start).unwrap();
    app.handle_event(key(Key::Backspace), start).unwrap();
    assert_eq!(
        app.active_practice().unwrap().engine.metrics(start),
        before_pause
    );

    app.handle_event(key(Key::Esc), start).unwrap();
    assert!(!app.active_practice().unwrap().engine.is_paused());
    let paste_at = start + Duration::from_secs(1);
    app.handle_event(InputEvent::Paste, paste_at).unwrap();
    let after_paste = app.active_practice().unwrap().engine.metrics(paste_at);
    assert_eq!(after_paste.correct_units, before_pause.correct_units);
    assert_eq!(after_paste.attempted_units, before_pause.attempted_units);
    assert_eq!(after_paste.errors, before_pause.errors);
    assert_eq!(after_paste.backspaces, before_pause.backspaces);
    assert_eq!(app.practice_status(), Some("Paste ignored"));
    let pasted = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(pasted.contains("Paste ignored"));
    assert!(pasted.contains("Bc"));

    app.tick(paste_at + Duration::from_millis(2_999)).unwrap();
    assert_eq!(app.practice_status(), Some("Paste ignored"));
    app.tick(paste_at + Duration::from_secs(3)).unwrap();
    assert_eq!(app.practice_status(), None);
    assert!(!buffer_text(&draw(&app, 80, 24).buffer).contains("Paste ignored"));
    app.active_practice_mut().unwrap().status = Some((
        "bad\u{1b}]0;owned\u{7} visible-status".into(),
        paste_at + Duration::from_secs(10),
    ));
    let sanitized = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(!sanitized.contains('\u{1b}'));
    assert!(!sanitized.contains('\u{7}'));
    assert!(sanitized.contains("visible-status"));

    app.handle_event(
        key_with(Key::Char('c'), KeyModifiers::NONE, KeyKind::Repeat),
        paste_at + Duration::from_secs(4),
    )
    .unwrap();
    assert_eq!(app.screen(), Screen::Result);
    let session = &app.result.as_ref().unwrap().session;
    assert_eq!(session.attempted_units, 4);
    assert_eq!(session.correct_units, 3);
    assert_eq!(session.errors, 1);
    assert_eq!(session.backspaces, 2);

    let (_korean_root, mut korean) = fixture_app();
    korean.settings.ui_language = Language::Ko;
    korean
        .start_mode(
            request(
                PracticeKind::Words,
                Language::Ko,
                "한글",
                StopRule::TargetEnd,
            ),
            start,
        )
        .unwrap();
    korean.handle_event(InputEvent::Paste, start).unwrap();
    assert_eq!(korean.practice_status(), Some("붙여넣기 무시됨"));
}

#[test]
fn errors_do_not_block_item_progress_and_backspace_reopens_the_previous_line() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_mode(
        ModeRequest {
            kind: PracticeKind::Sentence,
            language: Language::En,
            target: "ab cd".into(),
            mode: PracticeMode::Sentence {
                completed: 0,
                last_item: None,
            },
            stop: StopRule::TargetEnd,
            item_ends: vec![3, 5],
            content_ids: vec!["first".into(), "second".into()],
        },
        start,
    )
    .unwrap();

    type_text(&mut app, "ax", start);
    app.handle_event(key(Key::Enter), start).unwrap();
    assert_catalog_progress(&app, 1);
    assert_eq!(app.active_practice().unwrap().engine.cursor(), 3);

    app.handle_event(key(Key::Backspace), start).unwrap();
    assert_eq!(app.active_practice().unwrap().engine.cursor(), 3);
    let reopened = draw(&app, 80, 24);
    let styles = default_styles();
    assert_eq!(reopened.buffer[(4, 2)].symbol(), "·");
    assert_role_style(&reopened.buffer[(4, 2)], styles.error);
    app.handle_event(key(Key::Backspace), start).unwrap();
    let active = app.active_practice().unwrap();
    assert_eq!(active.engine.cursor(), 2);
    assert_eq!(active.engine.metrics(start).errors, 2);
    assert_catalog_progress(&app, 1);
}

#[test]
fn practice_rejects_modified_enter() {
    let (_root, mut app) = fixture_app();
    let now = Instant::now();
    app.start_mode(
        request(
            PracticeKind::Sentence,
            Language::En,
            "a\nb",
            StopRule::TargetEnd,
        ),
        now,
    )
    .unwrap();
    app.handle_event(key(Key::Char('a')), now).unwrap();

    for modifiers in [KeyModifiers::OTHER, KeyModifiers::CONTROL] {
        app.handle_event(key_with(Key::Enter, modifiers, KeyKind::Press), now)
            .unwrap();
    }

    let active = app.active_practice().unwrap();
    assert_eq!(active.engine.cursor(), 1);
    assert_eq!(active.engine.attempted_units(), 1);
}

#[test]
fn active_time_finishes_from_tick_not_from_target_exhaustion_and_saves_privately() {
    let (root, mut app) = fixture_app();
    app.warnings.clear();
    let start = Instant::now();
    let private_target = "private target phrase";
    app.start_mode(
        ModeRequest {
            content_ids: vec!["fixture-content".into()],
            ..request(
                PracticeKind::Test,
                Language::En,
                private_target,
                StopRule::ActiveTime(Duration::from_secs(5)),
            )
        },
        start,
    )
    .unwrap();

    let before = OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    app.handle_event(key(Key::Char('p')), start).unwrap();
    let after = OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    app.handle_event(InputEvent::Paste, start + Duration::from_secs(1))
        .unwrap();
    assert_eq!(app.screen(), Screen::Practice);
    app.tick(start + Duration::from_secs(4)).unwrap();
    assert_eq!(app.screen(), Screen::Practice);
    app.tick(start + Duration::from_secs(5)).unwrap();
    assert_eq!(app.screen(), Screen::Result);

    let result = app.result.as_ref().unwrap();
    assert!(result.save_error.is_none());
    assert_eq!(result.session.duration_ms, 5_000);
    assert!(result.session.started_at_unix_ms >= before);
    assert!(result.session.started_at_unix_ms <= after);
    assert_eq!(result.session.content_id, "fixture-content");
    assert_eq!(app.sessions.len(), 1);
    assert_eq!(app.sessions[0], result.session);

    let paths = fs::read_dir(&app.paths.sessions)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1);
    let json = fs::read_to_string(&paths[0]).unwrap();
    for private in [
        private_target,
        "private paste material",
        "\"target\"",
        "\"typed\"",
    ] {
        assert!(
            !json.contains(private),
            "private value leaked: {private}: {json}"
        );
    }
    assert!(root.path().exists());
}

#[test]
fn a_late_timed_poll_uses_the_selected_limit_for_session_daily_total_and_speed() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    app.settings.daily_minutes = 1;
    let start = Instant::now();
    app.start_mode(
        ModeRequest {
            content_ids: vec!["late-test".into()],
            ..request(
                PracticeKind::Test,
                Language::En,
                "abcde",
                StopRule::ActiveTime(Duration::from_secs(60)),
            )
        },
        start,
    )
    .unwrap();
    type_text(&mut app, "abcde", start);

    app.tick(start + Duration::from_secs(90)).unwrap();

    let result = app.result.as_ref().unwrap();
    assert_eq!(result.session.duration_ms, 60_000);
    assert_eq!(result.session.wpm, 1.0);
    assert!(result.daily_minutes_met);
    assert_eq!(summarize(&app.sessions).total, Duration::from_secs(60));
}

#[test]
fn quick_presets_and_catalog_selection_are_exact_and_seeded() {
    for seconds in [15, 30, 60, 120] {
        assert!(
            QuickOptions::new(
                Language::En,
                QuickSource::Words,
                StopRule::ActiveTime(Duration::from_secs(seconds)),
            )
            .is_ok()
        );
    }
    for count in [10, 25, 50, 100] {
        assert!(
            QuickOptions::new(Language::Ko, QuickSource::Quote, StopRule::Items(count),).is_ok()
        );
    }
    for stop in [
        StopRule::TargetEnd,
        StopRule::ActiveTime(Duration::from_secs(42)),
        StopRule::Items(0),
        StopRule::Items(11),
    ] {
        assert!(QuickOptions::new(Language::En, QuickSource::Words, stop).is_err());
    }

    let start = Instant::now();
    let (_left_root, mut left) = fixture_app();
    let (_right_root, mut right) = fixture_app();
    let options = QuickOptions::new(Language::En, QuickSource::Words, StopRule::Items(10)).unwrap();
    left.start_quick(options.clone(), 7, start).unwrap();
    right.start_quick(options, 7, start).unwrap();
    let left_active = left.active_practice().unwrap();
    let right_active = right.active_practice().unwrap();
    assert_eq!(left_active.content_ids, right_active.content_ids);
    assert_eq!(left_active.content_ids.len(), 10);
    assert_eq!(left_active.item_ends.len(), 10);
    assert_eq!(
        left_active.item_ends.last().copied(),
        Some(left_active.engine.target_len())
    );
    assert!(
        left_active
            .item_ends
            .windows(2)
            .all(|ends| ends[0] < ends[1])
    );
    let first_end = left_active.item_ends[0];
    assert_eq!(
        left_active
            .engine
            .target_cells()
            .nth(first_end - 1)
            .unwrap()
            .0,
        " "
    );
    assert!(left_active.content_ids.iter().all(|id| {
        left.content
            .items()
            .any(|item| item.id == *id && item.kind == ContentKind::Word)
    }));

    let (_quote_root, mut quotes) = fixture_app();
    quotes
        .start_quick(
            QuickOptions::new(Language::Ko, QuickSource::Quote, StopRule::Items(100)).unwrap(),
            9,
            start,
        )
        .unwrap();
    let quote_active = quotes.active_practice().unwrap();
    assert_eq!(quote_active.content_ids.len(), 100);
    assert!(quote_active.content_ids.iter().all(|id| {
        quotes
            .content
            .items()
            .any(|item| item.id == *id && item.kind == ContentKind::Quote)
    }));
    assert_eq!(
        quote_active
            .engine
            .target_cells()
            .nth(quote_active.item_ends[0] - 1)
            .unwrap()
            .0,
        "\n"
    );

    let (_word_root, mut words) = fixture_app();
    let mut weak_history = result_view("weak-history").session;
    weak_history.intended_keys.insert('x', [0, 10]);
    words.sessions.push(weak_history);
    let expected_first = adaptive_candidates(&words.content, &words.sessions, Language::En, 11)
        .into_iter()
        .find(|item| item.kind == ContentKind::Word && item.difficulty == Some(1))
        .unwrap()
        .id
        .clone();
    words
        .start_words(Language::En, Difficulty::Easy, 11, start)
        .unwrap();
    let word_active = words.active_practice().unwrap();
    assert_eq!(word_active.content_ids.first(), Some(&expected_first));
    assert!(word_active.content_ids.iter().all(|id| {
        words
            .content
            .items()
            .any(|item| item.id == *id && item.difficulty == Some(1))
    }));

    let (_sentence_root, mut sentences) = fixture_app();
    sentences.start_sentence(Language::Ko, 19, start).unwrap();
    let sentence_active = sentences.active_practice().unwrap();
    let first_end = sentence_active.item_ends[0];
    assert_eq!(
        sentence_active
            .engine
            .target_cells()
            .nth(first_end - 1)
            .unwrap()
            .0,
        "\n"
    );
    assert!(
        sentences
            .active_practice()
            .unwrap()
            .content_ids
            .iter()
            .all(|id| {
                sentences.content.items().any(|item| {
                    item.id == *id
                        && item.language == Language::Ko
                        && matches!(item.kind, ContentKind::Sentence | ContentKind::Quote)
                })
            })
    );
}

#[test]
fn public_progress_counters_saturate() {
    let now = Instant::now();
    for (kind, mode) in [
        (
            PracticeKind::Quick,
            PracticeMode::Quick {
                completed: usize::MAX,
            },
        ),
        (
            PracticeKind::Words,
            PracticeMode::Words {
                difficulty: Difficulty::Easy,
                completed: usize::MAX,
                streak: usize::MAX,
            },
        ),
        (
            PracticeKind::Sentence,
            PracticeMode::Sentence {
                completed: usize::MAX,
                last_item: None,
            },
        ),
    ] {
        let (_root, mut app) = fixture_app();
        app.start_mode(
            ModeRequest {
                kind,
                language: Language::En,
                target: "a".into(),
                mode,
                stop: StopRule::ActiveTime(Duration::from_secs(60)),
                item_ends: vec![1],
                content_ids: vec!["item".into()],
            },
            now,
        )
        .unwrap();
        app.handle_event(key(Key::Char('a')), now).unwrap();

        match &app.active_practice().unwrap().mode {
            PracticeMode::Quick { completed } | PracticeMode::Sentence { completed, .. } => {
                assert_eq!(*completed, usize::MAX);
            }
            PracticeMode::Words {
                completed, streak, ..
            } => {
                assert_eq!((*completed, *streak), (usize::MAX, usize::MAX));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn words_and_sentences_advance_from_engine_boundaries_without_resetting_it() {
    let start = Instant::now();
    let (_word_root, mut words) = fixture_app();
    words
        .start_mode(
            ModeRequest {
                kind: PracticeKind::Words,
                language: Language::En,
                target: "one two".into(),
                mode: PracticeMode::Words {
                    difficulty: Difficulty::Easy,
                    completed: 0,
                    streak: 0,
                },
                stop: StopRule::TargetEnd,
                item_ends: vec![4, 7],
                content_ids: vec!["one".into(), "two".into()],
            },
            start,
        )
        .unwrap();
    type_text(&mut words, "one", start);
    assert_eq!(words.word_progress(), (0, 0));
    type_text(&mut words, " ", start);
    assert_eq!(words.word_progress(), (1, 1));
    type_text(&mut words, "tx", start + Duration::from_secs(1));
    assert_eq!(words.word_progress(), (1, 0));
    words
        .handle_event(key(Key::Backspace), start + Duration::from_secs(2))
        .unwrap();
    type_text(&mut words, "wo", start + Duration::from_secs(2));
    assert_eq!(words.screen(), Screen::Result);
    assert_eq!(words.result.as_ref().unwrap().session.errors, 1);
    assert_eq!(words.result.as_ref().unwrap().session.backspaces, 1);

    let (_sentence_root, mut sentences) = fixture_app();
    sentences
        .start_mode(
            ModeRequest {
                kind: PracticeKind::Sentence,
                language: Language::En,
                target: "First.\nSecond.".into(),
                mode: PracticeMode::Sentence {
                    completed: 0,
                    last_item: None,
                },
                stop: StopRule::TargetEnd,
                item_ends: vec![7, 14],
                content_ids: vec!["first".into(), "second".into()],
            },
            start,
        )
        .unwrap();
    type_text(&mut sentences, "First.", start);
    assert!(sentences.sentence_delta().is_none());
    type_text(&mut sentences, "\n", start);
    assert_eq!(sentences.screen(), Screen::Practice);
    let delta = sentences.sentence_delta().unwrap();
    assert_eq!(delta.correct_units, 7);
    assert_eq!(delta.errors, 0);
    assert_eq!(delta.accuracy, 100.0);
    sentences
        .tick(start + Duration::from_millis(2_999))
        .unwrap();
    assert!(sentences.sentence_delta().is_some());
    sentences.tick(start + Duration::from_secs(3)).unwrap();
    assert!(sentences.sentence_delta().is_none());
}

#[test]
fn word_enter_accepts_the_item_separator() {
    let start = Instant::now();
    let (_root, mut app) = fixture_app();
    app.start_mode(
        ModeRequest {
            kind: PracticeKind::Words,
            language: Language::Ko,
            target: "한글 떡".into(),
            mode: PracticeMode::Words {
                difficulty: Difficulty::Easy,
                completed: 0,
                streak: 0,
            },
            stop: StopRule::TargetEnd,
            item_ends: vec![3, 4],
            content_ids: vec!["한글".into(), "떡".into()],
        },
        start,
    )
    .unwrap();

    type_text(&mut app, "한글", start);
    app.handle_event(key(Key::Enter), start).unwrap();

    assert_eq!(app.screen(), Screen::Practice);
    assert_eq!(app.word_progress(), (1, 1));
    let active = app.active_practice().unwrap();
    assert_eq!(active.engine.cursor(), 3);
    assert_eq!(active.engine.metrics(start).errors, 0);
}

#[test]
fn word_space_submits_an_incomplete_item() {
    let start = Instant::now();
    let (_root, mut app) = fixture_app();
    app.start_mode(
        ModeRequest {
            kind: PracticeKind::Words,
            language: Language::En,
            target: "cat dog".into(),
            mode: PracticeMode::Words {
                difficulty: Difficulty::Easy,
                completed: 0,
                streak: 0,
            },
            stop: StopRule::TargetEnd,
            item_ends: vec![4, 7],
            content_ids: vec!["cat".into(), "dog".into()],
        },
        start,
    )
    .unwrap();

    type_text(&mut app, "c", start);
    app.handle_event(key(Key::Char(' ')), start).unwrap();

    assert_eq!(app.screen(), Screen::Practice);
    assert_eq!(app.word_progress(), (1, 0));
    let active = app.active_practice().unwrap();
    assert_eq!(active.engine.cursor(), 4);
    assert_eq!(active.engine.metrics(start).errors, 2);
}

#[test]
fn timed_quick_extends_before_exhaustion_and_item_quick_stops_exactly() {
    let start = Instant::now();
    let (_timed_root, mut timed) = fixture_app();
    timed
        .start_quick(
            QuickOptions::new(
                Language::En,
                QuickSource::Words,
                StopRule::ActiveTime(Duration::from_secs(120)),
            )
            .unwrap(),
            23,
            start,
        )
        .unwrap();
    let initial_items = timed.active_practice().unwrap().item_ends.len();
    let initial_ids = timed.active_practice().unwrap().content_ids.clone();
    assert!(initial_items >= 10);
    let trigger_end = timed.active_practice().unwrap().item_ends[initial_items - 10];
    let prefix = timed
        .active_practice()
        .unwrap()
        .engine
        .target_cells()
        .take(trigger_end)
        .map(|(grapheme, _)| grapheme)
        .collect::<String>();
    type_text(&mut timed, &prefix, start);
    assert_eq!(timed.screen(), Screen::Practice);
    assert!(timed.active_practice().unwrap().item_ends.len() > initial_items);
    assert!(timed.active_practice().unwrap().engine.target_len() > trigger_end);
    timed.tick(start + Duration::from_secs(119)).unwrap();
    assert_eq!(timed.screen(), Screen::Practice);
    timed.tick(start + Duration::from_secs(120)).unwrap();
    assert_eq!(timed.screen(), Screen::Result);
    timed
        .handle_event(key(Key::Char('r')), start + Duration::from_secs(121))
        .unwrap();
    assert_eq!(timed.active_practice().unwrap().content_ids, initial_ids);
    let retry_prefix = timed
        .active_practice()
        .unwrap()
        .engine
        .target_cells()
        .take(trigger_end)
        .map(|(grapheme, _)| grapheme)
        .collect::<String>();
    type_text(&mut timed, &retry_prefix, start + Duration::from_secs(121));
    assert!(timed.active_practice().unwrap().item_ends.len() > initial_items);

    let (_count_root, mut counted) = fixture_app();
    counted
        .start_quick(
            QuickOptions::new(Language::En, QuickSource::Words, StopRule::Items(10)).unwrap(),
            29,
            start,
        )
        .unwrap();
    let target = counted
        .active_practice()
        .unwrap()
        .engine
        .target_cells()
        .map(|(grapheme, _)| grapheme)
        .collect::<String>();
    type_text(&mut counted, &target, start + Duration::from_secs(60));
    assert_eq!(counted.screen(), Screen::Result);
    assert_eq!(
        counted.result.as_ref().unwrap().session.mode,
        PracticeKind::Quick
    );
}

#[test]
fn practice_renderer_uses_stored_live_and_item_fields() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_mode(
        ModeRequest {
            kind: PracticeKind::Words,
            language: Language::Ko,
            target: "한글".into(),
            mode: PracticeMode::Words {
                difficulty: Difficulty::Easy,
                completed: 0,
                streak: 0,
            },
            stop: StopRule::TargetEnd,
            item_ends: vec![2],
            content_ids: vec!["한글".into()],
        },
        start,
    )
    .unwrap();
    type_text(&mut app, "한", start);
    app.settings.ui_language = Language::Ko;
    app.tick(start + Duration::from_millis(600)).unwrap();
    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for field in ["속도", "정확도", "오류", "연속", "진행"] {
        assert!(output.contains(field), "missing {field}: {output}");
    }
    for value in [
        "속도: 타수 300.0 타/분 · WPM 20.0",
        "현재: 타수 300.0 타/분 · WPM 20.0",
        "평균: 타수 300.0 타/분 · WPM 20.0",
    ] {
        assert!(output.contains(value), "missing {value:?}: {output}");
    }
    let metrics = app.active_practice().unwrap().live_metrics();
    assert_eq!(metrics.correct_units, 3);
    assert_eq!(metrics.attempted_units, 3);
    assert_eq!(
        app.active_practice()
            .unwrap()
            .current_item_delta()
            .unwrap()
            .correct_units,
        3
    );
    let delta = app.active_practice().unwrap().current_item_delta().unwrap();
    assert_eq!(delta.correct_units, 3);
    assert!((delta.kpm - 300.0).abs() < f64::EPSILON * 4.0);
    assert!((delta.wpm - 20.0).abs() < f64::EPSILON * 4.0);
}

#[test]
fn live_metric_visibility_settings_apply_in_both_languages_and_every_mode() {
    let now = Instant::now();
    for language in [Language::Ko, Language::En] {
        let (speed_label, accuracy_label, errors_label, remaining_label, streak_label) =
            match language {
                Language::Ko => ("속도:", "정확도:", "오류:", "남은", "연속"),
                Language::En => ("Speed:", "Accuracy:", "Errors:", "Remaining", "Streak"),
            };
        for (show_speed, show_accuracy) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            for kind in [
                PracticeKind::Quick,
                PracticeKind::Key,
                PracticeKind::Words,
                PracticeKind::Sentence,
                PracticeKind::Long,
                PracticeKind::Test,
            ] {
                let (_root, mut app) = fixture_app();
                app.warnings.clear();
                app.settings.ui_language = language;
                app.settings.show_live_speed = show_speed;
                app.settings.show_accuracy = show_accuracy;
                let stop = if matches!(kind, PracticeKind::Quick | PracticeKind::Test) {
                    StopRule::ActiveTime(Duration::from_secs(60))
                } else {
                    StopRule::TargetEnd
                };
                app.start_mode(request(kind, language, "ab", stop), now)
                    .unwrap();

                let output = buffer_text(&draw(&app, 80, 24).buffer);
                let zero_speeds = match language {
                    Language::Ko => "타수 0.0 타/분 · WPM 0.0",
                    Language::En => "KPM 0.0 · WPM 0.0",
                };
                assert_eq!(
                    output.contains(accuracy_label),
                    show_accuracy,
                    "{language:?} {kind:?} speed={show_speed} accuracy={show_accuracy}: {output}"
                );
                if kind != PracticeKind::Key {
                    assert_eq!(
                        output.contains(speed_label),
                        show_speed,
                        "{language:?} {kind:?} speed={show_speed} accuracy={show_accuracy}: {output}"
                    );
                    assert_eq!(
                        output.contains(&format!("{speed_label} {zero_speeds}")),
                        show_speed,
                        "{language:?} {kind:?} speed={show_speed}: {output}"
                    );
                    for unit in match language {
                        Language::Ko => ["타수", "WPM"],
                        Language::En => ["KPM", "WPM"],
                    } {
                        assert_eq!(
                            output.contains(unit),
                            show_speed,
                            "{language:?} {kind:?} speed={show_speed}: missing/toggled {unit:?}: {output}"
                        );
                    }
                }
                if kind == PracticeKind::Words {
                    for item_speed_label in match language {
                        Language::Ko => ["현재:", "평균:"],
                        Language::En => ["Current:", "Average:"],
                    } {
                        assert_eq!(
                            output.contains(item_speed_label),
                            show_speed,
                            "{language:?} {kind:?}: {output}"
                        );
                        assert_eq!(
                            output.contains(&format!("{item_speed_label} {zero_speeds}")),
                            show_speed,
                            "{language:?} {kind:?}: {output}"
                        );
                    }
                }
                if kind == PracticeKind::Sentence {
                    let item_speeds = match language {
                        Language::Ko => "타수 72.5 타/분 · WPM 14.5",
                        Language::En => "KPM 72.5 · WPM 14.5",
                    };
                    assert_eq!(
                        output.contains(&format!("{speed_label} {item_speeds}")),
                        show_speed,
                        "{language:?} {kind:?}: {output}"
                    );
                }
                assert!(
                    output.contains(errors_label),
                    "{language:?} {kind:?}: {output}"
                );
                assert!(
                    output.contains(match kind {
                        PracticeKind::Quick | PracticeKind::Test => remaining_label,
                        PracticeKind::Key | PracticeKind::Words => streak_label,
                        PracticeKind::Sentence | PracticeKind::Long => match language {
                            Language::Ko => "진행",
                            Language::En => "Progress",
                        },
                    }),
                    "{language:?} {kind:?}: {output}"
                );
            }
        }
    }
}

#[test]
fn paused_q_confirms_early_leave_and_saves_only_after_an_attempt() {
    let start = Instant::now();
    let (_attempted_root, mut attempted) = fixture_app();
    attempted
        .start_mode(
            ModeRequest {
                kind: PracticeKind::Words,
                language: Language::En,
                target: "ab".into(),
                mode: PracticeMode::Words {
                    difficulty: Difficulty::Easy,
                    completed: 0,
                    streak: 0,
                },
                stop: StopRule::TargetEnd,
                item_ends: vec![2],
                content_ids: vec!["ab".into()],
            },
            start,
        )
        .unwrap();
    type_text(&mut attempted, "a", start);
    attempted.handle_event(key(Key::Esc), start).unwrap();
    attempted.handle_event(key(Key::Char('q')), start).unwrap();
    assert_eq!(attempted.screen(), Screen::Practice);
    assert!(buffer_text(&draw(&attempted, 80, 24).buffer).contains("Resume: Esc / Ctrl+P"));
    assert!(buffer_text(&draw(&attempted, 80, 24).buffer).contains("again"));
    assert!(attempted.sessions.is_empty());
    attempted.handle_event(key(Key::Char('q')), start).unwrap();
    assert_eq!(attempted.screen(), Screen::Result);
    assert_eq!(attempted.sessions.len(), 1);

    let (_empty_root, mut empty) = fixture_app();
    empty
        .start_mode(
            request(PracticeKind::Words, Language::En, "ab", StopRule::TargetEnd),
            start,
        )
        .unwrap();
    empty.handle_event(key(Key::Esc), start).unwrap();
    empty.handle_event(key(Key::Char('q')), start).unwrap();
    empty.handle_event(key(Key::Char('q')), start).unwrap();
    assert_eq!(empty.screen(), Screen::Home);
    assert!(empty.result.is_none());
    assert!(empty.sessions.is_empty());
    assert!(!empty.paths.sessions.exists());
}
