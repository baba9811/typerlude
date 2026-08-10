use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::{Buffer, Cell},
    layout::Rect,
    style::Style,
};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use time::OffsetDateTime;
use time::macros::date;
use typeul::{
    app::{
        App, CustomTextSource, Grade, ItemDelta, ModeRequest, PracticeMode, QuickOptions,
        QuickSource, ResultView, Screen, StopRule, grade, key_sequence, key_stages,
    },
    config::Settings,
    content::{ContentCatalog, ContentKind},
    model::{Difficulty, Language, PracticeKind},
    practice::{InputOutcome, PracticeEngine},
    stats::{KeyAccuracy, adaptive_candidates},
    storage::{AppPaths, SessionRecord},
    theme::ThemeCatalog,
    ui::{practice_cursor, render},
};
use unicode_width::UnicodeWidthStr;

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "typeul-tui-{}-{}",
            std::process::id(),
            NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture_app() -> (TestDir, App) {
    let root = TestDir::new();
    let app = App::new(
        Settings::default(),
        AppPaths::from_override(root.path().join("home")),
        ContentCatalog::load_builtins().unwrap(),
        ThemeCatalog::load_builtins().unwrap(),
        Vec::new(),
        vec!["review warning".into()],
    );
    (root, app)
}

fn key(code: KeyCode) -> Event {
    key_with(code, KeyModifiers::NONE, KeyEventKind::Press)
}

fn key_with(code: KeyCode, modifiers: KeyModifiers, kind: KeyEventKind) -> Event {
    Event::Key(KeyEvent::new_with_kind(code, modifiers, kind))
}

fn type_text(app: &mut App, value: &str, now: Instant) {
    for character in value.chars() {
        let code = if character == '\n' {
            KeyCode::Enter
        } else {
            KeyCode::Char(character)
        };
        app.handle_event(key(code), now).unwrap();
    }
}

fn mode_for(kind: PracticeKind) -> PracticeMode {
    match kind {
        PracticeKind::Quick => PracticeMode::Quick { completed: 2 },
        PracticeKind::Key => PracticeMode::Key {
            stage: 3,
            random: true,
            weak_repeat: true,
        },
        PracticeKind::Words => PracticeMode::Words {
            difficulty: Difficulty::Hard,
            completed: 4,
            streak: 2,
        },
        PracticeKind::Sentence => PracticeMode::Sentence {
            completed: 5,
            last_item: Some(ItemDelta {
                correct_units: 7,
                attempted_units: 8,
                errors: 1,
                speed: 72.5,
                accuracy: 87.5,
            }),
        },
        PracticeKind::Long => PracticeMode::Long {
            item_id: "long-item".into(),
            paragraph: 2,
        },
        PracticeKind::Test => PracticeMode::Test {
            grade: Some(Grade::B),
        },
    }
}

fn request(kind: PracticeKind, language: Language, target: &str, stop: StopRule) -> ModeRequest {
    ModeRequest {
        kind,
        language,
        target: target.into(),
        mode: mode_for(kind),
        stop,
        item_ends: vec![1, target.chars().count()],
        content_ids: vec!["first-item".into(), "second-item".into()],
    }
}

fn result_view(id: &str) -> ResultView {
    ResultView {
        session: SessionRecord {
            schema_version: 1,
            id: id.into(),
            started_at_unix_ms: 1_786_029_600_000,
            local_date: date!(2026 - 08 - 07),
            language: Language::En,
            mode: PracticeKind::Words,
            content_id: "first-item".into(),
            difficulty: Some(1),
            duration_ms: 1_000,
            correct_units: 1,
            attempted_units: 1,
            errors: 0,
            backspaces: 0,
            cpm: 60.0,
            kpm: 60.0,
            wpm: 12.0,
            accuracy: 100.0,
            intended_keys: BTreeMap::new(),
        },
        previous_speed: Some(10.0),
        best_speed: Some(12.0),
        speed_delta: Some(2.0),
        speed_goal_met: true,
        accuracy_goal_met: true,
        daily_minutes_met: false,
        weak_keys: Vec::new(),
        grade: None,
        save_error: Some("preserve this result".into()),
        long: None,
    }
}

struct Drawn {
    buffer: Buffer,
    cursor: Option<(u16, u16)>,
}

fn draw(app: &App, width: u16, height: u16) -> Drawn {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, app)).unwrap();
    let backend = terminal.backend();
    let cursor = backend.cursor_visible().then(|| {
        let position = backend.cursor_position();
        (position.x, position.y)
    });
    Drawn {
        buffer: backend.buffer().clone(),
        cursor,
    }
}

fn buffer_text(buffer: &Buffer) -> String {
    buffer
        .content
        .chunks(buffer.area.width as usize)
        .map(|row| {
            let mut output = String::new();
            let mut hidden = 0_usize;
            for cell in row {
                if hidden == 0 {
                    output.push_str(cell.symbol());
                }
                hidden = hidden
                    .max(UnicodeWidthStr::width(cell.symbol()))
                    .saturating_sub(1);
            }
            output
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn required_label(screen: Screen, language: Language) -> &'static str {
    match (screen, language) {
        (Screen::Home, Language::Ko) => "타이플",
        (Screen::Home, Language::En) => "Typeul",
        (Screen::ModeSelect, Language::Ko) => "빠른 연습",
        (Screen::ModeSelect, Language::En) => "Quick practice",
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

fn assert_role_style(cell: &Cell, expected: Style) {
    assert_eq!(Some(cell.fg), expected.fg);
    assert_eq!(Some(cell.bg), expected.bg);
    assert_eq!(cell.modifier, expected.add_modifier);
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
fn home_renders_exactly_ten_actions_and_marks_the_focused_one() {
    let (_root, mut app) = fixture_app();
    for _ in 0..3 {
        app.handle_event(key(KeyCode::Tab), Instant::now()).unwrap();
    }

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for action in [
        "Quick practice",
        "Key practice",
        "Word practice",
        "Sentence practice",
        "Long-text practice",
        "Typing test",
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
fn practice_uses_role_styles_and_places_the_unicode_newline_cursor() {
    let (_root, mut app) = fixture_app();
    let start = Instant::now();
    app.start_mode(
        request(
            PracticeKind::Sentence,
            Language::Ko,
            "한x\n🙂e\u{301}Z",
            StopRule::TargetEnd,
        ),
        start,
    )
    .unwrap();
    app.active_practice_mut()
        .unwrap()
        .engine
        .input("한q\n", start);

    let drawn = draw(&app, 80, 24);
    let styles = app.themes.get("default").unwrap().styles().unwrap();
    assert_eq!(drawn.cursor, Some((1, 2)));
    assert_eq!(drawn.buffer[(1, 1)].symbol(), "한");
    assert_role_style(&drawn.buffer[(1, 1)], styles.correct);
    assert_eq!(drawn.buffer[(3, 1)].symbol(), "x");
    assert_role_style(&drawn.buffer[(3, 1)], styles.error);
    assert_eq!(drawn.buffer[(1, 2)].symbol(), "🙂");
    assert_role_style(&drawn.buffer[(1, 2)], styles.cursor);
    assert_eq!(drawn.buffer[(3, 2)].symbol(), "é");
    assert_role_style(&drawn.buffer[(3, 2)], styles.dim);
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
fn practice_scrolls_the_target_and_cursor_to_the_current_line() {
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
    assert!(output.contains("item20"), "{output}");
    assert!(!output.contains("item00"), "{output}");
    let cursor = drawn.cursor.unwrap();
    assert_eq!(drawn.buffer[cursor].symbol(), "i");
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
fn content_detail_preserves_exact_provenance_values() {
    let (_root, mut app) = fixture_app();
    app.open(Screen::ContentDetail);
    let output = buffer_text(&draw(&app, 120, 40).buffer);

    for value in [
        "en-tatoeba-331259",
        "Tatoeba CC0 contributors",
        "tatoeba:331259",
        "https://tatoeba.org/en/sentences/show/331259",
        "CC0-1.0",
        "https://creativecommons.org/publicdomain/zero/1.0/",
        "2026-08-07",
        "modified: no",
    ] {
        assert!(output.contains(value), "missing {value:?}: {output}");
    }
}

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
fn stats_with_multiple_sessions_renders_a_real_speed_chart() {
    let (_root, mut app) = fixture_app();
    let mut first = result_view("chart-first").session;
    first.wpm = 12.0;
    let mut second = result_view("chart-second").session;
    second.wpm = 24.0;
    app.sessions.extend([first, second]);
    app.open(Screen::Stats);

    let drawn = draw(&app, 80, 24);
    let output = buffer_text(&drawn.buffer);
    assert!(output.contains("Speed trend"), "{output}");
    assert!(
        drawn.buffer.content.iter().any(|cell| {
            cell.symbol()
                .chars()
                .any(|character| ('\u{2801}'..='\u{28ff}').contains(&character))
        }),
        "chart has no visible Braille data: {output}"
    );
}

#[test]
fn stats_uses_the_selected_language_and_30_days_from_the_latest_stored_date() {
    let (_root, mut app) = fixture_app();
    app.settings.language = Language::En;
    app.warnings.clear();
    let mut recent = result_view("recent-en").session;
    recent.local_date = date!(2026 - 08 - 06);
    recent.wpm = 20.0;
    recent.accuracy = 80.0;
    let mut boundary = result_view("boundary-en").session;
    boundary.local_date = date!(2026 - 07 - 09);
    boundary.wpm = 40.0;
    boundary.accuracy = 100.0;
    let mut too_old = result_view("old-en").session;
    too_old.local_date = date!(2026 - 07 - 08);
    too_old.wpm = 999.0;
    let mut latest_other_language = result_view("latest-ko").session;
    latest_other_language.local_date = date!(2026 - 08 - 07);
    latest_other_language.language = Language::Ko;
    latest_other_language.kpm = 777.0;
    app.sessions
        .extend([recent, boundary, too_old, latest_other_language]);
    app.open(Screen::Stats);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(output.contains("Sessions: 2"), "{output}");
    assert!(output.contains("Accuracy: 90.0%"), "{output}");
    assert!(output.contains("WPM 30.0/40.0"), "{output}");
    assert!(!output.contains("KPM "), "{output}");
    assert!(!output.contains("999.0"), "{output}");
    assert!(!output.contains("777.0"), "{output}");
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
    let mut older = result_view("older-en").session;
    older.local_date = date!(2026 - 08 - 06);
    older.started_at_unix_ms = 1;
    older.wpm = 10.0;
    let mut newer = result_view("newer-en").session;
    newer.local_date = date!(2026 - 08 - 07);
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
    korean.local_date = date!(2026 - 08 - 07);
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
    let mut newer = result_view("newer-session").session;
    newer.started_at_unix_ms = 2;
    let mut older = result_view("older-session").session;
    older.started_at_unix_ms = 1;
    app.sessions.extend([newer, older]);
    let stored = app.sessions.clone();
    app.open(Screen::History);

    let output = buffer_text(&draw(&app, 80, 24).buffer);
    let newer = output
        .find("newer-session")
        .expect("newer session is visible");
    let older = output
        .find("older-session")
        .expect("older session is visible");
    assert!(newer < older, "{output}");
    assert_eq!(app.sessions, stored);
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
        "12.0 WPM",
        "Accuracy: 100.0%",
        "Errors: 0",
        "Previous: 10.0",
        "Best: 12.0",
        "Typeul relative grade: B",
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
        "Speed: Goal met",
        "Accuracy: Goal met",
        "Daily minutes: Goal missed",
        "Weak keys",
        "x: 80.0%",
    ] {
        assert!(output.contains(value), "missing {value:?}: {output}");
    }
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
    assert!(output.contains("j: 50.0%"), "{output}");
    assert!(!output.contains("k: 50.0%"), "{output}");
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

    app.open(Screen::Settings);
    let settings = buffer_text(&draw(&app, 80, 24).buffer);
    for value in ["Language: ko", "UI language: en", "Theme: nord"] {
        assert!(settings.contains(value), "missing {value:?}: {settings}");
    }
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
    let expected = app.themes.get("default").unwrap().styles().unwrap().base;

    assert_role_style(&drawn.buffer[(70, 18)], expected);
    assert!(buffer_text(&drawn.buffer).contains("Typeul"));
}

#[test]
fn mode_select_and_help_render_keyboard_guidance() {
    for screen in [Screen::ModeSelect, Screen::Help] {
        let (_root, mut app) = fixture_app();
        app.open(screen);
        let output = buffer_text(&draw(&app, 80, 24).buffer);
        for key_name in ["Tab", "Enter", "Esc"] {
            assert!(output.contains(key_name), "{screen:?} {key_name}: {output}");
        }
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

#[test]
fn screen_all_is_exact_unique_and_app_starts_at_home() {
    assert_eq!(
        Screen::ALL,
        [
            Screen::Home,
            Screen::ModeSelect,
            Screen::Practice,
            Screen::Result,
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
    assert_eq!(Screen::ALL.into_iter().collect::<HashSet<_>>().len(), 13);

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

    app.handle_event(key(KeyCode::Esc), now).unwrap();
    assert_eq!(app.screen(), Screen::Settings);
    assert_eq!(app.parent(), Screen::Home);
    app.handle_event(key(KeyCode::Esc), now).unwrap();
    assert_eq!(app.screen(), Screen::Home);
    app.handle_event(key(KeyCode::Esc), now).unwrap();
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
    nested.handle_event(key(KeyCode::Esc), now).unwrap();
    assert_eq!(nested.screen(), Screen::Stats);
    assert_eq!(nested.parent(), Screen::Settings);
    nested.handle_event(key(KeyCode::Esc), now).unwrap();
    assert_eq!(nested.screen(), Screen::Settings);
}

#[test]
fn result_escape_always_returns_home() {
    let (_root, mut app) = fixture_app();
    app.open(Screen::Settings);
    app.open(Screen::Result);

    app.handle_event(key(KeyCode::Esc), Instant::now()).unwrap();

    assert_eq!(app.screen(), Screen::Home);
    assert_eq!(app.parent(), Screen::Home);
}

#[test]
fn global_and_printable_shortcuts_obey_screen_and_key_kind() {
    for screen in Screen::ALL {
        let (_root, mut app) = fixture_app();
        app.open(screen);
        app.handle_event(
            key_with(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL,
                KeyEventKind::Press,
            ),
            Instant::now(),
        )
        .unwrap();
        assert!(app.should_quit(), "{screen:?}");
    }

    let (_root, mut released) = fixture_app();
    released
        .handle_event(
            key_with(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL,
                KeyEventKind::Release,
            ),
            Instant::now(),
        )
        .unwrap();
    assert!(!released.should_quit());

    let (_root, mut outside) = fixture_app();
    outside
        .handle_event(key(KeyCode::Char('q')), Instant::now())
        .unwrap();
    assert!(outside.should_quit());

    let (_root, mut repeat) = fixture_app();
    repeat
        .handle_event(
            key_with(KeyCode::Char('q'), KeyModifiers::NONE, KeyEventKind::Repeat),
            Instant::now(),
        )
        .unwrap();
    assert!(repeat.should_quit());

    let (_root, mut modified) = fixture_app();
    modified
        .handle_event(
            key_with(KeyCode::Char('q'), KeyModifiers::ALT, KeyEventKind::Press),
            Instant::now(),
        )
        .unwrap();
    assert!(!modified.should_quit(), "only plain q is global");

    let (_root, mut help) = fixture_app();
    help.open(Screen::Stats);
    help.handle_event(
        key_with(KeyCode::Char('?'), KeyModifiers::SHIFT, KeyEventKind::Press),
        Instant::now(),
    )
    .unwrap();
    assert_eq!(help.screen(), Screen::Help);
    help.handle_event(key(KeyCode::Esc), Instant::now())
        .unwrap();
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
            .handle_event(key(KeyCode::Char(printable)), Instant::now())
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

    app.handle_event(key(KeyCode::BackTab), now).unwrap();
    assert_eq!(app.focus(), 9);
    app.handle_event(key(KeyCode::Enter), now).unwrap();
    assert_eq!(app.screen(), Screen::Settings);
    assert_eq!(app.focus(), 0);

    for backward in [KeyCode::Up, KeyCode::Char('k')] {
        app.open(Screen::Home);
        app.handle_event(key(backward), now).unwrap();
        assert_eq!(app.focus(), 9);
    }

    for forward in [KeyCode::Tab, KeyCode::Down, KeyCode::Char('j')] {
        app.open(Screen::Home);
        for _ in 0..10 {
            app.handle_event(key(forward), now).unwrap();
        }
        assert_eq!(app.focus(), 0);
    }

    app.handle_event(
        key_with(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Release),
        now,
    )
    .unwrap();
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

    for event in [
        Event::FocusGained,
        Event::FocusLost,
        Event::Resize(1, 1),
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: 3,
            row: 4,
            modifiers: KeyModifiers::NONE,
        }),
    ] {
        app.handle_event(event, now).unwrap();
    }

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
    let active = app.active_practice_mut().unwrap();
    assert_eq!(active.engine.input("a", start), InputOutcome::Accepted);
    assert!(!active.engine.is_finished(start + Duration::from_secs(4)));
    assert!(active.engine.is_finished(start + Duration::from_secs(5)));

    app.open(Screen::Result);
    app.result = Some(result_view("stale-retry-result"));
    app.handle_event(key(KeyCode::Char('r')), start + Duration::from_secs(6))
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
    assert_eq!(retried.engine.input("ab", start), InputOutcome::Finished);

    app.open(Screen::Result);
    app.handle_event(key(KeyCode::Char('n')), start).unwrap();
    assert_eq!(app.screen(), Screen::Result, "Task 2 leaves next inert");
    assert_eq!(app.retry_request(), Some(&requested));
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
    app.handle_event(key(KeyCode::Tab), now).unwrap();

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

    app.handle_event(
        key_with(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ),
        start,
    )
    .unwrap();
    app.handle_event(
        key_with(
            KeyCode::Char('z'),
            KeyModifiers::CONTROL,
            KeyEventKind::Press,
        ),
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

    app.handle_event(key(KeyCode::Char('x')), start).unwrap();
    app.handle_event(
        key_with(KeyCode::Backspace, KeyModifiers::NONE, KeyEventKind::Repeat),
        start,
    )
    .unwrap();
    app.handle_event(key(KeyCode::Char('a')), start).unwrap();
    app.handle_event(
        key_with(KeyCode::Char('B'), KeyModifiers::SHIFT, KeyEventKind::Press),
        start,
    )
    .unwrap();
    let before_pause = app.active_practice().unwrap().engine.metrics(start);
    assert_eq!(before_pause.attempted_units, 3);
    assert_eq!(before_pause.errors, 1);
    assert_eq!(before_pause.backspaces, 1);

    app.handle_event(
        key_with(
            KeyCode::Char('p'),
            KeyModifiers::CONTROL,
            KeyEventKind::Press,
        ),
        start,
    )
    .unwrap();
    assert!(app.active_practice().unwrap().engine.is_paused());
    assert!(buffer_text(&draw(&app, 80, 24).buffer).contains("Resume"));
    app.handle_event(key(KeyCode::Char('c')), start).unwrap();
    app.handle_event(key(KeyCode::Backspace), start).unwrap();
    assert_eq!(
        app.active_practice().unwrap().engine.metrics(start),
        before_pause
    );

    app.handle_event(key(KeyCode::Esc), start).unwrap();
    assert!(!app.active_practice().unwrap().engine.is_paused());
    let paste_at = start + Duration::from_secs(1);
    app.handle_event(Event::Paste("private-paste".into()), paste_at)
        .unwrap();
    let after_paste = app.active_practice().unwrap().engine.metrics(paste_at);
    assert_eq!(after_paste.correct_units, before_pause.correct_units);
    assert_eq!(after_paste.attempted_units, before_pause.attempted_units);
    assert_eq!(after_paste.errors, before_pause.errors);
    assert_eq!(after_paste.backspaces, before_pause.backspaces);
    assert_eq!(app.practice_status(), Some("Paste ignored"));
    let pasted = buffer_text(&draw(&app, 80, 24).buffer);
    assert!(pasted.contains("Paste ignored"));
    assert!(pasted.contains("aBc"));

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
        key_with(KeyCode::Char('c'), KeyModifiers::NONE, KeyEventKind::Repeat),
        paste_at + Duration::from_secs(4),
    )
    .unwrap();
    assert_eq!(app.screen(), Screen::Result);
    let session = &app.result.as_ref().unwrap().session;
    assert_eq!(session.attempted_units, 4);
    assert_eq!(session.correct_units, 3);
    assert_eq!(session.errors, 1);
    assert_eq!(session.backspaces, 1);

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
    korean
        .handle_event(Event::Paste("비공개".into()), start)
        .unwrap();
    assert_eq!(korean.practice_status(), Some("붙여넣기 무시됨"));
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
    app.handle_event(key(KeyCode::Char('a')), now).unwrap();

    for modifiers in [KeyModifiers::ALT, KeyModifiers::CONTROL] {
        app.handle_event(
            key_with(KeyCode::Enter, modifiers, KeyEventKind::Press),
            now,
        )
        .unwrap();
    }

    let active = app.active_practice().unwrap();
    assert_eq!(active.engine.cursor(), 1);
    assert_eq!(active.engine.attempted_units(), 1);
}

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

    for pause in [
        key(KeyCode::Esc),
        key_with(
            KeyCode::Char('p'),
            KeyModifiers::CONTROL,
            KeyEventKind::Press,
        ),
    ] {
        app.handle_event(pause, start).unwrap();
        assert_eq!(app.screen(), Screen::Practice);
        assert!(!app.active_practice().unwrap().engine.is_paused());
    }
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
    app.handle_event(key(KeyCode::Char('a')), start).unwrap();

    app.handle_event(key(KeyCode::Char('r')), start + Duration::from_secs(1))
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
    quit.handle_event(key(KeyCode::Char('a')), start).unwrap();
    quit.handle_event(
        key_with(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            KeyEventKind::Press,
        ),
        start + Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(quit.screen(), Screen::Result);
    assert!(quit.should_quit());
    assert_eq!(quit.sessions.len(), 1);
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
    app.handle_event(key(KeyCode::Char('p')), start).unwrap();
    let after = OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    app.handle_event(
        Event::Paste("private paste material".into()),
        start + Duration::from_secs(1),
    )
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
    failed.handle_event(key(KeyCode::Char('λ')), start).unwrap();
    failed
        .handle_event(key(KeyCode::Char('β')), start + Duration::from_secs(1))
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
        speed: f64,
    ) -> SessionRecord {
        let mut session = result_view(id).session;
        session.started_at_unix_ms = started_at_unix_ms;
        session.language = language;
        session.mode = mode;
        session.wpm = speed;
        session.kpm = speed;
        session
    }

    let (_root, mut app) = fixture_app();
    app.settings.target_wpm = 1;
    app.settings.target_accuracy = 98.0;
    app.settings.daily_minutes = 1;
    app.sessions = vec![
        prior("older", 100, Language::En, PracticeKind::Words, 0.5),
        prior("best", 200, Language::En, PracticeKind::Words, 1.2),
        prior("newest", 300, Language::En, PracticeKind::Words, 0.7),
        prior("other-mode", 400, Language::En, PracticeKind::Test, 999.0),
        prior(
            "other-language",
            500,
            Language::Ko,
            PracticeKind::Words,
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
    app.handle_event(key(KeyCode::Char('a')), start).unwrap();
    for character in ['b', 'c', 'd', 'e'] {
        app.handle_event(
            key(KeyCode::Char(character)),
            start + Duration::from_secs(60),
        )
        .unwrap();
    }

    let result = app.result.as_ref().unwrap();
    assert_eq!(result.session.wpm, 1.0);
    assert_eq!(result.previous_speed, Some(0.7));
    assert_eq!(result.best_speed, Some(1.2));
    assert!((result.speed_delta.unwrap() - 0.3).abs() < f64::EPSILON * 4.0);
    assert!(result.speed_goal_met);
    assert!(result.accuracy_goal_met);
    assert!(result.daily_minutes_met);
    assert_eq!(result.grade, None);
    assert_eq!(result.session.difficulty, Some(3));
    assert_eq!(app.sessions.len(), 6);

    assert_eq!(grade(80.0, 80.0, 98.0, 98.0), Grade::A);
    assert_eq!(grade(64.0, 80.0, 95.0, 98.0), Grade::B);
    assert_eq!(grade(48.0, 80.0, 90.0, 98.0), Grade::C);
    assert_eq!(grade(47.9, 80.0, 100.0, 98.0), Grade::D);
    assert_eq!(grade(80.0, 80.0, 96.0, 97.0), Grade::B);
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
        app.handle_event(key(KeyCode::Char('a')), now).unwrap();

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
        .handle_event(key(KeyCode::Backspace), start + Duration::from_secs(2))
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
        .handle_event(key(KeyCode::Char('r')), start + Duration::from_secs(121))
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
    type_text(&mut app, "one", start + Duration::from_secs(1));
    let output = buffer_text(&draw(&app, 80, 24).buffer);
    for field in ["Speed", "Accuracy", "Errors", "Streak", "Progress"] {
        assert!(output.contains(field), "missing {field}: {output}");
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
    app.handle_event(key(KeyCode::Backspace), start + Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        app.active_practice()
            .unwrap()
            .current_item_delta()
            .unwrap()
            .correct_units,
        2
    );
    app.tick(start + Duration::from_secs(61)).unwrap();
    assert!(
        (app.active_practice()
            .unwrap()
            .current_item_delta()
            .unwrap()
            .speed
            - 0.4)
            .abs()
            < f64::EPSILON * 4.0
    );
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
    attempted.handle_event(key(KeyCode::Esc), start).unwrap();
    attempted
        .handle_event(key(KeyCode::Char('q')), start)
        .unwrap();
    assert_eq!(attempted.screen(), Screen::Practice);
    assert!(buffer_text(&draw(&attempted, 80, 24).buffer).contains("again"));
    assert!(attempted.sessions.is_empty());
    attempted
        .handle_event(key(KeyCode::Char('q')), start)
        .unwrap();
    assert_eq!(attempted.screen(), Screen::Result);
    assert_eq!(attempted.sessions.len(), 1);

    let (_empty_root, mut empty) = fixture_app();
    empty
        .start_mode(
            request(PracticeKind::Words, Language::En, "ab", StopRule::TargetEnd),
            start,
        )
        .unwrap();
    empty.handle_event(key(KeyCode::Esc), start).unwrap();
    empty.handle_event(key(KeyCode::Char('q')), start).unwrap();
    empty.handle_event(key(KeyCode::Char('q')), start).unwrap();
    assert_eq!(empty.screen(), Screen::Home);
    assert!(empty.result.is_none());
    assert!(empty.sessions.is_empty());
    assert!(!empty.paths.sessions.exists());
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
    let styles = app.themes.get("default").unwrap().styles().unwrap();
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
    app.handle_event(key(KeyCode::Char('A')), start).unwrap();
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
    let styles = app.themes.get("default").unwrap().styles().unwrap();
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
        let styles = app.themes.get("default").unwrap().styles().unwrap();
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

#[test]
fn long_text_filters_metadata_tracks_paragraphs_and_centers_the_cursor() {
    let (_root, mut app) = fixture_app();
    let essays = app.long_items(Language::En, Some("essay"));
    assert_eq!(essays.len(), 2);
    assert!(essays.iter().all(|item| {
        item.language == Language::En
            && item.kind == ContentKind::Text
            && item.tags.iter().any(|tag| tag == "essay")
    }));

    let start = Instant::now();
    app.start_long("en-text-essay-useful-pause", start).unwrap();
    let metadata = app.long_metadata().unwrap();
    assert_eq!(metadata.title, "The Use of a Useful Pause");
    assert_eq!(metadata.author, "Typeul contributors");
    assert_eq!(metadata.license, "CC0-1.0");
    assert_eq!(metadata.difficulty, Some(2));
    assert_eq!(metadata.tags, ["essay"]);
    assert!(metadata.source.ends_with("/assets/content/en-texts.toml"));
    assert_eq!(
        app.long_scroll().unwrap(),
        typeul::app::LongScroll {
            active_paragraph: 1,
            total_paragraphs: 3,
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
    assert_eq!(progress.total_paragraphs, 3);
    assert!((1..100).contains(&progress.percent));
    let drawn = draw(&app, 120, 40);
    let output = buffer_text(&drawn.buffer);
    for marker in [
        "The Use of a Useful Pause",
        "Typeul contributors",
        "CC0-1.0",
        "https://github.com/baba9811/typeul/blob/v1.0.0/assets/content/en-texts.toml",
        "Difficulty: 2",
        "essay",
        "Paragraph 3/3",
    ] {
        assert!(output.contains(marker), "missing {marker}: {output}");
    }
    let (_, cursor_y) = drawn.cursor.unwrap();
    assert!(
        (8..=22).contains(&cursor_y),
        "cursor not centered: {cursor_y}"
    );
    app.handle_event(key(KeyCode::Esc), start).unwrap();
    app.handle_event(key(KeyCode::Char('q')), start).unwrap();
    app.handle_event(key(KeyCode::Char('q')), start).unwrap();
    app.handle_event(key(KeyCode::Char('r')), start).unwrap();
    assert_eq!(
        app.long_metadata().unwrap().title,
        "The Use of a Useful Pause"
    );
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
    let cursor = drawn.cursor.unwrap();
    assert_eq!(drawn.buffer[cursor].symbol(), "C");
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
        app.handle_event(key(KeyCode::Char('a')), start + Duration::from_secs(second))
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
    assert!((long.best_rolling_speed - 12.0).abs() < f64::EPSILON * 8.0);
    assert!((1..100).contains(&long.percent));
    app.settings.ui_language = Language::Ko;
    let korean_result = buffer_text(&draw(&app, 80, 24).buffer);
    for marker in ["최고 30초 속도", "글자: 31/", "진행:"] {
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

#[test]
fn typing_test_uses_allowed_durations_sentence_extension_and_relative_grade() {
    let start = Instant::now();
    for seconds in [60, 180, 300, 600] {
        let (_root, mut app) = fixture_app();
        app.start_test(Language::En, Some(seconds), 7, start)
            .unwrap();
        assert_eq!(
            app.active_practice().unwrap().stop,
            StopRule::ActiveTime(Duration::from_secs(seconds))
        );
    }
    let (_invalid_root, mut invalid) = fixture_app();
    assert!(
        invalid
            .start_test(Language::En, Some(120), 7, start)
            .is_err()
    );
    assert_eq!(invalid.screen(), Screen::Home);

    let (_root, mut app) = fixture_app();
    app.warnings.clear();
    app.start_test(Language::En, None, 11, start).unwrap();
    let active = app.active_practice().unwrap();
    assert_eq!(active.stop, StopRule::ActiveTime(Duration::from_secs(300)));
    assert_eq!(active.content_ids.len(), 10);
    assert!(
        active
            .content_ids
            .iter()
            .all(|id| app.content.items().any(|item| {
                item.id == *id
                    && item.language == Language::En
                    && matches!(item.kind, ContentKind::Sentence | ContentKind::Quote)
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
    assert!(app.active_practice().unwrap().content_ids.len() > 10);

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
    assert!(output.contains("Typeul relative grade"), "{output}");
}
