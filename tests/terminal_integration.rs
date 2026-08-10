use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::{
    io::{Read, Write},
    sync::mpsc,
    thread,
    time::Instant,
};
use typeul::{
    app::{CustomTextSource, Screen, StopRule},
    cli::{PracticeArgs, Startup, prepare_app},
    model::{Language, PracticeKind},
    storage::AppPaths,
    terminal::write_restore_sequence,
};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "typeul-terminal-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
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

fn command(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_typeul"))
        .args(args)
        .env("TYPEUL_HOME", root)
        .env("TYPEUL_NO_UPDATE_CHECK", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn noninteractive_launch_fails_before_enabling_raw_mode() {
    let root = TestDir::new();
    let output = command(root.path(), &["words"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("interactive terminal"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.stdout.contains(&0x1b));
    assert!(!output.stderr.contains(&0x1b));
    assert!(!root.path().join("sessions").exists());
}

#[test]
fn headless_commands_still_work_without_a_terminal() {
    let root = TestDir::new();
    let output = command(root.path(), &["--version"]);

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("typeul "));
    assert!(!output.stdout.contains(&0x1b));
    assert!(!output.stderr.contains(&0x1b));
}

#[test]
fn terminal_restore_sequence_is_safe_to_repeat() {
    let mut writer = Vec::new();

    write_restore_sequence(&mut writer).unwrap();
    write_restore_sequence(&mut writer).unwrap();

    assert_eq!(
        writer
            .windows(4)
            .filter(|window| *window == b"?25h")
            .count(),
        2
    );
    assert_eq!(
        writer
            .windows(8)
            .filter(|window| *window == b"\x1b[?2004l")
            .count(),
        2
    );
    assert_eq!(
        writer
            .windows(8)
            .filter(|window| *window == b"\x1b[?1049l")
            .count(),
        2
    );
}

#[test]
fn validated_startups_build_the_requested_app_before_terminal_entry() {
    let root = TestDir::new();
    let paths = AppPaths::from_override(root.path().join("home"));
    let mut quick = PracticeArgs::new(PracticeKind::Quick);
    quick.language = Some(Language::Ko);
    quick.seconds = Some(60);

    let app = prepare_app(Startup::Practice(quick), paths.clone()).unwrap();
    let active = app.active_practice().unwrap();
    assert_eq!(active.kind(), PracticeKind::Quick);
    assert_eq!(active.engine.language(), Language::Ko);
    assert_eq!(active.stop, StopRule::ActiveTime(Duration::from_secs(60)));

    let stats = prepare_app(Startup::Stats, paths.clone()).unwrap();
    assert_eq!(stats.screen(), Screen::Stats);
    assert!(stats.active_practice().is_none());

    let custom = prepare_app(
        Startup::CustomText {
            source: CustomTextSource::Stdin,
            name: "stdin".into(),
            text: "custom text".into(),
        },
        paths,
    )
    .unwrap();
    assert_eq!(custom.screen(), Screen::Practice);
    assert_eq!(custom.active_practice().unwrap().kind(), PracticeKind::Long);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn piped_stdin_keeps_reading_events_from_the_controlling_terminal() {
    let root = TestDir::new();
    let input = root.path().join("input.txt");
    fs::write(&input, "custom text\n").unwrap();

    let mut command = Command::new("/usr/bin/script");
    #[cfg(target_os = "macos")]
    command.args([
        "-q",
        "-e",
        "/dev/null",
        "/bin/sh",
        "-c",
        "exec \"$TYPEUL_TEST_BIN\" < \"$TYPEUL_TEST_INPUT\"",
    ]);
    #[cfg(target_os = "linux")]
    command.args([
        "-q",
        "-e",
        "-c",
        "exec \"$TYPEUL_TEST_BIN\" < \"$TYPEUL_TEST_INPUT\"",
        "/dev/null",
    ]);
    let mut child = command
        .env("TYPEUL_TEST_BIN", env!("CARGO_BIN_EXE_typeul"))
        .env("TYPEUL_TEST_INPUT", &input)
        .env("TYPEUL_HOME", root.path().join("home"))
        .env("TYPEUL_NO_UPDATE_CHECK", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdout = child.stdout.take().unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let stdout_reader = thread::spawn(move || {
        let mut output = Vec::new();
        let mut chunk = [0; 4_096];
        let mut entered = false;
        loop {
            let read = stdout.read(&mut chunk).unwrap();
            if read == 0 {
                break;
            }
            output.extend_from_slice(&chunk[..read]);
            if !entered && output.windows(8).any(|window| window == b"\x1b[?1049h") {
                let _ = entered_tx.send(());
                entered = true;
            }
        }
        output
    });
    let mut stderr = child.stderr.take().unwrap();
    let stderr_reader = thread::spawn(move || {
        let mut output = Vec::new();
        stderr.read_to_end(&mut output).unwrap();
        output
    });

    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("piped launch never entered the alternate screen");
    let _ = child.stdin.take().unwrap().write_all(&[3]);
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "piped launch did not exit");
        thread::sleep(Duration::from_millis(25));
    };
    let stdout = stdout_reader.join().unwrap();
    let stderr = stderr_reader.join().unwrap();
    let transcript = format!(
        "{}{}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    );

    assert!(status.success(), "{transcript}");
    assert!(transcript.contains("\x1b[?1049h"), "{transcript:?}");
    assert!(transcript.contains("\x1b[?1049l"), "{transcript:?}");
    assert!(!transcript.contains("Failed to initialize input reader"));
}
