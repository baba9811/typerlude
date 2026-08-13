use super::support::*;

fn nord_base_style() -> Style {
    Style::default()
        .fg(Color::Rgb(0xd8, 0xde, 0xe9))
        .bg(Color::Rgb(0x2e, 0x34, 0x40))
}

#[test]
fn goals_and_settings_render_the_saved_values_without_edit_state() {
    let (_root, mut app) = fixture_app();
    app.settings.language = Language::Ko;
    app.settings.ui_language = Language::En;
    app.settings.theme = "nord".into();
    app.settings.target_kpm = 321;
    app.settings.target_wpm = 65;
    app.settings.target_accuracy = 97.5;
    app.settings.daily_minutes = 22;

    app.open(Screen::Goals);
    let goals = buffer_text(&draw(&app, 80, 24).buffer);
    for value in ["321 KPM", "65 WPM", "97.5%", "22 min"] {
        assert!(goals.contains(value), "missing {value:?}: {goals}");
    }
    app.settings.ui_language = Language::Ko;
    let goals = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(goals.contains("321 타/분"), "{goals}");

    app.settings.ui_language = Language::En;
    app.open(Screen::Settings);
    let settings = buffer_text(&draw(&app, 80, 24).buffer);
    for value in ["Language: ko", "UI language: en", "Theme: nord"] {
        assert!(settings.contains(value), "missing {value:?}: {settings}");
    }
}

#[test]
fn goal_arrows_snap_minimum_and_off_grid_values_in_the_pressed_direction() {
    let (_root, mut app) = fixture_app();
    let now = Instant::now();
    app.settings.target_kpm = 501;
    app.settings.target_wpm = 1;
    app.settings.target_accuracy = 97.3;
    app.settings.daily_minutes = 6;
    app.open(Screen::Goals);

    app.handle_event(key(Key::Left), now).unwrap();
    assert_eq!(app.settings.target_kpm, 500);
    press(&mut app, Key::Tab, 1, now);
    app.handle_event(key(Key::Right), now).unwrap();
    assert_eq!(app.settings.target_wpm, 5);
    press(&mut app, Key::Tab, 1, now);
    app.handle_event(key(Key::Right), now).unwrap();
    assert_eq!(app.settings.target_accuracy, 97.5);
    press(&mut app, Key::Tab, 1, now);
    app.handle_event(key(Key::Left), now).unwrap();
    assert_eq!(app.settings.daily_minutes, 5);
    app.handle_event(key(Key::Right), now).unwrap();
    assert_eq!(app.settings.daily_minutes, 10);
}

#[test]
fn themes_lists_all_five_validated_builtin_ids() {
    let (_root, mut app) = fixture_app();
    app.open(Screen::Themes);
    let output = buffer_text(&draw(&app, 80, 24).buffer);

    for id in ["default", "matrix", "minimal", "monochrome", "nord"] {
        assert!(output.contains(id), "missing {id:?}: {output}");
    }
}

#[test]
fn focused_theme_previews_without_saving_and_escape_reverts() {
    let (_root, mut app) = fixture_app();
    let now = Instant::now();
    app.settings.theme = "default".into();
    app.warnings.clear();
    app.open(Screen::Themes);
    press(&mut app, Key::Tab, 4, now);

    let preview = draw(&app, 80, 24);
    assert_role_style(&preview.buffer[(70, 18)], nord_base_style());
    assert_eq!(app.settings.theme, "default");

    app.handle_event(key(Key::Esc), now).unwrap();
    let reverted = draw(&app, 80, 24);
    let default = default_styles();
    assert_role_style(&reverted.buffer[(70, 18)], default.base);
}

#[test]
fn goals_and_settings_save_atomically_and_failed_saves_preserve_memory() {
    let (root, mut app) = fixture_app();
    app.warnings.clear();
    app.set_target_kpm(510).unwrap();
    app.set_target_wpm(95).unwrap();
    app.set_target_accuracy(97.5).unwrap();
    app.set_daily_minutes(20).unwrap();
    app.select_theme("nord").unwrap();
    let loaded = Settings::load(&app.paths).unwrap().value;
    assert_eq!(loaded.target_kpm, 510);
    assert_eq!(loaded.target_wpm, 95);
    assert_eq!(loaded.target_accuracy, 97.5);
    assert_eq!(loaded.daily_minutes, 20);
    assert_eq!(loaded.theme, "nord");

    let before = app.settings.clone();
    let blocked = root.path().join("blocked");
    fs::write(&blocked, b"not a directory").unwrap();
    app.paths.config = blocked.join("config.toml");
    assert!(app.set_daily_minutes(21).is_err());
    assert_eq!(app.settings, before);
    assert!(
        app.warnings
            .last()
            .is_some_and(|warning| warning.contains("failed to save")),
        "{:?}",
        app.warnings
    );

    assert!(app.select_theme("not-installed").is_err());
    assert_eq!(app.settings, before);
}

#[test]
fn settings_actions_edit_every_requested_field_and_survive_reload() {
    let (_root, mut app) = fixture_app();
    let now = Instant::now();

    for focus in [0, 1, 3, 4, 5, 6, 7, 8] {
        app.open(Screen::Settings);
        for _ in 0..focus {
            app.handle_event(key(Key::Tab), now).unwrap();
        }
        app.handle_event(key(Key::Enter), now).unwrap();
    }
    app.open(Screen::Themes);
    for _ in 0..4 {
        app.handle_event(key(Key::Tab), now).unwrap();
    }
    app.handle_event(key(Key::Enter), now).unwrap();

    let loaded = Settings::load(&app.paths).unwrap().value;
    assert_eq!(loaded.language, Language::Ko);
    assert_eq!(loaded.ui_language, Language::Ko);
    assert_eq!(loaded.theme, "nord");
    assert!(!loaded.show_keyboard);
    assert!(!loaded.show_finger_guide);
    assert!(!loaded.show_live_speed);
    assert!(!loaded.show_accuracy);
    assert!(!loaded.adaptive);
    assert!(!loaded.check_updates);
}

#[test]
fn help_explains_all_keyboard_and_cli_actions_in_both_languages() {
    for (language, words) in [
        (Language::En, ["Move", "Select", "Back", "Quit", "Disable"]),
        (Language::Ko, ["이동", "선택", "뒤로", "종료", "비활성화"]),
    ] {
        let (_root, mut app) = fixture_app();
        app.settings.ui_language = language;
        app.warnings.clear();
        app.open(Screen::Help);
        let output = buffer_text(&draw(&app, 120, 40).buffer);
        for word in words {
            assert!(output.contains(word), "missing {word:?}: {output}");
        }
        assert!(output.contains("Shift+Tab"), "{output}");
        assert!(
            output.contains(match language {
                Language::Ko => "q를 두 번",
                Language::En => "press q twice",
            }),
            "{output}"
        );
        for command in [
            "typerlude quick|keys|words|sentence|long|test",
            "typerlude stats|history|themes",
            "typerlude content list",
            "typerlude content add PACK.toml",
            "typerlude content validate [PACK.toml]",
            "typerlude content disable PACK_ID",
            "typerlude paths|licenses|update",
            "typerlude --help|--version|--smoke",
        ] {
            assert!(output.contains(command), "missing {command:?}: {output}");
        }
    }
}

#[test]
fn help_distinguishes_test_leave_and_result_actions_at_minimum_size() {
    for (language, required) in [
        (
            Language::En,
            [
                "Non-Test: Esc / Ctrl+P pause",
                "Test: Esc opens/cancels leave · q confirms",
                "r: exact target/options",
                "n: Quick/Words/Sentence/catalog Long only",
            ],
        ),
        (
            Language::Ko,
            [
                "시험 외: Esc / Ctrl+P 일시 정지",
                "시험: Esc 나가기 확인 열기/취소 · q 확인",
                "r: 같은 대상/설정",
                "n: 빠른/단어/문장/카탈로그 긴 글만",
            ],
        ),
    ] {
        let (_root, mut app) = fixture_app();
        app.settings.ui_language = language;
        app.open(Screen::Help);
        let output = buffer_text(&draw(&app, 80, 24).buffer);
        for text in required {
            assert!(output.contains(text), "missing {text:?}: {output}");
        }
        assert!(
            output.contains("typerlude FILE | typerlude practice FILE"),
            "{output}"
        );
        assert!(output.contains("review warning"), "{output}");
    }
}

#[test]
fn warning_footer_collapses_leading_whitespace_and_wraps_multiple_useful_suffixes() {
    let (_root, mut app) = fixture_app();
    app.sessions.push(result_view("visible-session").session);
    app.warnings = vec![
        "\n\t leading warning".into(),
        format!("{}useful-tail", "long warning segment ".repeat(6)),
        "final suffix".into(),
    ];
    app.open(Screen::History);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for required in [
        "visible-session",
        "leading warning",
        "useful-tail",
        "final suffix",
    ] {
        assert!(output.contains(required), "missing {required:?}: {output}");
    }
}

#[test]
fn warning_footer_reserves_three_rows_for_word_boundary_wrapping() {
    let (_root, mut app) = fixture_app();
    app.sessions.push(result_view("visible-session").session);
    app.warnings = vec![format!(
        "FIRST{} SECOND{} THIRD{}",
        "a".repeat(35),
        "b".repeat(34),
        "c".repeat(35)
    )];
    app.open(Screen::History);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("visible-session"), "{output}");
    assert!(output.contains("FIRST"), "{output}");
    assert!(output.contains("SECOND"), "{output}");
    assert!(output.contains("THIRD"), "{output}");
}

#[test]
fn unknown_saved_theme_falls_back_to_validated_default_styles() {
    let (_root, mut app) = fixture_app();
    app.settings.theme = "missing-theme".into();
    app.warnings.clear();
    let drawn = draw(&app, 80, 24);
    let expected = default_styles().base;

    assert_role_style(&drawn.buffer[(70, 18)], expected);
    assert!(buffer_text(&drawn.buffer).contains("Typerlude"));
}

#[test]
fn help_renders_keyboard_guidance() {
    let (_root, mut app) = fixture_app();
    app.open(Screen::Help);
    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for key_name in ["Tab", "Enter", "Esc"] {
        assert!(output.contains(key_name), "{key_name}: {output}");
    }
}
