use std::{
    ffi::OsString,
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};
use typeul::{
    VERSION,
    cli::{Command, ContentCommand, Exit, PracticeArgs, Startup, parse_args, run},
    model::{Language, PracticeKind},
};

const MAX_INPUT_BYTES: u64 = 8 * 1024 * 1024;
static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "typeul-cli-{name}-{}-{}",
            std::process::id(),
            NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn home(&self) -> PathBuf {
        self.0.join("home")
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn binary(root: &Path, args: &[&str]) -> Output {
    ProcessCommand::new(env!("CARGO_BIN_EXE_typeul"))
        .args(args)
        .env("TYPEUL_HOME", root)
        .env("TYPEUL_TEST", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn piped_binary(root: &Path, args: &[&str], input: &[u8]) -> Output {
    let mut child = ProcessCommand::new(env!("CARGO_BIN_EXE_typeul"))
        .args(args)
        .env("TYPEUL_HOME", root)
        .env("TYPEUL_TEST", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let error = child.stdin.take().unwrap().write_all(input).err();
    assert!(
        error
            .as_ref()
            .is_none_or(|error| error.kind() == ErrorKind::BrokenPipe),
        "{error:?}"
    );
    child.wait_with_output().unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn pack(id: &str, item_id: &str, text: &str) -> String {
    format!(
        r#"schema_version = 1
id = "{id}"
title = "Test pack"
language = "en"

[source]
author = "Test author"
source_id = "test-source"
source_url = "https://example.com/source"
license = "CC0-1.0"
license_url = "https://creativecommons.org/publicdomain/zero/1.0/"
modified = false
retrieved_at = "2026-08-07"

[[items]]
id = "{item_id}"
kind = "word"
text = "{text}"
difficulty = 1
"#
    )
}

#[test]
fn parses_documented_launch_commands_without_a_framework() {
    assert_eq!(parse_args(Vec::new()).unwrap(), Command::Home);
    let quick = ["quick", "--lang", "ko", "--time", "60"]
        .into_iter()
        .map(OsString::from)
        .collect();
    assert!(matches!(
        parse_args(quick),
        Ok(Command::Practice(PracticeArgs {
            kind: PracticeKind::Quick,
            language: Some(Language::Ko),
            seconds: Some(60),
            word_count: None,
            file: None,
        }))
    ));

    let path = PathBuf::from("notes/readme.txt");
    assert_eq!(
        parse_args(vec![path.clone().into_os_string()]).unwrap(),
        Command::File(path.clone())
    );
    assert!(matches!(
        parse_args(vec![OsString::from("practice"), path.into_os_string()]),
        Ok(Command::Practice(PracticeArgs {
            kind: PracticeKind::Long,
            file: Some(_),
            ..
        }))
    ));

    for (args, expected) in [
        (
            vec!["keys"],
            Command::Practice(PracticeArgs::new(PracticeKind::Key)),
        ),
        (
            vec!["words"],
            Command::Practice(PracticeArgs::new(PracticeKind::Words)),
        ),
        (
            vec!["sentence"],
            Command::Practice(PracticeArgs::new(PracticeKind::Sentence)),
        ),
        (
            vec!["long"],
            Command::Practice(PracticeArgs::new(PracticeKind::Long)),
        ),
        (vec!["stats"], Command::Stats),
        (vec!["history"], Command::History),
        (vec!["themes"], Command::Themes),
        (vec!["paths"], Command::Paths),
        (vec!["licenses"], Command::Licenses),
        (vec!["update"], Command::Update),
        (vec!["--version"], Command::Version),
        (vec!["--help"], Command::Help),
        (vec!["--smoke"], Command::Smoke),
    ] {
        let args = args.into_iter().map(OsString::from).collect();
        assert_eq!(parse_args(args).unwrap(), expected);
    }

    assert_eq!(
        parse_args(
            ["content", "validate"]
                .into_iter()
                .map(OsString::from)
                .collect()
        )
        .unwrap(),
        Command::Content(ContentCommand::Validate(None))
    );
}

#[test]
fn parser_rejects_wrong_duplicate_missing_and_trailing_options() {
    for args in [
        vec!["quick", "--time", "42"],
        vec!["quick", "--time", "60", "--time", "120"],
        vec!["quick", "--lang"],
        vec!["quick", "--lang", "fr"],
        vec!["words", "--time", "60"],
        vec!["test", "--time", "120"],
        vec!["stats", "extra"],
        vec!["content", "list", "extra"],
        vec!["content", "disable"],
        vec!["--unknown"],
    ] {
        let args = args.into_iter().map(OsString::from).collect();
        assert!(parse_args(args).is_err());
    }

    for seconds in [15, 30, 60, 120] {
        let args = ["quick", "--time", &seconds.to_string()]
            .into_iter()
            .map(OsString::from)
            .collect();
        assert!(parse_args(args).is_ok(), "quick {seconds}");
    }
    for seconds in [60, 180, 300, 600] {
        let args = ["test", "--time", &seconds.to_string()]
            .into_iter()
            .map(OsString::from)
            .collect();
        assert!(parse_args(args).is_ok(), "test {seconds}");
    }
}

#[cfg(unix)]
#[test]
fn parser_rejects_non_unicode_option_values() {
    use std::os::unix::ffi::OsStringExt;
    assert!(
        parse_args(vec![
            OsString::from("quick"),
            OsString::from("--lang"),
            OsString::from_vec(vec![0xff]),
        ])
        .is_err()
    );
}

#[cfg(windows)]
#[test]
fn parser_rejects_non_unicode_option_values() {
    use std::os::windows::ffi::OsStringExt;
    assert!(
        parse_args(vec![
            OsString::from("quick"),
            OsString::from("--lang"),
            OsString::from_wide(&[0xd800]),
        ])
        .is_err()
    );
}

#[test]
fn version_help_licenses_and_paths_are_headless() {
    let root = TestDir::new("headless");
    let home = root.home();

    let version = binary(&home, &["--version"]);
    assert!(version.status.success());
    assert_eq!(stdout(&version).trim(), format!("typeul {VERSION}"));

    let help = binary(&home, &["--help"]);
    assert!(help.status.success());
    for command in ["quick", "content add", "paths", "licenses", "update"] {
        assert!(stdout(&help).contains(command), "{command}");
    }

    let licenses = binary(&home, &["licenses"]);
    assert!(licenses.status.success());
    for text in [
        "Typeul software: MIT",
        "THIRD_PARTY_NOTICES.md",
        "CC0 1.0 Universal",
        "Attribution 2.0 France",
    ] {
        assert!(stdout(&licenses).contains(text), "{text}");
    }

    let paths = binary(&home, &["paths"]);
    assert!(paths.status.success());
    let paths = stdout(&paths);
    for expected in [
        home.join("config.toml"),
        home.join("sessions"),
        home.join("content"),
        home.join("themes"),
        home.join("cache/update.json"),
    ] {
        assert!(paths.contains(expected.to_str().unwrap()), "{expected:?}");
    }
    assert!(!home.exists());
}

#[test]
fn parse_and_input_errors_exit_two_but_storage_errors_exit_one() {
    let root = TestDir::new("exit-codes");
    assert_eq!(binary(&root.home(), &["--unknown"]).status.code(), Some(2));

    let empty = root.path().join("empty.txt");
    fs::write(&empty, []).unwrap();
    assert_eq!(
        binary(&root.home(), &[empty.to_str().unwrap()])
            .status
            .code(),
        Some(2)
    );

    let runtime_home = root.path().join("runtime-home");
    fs::create_dir_all(runtime_home.join("config.toml")).unwrap();
    let runtime = binary(&runtime_home, &["--smoke"]);
    assert_eq!(runtime.status.code(), Some(1), "{}", stderr(&runtime));
}

#[test]
fn direct_files_are_bounded_nonempty_utf8_custom_text() {
    let root = TestDir::new("direct-file");
    let valid = root.path().join("한글.txt");
    fs::write(&valid, "연습 text\n").unwrap();

    let exit = run(Command::File(valid.clone())).unwrap();
    assert_eq!(
        exit,
        Exit::Launch(Startup::CustomText {
            name: "한글.txt".into(),
            text: "연습 text\n".into(),
        })
    );
    assert!(
        binary(&root.home(), &[valid.to_str().unwrap()])
            .status
            .success()
    );

    let whitespace = root.path().join("whitespace.txt");
    fs::write(&whitespace, " \n\t").unwrap();
    assert_eq!(
        binary(&root.home(), &[whitespace.to_str().unwrap()])
            .status
            .code(),
        Some(2)
    );

    let invalid = root.path().join("invalid.txt");
    fs::write(&invalid, [0xff]).unwrap();
    assert_eq!(
        binary(&root.home(), &[invalid.to_str().unwrap()])
            .status
            .code(),
        Some(2)
    );

    let oversized = root.path().join("oversized.txt");
    fs::File::create(&oversized)
        .unwrap()
        .set_len(MAX_INPUT_BYTES + 1)
        .unwrap();
    assert_eq!(
        binary(&root.home(), &[oversized.to_str().unwrap()])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn no_arg_nonterminal_stdin_is_bounded_custom_text() {
    let root = TestDir::new("stdin");
    assert!(
        piped_binary(&root.home(), &[], "stdin 연습".as_bytes())
            .status
            .success()
    );
    assert_eq!(piped_binary(&root.home(), &[], b"").status.code(), Some(2));
    assert_eq!(
        piped_binary(&root.home(), &[], b" \n\t").status.code(),
        Some(2)
    );
    assert_eq!(
        piped_binary(&root.home(), &[], &[0xff]).status.code(),
        Some(2)
    );
    assert_eq!(
        piped_binary(&root.home(), &[], &vec![b'a'; MAX_INPUT_BYTES as usize + 1])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn smoke_loads_everything_and_preserves_invalid_files_as_warnings() {
    let root = TestDir::new("smoke");
    let home = root.home();
    fs::create_dir_all(home.join("sessions")).unwrap();
    fs::create_dir_all(home.join("content")).unwrap();
    let config = home.join("config.toml");
    let session = home.join("sessions/broken.json");
    let content = home.join("content/broken.toml");
    let oversized = home.join("content/oversized.toml");
    fs::write(&config, b"schema_version = [").unwrap();
    fs::write(&session, b"{").unwrap();
    fs::write(&content, b"not = [toml").unwrap();
    fs::File::create(&oversized)
        .unwrap()
        .set_len(MAX_INPUT_BYTES + 1)
        .unwrap();

    let output = binary(&home, &["--smoke"]);
    assert!(output.status.success(), "{}", stderr(&output));
    for name in ["config.toml", "broken.json", "broken", "oversized"] {
        assert!(
            stderr(&output).contains(name),
            "{name}: {}",
            stderr(&output)
        );
    }
    assert_eq!(fs::read(&config).unwrap(), b"schema_version = [");
    assert_eq!(fs::read(&session).unwrap(), b"{");
    assert_eq!(fs::read(&content).unwrap(), b"not = [toml");
    assert_eq!(fs::metadata(&oversized).unwrap().len(), MAX_INPUT_BYTES + 1);
}

#[test]
fn content_validate_and_add_use_the_startup_conflict_rules() {
    let root = TestDir::new("content-add");
    let home = root.home();
    let candidate = root.path().join("candidate.toml");
    let original = pack("user-pack", "user-item", "zephyr");
    fs::write(&candidate, &original).unwrap();

    let validated = binary(&home, &["content", "validate", candidate.to_str().unwrap()]);
    assert!(validated.status.success(), "{}", stderr(&validated));
    assert!(!home.exists());

    let added = binary(&home, &["content", "add", candidate.to_str().unwrap()]);
    assert!(added.status.success(), "{}", stderr(&added));
    assert_eq!(
        fs::read(home.join("content/user-pack.toml")).unwrap(),
        original.as_bytes()
    );

    let list = binary(&home, &["content", "list"]);
    let listed = stdout(&list);
    assert!(list.status.success(), "{}", stderr(&list));
    for field in [
        "user-pack",
        "language=en",
        "items=1",
        "license=CC0-1.0",
        "source=https://example.com/source",
    ] {
        assert!(listed.contains(field), "{field}: {listed}");
    }

    let conflicting = root.path().join("conflicting.toml");
    fs::write(
        &conflicting,
        pack("another-pack", "user-item", "another-word"),
    )
    .unwrap();
    let output = binary(&home, &["content", "add", conflicting.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(!home.join("content/another-pack.toml").exists());

    let builtin = root.path().join("builtin.toml");
    fs::write(&builtin, pack("en-words", "unique-item", "newword")).unwrap();
    assert_eq!(
        binary(&home, &["content", "add", builtin.to_str().unwrap()])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn content_add_rejects_unsafe_ids_and_never_overwrites() {
    let root = TestDir::new("content-no-overwrite");
    let home = root.home();
    let unsafe_pack = root.path().join("unsafe.toml");
    fs::write(&unsafe_pack, pack("../escaped", "safe-item", "safe-text")).unwrap();

    let unsafe_output = binary(&home, &["content", "add", unsafe_pack.to_str().unwrap()]);
    assert_eq!(unsafe_output.status.code(), Some(2));
    assert!(!root.path().join("escaped.toml").exists());

    fs::create_dir_all(home.join("content")).unwrap();
    let destination = home.join("content/existing.toml");
    fs::write(&destination, b"review pending").unwrap();
    let candidate = root.path().join("existing.toml");
    fs::write(&candidate, pack("existing", "existing-item", "quartz")).unwrap();
    let output = binary(&home, &["content", "add", candidate.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(fs::read(destination).unwrap(), b"review pending");
}

#[test]
fn content_disable_resolves_pack_id_and_moves_only_that_user_file() {
    let root = TestDir::new("content-disable");
    let home = root.home();
    let content = home.join("content");
    fs::create_dir_all(&content).unwrap();
    let enabled = content.join("reviewed-filename.toml");
    let original = pack("user-disable", "disable-item", "fjord");
    fs::write(&enabled, &original).unwrap();

    let output = binary(&home, &["content", "disable", "user-disable"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(!enabled.exists());
    assert_eq!(
        fs::read(content.join("disabled/reviewed-filename.toml")).unwrap(),
        original.as_bytes()
    );

    assert_eq!(
        binary(&home, &["content", "disable", "en-words"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        binary(&home, &["content", "disable", "../outside"])
            .status
            .code(),
        Some(2)
    );
    assert!(!root.path().join("outside.toml").exists());
}

#[test]
fn content_disable_refuses_an_existing_disabled_destination() {
    let root = TestDir::new("disable-no-overwrite");
    let home = root.home();
    let content = home.join("content");
    let disabled = content.join("disabled");
    fs::create_dir_all(&disabled).unwrap();
    let enabled = content.join("same.toml");
    let original = pack("same", "same-item", "vivid");
    fs::write(&enabled, &original).unwrap();
    fs::write(disabled.join("same.toml"), b"older disabled pack").unwrap();

    let output = binary(&home, &["content", "disable", "same"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert_eq!(fs::read(enabled).unwrap(), original.as_bytes());
    assert_eq!(
        fs::read(disabled.join("same.toml")).unwrap(),
        b"older disabled pack"
    );
}

#[test]
fn content_validate_reports_active_pack_warnings_as_input_errors() {
    let root = TestDir::new("validate-active");
    let home = root.home();
    let broken = home.join("content/broken.toml");
    fs::create_dir_all(broken.parent().unwrap()).unwrap();
    fs::write(&broken, b"schema_version = [").unwrap();

    let output = binary(&home, &["content", "validate"]);

    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(stderr(&output).contains("broken"));
    assert_eq!(fs::read(broken).unwrap(), b"schema_version = [");
}

#[cfg(unix)]
#[test]
fn concurrent_adds_with_different_pack_ids_cannot_commit_one_item_twice() {
    let root = TestDir::new("concurrent-content-conflict");
    let home = root.home();
    let first_pipe = root.path().join("first.pipe");
    let second_pipe = root.path().join("second.pipe");
    let status = ProcessCommand::new("mkfifo")
        .arg(&first_pipe)
        .arg(&second_pipe)
        .status()
        .unwrap();
    assert!(status.success());

    let spawn = |path: &Path| {
        ProcessCommand::new(env!("CARGO_BIN_EXE_typeul"))
            .args(["content", "add"])
            .arg(path)
            .env("TYPEUL_HOME", &home)
            .env("TYPEUL_TEST", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let first = spawn(&first_pipe);
    let second = spawn(&second_pipe);

    // Both writer opens complete only after both children have reached their
    // candidate read. In the pre-fix flow that is after each catalog snapshot.
    let mut first_writer = fs::OpenOptions::new()
        .write(true)
        .open(&first_pipe)
        .unwrap();
    let mut second_writer = fs::OpenOptions::new()
        .write(true)
        .open(&second_pipe)
        .unwrap();
    first_writer
        .write_all(pack("parallel-a", "shared-item", "shared-text").as_bytes())
        .unwrap();
    second_writer
        .write_all(pack("parallel-b", "shared-item", "shared-text").as_bytes())
        .unwrap();
    drop(first_writer);
    drop(second_writer);

    let outputs = [
        first.wait_with_output().unwrap(),
        second.wait_with_output().unwrap(),
    ];
    let successes = outputs
        .iter()
        .filter(|output| output.status.success())
        .count();
    assert_eq!(
        successes,
        1,
        "first={} second={}",
        stderr(&outputs[0]),
        stderr(&outputs[1])
    );
    assert_eq!(
        fs::read_dir(home.join("content"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "toml"))
            .count(),
        1
    );
}
