use atomic_write_file::AtomicWriteFile;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Barrier,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use time::{OffsetDateTime, macros::date};
use typeul::{
    config::Settings,
    i18n::initial_ui_language,
    model::{Language, PracticeKind},
    practice::PracticeEngine,
    storage::{AppPaths, SessionRecord, load_sessions, save_session},
};

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "typeul-storage-{}-{}",
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

fn fixture_session(id: &str) -> SessionRecord {
    SessionRecord {
        schema_version: 1,
        id: id.into(),
        started_at_unix_ms: 1_786_029_600_000,
        local_date: date!(2026 - 08 - 07),
        language: Language::En,
        mode: PracticeKind::Words,
        content_id: "en-word-001".into(),
        difficulty: Some(1),
        duration_ms: 60_000,
        correct_units: 4,
        attempted_units: 5,
        errors: 1,
        backspaces: 1,
        cpm: 4.0,
        kpm: 4.0,
        wpm: 0.8,
        accuracy: 80.0,
        intended_keys: BTreeMap::from([('a', [4, 1])]),
    }
}

#[test]
fn override_keeps_every_path_under_one_root() {
    let root = PathBuf::from("portable-typeul");
    let paths = AppPaths::from_override(root.clone());

    assert_eq!(paths.config, root.join("config.toml"));
    assert_eq!(paths.sessions, root.join("sessions"));
    assert_eq!(paths.content, root.join("content"));
    assert_eq!(paths.themes, root.join("themes"));
    assert_eq!(paths.update_cache, root.join("cache/update.json"));
}

#[test]
fn missing_and_partial_config_use_defaults_without_eager_writes() {
    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));

    let missing = Settings::load(&paths).unwrap();
    let mut expected = Settings::default();
    let lc_all = std::env::var("LC_ALL").ok();
    let lang = std::env::var("LANG").ok();
    expected.ui_language = initial_ui_language(lc_all.as_deref(), lang.as_deref());
    assert_eq!(missing.value, expected);
    assert!(missing.warnings.is_empty());
    assert!(!paths.config.exists());

    fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
    fs::write(
        &paths.config,
        "schema_version = 1\nlanguage = \"ko\"\nfuture_option = true\n",
    )
    .unwrap();
    let partial = Settings::load(&paths).unwrap();
    assert_eq!(partial.value.language, Language::Ko);
    assert_eq!(partial.value.target_wpm, Settings::default().target_wpm);
    assert!(partial.warnings.is_empty());
}

#[test]
fn config_round_trip_atomically_replaces_the_previous_value() {
    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));
    let mut settings = Settings {
        language: Language::Ko,
        daily_minutes: 25,
        ..Settings::default()
    };
    settings.save(&paths).unwrap();

    settings.daily_minutes = 40;
    settings.save(&paths).unwrap();

    let loaded = Settings::load(&paths).unwrap();
    assert_eq!(loaded.value, settings);
    assert!(loaded.warnings.is_empty());
    assert_eq!(
        fs::read_dir(paths.config.parent().unwrap())
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn discarded_atomic_write_leaves_saved_config_unchanged() {
    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));
    Settings::default().save(&paths).unwrap();
    let original = fs::read(&paths.config).unwrap();

    let mut uncommitted = AtomicWriteFile::open(&paths.config).unwrap();
    uncommitted.write_all(b"not a complete config").unwrap();
    drop(uncommitted);

    assert_eq!(fs::read(&paths.config).unwrap(), original);
}

#[test]
fn corrupt_and_unsupported_configs_are_preserved_with_a_warning() {
    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));
    fs::create_dir_all(paths.config.parent().unwrap()).unwrap();

    for original in [
        b"schema_version = [".as_slice(),
        b"schema_version = 2\nlanguage = \"ko\"\n".as_slice(),
        b"schema_version = 1\ntarget_accuracy = 101.0\n".as_slice(),
    ] {
        fs::write(&paths.config, original).unwrap();
        let loaded = Settings::load(&paths).unwrap();
        assert_eq!(loaded.value, Settings::default());
        assert_eq!(loaded.warnings.len(), 1);
        assert_eq!(loaded.warnings[0].path, paths.config);
        assert_eq!(fs::read(&paths.config).unwrap(), original);
    }
}

#[test]
fn session_from_result_copies_only_aggregate_engine_state() {
    let start = Instant::now();
    let mut engine = PracticeEngine::new(Language::En, PracticeKind::Words, "a", None).unwrap();
    engine.input("private wrong input", start);
    assert!(engine.backspace());
    engine.input("a", start + Duration::from_secs(60));
    let metrics = engine.metrics(start + Duration::from_secs(60));
    let started_at = OffsetDateTime::from_unix_timestamp(1_786_093_200).unwrap();

    let first = SessionRecord::from_result(
        started_at,
        Language::En,
        PracticeKind::Words,
        "en-word-001",
        Some(1),
        &metrics,
        engine.intended_keys(),
    );
    let second = SessionRecord::from_result(
        started_at,
        Language::En,
        PracticeKind::Words,
        "en-word-001",
        Some(1),
        &metrics,
        engine.intended_keys(),
    );

    assert_ne!(first.id, second.id);
    assert!(
        first
            .id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    );
    assert_eq!(first.started_at_unix_ms, 1_786_093_200_000);
    assert_eq!(first.duration_ms, 60_000);
    assert_eq!(first.correct_units, 1);
    assert_eq!(first.attempted_units, 2);
    assert_eq!(first.errors, 1);
    assert_eq!(first.backspaces, 1);
    assert_eq!(first.intended_keys.get(&'a'), Some(&[1, 1]));
}

#[test]
fn session_round_trip_uses_one_immutable_file() {
    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));
    let session = fixture_session("session-a");

    let saved = save_session(&paths, &session).unwrap();
    assert_eq!(saved, paths.sessions.join("session-a.json"));
    assert_eq!(load_sessions(&paths).unwrap().values, vec![session.clone()]);

    let original = fs::read(&saved).unwrap();
    assert!(save_session(&paths, &session).is_err());
    assert_eq!(fs::read(saved).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn session_save_preserves_a_dangling_destination_symlink() {
    use std::os::unix::fs::symlink;

    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));
    fs::create_dir_all(&paths.sessions).unwrap();
    let destination = paths.sessions.join("dangling.json");
    let missing = root.path().join("missing-session");
    symlink(&missing, &destination).unwrap();

    let error = save_session(&paths, &fixture_session("dangling")).unwrap_err();

    assert!(error.to_string().contains("already exists"), "{error:#}");
    assert_eq!(fs::read_link(&destination).unwrap(), missing);
    assert!(!root.path().join("missing-session").exists());
}

#[test]
fn session_save_preserves_a_destination_created_during_publish() {
    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));
    fs::create_dir_all(&paths.sessions).unwrap();
    let destination = paths.sessions.join("racing.json");
    let mut session = fixture_session("racing");
    session.intended_keys = (0x1000..0x196a0)
        .filter_map(char::from_u32)
        .map(|character| (character, [1, 0]))
        .collect();

    let watched_sessions = paths.sessions.clone();
    let watched_destination = destination.clone();
    let done = Arc::new(AtomicBool::new(false));
    let watcher_done = Arc::clone(&done);
    let watcher = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !watcher_done.load(Ordering::Acquire) && Instant::now() < deadline {
            let temporary_exists = fs::read_dir(&watched_sessions)
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".racing.json.")
                });
            if temporary_exists {
                match fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&watched_destination)
                {
                    Ok(mut file) => {
                        file.write_all(b"noncooperating writer").unwrap();
                        file.sync_all().unwrap();
                        return true;
                    }
                    Err(error) if error.kind() == ErrorKind::AlreadyExists => return false,
                    Err(error) => panic!("failed to create racing destination: {error}"),
                }
            }
            thread::yield_now();
        }
        false
    });

    let result = save_session(&paths, &session);
    done.store(true, Ordering::Release);

    assert!(watcher.join().unwrap(), "temporary file was not observed");
    let error = result.unwrap_err();
    assert!(error.to_string().contains("already exists"), "{error:#}");
    assert_eq!(fs::read(destination).unwrap(), b"noncooperating writer");
}

#[test]
fn an_empty_session_is_rejected_before_creating_storage() {
    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));
    let mut empty = fixture_session("empty");
    empty.attempted_units = 0;

    assert!(save_session(&paths, &empty).is_err());
    assert!(!paths.sessions.exists());
}

#[test]
fn a_preheld_session_lock_blocks_writes_and_preserves_the_destination() {
    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));
    let session = fixture_session("locked");
    fs::create_dir_all(&paths.sessions).unwrap();
    let lock_path = paths.sessions.join(".locked.lock");
    let _lock = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .unwrap();
    let destination = paths.sessions.join("locked.json");

    assert!(save_session(&paths, &session).is_err());
    assert!(!destination.exists());

    fs::write(&destination, b"existing session bytes").unwrap();
    assert!(save_session(&paths, &session).is_err());
    assert_eq!(fs::read(&destination).unwrap(), b"existing session bytes");
}

#[test]
fn concurrent_saves_of_one_id_create_exactly_one_session() {
    const WRITERS: usize = 8;
    let root = TestDir::new();
    let paths = Arc::new(AppPaths::from_override(root.path().join("home")));
    let session = Arc::new(fixture_session("shared"));
    let start = Arc::new(Barrier::new(WRITERS));
    let writes = (0..WRITERS)
        .map(|_| {
            let paths = Arc::clone(&paths);
            let session = Arc::clone(&session);
            let start = Arc::clone(&start);
            thread::spawn(move || {
                start.wait();
                save_session(&paths, &session)
            })
        })
        .collect::<Vec<_>>();

    let successes = writes
        .into_iter()
        .map(|write| write.join().unwrap())
        .filter(Result::is_ok)
        .count();

    assert_eq!(successes, 1);
    assert_eq!(
        load_sessions(&paths).unwrap().values,
        vec![(*session).clone()]
    );
    assert!(!paths.sessions.join(".shared.lock").exists());
}

#[test]
fn a_filename_id_mismatch_is_preserved_but_not_loaded_twice() {
    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));
    let session = fixture_session("canonical");
    let canonical = save_session(&paths, &session).unwrap();
    let alias = paths.sessions.join("alias.json");
    fs::copy(&canonical, &alias).unwrap();
    let original_alias = fs::read(&alias).unwrap();

    let loaded = load_sessions(&paths).unwrap();

    assert_eq!(loaded.values, vec![session]);
    assert_eq!(loaded.warnings.len(), 1);
    assert_eq!(loaded.warnings[0].path, alias);
    assert_eq!(fs::read(&loaded.warnings[0].path).unwrap(), original_alias);
}

#[test]
fn sessions_load_by_filename_while_bad_siblings_remain_untouched() {
    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));
    save_session(&paths, &fixture_session("b-session")).unwrap();
    save_session(&paths, &fixture_session("a-session")).unwrap();
    let broken = paths.sessions.join("broken.json");
    let unsupported = paths.sessions.join("unsupported.json");
    fs::write(&broken, b"{").unwrap();
    fs::write(
        &unsupported,
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 2,
            "id": "unsupported"
        }))
        .unwrap(),
    )
    .unwrap();

    let loaded = load_sessions(&paths).unwrap();

    assert_eq!(
        loaded
            .values
            .iter()
            .map(|value| value.id.as_str())
            .collect::<Vec<_>>(),
        ["a-session", "b-session"]
    );
    assert_eq!(loaded.warnings.len(), 2);
    assert_eq!(fs::read_to_string(&broken).unwrap(), "{");
    assert!(unsupported.exists());
}

#[test]
fn saved_json_contains_only_the_privacy_minimal_schema() {
    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));
    let saved = save_session(&paths, &fixture_session("privacy")).unwrap();
    let bytes = fs::read(&saved).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let keys = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        keys,
        BTreeSet::from([
            "accuracy",
            "attempted_units",
            "backspaces",
            "content_id",
            "correct_units",
            "cpm",
            "difficulty",
            "duration_ms",
            "errors",
            "id",
            "intended_keys",
            "kpm",
            "language",
            "local_date",
            "mode",
            "schema_version",
            "started_at_unix_ms",
            "wpm",
        ])
    );
    let text = String::from_utf8(bytes).unwrap();
    for forbidden in [
        "typed_text",
        "target_text",
        "custom_text",
        "keystrokes",
        "timestamps",
        "filename",
        "replay",
    ] {
        assert!(
            !text.contains(forbidden),
            "found forbidden field {forbidden}"
        );
    }
}
