use super::support::*;

fn finish_started_practice(app: &mut App, now: Instant) {
    let first = app
        .active_practice()
        .unwrap()
        .engine
        .target_cells()
        .next()
        .unwrap()
        .0
        .to_owned();
    assert!(matches!(
        app.active_practice_mut().unwrap().engine.input(&first, now),
        InputOutcome::Accepted | InputOutcome::Finished
    ));
    app.finish_practice(now + Duration::from_secs(1)).unwrap();
    assert_eq!(app.screen(), Screen::Result);
}

fn type_first_item(app: &mut App, now: Instant) {
    let end = app.active_practice().unwrap().item_ends[0];
    let item = app
        .active_practice()
        .unwrap()
        .engine
        .target_cells()
        .take(end)
        .map(|(grapheme, _)| grapheme)
        .collect::<String>();
    type_text(app, &item, now);
}

fn finish_after_timed_quick_extension(app: &mut App, now: Instant) {
    let initial_items = app.active_practice().unwrap().item_ends.len();
    let trigger_end = app.active_practice().unwrap().item_ends[initial_items - 10];
    let prefix = app
        .active_practice()
        .unwrap()
        .engine
        .target_cells()
        .take(trigger_end)
        .map(|(grapheme, _)| grapheme)
        .collect::<String>();
    type_text(app, &prefix, now);
    assert!(app.active_practice().unwrap().item_ends.len() > initial_items);
    app.finish_practice(now + Duration::from_secs(1)).unwrap();
    assert_eq!(app.screen(), Screen::Result);
}

fn start_result_next_catalog_case(app: &mut App, kind: PracticeKind, seed: u64, now: Instant) {
    match kind {
        PracticeKind::Quick => app
            .start_quick(
                QuickOptions::new(Language::Ko, QuickSource::Quote, StopRule::Items(10)).unwrap(),
                seed,
                now,
            )
            .unwrap(),
        PracticeKind::Words => app
            .start_words(Language::En, Difficulty::Hard, seed, now)
            .unwrap(),
        PracticeKind::Sentence => app.start_sentence(Language::Ko, seed, now).unwrap(),
        _ => unreachable!(),
    }
}

fn assert_result_next_unavailable_and_retry_exact(app: &mut App, now: Instant) {
    let request = app.retry_request().unwrap().clone();
    finish_started_practice(app, now);
    let result = app.result.clone();
    let output = buffer_text(&draw(app, 80, 24).buffer);
    assert!(output.contains("r/ㄱ: Retry"), "{output}");
    assert!(output.contains("Esc: Menu"), "{output}");
    assert!(!output.contains("n: Next"), "{output}");

    app.handle_event(key(Key::Char('n')), now + Duration::from_secs(2))
        .unwrap();
    assert_eq!(app.screen(), Screen::Result);
    assert_eq!(app.result, result);
    assert_eq!(app.retry_request(), Some(&request));

    app.handle_event(key(Key::Char('r')), now + Duration::from_secs(3))
        .unwrap();
    assert_eq!(app.screen(), Screen::Practice);
    assert_eq!(app.retry_request(), Some(&request));
}

#[test]
fn korean_giyeok_retries_the_exact_practice_result() {
    let (_root, mut app) = fixture_app();
    app.settings.ui_language = Language::Ko;
    let now = Instant::now();
    app.start_words(Language::Ko, Difficulty::Easy, 7, now)
        .unwrap();
    let request = app.retry_request().unwrap().clone();
    finish_started_practice(&mut app, now);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("r/ㄱ: 다시 연습"), "{output}");
    app.handle_event(
        key_with(Key::Char('ㄱ'), KeyModifiers::NONE, KeyKind::Repeat),
        now + Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(app.screen(), Screen::Result);

    app.handle_event(key(Key::Char('ㄱ')), now + Duration::from_secs(2))
        .unwrap();

    assert_eq!(app.screen(), Screen::Practice);
    assert_eq!(app.retry_request(), Some(&request));
}

fn assert_custom_long_collision_is_not_next(source: CustomTextSource, item_id: &str) {
    let (_root, mut app) = fixture_app();
    fs::create_dir_all(&app.paths.content).unwrap();
    fs::write(
        app.paths.content.join("collision.toml"),
        user_long_pack(item_id),
    )
    .unwrap();
    let loaded = ContentCatalog::load(&app.paths.content).unwrap();
    assert!(loaded.warnings.is_empty());
    app.content = loaded.catalog;
    assert!(
        app.long_items(Language::En, None)
            .iter()
            .any(|item| item.id == item_id)
    );

    let start = Instant::now();
    app.start_custom_text(source, item_id, "Private custom text", start)
        .unwrap();
    assert_result_next_unavailable_and_retry_exact(&mut app, start);
}

fn user_long_pack(item_id: &str) -> String {
    user_pack("collision")
        .replace("id = \"collision-item\"", &format!("id = \"{item_id}\""))
        .replace("kind = \"word\"", "kind = \"text\"")
}

#[test]
fn populated_result_renders_only_its_stored_outcome_fields() {
    let (_root, mut app) = fixture_app();
    let mut result = result_view("rendered-result");
    result.grade = Some(Grade::B);
    app.result = Some(result);
    app.open(Screen::Result);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for value in [
        "KPM 60.0 · WPM 12.0",
        "Accuracy: 100.0%",
        "Errors: 0",
        "Previous: KPM 50.0 · WPM 10.0",
        "Best: KPM 60.0 · WPM 12.0",
        "KPM +10.0 · WPM +2.0",
        "Typerlude relative grade: B",
        "preserve this result",
    ] {
        assert!(output.contains(value), "missing {value:?}: {output}");
    }
}

#[test]
fn populated_result_renders_stored_goal_and_weak_key_outcomes() {
    let (_root, mut app) = fixture_app();
    let mut result = result_view("goal-result");
    result.weak_keys.push(KeyAccuracy {
        key: 'x',
        correct: 8,
        errors: 2,
        accuracy: 80.0,
    });
    app.result = Some(result);
    app.open(Screen::Result);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for value in [
        "Speed: KPM 60.0 · WPM 12.0 / goal 80 WPM · Goal met",
        "Accuracy: 100.0% / goal 98.0% · Goal met",
        "Daily minutes: goal 15 min · Goal missed",
        "Weak keys: x 80.0%",
    ] {
        assert!(output.contains(value), "missing {value:?}: {output}");
    }
}

#[test]
fn result_shows_snapshot_targets_readable_time_and_compact_weak_keys() {
    let (_root, mut app) = fixture_app();
    let mut result = result_view("compact-result");
    result.session.duration_ms = 2_673_444;
    result.speed_goal = 80.0;
    result.accuracy_goal = 98.0;
    result.daily_minutes_goal = 15;
    result.weak_keys = vec![
        KeyAccuracy {
            key: 'x',
            correct: 8,
            errors: 2,
            accuracy: 80.0,
        },
        KeyAccuracy {
            key: 'y',
            correct: 9,
            errors: 1,
            accuracy: 90.0,
        },
    ];
    app.result = Some(result);
    app.open(Screen::Result);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for value in [
        "Speed: KPM 60.0 · WPM 12.0 / goal 80 WPM · Goal met",
        "Accuracy: 100.0% / goal 98.0% · Goal met",
        "Errors: 0 · Duration: 44 min 33.44 sec",
        "Daily minutes: goal 15 min · Goal missed",
        "Weak keys: x 80.0% · y 90.0%",
    ] {
        assert!(output.contains(value), "missing {value:?}: {output}");
    }
    assert!(!output.contains("2673444 ms"), "{output}");
}

#[test]
fn perfect_keys_are_not_labeled_weak() {
    let (_root, mut app) = fixture_app();
    let mut result = result_view("perfect-result");
    result.session.intended_keys = BTreeMap::from([('a', [10, 0])]);
    app.result = Some(result);
    app.open(Screen::Result);

    let english = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(
        english.contains("No weak keys · All analyzed keys are 100% accurate"),
        "{english}"
    );
    assert!(!english.contains("a: 100.0%"), "{english}");

    app.settings.ui_language = Language::Ko;
    let korean = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(
        korean.contains("약한 키 없음 · 분석된 모든 키 정확도 100%"),
        "{korean}"
    );
}

#[test]
fn result_reserves_space_for_save_failure_before_bounded_weak_rows() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let mut result = result_view("many-weak-keys");
    result.weak_keys = (b'a'..=b'z')
        .map(|key| KeyAccuracy {
            key: char::from(key),
            correct: 1,
            errors: 1,
            accuracy: 50.0,
        })
        .collect();
    app.result = Some(result);
    app.open(Screen::Result);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(
        output.contains("Save failed: preserve this result"),
        "{output}"
    );
    assert!(output.contains("e 50.0%"), "{output}");
    assert!(!output.contains("f 50.0%"), "{output}");
}

#[test]
fn result_next_retains_options_and_builds_fresh_quick_words_sentence_content() {
    let start = Instant::now();

    for (kind, seed) in [
        (PracticeKind::Quick, 41),
        (PracticeKind::Words, 11),
        (PracticeKind::Sentence, 19),
    ] {
        let (_root, mut app) = fixture_app();
        app.settings.ui_language = Language::Ko;
        if kind == PracticeKind::Words {
            let mut weak_history = result_view("next-weak-history").session;
            weak_history.intended_keys.insert('x', [0, 20]);
            app.sessions.push(weak_history);
        }
        start_result_next_catalog_case(&mut app, kind, seed, start);
        type_first_item(&mut app, start);
        assert_catalog_progress(&app, 1);
        app.finish_practice(start + Duration::from_secs(1)).unwrap();
        assert_eq!(app.screen(), Screen::Result);
        if kind == PracticeKind::Quick {
            let output = buffer_text(&draw(&app, 80, 24).buffer);
            for action in ["r/ㄱ: 다시 연습", "n: 다음", "Esc: 메뉴"] {
                assert!(output.contains(action), "missing {action:?}: {output}");
            }
        }

        let (_expected_root, mut expected) = fixture_app();
        expected.sessions = app.sessions.clone();
        if kind == PracticeKind::Words {
            app.settings.adaptive = false;
        }
        start_result_next_catalog_case(
            &mut expected,
            kind,
            seed + 1,
            start + Duration::from_secs(2),
        );
        let expected_request = expected.retry_request().unwrap().clone();

        app.handle_event(key(Key::Char('n')), start + Duration::from_secs(2))
            .unwrap();

        assert_eq!(app.screen(), Screen::Practice);
        assert_eq!(app.retry_request(), Some(&expected_request), "{kind:?}");
        assert_catalog_progress(&app, 0);

        if kind == PracticeKind::Quick {
            finish_started_practice(&mut app, start + Duration::from_secs(3));
            let (_second_root, mut second) = fixture_app();
            second.sessions = app.sessions.clone();
            start_result_next_catalog_case(
                &mut second,
                kind,
                seed + 2,
                start + Duration::from_secs(5),
            );
            let second_request = second.retry_request().unwrap().clone();
            app.handle_event(key(Key::Char('n')), start + Duration::from_secs(5))
                .unwrap();
            assert_eq!(app.retry_request(), Some(&second_request));
        }
    }
}

#[test]
fn result_next_timed_quick_skips_stream_seeds_consumed_before_result() {
    let start = Instant::now();
    let options = QuickOptions::new(
        Language::En,
        QuickSource::Words,
        StopRule::ActiveTime(Duration::from_secs(120)),
    )
    .unwrap();
    let (_root, mut app) = fixture_app();
    app.start_quick(options.clone(), 23, start).unwrap();
    finish_after_timed_quick_extension(&mut app, start);

    let (_expected_root, mut expected) = fixture_app();
    expected.sessions = app.sessions.clone();
    expected
        .start_quick(options.clone(), 25, start + Duration::from_secs(2))
        .unwrap();
    let expected_request = expected.retry_request().unwrap().clone();
    app.handle_event(key(Key::Char('n')), start + Duration::from_secs(2))
        .unwrap();
    assert_eq!(app.retry_request(), Some(&expected_request));

    finish_started_practice(&mut app, start + Duration::from_secs(3));
    let (_second_root, mut second) = fixture_app();
    second.sessions = app.sessions.clone();
    second
        .start_quick(options.clone(), 26, start + Duration::from_secs(5))
        .unwrap();
    let second_request = second.retry_request().unwrap().clone();
    app.handle_event(key(Key::Char('n')), start + Duration::from_secs(5))
        .unwrap();
    assert_eq!(app.retry_request(), Some(&second_request));

    let (_retry_root, mut retry) = fixture_app();
    retry.start_quick(options, 23, start).unwrap();
    let initial_request = retry.retry_request().unwrap().clone();
    finish_after_timed_quick_extension(&mut retry, start);
    retry
        .handle_event(key(Key::Char('r')), start + Duration::from_secs(2))
        .unwrap();
    assert_eq!(retry.screen(), Screen::Practice);
    assert_eq!(retry.retry_request(), Some(&initial_request));
}

#[test]
fn result_next_moves_to_the_next_long_item_and_wraps() {
    let start = Instant::now();
    let (_catalog_root, catalog) = fixture_app();
    let ids = catalog
        .long_items(Language::En, None)
        .into_iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    assert!(ids.len() > 1);

    for (current, next) in [(&ids[0], &ids[1]), (&ids[ids.len() - 1], &ids[0])] {
        let (_root, mut app) = fixture_app();
        app.start_long(current, start).unwrap();
        finish_started_practice(&mut app, start);

        let (_expected_root, mut expected) = fixture_app();
        expected
            .start_long(next, start + Duration::from_secs(2))
            .unwrap();
        let expected_request = expected.retry_request().unwrap().clone();
        let expected_metadata = expected.long_metadata().unwrap().clone();

        app.handle_event(key(Key::Char('n')), start + Duration::from_secs(2))
            .unwrap();

        assert_eq!(app.retry_request(), Some(&expected_request));
        assert_eq!(app.long_metadata(), Some(&expected_metadata));
        assert!(matches!(
            &app.retry_request().unwrap().mode,
            PracticeMode::Long { item_id, paragraph: 0 } if item_id == next
        ));
    }
}

#[test]
fn result_next_is_unavailable_for_key_and_test_but_retry_is_exact() {
    let start = Instant::now();

    let (_key_root, mut keys) = fixture_app();
    keys.start_key(Language::En, 4, true, false, 7, start)
        .unwrap();
    assert_result_next_unavailable_and_retry_exact(&mut keys, start);

    let (_test_root, mut test) = fixture_app();
    test.start_test(Language::Ko, Some(60), None, 13, start)
        .unwrap();
    assert_result_next_unavailable_and_retry_exact(&mut test, start);
}

#[test]
fn result_next_rejects_custom_file_catalog_id_collision() {
    assert_custom_long_collision_is_not_next(CustomTextSource::File, "custom-file");
}

#[test]
fn result_next_rejects_stdin_catalog_id_collision() {
    assert_custom_long_collision_is_not_next(CustomTextSource::Stdin, "stdin");
}

#[test]
fn zero_attempt_finish_is_transactional_and_save_failure_stays_in_result() {
    let (root, mut empty) = fixture_app();
    let start = Instant::now();
    empty
        .start_mode(
            request(PracticeKind::Words, Language::En, "a", StopRule::TargetEnd),
            start,
        )
        .unwrap();
    assert!(empty.finish_practice(start).is_err());
    assert_eq!(empty.screen(), Screen::Practice);
    assert!(empty.active_practice().is_some());
    assert!(empty.result.is_none());
    assert!(empty.sessions.is_empty());
    assert!(!empty.paths.sessions.exists());

    let (failure_root, mut failed) = fixture_app();
    fs::create_dir_all(failed.paths.sessions.parent().unwrap()).unwrap();
    fs::write(&failed.paths.sessions, b"preserved sentinel").unwrap();
    failed
        .start_mode(
            ModeRequest {
                content_ids: vec!["failure-content".into()],
                ..request(PracticeKind::Words, Language::En, "λβ", StopRule::TargetEnd)
            },
            start,
        )
        .unwrap();
    failed.handle_event(key(Key::Char('λ')), start).unwrap();
    failed
        .handle_event(key(Key::Char('β')), start + Duration::from_secs(1))
        .unwrap();

    assert_eq!(failed.screen(), Screen::Result);
    assert!(failed.sessions.is_empty());
    assert_eq!(
        fs::read(&failed.paths.sessions).unwrap(),
        b"preserved sentinel"
    );
    let error = failed
        .result
        .as_ref()
        .unwrap()
        .save_error
        .as_deref()
        .unwrap();
    assert!(!error.is_empty());
    assert!(!error.contains("λβ"));
    assert!(!error.contains(failure_root.path().to_string_lossy().as_ref()));
    assert!(buffer_text(&draw(&failed, 80, 24).buffer).contains("Save failed"));
    assert!(root.path().exists());
}

#[test]
fn result_uses_same_language_mode_history_goals_and_relative_grade_boundaries() {
    fn prior(
        id: &str,
        started_at_unix_ms: i128,
        language: Language,
        mode: PracticeKind,
        kpm: f64,
        wpm: f64,
    ) -> SessionRecord {
        let mut session = result_view(id).session;
        session.started_at_unix_ms = started_at_unix_ms;
        session.language = language;
        session.mode = mode;
        session.kpm = kpm;
        session.wpm = wpm;
        session
    }

    let (_root, mut app) = fixture_app();
    app.settings.target_wpm = 1;
    app.settings.target_accuracy = 98.0;
    app.settings.daily_minutes = 1;
    app.sessions = vec![
        prior("older", 100, Language::En, PracticeKind::Words, 2.5, 0.5),
        prior("best-wpm", 200, Language::En, PracticeKind::Words, 6.0, 1.2),
        prior("best-kpm", 250, Language::En, PracticeKind::Words, 9.0, 0.6),
        prior("newest-a", 300, Language::En, PracticeKind::Words, 3.5, 0.7),
        prior("newest-b", 300, Language::En, PracticeKind::Words, 4.0, 0.8),
        prior(
            "invalid-newer",
            400,
            Language::En,
            PracticeKind::Words,
            f64::NAN,
            8.0,
        ),
        prior(
            "other-mode",
            500,
            Language::En,
            PracticeKind::Test,
            999.0,
            999.0,
        ),
        prior(
            "other-language",
            600,
            Language::Ko,
            PracticeKind::Words,
            999.0,
            999.0,
        ),
    ];
    let start = Instant::now();
    app.start_mode(
        ModeRequest {
            content_ids: vec!["comparison-content".into()],
            ..request(
                PracticeKind::Words,
                Language::En,
                "abcde",
                StopRule::TargetEnd,
            )
        },
        start,
    )
    .unwrap();
    app.handle_event(key(Key::Char('a')), start).unwrap();
    for character in ['b', 'c', 'd', 'e'] {
        app.handle_event(key(Key::Char(character)), start + Duration::from_secs(60))
            .unwrap();
    }

    let result = app.result.as_ref().unwrap();
    assert_eq!(result.session.wpm, 1.0);
    assert_eq!(result.previous_kpm, Some(4.0));
    assert_eq!(result.previous_wpm, Some(0.8));
    assert_eq!(result.best_kpm, Some(9.0));
    assert_eq!(result.best_wpm, Some(8.0));
    assert!((result.kpm_delta.unwrap() - 1.0).abs() < f64::EPSILON * 4.0);
    assert!((result.wpm_delta.unwrap() - 0.2).abs() < f64::EPSILON * 4.0);
    assert!(result.speed_goal_met);
    assert!(result.accuracy_goal_met);
    assert!(result.daily_minutes_met);
    assert_eq!(result.grade, None);
    assert_eq!(result.session.difficulty, Some(3));
    assert_eq!(app.sessions.len(), 9);

    assert_eq!(grade(80.0, 80.0, 98.0, 98.0), Grade::A);
    assert_eq!(grade(64.0, 80.0, 95.0, 98.0), Grade::B);
    assert_eq!(grade(48.0, 80.0, 90.0, 98.0), Grade::C);
    assert_eq!(grade(47.9, 80.0, 100.0, 98.0), Grade::D);
    assert_eq!(grade(80.0, 80.0, 96.0, 97.0), Grade::B);
}

#[test]
fn result_metrics_ignore_live_visibility_settings() {
    for language in [Language::Ko, Language::En] {
        for (show_speed, show_accuracy) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            let (_root, mut app) = fixture_app();
            app.warnings.clear();
            app.settings.ui_language = language;
            app.settings.show_live_speed = show_speed;
            app.settings.show_accuracy = show_accuracy;
            let mut result = result_view("visibility-result");
            result.session.language = language;
            app.result = Some(result);
            app.open(Screen::Result);

            let output = buffer_text(&draw(&app, 80, 24).buffer);
            assert!(
                output.contains(match language {
                    Language::Ko => "타수 60.0 타/분 · WPM 12.0",
                    Language::En => "KPM 60.0 · WPM 12.0",
                }),
                "{language:?} speed={show_speed} accuracy={show_accuracy}: {output}"
            );
            assert!(
                output.contains(match language {
                    Language::Ko => "정확도: 100.0%",
                    Language::En => "Accuracy: 100.0%",
                }),
                "{language:?} speed={show_speed} accuracy={show_accuracy}: {output}"
            );
        }
    }
}
