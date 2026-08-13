use super::support::*;

#[test]
fn long_text_filters_metadata_tracks_paragraphs_and_centers_the_cursor() {
    let (_root, mut app) = fixture_app();
    let documents = app.long_items(Language::En, Some("public-domain"));
    assert_eq!(documents.len(), 3);
    assert!(documents.iter().all(|item| {
        item.language == Language::En
            && item.kind == ContentKind::Text
            && item.tags.iter().any(|tag| tag == "public-domain")
    }));

    let start = Instant::now();
    app.start_long("en-text-gettysburg-address", start).unwrap();
    let metadata = app.long_metadata().unwrap();
    assert_eq!(metadata.title, "The Gettysburg Address");
    assert_eq!(metadata.author, "Abraham Lincoln");
    assert_eq!(metadata.license, "LicenseRef-Public-Domain");
    assert_eq!(metadata.difficulty, Some(3));
    assert_eq!(metadata.tags, ["public-domain"]);
    assert_eq!(
        metadata.source,
        "https://www.nps.gov/linc/learn/historyculture/gettysburgaddress.htm"
    );
    assert_eq!(
        app.long_scroll().unwrap(),
        typerlude::app::LongScroll {
            active_paragraph: 1,
            total_paragraphs: 4,
            percent: 0,
        }
    );

    let second_end = app.active_practice().unwrap().item_ends[1];
    let prefix = app
        .active_practice()
        .unwrap()
        .engine
        .target_cells()
        .take(second_end)
        .map(|(grapheme, _)| grapheme)
        .collect::<String>();
    type_text(&mut app, &prefix, start + Duration::from_secs(30));

    let progress = app.long_scroll().unwrap();
    assert_eq!(progress.active_paragraph, 3);
    assert_eq!(progress.total_paragraphs, 4);
    assert!((1..100).contains(&progress.percent));
    let drawn = draw(&app, 120, 40);
    let output = buffer_text(&drawn.buffer);
    for marker in [
        "The Gettysburg Address",
        "Abraham Lincoln",
        "LicenseRef-Public-Domain",
        "https://www.nps.gov/linc/learn/historyculture/gettysburgaddress.htm",
        "Difficulty: 3",
        "public-domain",
        "Paragraph 3/4",
    ] {
        assert!(output.contains(marker), "missing {marker}: {output}");
    }
    assert_eq!(drawn.cursor.unwrap().1, 2);
    app.handle_event(key(Key::Esc), start).unwrap();
    app.handle_event(key(Key::Char('q')), start).unwrap();
    app.handle_event(key(Key::Char('q')), start).unwrap();
    app.handle_event(key(Key::Char('r')), start).unwrap();
    assert_eq!(app.long_metadata().unwrap().title, "The Gettysburg Address");
}

#[test]
fn long_viewport_reaches_valid_custom_text_beyond_u16_rows() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let start = Instant::now();
    let prefix = "a\n".repeat(66_000);
    let target = format!("{prefix}CURRENT-PARAGRAPH");
    app.start_custom_text(CustomTextSource::File, "large-lines.txt", &target, start)
        .unwrap();
    app.active_practice_mut()
        .unwrap()
        .engine
        .input(&prefix, start);

    let drawn = draw(&app, 80, 24);
    let output = buffer_text(&drawn.buffer);
    assert!(output.contains("CURRENT-PARAGRAPH"), "{output}");
    assert_eq!(drawn.cursor, Some((2, 2)));
}

#[test]
fn custom_long_text_is_memory_only_and_uses_safe_content_ids() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let start = Instant::now();
    let name = "private-target-name.txt";
    let target = format!("{} PRIVATE-TARGET-SHOULD-STAY-IN-MEMORY", "a".repeat(31));
    app.start_custom_text(CustomTextSource::File, name, &target, start)
        .unwrap();
    assert_eq!(app.active_practice().unwrap().content_ids, ["custom-file"]);
    app.settings.ui_language = Language::Ko;
    let korean_metadata = buffer_text(&draw(&app, 80, 24).buffer);
    for marker in ["로컬 파일", "사용자 제공 텍스트", "재배포하지 않음"] {
        assert!(
            korean_metadata.contains(marker),
            "missing {marker}: {korean_metadata}"
        );
    }
    for english in ["Local file", "User-provided text", "Not redistributed"] {
        assert!(!korean_metadata.contains(english), "{korean_metadata}");
    }
    app.settings.ui_language = Language::En;

    for second in 0..=30 {
        app.handle_event(key(Key::Char('a')), start + Duration::from_secs(second))
            .unwrap();
    }
    let result = app
        .finish_practice(start + Duration::from_secs(30))
        .unwrap();
    assert_eq!(result.session.mode, PracticeKind::Long);
    assert_eq!(result.session.content_id, "custom-file");
    let long = result.long.unwrap();
    assert_eq!(long.completed_graphemes, 31);
    assert!(long.total_graphemes > long.completed_graphemes);
    assert!((long.best_rolling_kpm - 60.0).abs() < f64::EPSILON * 8.0);
    assert!((long.best_rolling_wpm - 12.0).abs() < f64::EPSILON * 8.0);
    assert!((1..100).contains(&long.percent));
    app.settings.ui_language = Language::Ko;
    let korean_result = buffer_text(&draw(&app, 80, 24).buffer);
    for marker in [
        "최고 30초 속도: 타수 60.0 타/분 · WPM 12.0",
        "글자: 31/",
        "진행:",
    ] {
        assert!(
            korean_result.contains(marker),
            "missing {marker}: {korean_result}"
        );
    }

    let stored = fs::read_dir(&app.paths.sessions)
        .unwrap()
        .map(|entry| fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect::<String>();
    for private in [
        name,
        target.as_str(),
        "PRIVATE-TARGET-SHOULD-STAY-IN-MEMORY",
    ] {
        assert!(
            !stored.contains(private),
            "private text persisted: {stored}"
        );
    }

    let (_stdin_root, mut stdin) = fixture_app();
    stdin
        .start_custom_text(CustomTextSource::Stdin, "stdin", "stdin text", start)
        .unwrap();
    assert_eq!(stdin.active_practice().unwrap().content_ids, ["stdin"]);
    assert!(
        stdin
            .start_custom_text(CustomTextSource::Stdin, "stdin", " \n\t", start)
            .is_err()
    );
    assert!(
        stdin
            .start_custom_text(CustomTextSource::File, "bad\u{1b}", "safe", start)
            .is_err()
    );
    assert!(
        stdin
            .start_custom_text(
                CustomTextSource::File,
                "large.txt",
                &"a".repeat(8 * 1024 * 1024 + 1),
                start,
            )
            .is_err()
    );
}
