use super::support::*;

fn launch_mode_options(
    app: &mut App,
    mode: usize,
    language: Language,
    changes: &[(usize, usize)],
    start: usize,
    now: Instant,
) {
    open_mode_options(app, mode, now);
    if language == Language::Ko {
        press(app, Key::Right, 1, now);
    }
    let mut focus = 0;
    for &(row, count) in changes {
        press(app, Key::Tab, row - focus, now);
        press(app, Key::Right, count, now);
        focus = row;
    }
    press(app, Key::Tab, start - focus, now);
    app.handle_event(key(Key::Enter), now).unwrap();
}

fn many_long_pack(count: usize) -> String {
    let mut pack = r#"schema_version = 1
id = "viewport"
title = "Viewport test pack"
language = "en"

[source]
author = "Viewport author"
source_id = "viewport-pack"
source_url = "https://example.com/viewport"
license = "CC0-1.0"
license_url = "https://creativecommons.org/publicdomain/zero/1.0/"
modified = false
retrieved_at = "2026-08-10"
"#
    .to_owned();
    for index in 0..count {
        let title = if index + 1 == count {
            "界".repeat(160)
        } else {
            format!("Viewport item {index:02}")
        };
        let tags = if index + 1 == count {
            format!("[\"{}\", \"{}\"]", "界".repeat(160), "界".repeat(160))
        } else {
            format!("[\"viewport-tag-{index:02}\"]")
        };
        pack.push_str(&format!(
            r#"
[[items]]
id = "viewport-text-{index:02}"
kind = "text"
title = "{title}"
difficulty = 3
tags = {tags}
text = "Unique viewport text {index:02}."

[items.source]
author = "Viewport author"
source_id = "viewport-source-{index:02}"
source_url = "https://example.com/viewport/{index:02}"
license = "CC0-1.0"
license_url = "https://creativecommons.org/publicdomain/zero/1.0/"
modified = false
retrieved_at = "2026-08-10"
"#
        ));
    }
    pack
}

#[test]
fn home_renders_exactly_eleven_actions_and_marks_the_focused_one() {
    let (_root, mut app) = fixture_app();
    for _ in 0..3 {
        app.handle_event(key(Key::Tab), Instant::now()).unwrap();
    }

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for action in [
        "Quick practice",
        "Key practice",
        "Word practice",
        "Sentence practice",
        "Long-text practice",
        "Typing test",
        "Games",
        "Statistics",
        "Goals",
        "Content",
        "Settings",
    ] {
        assert_eq!(output.matches(action).count(), 1, "{action}: {output}");
    }
    assert!(output.contains("> Sentence practice"), "{output}");
}

#[test]
fn every_home_practice_action_opens_its_matching_mode_options() {
    let now = Instant::now();
    for (index, kind, label) in [
        (0, PracticeKind::Quick, "Quick practice"),
        (1, PracticeKind::Key, "Key practice"),
        (2, PracticeKind::Words, "Word practice"),
        (3, PracticeKind::Sentence, "Sentence practice"),
        (4, PracticeKind::Long, "Long-text practice"),
        (5, PracticeKind::Test, "Typing test"),
    ] {
        let (_root, mut app) = fixture_app();
        app.warnings.clear();
        for _ in 0..index {
            app.handle_event(key(Key::Tab), now).unwrap();
        }

        app.handle_event(key(Key::Enter), now).unwrap();
        assert_eq!(app.screen(), Screen::ModeOptions, "{kind:?}");
        assert_eq!(app.focus(), 0, "{kind:?}");
        assert!(app.active_practice().is_none(), "{kind:?}");
        let options = buffer_text(&draw(&app, 80, 24).buffer);
        assert!(options.contains(label), "{options}");
    }
}

#[test]
fn mode_options_reach_every_documented_quick_key_word_and_test_choice() {
    let now = Instant::now();

    for (source, source_changes, kind) in [
        (QuickSource::Words, 0, ContentKind::Word),
        (QuickSource::Quote, 1, ContentKind::Quote),
    ] {
        for (items, preset, stop) in [
            (false, 0, StopRule::ActiveTime(Duration::from_secs(15))),
            (false, 1, StopRule::ActiveTime(Duration::from_secs(30))),
            (false, 2, StopRule::ActiveTime(Duration::from_secs(60))),
            (false, 3, StopRule::ActiveTime(Duration::from_secs(120))),
            (true, 0, StopRule::Items(10)),
            (true, 1, StopRule::Items(25)),
            (true, 2, StopRule::Items(50)),
            (true, 3, StopRule::Items(100)),
        ] {
            let (_root, mut app) = fixture_app();
            launch_mode_options(
                &mut app,
                0,
                Language::En,
                &[
                    (1, source_changes),
                    (2, items as usize),
                    (3, (preset + 3) % 4),
                ],
                4,
                now,
            );

            let active = app.active_practice().unwrap();
            assert_eq!(active.kind(), PracticeKind::Quick);
            assert_eq!(active.engine.language(), Language::En);
            assert_eq!(active.stop, stop);
            assert!(
                active.content_ids.iter().all(|id| {
                    app.content
                        .items()
                        .any(|item| item.id == *id && item.kind == kind)
                }),
                "{source:?} {stop:?}"
            );
        }
    }

    for language in [Language::En, Language::Ko] {
        for stage in 1..=key_stages(language).len() as u8 {
            for random in [false, true] {
                for weak_repeat in [false, true] {
                    let (_root, mut app) = fixture_app();
                    launch_mode_options(
                        &mut app,
                        1,
                        language,
                        &[
                            (1, usize::from(stage - 1)),
                            (2, random as usize),
                            (3, weak_repeat as usize),
                        ],
                        4,
                        now,
                    );

                    let active = app.active_practice().unwrap();
                    assert_eq!(active.engine.language(), language);
                    assert_eq!(
                        active.mode,
                        PracticeMode::Key {
                            stage,
                            random,
                            weak_repeat,
                        }
                    );
                }
            }
        }
    }

    for (difficulty, changes) in [
        (Difficulty::Easy, 1),
        (Difficulty::Medium, 2),
        (Difficulty::Hard, 3),
        (Difficulty::Mixed, 4),
    ] {
        let (_root, mut app) = fixture_app();
        launch_mode_options(&mut app, 2, Language::Ko, &[(1, changes)], 2, now);

        let active = app.active_practice().unwrap();
        assert_eq!(active.engine.language(), Language::Ko);
        assert_eq!(
            active.mode,
            PracticeMode::Words {
                difficulty,
                completed: 0,
                streak: 0,
            }
        );
    }

    let (_root, mut sentence) = fixture_app();
    launch_mode_options(&mut sentence, 3, Language::Ko, &[], 1, now);
    assert_eq!(
        sentence.active_practice().unwrap().kind(),
        PracticeKind::Sentence
    );
    assert_eq!(
        sentence.active_practice().unwrap().engine.language(),
        Language::Ko
    );

    for (preset, seconds) in [60, 180, 300, 600].into_iter().enumerate() {
        let (_root, mut app) = fixture_app();
        let language = if preset % 2 == 0 {
            Language::Ko
        } else {
            Language::En
        };
        launch_mode_options(&mut app, 5, language, &[(1, (preset + 2) % 4)], 3, now);

        let active = app.active_practice().unwrap();
        assert_eq!(active.kind(), PracticeKind::Test);
        assert_eq!(active.engine.language(), language);
        assert_eq!(
            active.stop,
            StopRule::ActiveTime(Duration::from_secs(seconds))
        );
    }

    let (_root, mut quick) = fixture_app();
    open_mode_options(&mut quick, 0, now);
    press(&mut quick, Key::BackTab, 1, now);
    assert_eq!(quick.focus(), 4);
    press(&mut quick, Key::Tab, 1, now);
    assert_eq!(quick.focus(), 0);
    quick.handle_event(key(Key::Esc), now).unwrap();
    assert_eq!(quick.screen(), Screen::Home);

    let (_root, mut key_app) = fixture_app();
    open_mode_options(&mut key_app, 1, now);
    press(&mut key_app, Key::Tab, 1, now);
    press(
        &mut key_app,
        Key::Right,
        key_stages(Language::En).len() - 1,
        now,
    );
    press(&mut key_app, Key::BackTab, 1, now);
    press(&mut key_app, Key::Right, 1, now);
    press(&mut key_app, Key::Tab, 4, now);
    key_app.handle_event(key(Key::Enter), now).unwrap();
    assert_eq!(
        key_app.active_practice().unwrap().mode,
        PracticeMode::Key {
            stage: key_stages(Language::Ko).len() as u8,
            random: false,
            weak_repeat: false,
        }
    );
}

#[test]
fn long_options_render_metadata_and_launch_every_filtered_item() {
    let now = Instant::now();
    for language in [Language::En, Language::Ko] {
        let (_root, app) = fixture_app();
        let items = app
            .long_items(language, None)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(!items.is_empty());
        drop(app);

        for (index, item) in items.into_iter().enumerate() {
            let (_root, mut app) = fixture_app();
            open_mode_options(&mut app, 4, now);
            if language == Language::Ko {
                press(&mut app, Key::Right, 1, now);
            }
            press(&mut app, Key::Tab, index + 1, now);

            let output = buffer_text(&draw(&app, 80, 24).buffer);
            assert!(
                output.contains(item.title.as_deref().unwrap_or(&item.id)),
                "{output}"
            );
            assert!(output.contains(&item.source.author), "{output}");
            let source = format!("Source: {}", item.source.source_url);
            if UnicodeWidthStr::width(source.as_str()) <= 78 {
                assert!(output.contains(&source), "{output}");
            } else {
                assert!(
                    output
                        .lines()
                        .any(|line| line.contains("Source: ") && line.contains('…')),
                    "{output}"
                );
            }
            assert!(output.contains(&item.source.license), "{output}");
            assert!(
                output.contains(&item.difficulty.unwrap_or_default().to_string()),
                "{output}"
            );
            for tag in &item.tags {
                assert!(output.contains(tag), "{output}");
            }

            app.handle_event(key(Key::Enter), now).unwrap();
            let active = app.active_practice().unwrap();
            assert_eq!(active.engine.language(), language);
            assert_eq!(
                active.mode,
                PracticeMode::Long {
                    item_id: item.id.clone(),
                    paragraph: 0,
                }
            );
            assert_eq!(active.content_ids, [item.id]);
        }
    }

    let (_root, mut app) = fixture_app();
    let (english_count, expected) = {
        let english_items = app.long_items(Language::En, None);
        let korean_items = app.long_items(Language::Ko, None);
        let selection = 0;
        (
            english_items.len(),
            korean_items[selection]
                .title
                .as_deref()
                .unwrap_or(&korean_items[selection].id)
                .to_owned(),
        )
    };
    open_mode_options(&mut app, 4, now);
    press(&mut app, Key::Tab, english_count, now);
    press(&mut app, Key::BackTab, english_count, now);
    press(&mut app, Key::Right, 1, now);
    assert!(buffer_text(&draw(&app, 80, 24).buffer).contains(&expected));

    let (_root, mut empty) = fixture_app();
    empty.content = ContentCatalog::default();
    open_mode_options(&mut empty, 4, now);
    press(&mut empty, Key::Tab, 1, now);
    empty.handle_event(key(Key::Enter), now).unwrap();
    assert_eq!(empty.screen(), Screen::ModeOptions);
    assert!(empty.active_practice().is_none());
}

#[test]
fn long_options_keep_the_focused_tail_and_metadata_visible_with_warnings() {
    let (_root, mut app) = fixture_app();
    fs::create_dir_all(&app.paths.content).unwrap();
    fs::write(app.paths.content.join("viewport.toml"), many_long_pack(24)).unwrap();
    let loaded = ContentCatalog::load(&app.paths.content).unwrap();
    assert!(loaded.warnings.is_empty());
    app.content = loaded.catalog;
    let expected_id = "viewport-text-23";
    let index = app
        .long_items(Language::En, None)
        .iter()
        .position(|item| item.id == expected_id)
        .unwrap();

    let now = Instant::now();
    open_mode_options(&mut app, 4, now);
    press(&mut app, Key::Tab, index + 1, now);
    let output = buffer_text(&draw(&app, 80, 24).buffer);

    for visible in [
        "> 界",
        "Language: en",
        "Title: 界",
        "Author: Viewport author",
        "Source: https://example.com/viewport/23",
        "License: CC0-1.0",
        "Difficulty: 3",
        "Tags: 界",
        "Enter Confirm",
        "Esc Back",
        "review warning",
    ] {
        assert!(output.contains(visible), "missing {visible:?}: {output}");
    }
    for visible in ["> ", "Title: ", "Tags: "] {
        assert!(
            output
                .lines()
                .any(|line| line.contains(visible) && line.contains('…')),
            "missing truncated {visible:?}: {output}"
        );
    }
    assert!(!output.contains("Viewport item 00"), "{output}");

    app.handle_event(key(Key::Enter), now).unwrap();
    assert_eq!(
        app.active_practice().unwrap().mode,
        PracticeMode::Long {
            item_id: expected_id.into(),
            paragraph: 0,
        }
    );
}
