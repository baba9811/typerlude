use super::support::*;

#[test]
fn key_practice_keeps_its_existing_target_layout() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_mode(
        ModeRequest {
            kind: PracticeKind::Key,
            language: Language::En,
            target: "fj".into(),
            mode: PracticeMode::Key {
                stage: 1,
                random: false,
                weak_repeat: false,
            },
            stop: StopRule::TargetEnd,
            item_ends: vec![2],
            content_ids: Vec::new(),
        },
        start,
    )
    .unwrap();

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert_eq!(output.matches("Input").count(), 1, "{output}");
    assert!(!output.contains("Prompt"), "{output}");
    assert!(output.contains("[F]"), "{output}");
}

#[test]
fn key_stages_unlock_progressively_and_sequences_are_seeded() {
    for language in [Language::Ko, Language::En] {
        let stages = key_stages(language);
        assert!(!stages.is_empty());
        assert!(
            stages
                .windows(2)
                .all(|pair| { pair[0].keys.iter().all(|key| pair[1].keys.contains(key)) })
        );
        assert!(stages.last().unwrap().keys.len() >= 26);
        assert!(key_sequence(language, 0, false, &[], 7).is_err());
        assert!(key_sequence(language, stages.len() as u8 + 1, false, &[], 7).is_err());
    }

    let ordered = key_sequence(Language::En, 4, false, &[], 7).unwrap();
    assert_eq!(ordered.chars().count(), 120);
    assert!(ordered.starts_with("fjdksla;"));
    let random = key_sequence(Language::En, 4, true, &[], 7).unwrap();
    assert_eq!(random, key_sequence(Language::En, 4, true, &[], 7).unwrap());
    assert_ne!(random, key_sequence(Language::En, 4, true, &[], 8).unwrap());

    let weak = key_sequence(Language::En, 4, false, &['f'], 7).unwrap();
    assert_eq!(weak.chars().count(), 120);
    assert!(weak.matches('f').count() > ordered.matches('f').count());
    assert_eq!(
        weak,
        key_sequence(Language::En, 4, false, &['f'], 7).unwrap()
    );
    assert_eq!(
        key_sequence(Language::En, 4, false, &['x'], 7).unwrap(),
        ordered,
        "locked weak keys must not bypass the selected stage"
    );
}

#[test]
fn weak_repetition_uses_only_sufficient_same_language_history() {
    let start = Instant::now();
    let (_root, mut insufficient) = fixture_app();
    let mut english = result_view("weak-en").session;
    english.intended_keys.insert('f', [0, 9]);
    let mut korean = result_view("weak-ko").session;
    korean.language = Language::Ko;
    korean.intended_keys.insert('f', [0, 100]);
    insufficient.sessions.extend([english.clone(), korean]);
    insufficient
        .start_key(Language::En, 4, false, true, 7, start)
        .unwrap();
    let insufficient_target = insufficient
        .active_practice()
        .unwrap()
        .engine
        .target_cells()
        .map(|(key, _)| key)
        .collect::<String>();
    let normal = key_sequence(Language::En, 4, false, &[], 7).unwrap();
    assert_eq!(insufficient_target, normal);

    let (_root, mut sufficient) = fixture_app();
    english.intended_keys.insert('f', [9, 1]);
    for locked in ['x', 'z', 'v'] {
        english.intended_keys.insert(locked, [0, 10]);
    }
    sufficient.sessions.push(english);
    sufficient
        .start_key(Language::En, 4, false, true, 7, start)
        .unwrap();
    let active = sufficient.active_practice().unwrap();
    let sufficient_target = active
        .engine
        .target_cells()
        .map(|(key, _)| key)
        .collect::<String>();
    assert!(sufficient_target.matches('f').count() > normal.matches('f').count());
    assert_eq!(active.engine.target_len(), 120);
    assert_eq!(active.item_ends, [120]);
    assert!(active.content_ids.is_empty());
    assert_eq!(
        active.mode,
        PracticeMode::Key {
            stage: 4,
            random: false,
            weak_repeat: true,
        }
    );
}

#[test]
fn key_keyboard_and_finger_guide_follow_settings_and_shift_state() {
    let start = Instant::now();
    let (_root, mut app) = fixture_app();
    app.start_mode(
        ModeRequest {
            kind: PracticeKind::Key,
            language: Language::En,
            target: "A".into(),
            mode: PracticeMode::Key {
                stage: 9,
                random: false,
                weak_repeat: false,
            },
            stop: StopRule::ActiveTime(Duration::from_secs(60)),
            item_ends: vec![1],
            content_ids: Vec::new(),
        },
        start,
    )
    .unwrap();
    let visible = draw(&app, 80, 24);
    let output = buffer_text(&visible.buffer);
    for label in ["Stage 9", "Accuracy", "Streak", "Progress", "Errors"] {
        assert!(output.contains(label), "missing {label}: {output}");
    }
    assert!(output.contains("Shift"), "{output}");
    assert!(output.contains("Finger: left pinky"), "{output}");
    let styles = default_styles();
    assert!(
        visible
            .buffer
            .content
            .iter()
            .filter(|cell| cell.symbol() == "A")
            .filter(|cell| {
                Some(cell.fg) == styles.cursor.fg
                    && Some(cell.bg) == styles.cursor.bg
                    && cell.modifier == styles.cursor.add_modifier
            })
            .count()
            >= 2
    );
    app.handle_event(key(Key::Char('A')), start).unwrap();
    let active = app.active_practice().unwrap();
    assert_eq!(active.engine.attempted_units(), 1);
    assert_eq!(active.engine.intended_keys().get(&'a'), Some(&[1, 0]));

    app.settings.show_keyboard = false;
    app.settings.show_finger_guide = false;
    let hidden = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(!hidden.contains("[Shift]"), "{hidden}");
    assert!(!hidden.contains("Finger:"), "{hidden}");

    let (_root, mut korean) = fixture_app();
    korean.settings.ui_language = Language::Ko;
    korean
        .start_mode(
            ModeRequest {
                kind: PracticeKind::Key,
                language: Language::Ko,
                target: "ㅂ".into(),
                mode: PracticeMode::Key {
                    stage: 1,
                    random: false,
                    weak_repeat: false,
                },
                stop: StopRule::ActiveTime(Duration::from_secs(60)),
                item_ends: vec![1],
                content_ids: Vec::new(),
            },
            start,
        )
        .unwrap();
    let korean_output = buffer_text(&draw(&korean, 80, 24).buffer);
    assert!(korean_output.contains("[ㅂ]"), "{korean_output}");
    assert!(
        korean_output.contains("손가락: 왼쪽 새끼"),
        "{korean_output}"
    );
}

#[test]
fn key_keyboard_maps_shifted_colon_to_the_semicolon_key() {
    let (_root, mut app) = fixture_app();
    app.start_mode(
        ModeRequest {
            kind: PracticeKind::Key,
            language: Language::En,
            target: ":".into(),
            mode: PracticeMode::Key {
                stage: 9,
                random: false,
                weak_repeat: false,
            },
            stop: StopRule::ActiveTime(Duration::from_secs(60)),
            item_ends: vec![1],
            content_ids: Vec::new(),
        },
        Instant::now(),
    )
    .unwrap();

    let drawn = draw(&app, 80, 24);
    let output = buffer_text(&drawn.buffer);
    assert!(output.contains("Finger: right pinky"), "{output}");
    let styles = default_styles();
    let semicolon = drawn
        .buffer
        .content
        .iter()
        .find(|cell| cell.symbol() == ";")
        .unwrap();
    assert_role_style(semicolon, styles.cursor);
}

#[test]
fn key_keyboard_includes_every_supported_punctuation_key() {
    for target in ['`', '-', '=', '[', ']', '\\', '\''] {
        let (_root, mut app) = fixture_app();
        app.start_mode(
            ModeRequest {
                kind: PracticeKind::Key,
                language: Language::En,
                target: target.to_string(),
                mode: PracticeMode::Key {
                    stage: 10,
                    random: false,
                    weak_repeat: false,
                },
                stop: StopRule::ActiveTime(Duration::from_secs(60)),
                item_ends: vec![1],
                content_ids: Vec::new(),
            },
            Instant::now(),
        )
        .unwrap();
        let drawn = draw(&app, 80, 24);
        let styles = default_styles();
        let highlighted = drawn
            .buffer
            .content
            .iter()
            .filter(|cell| cell.symbol() == target.to_string())
            .filter(|cell| {
                Some(cell.fg) == styles.cursor.fg
                    && Some(cell.bg) == styles.cursor.bg
                    && cell.modifier == styles.cursor.add_modifier
            })
            .count();
        assert!(highlighted >= 2, "missing keyboard key {target:?}");
    }
}
