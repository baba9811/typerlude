use super::support::*;

#[test]
fn stats_shows_default_ranges_no_data_and_stored_session_data() {
    let (_root, mut app) = fixture_app();
    app.open(Screen::Stats);
    let empty = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(empty.contains("7  [30]  90  All"), "{empty}");
    assert!(empty.contains("No data"), "{empty}");

    app.sessions.push(result_view("visible-session").session);
    let populated = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(populated.contains("Sessions: 1"), "{populated}");
    assert!(populated.contains("Accuracy: 100.0%"), "{populated}");
    assert!(!populated.contains("No data"), "{populated}");
}

#[test]
fn stats_with_multiple_sessions_renders_two_speed_series() {
    let (_root, mut app) = fixture_app();
    let mut first = result_view("chart-first").session;
    first.kpm = 100.0;
    first.wpm = 20.0;
    let mut second = result_view("chart-second").session;
    second.kpm = 300.0;
    second.wpm = 60.0;
    app.sessions.extend([first, second]);
    app.open(Screen::Stats);

    let drawn = draw(&app, 80, 24);
    let output = buffer_text(&drawn.buffer);
    assert!(output.contains("Speed trend"), "{output}");
    assert!(output.contains("KPM"), "{output}");
    assert!(output.contains("WPM"), "{output}");
    let styles = default_styles();
    let has_braille = |style: Style| {
        drawn.buffer.content.iter().any(|cell| {
            cell.style().fg == style.fg
                && cell
                    .symbol()
                    .chars()
                    .any(|character| ('\u{2801}'..='\u{28ff}').contains(&character))
        })
    };
    assert!(has_braille(styles.accent), "KPM chart is missing: {output}");
    assert!(
        has_braille(styles.correct),
        "WPM chart is missing: {output}"
    );
}

#[test]
fn subminute_practice_remains_visible_in_the_minutes_trend() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let today = local_today();
    let mut first = result_view("fifteen-seconds").session;
    first.local_date = today.saturating_sub(time::Duration::days(1));
    first.duration_ms = 15_000;
    let mut second = result_view("thirty-seconds").session;
    second.local_date = today;
    second.duration_ms = 30_000;
    app.sessions.extend([first, second]);
    app.open(Screen::Stats);

    let drawn = draw(&app, 100, 30);
    let output = buffer_text(&drawn.buffer);
    assert!(output.contains("Minutes trend"), "{output}");
    assert!(
        (19..99).any(|x| drawn.buffer[(x, 13)].symbol() != " "),
        "subminute trend is blank: {output}"
    );
}

#[test]
fn stats_uses_the_selected_language_and_30_days_from_local_today() {
    let (_root, mut app) = fixture_app();
    app.settings.language = Language::En;
    app.warnings.clear();
    let today = local_today();
    let mut recent = result_view("recent-en").session;
    recent.local_date = today.saturating_sub(time::Duration::days(1));
    recent.kpm = 100.0;
    recent.wpm = 20.0;
    recent.accuracy = 80.0;
    let mut boundary = result_view("boundary-en").session;
    boundary.local_date = today.saturating_sub(time::Duration::days(29));
    boundary.kpm = 300.0;
    boundary.wpm = 60.0;
    boundary.accuracy = 100.0;
    let mut too_old = result_view("old-en").session;
    too_old.local_date = today.saturating_sub(time::Duration::days(30));
    too_old.kpm = 999.0;
    too_old.wpm = 999.0;
    let mut latest_other_language = result_view("latest-ko").session;
    latest_other_language.local_date = today;
    latest_other_language.language = Language::Ko;
    latest_other_language.kpm = 777.0;
    latest_other_language.wpm = 777.0;
    app.sessions
        .extend([recent, boundary, too_old, latest_other_language]);
    app.open(Screen::Stats);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("Sessions: 2"), "{output}");
    assert!(output.contains("Accuracy: 90.0%"), "{output}");
    assert!(output.contains("KPM 200.0/300.0"), "{output}");
    assert!(output.contains("WPM 40.0/60.0"), "{output}");
    assert!(!output.contains("999.0"), "{output}");
    assert!(!output.contains("777.0"), "{output}");

    app.settings.ui_language = Language::Ko;
    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("타수 200.0/300.0 타/분"), "{output}");
    assert!(output.contains("WPM 40.0/60.0"), "{output}");
}

#[test]
fn korean_stats_goal_uses_localized_kpm_terminology() {
    let (_root, mut app) = fixture_app();
    app.settings.ui_language = Language::Ko;
    app.settings.target_kpm = 450;
    app.set_stats_language(Language::Ko);
    let mut session = result_view("korean-stats-goal").session;
    session.language = Language::Ko;
    session.kpm = 321.0;
    app.sessions.push(session);
    app.open(Screen::Stats);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("목표: 타수 321/450 타/분"), "{output}");
}

#[test]
fn stats_trends_are_chronological_regardless_of_storage_order() {
    let (_first_root, mut first) = fixture_app();
    let (_second_root, mut second) = fixture_app();
    for app in [&mut first, &mut second] {
        app.settings.language = Language::En;
        app.warnings.clear();
        app.open(Screen::Stats);
    }
    let today = local_today();
    let mut older = result_view("older-en").session;
    older.local_date = today.saturating_sub(time::Duration::days(1));
    older.started_at_unix_ms = 1;
    older.wpm = 10.0;
    let mut newer = result_view("newer-en").session;
    newer.local_date = today;
    newer.started_at_unix_ms = 2;
    newer.wpm = 90.0;
    first.sessions.extend([newer.clone(), older.clone()]);
    second.sessions.extend([older, newer]);

    assert_eq!(draw(&first, 80, 24).buffer, draw(&second, 80, 24).buffer);
}

#[test]
fn stats_with_no_selected_language_session_in_30_days_renders_no_data() {
    let (_root, mut app) = fixture_app();
    app.settings.language = Language::En;
    app.warnings.clear();
    let mut korean = result_view("only-ko").session;
    korean.language = Language::Ko;
    korean.local_date = local_today();
    app.sessions.push(korean);
    app.open(Screen::Stats);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("No data"), "{output}");
    assert!(!output.contains("Sessions:"), "{output}");
}

#[test]
fn non_finite_stored_accuracy_cannot_panic_stats_rendering() {
    let (_root, mut app) = fixture_app();
    let mut session = result_view("nan-accuracy").session;
    session.accuracy = f64::NAN;
    app.sessions.push(session);
    app.open(Screen::Stats);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("Accuracy: 0.0%"), "{output}");
}

#[test]
fn korean_data_screens_do_not_fall_back_to_english_prose() {
    let (_root, mut app) = fixture_app();
    app.settings.ui_language = Language::Ko;
    app.settings.language = Language::Ko;
    app.settings.daily_minutes = 22;
    app.sessions.push(result_view("korean-row").session);

    for (screen, required, forbidden) in [
        (Screen::Stats, "7  [30]  90  전체", "All"),
        (Screen::History, "단어 연습", "Words"),
        (Screen::Content, "문장 연습", "Sentence"),
        (Screen::ContentDetail, "수정됨: 아니요", "modified: no"),
        (Screen::Goals, "22분", "22 min"),
        (Screen::Settings, "키보드: 켜짐", "keyboard: true"),
    ] {
        app.open(screen);
        let output = buffer_text(&draw(&app, 80, 24).buffer);
        assert!(output.contains(required), "{screen:?}: {output}");
        assert!(!output.contains(forbidden), "{screen:?}: {output}");
    }
    app.open(Screen::Stats);
    let stats = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(stats.contains("/22 분"), "{stats}");
    assert!(!stats.contains("/22 min"), "{stats}");
    app.open(Screen::ContentDetail);
    let detail = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(detail.contains("활성"), "{detail}");
    assert!(!detail.contains("enabled"), "{detail}");

    app.open(Screen::Settings);
    let settings = buffer_text(&draw(&app, 80, 24).buffer);
    for required in [
        "손가락 안내: 켜짐",
        "실시간 속도: 켜짐",
        "적응형: 켜짐",
        "업데이트 확인: 켜짐",
    ] {
        assert!(
            settings.contains(required),
            "missing {required:?}: {settings}"
        );
    }
    assert!(!settings.contains("true"), "{settings}");
    assert!(!settings.contains("false"), "{settings}");
}

#[test]
fn history_renders_newest_session_first_without_mutating_storage_order() {
    let (_root, mut app) = fixture_app();
    let mut newer = result_view("1786029600000000000-12345-1").session;
    newer.started_at_unix_ms = 2;
    newer.mode = PracticeKind::Long;
    newer.kpm = 200.0;
    newer.wpm = 40.0;
    let mut older = result_view("1786029600000000000-12345-0").session;
    older.started_at_unix_ms = 1;
    app.sessions.extend([newer, older]);
    let stored = app.sessions.clone();
    app.open(Screen::History);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    let newer = output
        .find("KPM 200.0 · WPM 40.0")
        .expect("newer session is visible");
    let older = output
        .find("KPM 60.0 · WPM 12.0")
        .expect("older session is visible");
    assert!(newer < older, "{output}");
    let newer_row = output
        .lines()
        .find(|line| line.contains("KPM 200.0 · WPM 40.0"))
        .expect("both newer speeds are visible on one row");
    let id = newer_row
        .find("178602960000")
        .expect("production-length session ID prefix is visible");
    let speeds = newer_row.find("KPM 200.0 · WPM 40.0").unwrap();
    assert!(speeds < id, "{newer_row}");
    assert_eq!(app.sessions, stored);
}

#[test]
fn weak_keys_renders_derived_attempts_and_accuracy() {
    let (_root, mut app) = fixture_app();
    let mut session = result_view("weak-key-session").session;
    session.intended_keys = BTreeMap::from([('a', [8, 2])]);
    app.sessions.push(session);
    app.open(Screen::WeakKeys);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("a: 80.0% (10)"), "{output}");
}

#[test]
fn weak_key_screen_distinguishes_perfect_from_insufficient_data() {
    let (_root, mut perfect) = fixture_app();
    let mut session = result_view("perfect-key-session").session;
    session.intended_keys = BTreeMap::from([('a', [10, 0])]);
    perfect.sessions.push(session);
    perfect.open(Screen::WeakKeys);

    let output = buffer_text(&draw(&perfect, 80, 24).buffer);
    assert!(
        output.contains("No weak keys · All analyzed keys are 100% accurate"),
        "{output}"
    );
    assert!(!output.contains("a: 100.0%"), "{output}");

    let (_root, mut insufficient) = fixture_app();
    let mut session = result_view("insufficient-key-session").session;
    session.intended_keys = BTreeMap::from([('a', [9, 0])]);
    insufficient.sessions.push(session);
    insufficient.open(Screen::WeakKeys);

    let output = buffer_text(&draw(&insufficient, 80, 24).buffer);
    assert!(output.contains("No data"), "{output}");
    assert!(
        !output.contains("All analyzed keys are 100% accurate"),
        "{output}"
    );
}

#[test]
fn weak_key_screen_reserves_a_visible_row_for_suggested_content() {
    let (_root, mut app) = fixture_app();
    let mut session = result_view("many-weak-keys").session;
    session.intended_keys = (b'a'..=b'z').map(|key| (char::from(key), [8, 2])).collect();
    app.sessions.push(session);
    app.open(Screen::WeakKeys);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("a: 80.0% (10)"), "{output}");
    assert!(output.contains("Suggested content"), "{output}");
}

#[test]
fn weak_keys_uses_only_the_saved_practice_language() {
    let (_root, mut app) = fixture_app();
    app.settings.language = Language::En;
    let mut english = result_view("weak-en").session;
    english.intended_keys = BTreeMap::from([('x', [8, 2])]);
    let mut korean = result_view("weak-ko").session;
    korean.language = Language::Ko;
    korean.intended_keys = BTreeMap::from([('한', [0, 10])]);
    app.sessions.extend([english, korean]);
    app.open(Screen::WeakKeys);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("x: 80.0% (10)"), "{output}");
    assert!(!output.contains("한: 0.0% (10)"), "{output}");
}

#[test]
fn stats_filters_change_derived_points_without_mutating_sessions() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    let today = local_today();
    let mut recent = result_view("recent-words").session;
    recent.local_date = today.saturating_sub(time::Duration::days(1));
    recent.mode = PracticeKind::Words;
    recent.wpm = 40.0;
    recent.accuracy = 90.0;
    recent.duration_ms = 120_000;
    let mut old = result_view("old-words").session;
    old.local_date = today.saturating_sub(time::Duration::days(60));
    old.mode = PracticeKind::Words;
    old.wpm = 20.0;
    let mut other_mode = result_view("recent-test").session;
    other_mode.local_date = today;
    other_mode.mode = PracticeKind::Test;
    other_mode.wpm = 80.0;
    let mut korean = result_view("recent-korean").session;
    korean.local_date = today;
    korean.language = Language::Ko;
    korean.kpm = 500.0;
    app.sessions.extend([recent, old, other_mode, korean]);
    let stored = app.sessions.clone();

    app.set_stats_language(Language::En);
    app.set_stats_mode(Some(PracticeKind::Words));
    app.set_stats_range(Range::Days7);
    assert_eq!(app.stats_points().len(), 1);
    assert_eq!(app.stats_points()[0].wpm, 40.0);
    app.set_stats_range(Range::All);
    assert_eq!(app.stats_points().len(), 2);
    assert_eq!(app.sessions, stored);

    app.set_stats_range(Range::Days7);
    app.open(Screen::Stats);
    let output = buffer_text(&draw(&app, 100, 30).buffer);
    for value in [
        "Range: [7]  30  90  All",
        "Language: en",
        "Mode: Word practice",
        "Sessions: 1",
        "Total time: 2 min",
        "Accuracy: 90.0%",
        "WPM 40.0/40.0",
        "Streak: 2",
        "Goal",
        "Speed trend",
        "Accuracy trend",
        "Minutes trend",
    ] {
        assert!(output.contains(value), "missing {value:?}: {output}");
    }
}

#[test]
fn finite_stats_ranges_use_local_today_and_exclude_future_records() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    app.set_stats_language(Language::En);
    app.set_stats_mode(Some(PracticeKind::Words));
    app.set_stats_range(Range::Days30);
    let today = local_today();
    let mut recent = result_view("recent-current-date").session;
    recent.local_date = today.saturating_sub(time::Duration::days(1));
    recent.wpm = 33.0;
    let mut future = result_view("future-corrupt-date").session;
    future.local_date = today.saturating_add(time::Duration::days(60));
    future.wpm = 999.0;
    app.sessions.extend([recent, future]);
    app.open(Screen::Stats);

    let output = buffer_text(&draw(&app, 100, 30).buffer);
    assert!(output.contains("Sessions: 1"), "{output}");
    assert!(output.contains("WPM 33.0/33.0"), "{output}");
    assert!(!output.contains("999.0"), "{output}");
}

#[test]
fn streak_and_daily_goal_use_all_today_sessions_not_the_visible_filter() {
    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    app.settings.daily_minutes = 15;
    app.set_stats_language(Language::En);
    app.set_stats_mode(Some(PracticeKind::Words));
    app.set_stats_range(Range::Days7);

    let today = local_today();
    let mut visible = result_view("visible-en-words").session;
    visible.local_date = today;
    visible.duration_ms = 60_000;
    let mut other_today = result_view("other-ko-test").session;
    other_today.local_date = today;
    other_today.language = Language::Ko;
    other_today.mode = PracticeKind::Test;
    other_today.duration_ms = 14 * 60_000;
    let mut yesterday = result_view("yesterday-ko-test").session;
    yesterday.local_date = today.saturating_sub(time::Duration::days(1));
    yesterday.language = Language::Ko;
    yesterday.mode = PracticeKind::Test;
    app.sessions.extend([visible, other_today, yesterday]);
    app.open(Screen::Stats);

    let output = buffer_text(&draw(&app, 100, 30).buffer);
    assert!(output.contains("Sessions: 1"), "{output}");
    assert!(output.contains("Total time: 1 min"), "{output}");
    assert!(output.contains("Streak: 2"), "{output}");
    assert!(output.contains("15/15 min"), "{output}");
}
