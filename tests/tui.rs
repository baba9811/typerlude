use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use time::macros::date;
use typeul::{
    app::{App, Grade, ItemDelta, ModeRequest, PracticeMode, ResultView, Screen, StopRule},
    config::Settings,
    content::ContentCatalog,
    model::{Difficulty, Language, PracticeKind},
    practice::InputOutcome,
    storage::{AppPaths, SessionRecord},
    theme::ThemeCatalog,
};

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
    }
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
            request(PracticeKind::Words, Language::En, "ab", StopRule::TargetEnd),
            Instant::now(),
        )
        .unwrap();
    for printable in ['q', '?', 'j', 'k'] {
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
            0
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
        Event::Paste("ab".into()),
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
