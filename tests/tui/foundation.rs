use super::support::*;

fn required_label(screen: Screen, language: Language) -> &'static str {
    match (screen, language) {
        (Screen::Home, Language::Ko) => "Typerlude",
        (Screen::Home, Language::En) => "Typerlude",
        (Screen::ModeOptions, Language::Ko) => "빠른 연습",
        (Screen::ModeOptions, Language::En) => "Quick practice",
        (Screen::Practice, Language::Ko) => "진행",
        (Screen::Practice, Language::En) => "Progress",
        (Screen::Result, Language::Ko) => "결과",
        (Screen::Result, Language::En) => "Result",
        (Screen::Stats, Language::Ko) => "통계",
        (Screen::Stats, Language::En) => "Statistics",
        (Screen::History, Language::Ko) => "기록",
        (Screen::History, Language::En) => "History",
        (Screen::WeakKeys, Language::Ko) => "약한 키",
        (Screen::WeakKeys, Language::En) => "Weak keys",
        (Screen::Goals, Language::Ko) => "목표",
        (Screen::Goals, Language::En) => "Goals",
        (Screen::Content, Language::Ko) => "콘텐츠",
        (Screen::Content, Language::En) => "Content",
        (Screen::ContentDetail, Language::Ko) => "출처",
        (Screen::ContentDetail, Language::En) => "Sources",
        (Screen::Settings, Language::Ko) => "설정",
        (Screen::Settings, Language::En) => "Settings",
        (Screen::Themes, Language::Ko) => "테마",
        (Screen::Themes, Language::En) => "Theme",
        (Screen::Help, Language::Ko) => "도움말",
        (Screen::Help, Language::En) => "Help",
    }
}

#[test]
fn engine_exposes_only_the_borrowed_target_render_view() {
    let start = Instant::now();
    let mut engine =
        PracticeEngine::new(Language::Ko, PracticeKind::Sentence, "한글\n🙂", None).unwrap();
    engine.input("한강", start);

    assert_eq!(engine.cursor(), 2);
    assert_eq!(engine.target_len(), 4);
    assert_eq!(
        engine
            .target_cells()
            .map(|(grapheme, entered)| (grapheme.to_owned(), entered))
            .collect::<Vec<_>>(),
        [
            ("한".into(), Some(true)),
            ("글".into(), Some(false)),
            ("\n".into(), None),
            ("🙂".into(), None),
        ]
    );
}

#[test]
fn every_screen_renders_its_bilingual_identity_at_supported_sizes() {
    for language in [Language::Ko, Language::En] {
        for screen in Screen::ALL {
            for (width, height) in [(80, 24), (120, 40)] {
                let (_root, mut app) = fixture_app();
                app.settings.ui_language = language;
                app.open(screen);

                let drawn = draw(&app, width, height);
                let output = buffer_text(&drawn.buffer);
                assert!(
                    output.contains(required_label(screen, language)),
                    "{language:?} {screen:?} at {width}x{height}: {output}"
                );
            }
        }
    }
}

#[test]
fn tiny_terminals_return_before_layout_and_hide_the_cursor() {
    for language in [Language::Ko, Language::En] {
        for (width, height) in [(1, 1), (40, 10), (79, 23)] {
            let (_root, mut app) = fixture_app();
            app.settings.ui_language = language;
            app.start_mode(
                request(
                    PracticeKind::Words,
                    language,
                    if language == Language::Ko {
                        "한글"
                    } else {
                        "text"
                    },
                    StopRule::TargetEnd,
                ),
                Instant::now(),
            )
            .unwrap();

            let drawn = draw(&app, width, height);
            let output = buffer_text(&drawn.buffer);
            assert!(!output.contains(required_label(Screen::Home, language)));
            assert_eq!(drawn.cursor, None, "{width}x{height}");
            if width >= 40 && height >= 10 {
                assert!(!output.contains(required_label(Screen::Practice, language)));
                assert!(
                    output.contains(match language {
                        Language::Ko => "터미널이 너무 작습니다",
                        Language::En => "Terminal is too small",
                    }),
                    "{output}"
                );
                assert!(output.contains("80x24"), "{output}");
                assert!(output.contains(&format!("{width}x{height}")), "{output}");
            }
        }
    }
}

#[test]
fn payload_free_practice_and_result_render_localized_no_data() {
    for language in [Language::Ko, Language::En] {
        for screen in [Screen::Practice, Screen::Result] {
            let (_root, mut app) = fixture_app();
            app.settings.ui_language = language;
            app.open(screen);
            let output = buffer_text(&draw(&app, 80, 24).buffer);
            assert!(
                output.contains(match language {
                    Language::Ko => "데이터 없음",
                    Language::En => "No data",
                }),
                "{language:?} {screen:?}: {output}"
            );
        }
    }
}

#[test]
fn valid_history_and_content_remain_visible_with_sanitized_warnings() {
    for screen in [Screen::History, Screen::Content] {
        let (_root, mut app) = fixture_app();
        app.sessions.push(result_view("visible-session").session);
        app.warnings = vec!["bad\u{1b}]0;hidden\u{7} visible-warning".into()];
        app.open(screen);

        let drawn = draw(&app, 80, 24);
        let output = buffer_text(&drawn.buffer);
        let expected_content = match screen {
            Screen::History => "visible-session",
            Screen::Content => "en-tatoeba-331259",
            _ => unreachable!(),
        };
        assert!(output.contains(expected_content), "{screen:?}: {output}");
        assert!(output.contains("visible-warning"), "{screen:?}: {output}");
        assert!(
            drawn
                .buffer
                .content
                .iter()
                .all(|cell| !cell.symbol().contains('\u{1b}') && !cell.symbol().contains('\u{7}')),
            "{screen:?}: {output}"
        );
    }
}

#[test]
fn rendering_is_read_only_for_app_sessions_and_engine_metrics() {
    let (_root, mut app) = fixture_app();
    let now = Instant::now();
    app.start_mode(
        request(PracticeKind::Words, Language::En, "ab", StopRule::TargetEnd),
        now,
    )
    .unwrap();
    app.active_practice_mut().unwrap().engine.input("a", now);
    app.sessions.push(result_view("preserved-session").session);

    let screen = app.screen();
    let parent = app.parent();
    let focus = app.focus();
    let quit = app.should_quit();
    let retry = app.retry_request().cloned();
    let sessions = app.sessions.clone();
    let metrics = app.active_practice().unwrap().engine.metrics(now);

    draw(&app, 80, 24);

    assert_eq!(app.screen(), screen);
    assert_eq!(app.parent(), parent);
    assert_eq!(app.focus(), focus);
    assert_eq!(app.should_quit(), quit);
    assert_eq!(app.retry_request(), retry.as_ref());
    assert_eq!(app.sessions, sessions);
    assert_eq!(app.active_practice().unwrap().engine.metrics(now), metrics);
}
