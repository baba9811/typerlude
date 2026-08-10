use crate::{
    VERSION,
    config::Settings,
    storage::{AppPaths, atomic_write},
};
use anyhow::{Context, Result, bail};
use command_group::CommandGroup;
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::{
    env,
    ffi::OsStr,
    fmt, fs,
    io::{IsTerminal, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr,
    sync::mpsc::{Receiver, sync_channel},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const CACHE_INTERVAL_SECONDS: i64 = 24 * 60 * 60;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(25);
const MAX_COMMAND_OUTPUT_BYTES: usize = 4 * 1024;
const MAX_CACHE_BYTES: u64 = 4 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StableVersion(pub u64, pub u64, pub u64);

impl FromStr for StableVersion {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        let mut parts = value.split('.');
        let mut next = || -> Result<u64> {
            let part = parts.next().context("version must have three components")?;
            if part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
            {
                bail!("invalid stable version component");
            }
            part.parse().context("version component is too large")
        };
        let version = Self(next()?, next()?, next()?);
        if parts.next().is_some() {
            bail!("version must have exactly three components");
        }
        Ok(version)
    }
}

impl fmt::Display for StableVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.0, self.1, self.2)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallMethod {
    Npm,
    Cargo,
    Standalone,
}

impl InstallMethod {
    pub const fn instructions(self) -> &'static str {
        match self {
            Self::Npm => "npm install -g typeul@latest · npx typeul@latest",
            Self::Cargo => "cargo install --force typeul",
            Self::Standalone => "https://github.com/baba9811/typeul/releases",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateNotice {
    pub current: StableVersion,
    pub latest: StableVersion,
    pub method: InstallMethod,
}

impl UpdateNotice {
    pub const fn instructions(&self) -> &'static str {
        self.method.instructions()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UpdateCache {
    pub schema_version: u16,
    pub checked_at_unix: i64,
    pub latest: String,
}

pub fn should_check(cache: Option<&UpdateCache>, now: i64) -> bool {
    let Some(cache) = cache
        .filter(|cache| cache.schema_version == 1 && cache.latest.parse::<StableVersion>().is_ok())
    else {
        return true;
    };
    !now.checked_sub(cache.checked_at_unix)
        .is_some_and(|age| (0..CACHE_INTERVAL_SECONDS).contains(&age))
}

pub fn notice(
    current: &str,
    latest: &str,
    skipped: &str,
    method: InstallMethod,
) -> Option<UpdateNotice> {
    let current = current.parse::<StableVersion>().ok()?;
    let latest = latest.parse::<StableVersion>().ok()?;
    (latest > current && skipped != latest.to_string()).then_some(UpdateNotice {
        current,
        latest,
        method,
    })
}

pub fn detect_install_method() -> InstallMethod {
    let marker = env::var_os("TYPEUL_INSTALL_METHOD");
    if marker.as_deref() == Some(OsStr::new("npm")) {
        return InstallMethod::Npm;
    }
    let Some(executable) = env::current_exe().ok().map(resolve_path) else {
        return InstallMethod::Standalone;
    };
    let Some(cargo_home) = cargo_home().map(resolve_path) else {
        return InstallMethod::Standalone;
    };
    install_method_from(None, &executable, &cargo_home)
}

fn install_method_from(
    marker: Option<&OsStr>,
    executable: &Path,
    cargo_home: &Path,
) -> InstallMethod {
    if marker == Some(OsStr::new("npm")) {
        return InstallMethod::Npm;
    }
    if executable.parent() == Some(cargo_home.join("bin").as_path()) {
        InstallMethod::Cargo
    } else {
        InstallMethod::Standalone
    }
}

fn cargo_home() -> Option<PathBuf> {
    env::var_os("CARGO_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| BaseDirs::new().map(|dirs| dirs.home_dir().join(".cargo")))
}

fn resolve_path(path: PathBuf) -> PathBuf {
    let path = if path.is_absolute() {
        path
    } else {
        env::current_dir().map_or(path.clone(), |current| current.join(path))
    };
    fs::canonicalize(&path).unwrap_or(path)
}

fn load_cache(path: &Path) -> Option<UpdateCache> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_CACHE_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(MAX_CACHE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > MAX_CACHE_BYTES as usize {
        return None;
    }
    let cache = serde_json::from_slice::<UpdateCache>(&bytes).ok()?;
    (cache.schema_version == 1 && cache.latest.parse::<StableVersion>().is_ok()).then_some(cache)
}

fn write_cache(path: &Path, cache: &UpdateCache) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(cache).context("failed to serialize update cache")?;
    atomic_write(path, &bytes).with_context(|| format!("failed to save {}", path.display()))
}

fn parse_npm_version(output: &str) -> Result<StableVersion> {
    if output.trim() != output || output.contains(['\r', '\n']) {
        bail!("npm returned an invalid version line");
    }
    output
        .parse()
        .context("npm returned an invalid stable version")
}

fn parse_cargo_version(output: &str) -> Result<StableVersion> {
    let value = output
        .lines()
        .next()
        .context("cargo search returned no results")?
        .strip_prefix("typeul = \"")
        .context("cargo search did not return the typeul crate")?;
    let (version, suffix) = value
        .split_once('"')
        .context("cargo search returned an invalid version field")?;
    if !(suffix.is_empty()
        || suffix.starts_with(' ') && suffix.trim_start_matches(' ').starts_with('#'))
    {
        bail!("cargo search returned an invalid first field");
    }
    version
        .parse()
        .context("cargo search returned an invalid stable version")
}

fn run_registry_command(
    executable: &Path,
    arguments: &[&str],
    timeout: Duration,
) -> Result<String> {
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .group_spawn()
        .with_context(|| format!("failed to start {}", executable.display()))?;
    let deadline = Instant::now()
        .checked_add(timeout)
        .unwrap_or_else(Instant::now);
    let mut timed_out = false;
    let finish = loop {
        match child.inner().try_wait() {
            Ok(Some(_)) => break Ok(()),
            Ok(None) => {}
            Err(error) => break Err(error),
        }
        if Instant::now() >= deadline {
            timed_out = true;
            break Ok::<_, std::io::Error>(());
        }
        thread::sleep(
            COMMAND_POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
        );
    };
    let _ = child.kill();
    let output = child
        .wait_with_output()
        .context("failed to collect registry command output")?;
    finish.context("failed to poll registry command")?;
    if timed_out {
        bail!("registry command timed out");
    }
    if output.stdout.len() > MAX_COMMAND_OUTPUT_BYTES
        || output.stderr.len() > MAX_COMMAND_OUTPUT_BYTES
    {
        bail!("registry command output exceeded 4 KiB");
    }
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        bail!("registry command failed: {}", error.escape_debug());
    }
    let output =
        String::from_utf8(output.stdout).context("registry command output is not valid UTF-8")?;
    let output = output
        .strip_suffix("\r\n")
        .or_else(|| output.strip_suffix('\n'))
        .unwrap_or(&output);
    if output.is_empty() {
        bail!("registry command returned no output");
    }
    Ok(output.to_owned())
}

fn npm_executable() -> &'static Path {
    if cfg!(windows) {
        Path::new("npm.cmd")
    } else {
        Path::new("npm")
    }
}

fn registry_version(method: InstallMethod, timeout: Duration) -> Result<Option<StableVersion>> {
    match method {
        InstallMethod::Npm => run_registry_command(
            npm_executable(),
            &["view", "typeul", "version", "--silent"],
            timeout,
        )
        .and_then(|output| parse_npm_version(&output))
        .map(Some),
        InstallMethod::Cargo => run_registry_command(
            Path::new("cargo"),
            &["search", "typeul", "--limit", "1"],
            timeout,
        )
        .and_then(|output| parse_cargo_version(&output))
        .map(Some),
        InstallMethod::Standalone => Ok(None),
    }
}

fn unix_now() -> Result<i64> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_secs();
    i64::try_from(seconds).context("system clock is out of range")
}

pub fn foreground_check(
    paths: &AppPaths,
) -> Result<(InstallMethod, StableVersion, Option<StableVersion>)> {
    let method = detect_install_method();
    let current = VERSION.parse::<StableVersion>()?;
    let latest = registry_version(method, COMMAND_TIMEOUT)?;
    if let Some(latest) = latest {
        write_cache(
            &paths.update_cache,
            &UpdateCache {
                schema_version: 1,
                checked_at_unix: unix_now()?,
                latest: latest.to_string(),
            },
        )?;
    }
    Ok((method, current, latest))
}

fn automatic_allowed(
    method: InstallMethod,
    stdin_terminal: bool,
    stdout_terminal: bool,
    suppressed: bool,
) -> bool {
    method == InstallMethod::Npm && stdin_terminal && stdout_terminal && !suppressed
}

pub fn start_background_check(
    settings: &Settings,
    paths: &AppPaths,
) -> Option<Receiver<Option<UpdateNotice>>> {
    let method = detect_install_method();
    let suppressed = cfg!(test)
        || !settings.check_updates
        || env::var_os("CI").is_some()
        || env::var_os("TYPEUL_NO_UPDATE_CHECK").as_deref() == Some(OsStr::new("1"))
        || env::var_os("TYPEUL_TEST").as_deref() == Some(OsStr::new("1"));
    if !automatic_allowed(
        method,
        std::io::stdin().is_terminal(),
        std::io::stdout().is_terminal(),
        suppressed,
    ) {
        return None;
    }
    let now = unix_now().ok()?;
    if !should_check(load_cache(&paths.update_cache).as_ref(), now) {
        return None;
    }
    let path = paths.update_cache.clone();
    let skipped = settings.skipped_update_version.clone();
    let (sender, receiver) = sync_channel(1);
    thread::spawn(move || {
        let update = registry_version(method, COMMAND_TIMEOUT)
            .and_then(|latest| latest.context("npm returned no version"))
            .and_then(|latest| {
                write_cache(
                    &path,
                    &UpdateCache {
                        schema_version: 1,
                        checked_at_unix: now,
                        latest: latest.to_string(),
                    },
                )?;
                Ok(latest)
            })
            .ok()
            .and_then(|latest| notice(VERSION, &latest.to_string(), &skipped, method));
        let _ = sender.send(update);
    });
    Some(receiver)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::start_background_check;
    use super::{
        InstallMethod, StableVersion, UpdateCache, automatic_allowed, install_method_from,
        load_cache, notice, npm_executable, parse_cargo_version, parse_npm_version,
        run_registry_command, should_check, write_cache,
    };
    #[cfg(unix)]
    use crate::{config::Settings, storage::AppPaths};
    use std::{
        ffi::OsStr,
        fs,
        path::{Path, PathBuf},
        time::Duration,
    };
    #[cfg(unix)]
    use std::{
        process::{Command, Stdio},
        time::Instant,
    };

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "typeul-update-unit-{}-{}",
                std::process::id(),
                fastrand::u64(..)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn script(&self, name: &str, output: &str, exit: i32) -> PathBuf {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let path = self.0.join(name);
                fs::write(
                    &path,
                    format!("#!/bin/sh\nprintf '%s' '{output}'\nexit {exit}\n"),
                )
                .unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
                path
            }
            #[cfg(windows)]
            {
                let path = self.0.join(format!("{name}.cmd"));
                let command = output.strip_suffix('\n').map_or_else(
                    || format!("<nul set /p ={output}"),
                    |output| format!("echo {output}"),
                );
                fs::write(
                    &path,
                    format!("@echo off\r\n{command}\r\nexit /b {exit}\r\n"),
                )
                .unwrap();
                path
            }
        }

        fn sleeping_script(&self) -> PathBuf {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let path = self.0.join("sleeping");
                fs::write(&path, "#!/bin/sh\nexec sleep 5\n").unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
                path
            }
            #[cfg(windows)]
            {
                let path = self.0.join("sleeping.cmd");
                fs::write(&path, "@echo off\r\n:loop\r\ngoto loop\r\n").unwrap();
                path
            }
        }

        #[cfg(unix)]
        fn descendant_script(&self, name: &str, wait: bool) -> PathBuf {
            use std::os::unix::fs::PermissionsExt;

            let path = self.0.join(name);
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\nsleep 3 &\nprintf '%s\\n' \"$!\" > \"$1\"\n{}\n",
                    if wait { "wait" } else { "exit 0" }
                ),
            )
            .unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(unix)]
    fn process_exists(pid: &str) -> bool {
        Command::new("kill")
            .args(["-0", pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(unix)]
    fn assert_process_gone(path: &Path) {
        let pid = fs::read_to_string(path).unwrap();
        let pid = pid.trim();
        for _ in 0..40 {
            if !process_exists(pid) {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let _ = Command::new("kill").args(["-9", pid]).status();
        panic!("registry descendant {pid} survived");
    }

    #[test]
    fn strict_versions_reject_every_nonstable_shape_and_overflow() {
        assert_eq!(
            "0.12.3".parse::<StableVersion>().unwrap().to_string(),
            "0.12.3"
        );
        for value in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "v1.2.3",
            "1.2.3-beta",
            "1.02.3",
            "1..3",
            "+1.2.3",
            "18446744073709551616.0.0",
        ] {
            assert!(value.parse::<StableVersion>().is_err(), "{value}");
        }
    }

    #[test]
    fn cache_requires_supported_valid_data_and_exact_twenty_four_hours() {
        let now = 1_786_060_800;
        let mut cache = UpdateCache {
            schema_version: 1,
            checked_at_unix: now - 86_399,
            latest: "1.2.3".into(),
        };
        assert!(!should_check(Some(&cache), now));
        cache.checked_at_unix -= 1;
        assert!(should_check(Some(&cache), now));
        cache.schema_version = 2;
        assert!(should_check(Some(&cache), now));
        cache.schema_version = 1;
        cache.latest = "1.2.3-beta".into();
        assert!(should_check(Some(&cache), now));
        assert!(should_check(None, now));
    }

    #[test]
    fn notices_require_a_newer_unskipped_stable_version() {
        assert!(notice("1.2.3", "1.2.3", "", InstallMethod::Npm).is_none());
        assert!(notice("1.2.3", "1.2.2", "", InstallMethod::Npm).is_none());
        assert!(notice("1.2.3", "1.2.4", "1.2.4", InstallMethod::Npm).is_none());
        let notice = notice("1.2.3", "1.2.4", "", InstallMethod::Cargo).unwrap();
        assert_eq!(notice.instructions(), "cargo install --force typeul");
    }

    #[test]
    fn npm_marker_wins_then_only_cargo_bin_is_detected() {
        let cargo_home = Path::new("/resolved/cargo");
        assert_eq!(
            install_method_from(
                Some(OsStr::new("npm")),
                Path::new("/elsewhere/typeul"),
                cargo_home
            ),
            InstallMethod::Npm
        );
        assert_eq!(
            install_method_from(None, Path::new("/resolved/cargo/bin/typeul"), cargo_home),
            InstallMethod::Cargo
        );
        assert_eq!(
            install_method_from(None, Path::new("/resolved/cargo/tools/typeul"), cargo_home),
            InstallMethod::Standalone
        );
    }

    #[test]
    fn npm_uses_the_platform_executable_name() {
        #[cfg(windows)]
        assert_eq!(npm_executable(), Path::new("npm.cmd"));
        #[cfg(not(windows))]
        assert_eq!(npm_executable(), Path::new("npm"));
    }

    #[test]
    fn npm_and_cargo_outputs_accept_only_the_expected_first_field() {
        assert_eq!(parse_npm_version("1.4.0").unwrap().to_string(), "1.4.0");
        for output in [" 1.4.0", "1.4.0 ", "1.4.0\n1.5.0", "v1.4.0"] {
            assert!(parse_npm_version(output).is_err(), "{output:?}");
        }
        assert_eq!(
            parse_cargo_version(
                "typeul = \"1.4.0\"    # Offline typing tutor\n\
                 ... and 4 crates more (use --limit N to see more)",
            )
            .unwrap()
            .to_string(),
            "1.4.0"
        );
        for output in [
            "other = \"1.4.0\"",
            "typeul-cli = \"1.4.0\"",
            "typeul = \"1.4.0-beta\"",
            "typeul = \"1.4.0\" junk",
        ] {
            assert!(parse_cargo_version(output).is_err(), "{output:?}");
        }
    }

    #[test]
    fn registry_commands_are_bounded_reaped_and_timed_out() {
        let root = TestDir::new();
        let success = root.script("success", "1.4.0\n", 0);
        assert_eq!(
            run_registry_command(
                &success,
                &["view", "typeul", "version", "--silent"],
                Duration::from_secs(1),
            )
            .unwrap(),
            "1.4.0"
        );
        let cargo = root.script(
            "cargo-output",
            "typeul = \"1.4.0\"    # Offline typing tutor\n... and 4 crates more\n",
            0,
        );
        let output = run_registry_command(
            &cargo,
            &["search", "typeul", "--limit", "1"],
            Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(
            parse_cargo_version(&output).unwrap(),
            StableVersion(1, 4, 0)
        );
        let failure = root.script("failure", "registry unavailable\n", 7);
        assert!(run_registry_command(&failure, &[], Duration::from_secs(1)).is_err());
        let oversized = root.script("oversized", &"x".repeat(4_097), 0);
        assert!(run_registry_command(&oversized, &[], Duration::from_secs(1)).is_err());
        assert!(
            run_registry_command(&root.sleeping_script(), &[], Duration::from_millis(100)).is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_registry_leader_exit_cannot_hold_output_pipes_past_the_deadline() {
        let root = TestDir::new();
        let pid = root.0.join("leader-exits.pid");
        let started = Instant::now();
        assert!(
            run_registry_command(
                &root.descendant_script("leader-exits", false),
                &[pid.to_str().unwrap()],
                Duration::from_secs(1),
            )
            .is_err()
        );
        assert_process_gone(&pid);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn a_registry_timeout_kills_the_whole_process_tree() {
        let root = TestDir::new();
        let pid = root.0.join("leader-waits.pid");
        assert!(
            run_registry_command(
                &root.descendant_script("leader-waits", true),
                &[pid.to_str().unwrap()],
                Duration::from_secs(1),
            )
            .is_err()
        );
        assert_process_gone(&pid);
    }

    #[test]
    fn malformed_cache_is_preserved_and_valid_cache_is_atomically_replaced() {
        let root = TestDir::new();
        let path = root.0.join("cache/update.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"not json").unwrap();
        assert!(load_cache(&path).is_none());
        assert_eq!(fs::read(&path).unwrap(), b"not json");

        let cache = UpdateCache {
            schema_version: 1,
            checked_at_unix: 1_786_060_800,
            latest: "1.4.0".into(),
        };
        write_cache(&path, &cache).unwrap();
        assert_eq!(load_cache(&path), Some(cache));
        let json = serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            json.as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            ["checked_at_unix", "latest", "schema_version"]
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn cache_reads_only_bounded_regular_files() {
        let root = TestDir::new();
        let path = root.0.join("update.json");
        let oversized = format!(
            "{{\"schema_version\":1,\"checked_at_unix\":1,\"latest\":\"1.4.0\"}}{}",
            " ".repeat(4_096)
        );
        fs::write(&path, oversized.as_bytes()).unwrap();
        assert!(load_cache(&path).is_none());
        assert_eq!(fs::read(&path).unwrap(), oversized.as_bytes());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let target = root.0.join("target.json");
            write_cache(
                &target,
                &UpdateCache {
                    schema_version: 1,
                    checked_at_unix: 1,
                    latest: "1.4.0".into(),
                },
            )
            .unwrap();
            let link = root.0.join("linked.json");
            symlink(&target, &link).unwrap();
            assert!(load_cache(&link).is_none());
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_suppressed_check_never_opens_the_cache() {
        let root = TestDir::new();
        let mut paths = AppPaths::from_override(root.0.join("home"));
        paths.update_cache = root.0.join("update.fifo");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&paths.update_cache)
                .status()
                .unwrap()
                .success()
        );
        let fifo = paths.update_cache.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let settings = Settings {
                check_updates: false,
                ..Settings::default()
            };
            let result = start_background_check(&settings, &paths);
            sender.send(result.is_none()).unwrap();
        });

        match receiver.recv_timeout(Duration::from_millis(500)) {
            Ok(true) => handle.join().unwrap(),
            result => {
                drop(fs::OpenOptions::new().write(true).open(fifo).unwrap());
                handle.join().unwrap();
                panic!("suppressed update check touched the cache: {result:?}");
            }
        }
    }

    #[test]
    fn automatic_policy_requires_every_privacy_and_freshness_condition() {
        let now = 1_786_060_800;
        assert!(automatic_allowed(InstallMethod::Npm, true, true, false));
        assert!(!automatic_allowed(InstallMethod::Cargo, true, true, false));
        assert!(!automatic_allowed(InstallMethod::Npm, false, true, false));
        assert!(!automatic_allowed(InstallMethod::Npm, true, false, false));
        assert!(!automatic_allowed(InstallMethod::Npm, true, true, true));
        let fresh = UpdateCache {
            schema_version: 1,
            checked_at_unix: now,
            latest: "1.4.0".into(),
        };
        assert!(!should_check(Some(&fresh), now));
    }
}
