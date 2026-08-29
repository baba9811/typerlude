pub(super) use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::{Buffer, Cell},
    layout::Rect,
    style::{Color, Modifier, Style},
};
pub(super) use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
pub(super) use time::{Date, OffsetDateTime, UtcOffset};
pub(super) use typerlude::{
    app::{
        App, CustomTextSource, Grade, InputEvent, ItemDelta, Key, KeyInput, KeyKind, KeyModifiers,
        ModeRequest, PracticeMode, QuickOptions, QuickSource, ResultView, Screen, StopRule, grade,
        key_sequence, key_stages,
    },
    config::Settings,
    content::{ContentCatalog, ContentKind},
    model::{Difficulty, Language, PracticeKind},
    practice::{InputOutcome, PracticeEngine},
    stats::{KeyAccuracy, Range, adaptive_candidates, summarize},
    storage::{AppPaths, SessionRecord},
    theme::ThemeCatalog,
    tui::{practice_cursor, render},
};
pub(super) use unicode_segmentation::UnicodeSegmentation;
pub(super) use unicode_width::UnicodeWidthStr;

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

pub(super) struct TestDir(PathBuf);

impl TestDir {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "typerlude-tui-{}-{}",
            std::process::id(),
            NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn fixture_app() -> (TestDir, App) {
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

pub(super) fn local_today() -> Date {
    let now = OffsetDateTime::now_utc();
    now.to_offset(UtcOffset::local_offset_at(now).unwrap_or(UtcOffset::UTC))
        .date()
}

pub(super) fn key(code: Key) -> InputEvent {
    key_with(code, KeyModifiers::NONE, KeyKind::Press)
}

pub(super) fn key_with(code: Key, modifiers: KeyModifiers, kind: KeyKind) -> InputEvent {
    InputEvent::Key(KeyInput {
        key: code,
        modifiers,
        kind,
    })
}

pub(super) fn open_mode_options(app: &mut App, index: usize, now: Instant) {
    for _ in 0..index {
        app.handle_event(key(Key::Tab), now).unwrap();
    }
    app.handle_event(key(Key::Enter), now).unwrap();
    assert_eq!(app.screen(), Screen::ModeOptions);
    assert_eq!(app.focus(), 0);
}

pub(super) fn press(app: &mut App, code: Key, count: usize, now: Instant) {
    for _ in 0..count {
        app.handle_event(key(code), now).unwrap();
    }
}

pub(super) fn type_text(app: &mut App, value: &str, now: Instant) {
    for character in value.chars() {
        let code = if character == '\n' {
            Key::Enter
        } else {
            Key::Char(character)
        };
        app.handle_event(key(code), now).unwrap();
    }
}

pub(super) fn assert_catalog_progress(app: &App, expected: usize) {
    match &app.active_practice().unwrap().mode {
        PracticeMode::Quick { completed } | PracticeMode::Sentence { completed, .. } => {
            assert_eq!(*completed, expected);
        }
        PracticeMode::Words {
            completed, streak, ..
        } => assert_eq!((*completed, *streak), (expected, expected)),
        _ => unreachable!(),
    }
}

pub(super) fn mode_for(kind: PracticeKind) -> PracticeMode {
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
                kpm: 72.5,
                wpm: 14.5,
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

pub(super) fn request(
    kind: PracticeKind,
    language: Language,
    target: &str,
    stop: StopRule,
) -> ModeRequest {
    let target_len = target.graphemes(true).count();
    ModeRequest {
        kind,
        language,
        target: target.into(),
        mode: mode_for(kind),
        stop,
        item_ends: if target_len > 1 {
            vec![1, target_len]
        } else {
            vec![target_len]
        },
        content_ids: vec!["first-item".into(), "second-item".into()],
    }
}

pub(super) fn result_view(id: &str) -> ResultView {
    ResultView {
        session: SessionRecord {
            schema_version: 1,
            id: id.into(),
            started_at_unix_ms: 1_786_029_600_000,
            local_date: local_today(),
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
        previous_kpm: Some(50.0),
        previous_wpm: Some(10.0),
        best_kpm: Some(60.0),
        best_wpm: Some(12.0),
        kpm_delta: Some(10.0),
        wpm_delta: Some(2.0),
        speed_goal: 80.0,
        accuracy_goal: 98.0,
        daily_minutes_goal: 15,
        speed_goal_met: true,
        accuracy_goal_met: true,
        daily_minutes_met: false,
        weak_keys: Vec::new(),
        grade: None,
        save_error: Some("preserve this result".into()),
        long: None,
    }
}

pub(super) fn user_pack(id: &str) -> String {
    format!(
        r#"schema_version = 1
id = "{id}"
title = "TUI test pack"
language = "en"

[source]
author = "Pack author"
source_id = "pack-source"
source_url = "https://example.com/pack-source"
license = "CC-BY-4.0"
license_url = "https://creativecommons.org/licenses/by/4.0/"
modified = false
retrieved_at = "2026-08-07"

[[items]]
id = "{id}-item"
kind = "word"
text = "zephyr"
difficulty = 1

[items.source]
author = "Test author"
source_id = "test-source"
source_url = "https://example.com/source"
license = "CC0-1.0"
license_url = "https://creativecommons.org/publicdomain/zero/1.0/"
modified = false
retrieved_at = "2026-08-07"
"#
    )
}

pub(super) struct Drawn {
    pub(super) buffer: Buffer,
    pub(super) cursor: Option<(u16, u16)>,
}

pub(super) fn draw(app: &App, width: u16, height: u16) -> Drawn {
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

pub(super) fn buffer_text(buffer: &Buffer) -> String {
    buffer
        .content
        .chunks(buffer.area.width as usize)
        .map(|row| {
            let mut output = String::new();
            let mut hidden = 0_usize;
            for cell in row {
                if hidden == 0 {
                    output.push_str(cell.symbol());
                    hidden = UnicodeWidthStr::width(cell.symbol()).saturating_sub(1);
                } else {
                    hidden -= 1;
                }
            }
            output
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn visible_buffer(buffer: &Buffer) -> Buffer {
    let mut visible = buffer.clone();
    for row in visible.content.chunks_mut(visible.area.width as usize) {
        let mut hidden = 0_usize;
        for cell in row {
            if hidden == 0 {
                hidden = UnicodeWidthStr::width(cell.symbol()).saturating_sub(1);
            } else {
                cell.reset();
                hidden -= 1;
            }
        }
    }
    visible
}

pub(super) fn assert_role_style(cell: &Cell, expected: Style) {
    assert_eq!(Some(cell.fg), expected.fg);
    assert_eq!(Some(cell.bg), expected.bg);
    assert_eq!(cell.modifier, expected.add_modifier);
}

#[derive(Clone, Copy)]
pub(super) struct ExpectedThemeStyles {
    pub(super) base: Style,
    pub(super) accent: Style,
    pub(super) correct: Style,
    pub(super) error: Style,
    pub(super) cursor: Style,
    pub(super) dim: Style,
}

pub(super) fn default_styles() -> ExpectedThemeStyles {
    let role = |color| Style::default().fg(color).bg(Color::Black);
    ExpectedThemeStyles {
        base: role(Color::White),
        accent: role(Color::Cyan),
        correct: role(Color::Green),
        error: role(Color::Red).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        cursor: role(Color::Yellow).add_modifier(Modifier::BOLD | Modifier::REVERSED),
        dim: role(Color::DarkGray),
    }
}
