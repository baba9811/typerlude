use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{Terminal, backend::TestBackend};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc::sync_channel,
    time::Instant,
};
use typerlude::{
    VERSION,
    app::{App, Screen},
    config::Settings,
    content::ContentCatalog,
    storage::AppPaths,
    theme::ThemeCatalog,
    ui::render,
    update::{InstallMethod, StableVersion, UpdateCache, UpdateNotice, notice, should_check},
};
use unicode_width::UnicodeWidthStr;

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "typerlude-update-{}-{}",
            std::process::id(),
            fastrand::u64(..)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
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
        AppPaths::from_override(root.0.join("home")),
        ContentCatalog::load_builtins().unwrap(),
        ThemeCatalog::load_builtins().unwrap(),
        Vec::new(),
        Vec::new(),
    );
    (root, app)
}

fn screen_text(app: &App) -> String {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
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

fn update_notice(version: &str) -> UpdateNotice {
    UpdateNotice {
        current: "1.0.0".parse().unwrap(),
        latest: version.parse().unwrap(),
        method: InstallMethod::Npm,
    }
}

#[test]
fn only_stable_three_part_versions_compare() {
    assert!("1.2.3".parse::<StableVersion>().unwrap() > "1.2.2".parse().unwrap());
    for value in [
        "v1.2.3",
        "1.2",
        "1.2.3-beta.1",
        "1.02.3",
        "1.2.3.4",
        "latest",
    ] {
        assert!(value.parse::<StableVersion>().is_err(), "{value}");
    }
}

#[test]
fn fresh_cache_and_exact_skipped_version_suppress_a_notice() {
    let now = 1_786_060_800;
    let cache = UpdateCache {
        schema_version: 1,
        checked_at_unix: now - 60,
        latest: "1.2.3".into(),
    };
    assert!(!should_check(Some(&cache), now));
    assert!(notice("1.0.0", "1.2.3", "1.2.3", InstallMethod::Npm).is_none());
    assert!(notice("1.0.0", "1.2.4", "1.2.3", InstallMethod::Npm).is_some());
}

#[test]
fn practice_queues_a_notice_home_and_result_render_it_and_skip_persists() {
    let (_root, mut app) = fixture_app();
    let (sender, receiver) = sync_channel(1);
    app.set_update_receiver(receiver);
    app.open(Screen::Practice);
    sender.send(Some(update_notice("1.2.3"))).unwrap();
    app.tick(Instant::now()).unwrap();
    assert!(app.update_notice.is_some());
    assert!(!screen_text(&app).contains("Update available"));

    app.open(Screen::Home);
    assert!(screen_text(&app).contains("Update available"));
    app.settings.ui_language = typerlude::model::Language::Ko;
    let korean = screen_text(&app);
    assert!(korean.contains("업데이트 가능"), "{korean}");
    assert!(korean.contains("나중에"), "{korean}");
    assert!(korean.contains("이번 버전 건너뛰기"), "{korean}");
    app.settings.ui_language = typerlude::model::Language::En;
    app.handle_event(
        Event::Key(KeyEvent::from(KeyCode::Char('l'))),
        Instant::now(),
    )
    .unwrap();
    assert!(app.update_notice.is_none());
    assert!(!app.paths.config.exists());

    let (sender, receiver) = sync_channel(1);
    app.set_update_receiver(receiver);
    sender.send(Some(update_notice("1.2.4"))).unwrap();
    app.tick(Instant::now()).unwrap();
    app.open(Screen::Result);
    let result = screen_text(&app);
    assert!(result.contains("Update available"), "{result}");
    assert!(
        result.contains("npm install -g @baba9811/typerlude@latest"),
        "{result}"
    );
    app.handle_event(
        Event::Key(KeyEvent::from(KeyCode::Char('s'))),
        Instant::now(),
    )
    .unwrap();
    assert!(app.update_notice.is_none());
    assert_eq!(app.settings.skipped_update_version, "1.2.4");
    assert_eq!(
        Settings::load(&app.paths)
            .unwrap()
            .value
            .skipped_update_version,
        "1.2.4"
    );
}

#[test]
fn failed_skip_save_preserves_the_notice_and_prior_setting() {
    let (root, mut app) = fixture_app();
    let blocked = root.0.join("blocked");
    fs::write(&blocked, b"not a directory").unwrap();
    app.paths.config = blocked.join("config.toml");
    app.update_notice = Some(update_notice("1.2.3"));
    app.open(Screen::Home);

    app.handle_event(
        Event::Key(KeyEvent::from(KeyCode::Char('s'))),
        Instant::now(),
    )
    .unwrap();

    assert!(app.update_notice.is_some());
    assert!(app.settings.skipped_update_version.is_empty());
    assert!(
        app.warnings
            .iter()
            .any(|warning| warning.contains("settings"))
    );
}

#[test]
fn a_silent_background_error_does_not_clear_an_existing_notice() {
    let (_root, mut app) = fixture_app();
    app.update_notice = Some(update_notice("1.2.3"));
    let (sender, receiver) = sync_channel(1);
    app.set_update_receiver(receiver);
    sender.send(None).unwrap();

    app.poll_update();

    assert_eq!(app.update_notice, Some(update_notice("1.2.3")));
}

#[test]
fn the_event_that_receives_a_notice_cannot_skip_it_before_first_render() {
    let (_root, mut app) = fixture_app();
    let (sender, receiver) = sync_channel(1);
    app.set_update_receiver(receiver);
    sender.send(Some(update_notice("1.2.3"))).unwrap();

    app.handle_event(
        Event::Key(KeyEvent::from(KeyCode::Char('s'))),
        Instant::now(),
    )
    .unwrap();

    assert_eq!(app.update_notice, Some(update_notice("1.2.3")));
    assert!(app.settings.skipped_update_version.is_empty());
    assert!(!app.paths.config.exists());
}

#[test]
fn foreground_standalone_check_is_headless_and_never_installs() {
    let root = TestDir::new();
    let home = root.0.join("home");
    let output = Command::new(env!("CARGO_BIN_EXE_typerlude"))
        .arg("update")
        .env("TYPERLUDE_HOME", &home)
        .env_remove("TYPERLUDE_INSTALL_METHOD")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains(&format!("current: {VERSION}")), "{stdout}");
    assert!(
        stdout.contains("latest: see https://github.com/baba9811/typerlude/releases"),
        "{stdout}"
    );
    assert!(
        stdout.contains("never installs updates automatically"),
        "{stdout}"
    );
    assert!(!home.exists());
}
